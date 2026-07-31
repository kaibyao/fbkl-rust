//! Building a season's veteran auction pool and opening its scheduled auctions (rules §6.3).
//!
//! Pool membership comes from `eligibility::build_veteran_auction_pool` — this module only turns
//! that membership into pooled contracts plus `auction_schedule` rows (release date, nomination
//! rank, min-bid tier), then opens an auction per row when its release date arrives.
//!
//! The ranked top-150 list, the players-per-day count, and the tier values are all per-season
//! commissioner/import inputs (§6.3.6): the ranking is passed in, the tiers are read from
//! `min_bid_tier_config`.

use std::{collections::HashMap, fmt::Debug};

use chrono::Days;
use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use fbkl_constants::league_rules::{
    VETERAN_AUCTION_PLAYERS_RELEASED_PER_DAY, VETERAN_AUCTION_RFA_WEEK_DAYS,
};
use fbkl_entity::{
    auction::{self, AuctionKind},
    auction_queries::{self, NewAuction},
    auction_schedule,
    auction_schedule_queries::{self, NewAuctionScheduleRow},
    contract::{self, ContractKind, RelatedPlayer},
    contract_queries,
    deadline::DeadlineKind,
    deadline_queries,
    sea_orm::{
        ConnectionTrait, TransactionTrait,
        prelude::{Date, DateTimeWithTimeZone},
    },
};
use tracing::instrument;

use super::{
    auction_close_at, auction_quiet_window, find_auction_mode_deadlines,
    get_or_create_player_contract_for_veteran_auction,
};
use crate::eligibility::build_veteran_auction_pool;

/// Contract kinds that open at their carry salary instead of a tier value (rules §15.3.1, §16).
static CARRY_SALARY_CONTRACT_KINDS: &[ContractKind] = &[
    ContractKind::RestrictedFreeAgent,
    ContractKind::UnrestrictedFreeAgentOriginalTeam,
    ContractKind::UnrestrictedFreeAgentVeteran,
];

/// Assembles the season's veteran auction pool at/after the keeper deadline.
///
/// Creates the pooled contract for every eligible unkept veteran and writes the `auction_schedule`
/// rows that the daily release tick consumes. `ranked_player_ids` is the season's ranked top-150
/// (§6.3.2) in rank order; pooled players outside it are open-nomination.
#[instrument(skip(db))]
pub async fn assemble_veteran_auction_pool<C>(
    league_id: i64,
    end_of_season_year: i16,
    ranked_player_ids: &[i64],
    db: &C,
) -> Result<Vec<NewAuctionScheduleRow>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let tiers = auction_schedule_queries::find_min_bid_tiers(league_id, end_of_season_year, db)
        .await?
        .into_iter()
        .map(|tier_model| tier_model.tier_index)
        .collect::<Vec<_>>();
    let Some(&bottom_tier) = tiers.last() else {
        bail!(
            "League {league_id} has no configured minimum bid tiers for season {end_of_season_year}."
        );
    };
    let tier_count = tiers.len();

    let auction_start_date = deadline_queries::find_deadline_for_season_by_type(
        league_id,
        end_of_season_year,
        DeadlineKind::PreseasonVeteranAuctionStart,
        db,
    )
    .await?
    .date_time
    .date_naive();

    let pool = build_veteran_auction_pool(league_id, end_of_season_year, db).await?;
    // Only real NBA players can be pooled here; league-only players are rookie draft material.
    let rfa_player_ids: Vec<i64> = pool
        .restricted_free_agents
        .iter()
        .filter_map(real_player_id)
        .collect();
    let ranks: HashMap<i64, usize> = ranked_player_ids
        .iter()
        .enumerate()
        .map(|(index, player_id)| (*player_id, index))
        .collect();
    let mut other_player_ids: Vec<i64> = pool
        .unrestricted_free_agents
        .iter()
        .chain(pool.free_agents.iter())
        .filter_map(real_player_id)
        .collect();
    other_player_ids.sort_by_key(|player_id| ranks.get(player_id).copied().unwrap_or(usize::MAX));
    let ranked_count = other_player_ids
        .iter()
        .filter(|player_id| ranks.contains_key(player_id))
        .count();

    // §6.3.1: the rest of the pool only starts releasing after RFA week.
    let first_other_release_date = if rfa_player_ids.is_empty() {
        auction_start_date
    } else {
        auction_start_date
            .checked_add_days(Days::new(VETERAN_AUCTION_RFA_WEEK_DAYS))
            .ok_or_else(|| eyre!("Veteran auction start date + RFA week overflowed."))?
    };

    let mut rows = Vec::with_capacity(rfa_player_ids.len() + other_player_ids.len());
    for player_id in rfa_player_ids {
        rows.push(NewAuctionScheduleRow {
            player_id,
            scheduled_release_date: auction_start_date,
            nomination_rank: rank_number(ranks.get(&player_id).copied()),
            // RFAs open at their 4th-year carry salary, so their tier is never read.
            min_bid_tier: bottom_tier,
            is_rfa_week: true,
        });
    }
    for (position, player_id) in other_player_ids.into_iter().enumerate() {
        let min_bid_tier = if position < ranked_count {
            tiers[tier_slot(position, ranked_count, tier_count)]
        } else {
            bottom_tier
        };
        rows.push(NewAuctionScheduleRow {
            player_id,
            scheduled_release_date: release_date(
                first_other_release_date,
                position,
                VETERAN_AUCTION_PLAYERS_RELEASED_PER_DAY,
            )?,
            nomination_rank: rank_number(ranks.get(&player_id).copied()),
            min_bid_tier,
            is_rfa_week: false,
        });
    }

    let db_txn = db.begin().await?;
    for row in &rows {
        get_or_create_player_contract_for_veteran_auction(
            league_id,
            end_of_season_year,
            row.player_id,
            &db_txn,
        )
        .await?;
    }
    auction_schedule_queries::insert_auction_schedule_rows(
        league_id,
        end_of_season_year,
        rows.clone(),
        &db_txn,
    )
    .await?;
    db_txn.commit().await?;

    Ok(rows)
}

/// Opens the auction for one released schedule row (rules §6.3.3). RFA/UFA auctions carry their
/// original owner so bids from that team are rejected and their close routes to RFA resolution.
///
/// Idempotent: a schedule row stays due after its release date passes, so a row whose auction was
/// already opened returns that auction instead of opening a second one. The lookup is by player
/// rather than by pooled contract so it survives the contract chain advancing when the auction
/// settles — signing replaces the pooled contract, expiring leaves no active one at all.
#[instrument(skip(db))]
pub async fn open_scheduled_auction<C>(
    schedule_row: &auction_schedule::Model,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    if let Some(existing_auction) = auction_queries::find_auction_for_player_in_season(
        schedule_row.league_id,
        schedule_row.end_of_season_year,
        schedule_row.player_id,
        AuctionKind::PreseasonVeteranAuction,
        db,
    )
    .await?
    {
        return Ok(existing_auction);
    }

    let pooled_contract = get_or_create_player_contract_for_veteran_auction(
        schedule_row.league_id,
        schedule_row.end_of_season_year,
        schedule_row.player_id,
        db,
    )
    .await?;

    let minimum_bid_amount = if CARRY_SALARY_CONTRACT_KINDS.contains(&pooled_contract.kind) {
        pooled_contract.salary
    } else {
        auction_schedule_queries::find_min_bid_tier_by_index(
            schedule_row.league_id,
            schedule_row.end_of_season_year,
            schedule_row.min_bid_tier,
            db,
        )
        .await?
        .ok_or_else(|| {
            eyre!(
                "Minimum bid tier {} is not configured for league {} season {}.",
                schedule_row.min_bid_tier,
                schedule_row.league_id,
                schedule_row.end_of_season_year
            )
        })?
        .min_bid_amount
    };

    let maybe_original_owner_team_id =
        if CARRY_SALARY_CONTRACT_KINDS.contains(&pooled_contract.kind) {
            find_original_owner_team_id(&pooled_contract, db).await?
        } else {
            None
        };

    // With no bids the §6.3.4 tier ladder is the clock; the daily slide pushes this close time out.
    let mode_deadlines = find_auction_mode_deadlines(
        AuctionKind::PreseasonVeteranAuction,
        schedule_row.league_id,
        schedule_row.end_of_season_year,
        now,
        db,
    )
    .await?;
    let auction_model = auction_queries::insert_new_auction(
        NewAuction {
            contract_id: pooled_contract.id,
            kind: AuctionKind::PreseasonVeteranAuction,
            minimum_bid_amount,
            start_timestamp: now,
            close_at_timestamp: auction_close_at(
                now,
                auction_quiet_window(now, None),
                None,
                mode_deadlines.hard_deadline,
            )?,
            all_bid_deadline_timestamp: None,
            original_owner_team_id: maybe_original_owner_team_id,
        },
        db,
    )
    .await?;

    Ok(auction_model)
}

/// Drops every unbid open veteran auction to the next-lower min-bid tier (rules §6.3.4-.5) and gives
/// it another day of clock.
///
/// Run by the daily release tick, which must run *before* the close tick: the ladder is the only
/// clock an unbid veteran auction has, so a close tick running first expires it the moment it becomes
/// slide-eligible. The slide is a single per-auction tier lookup, never a cascade: moving a player
/// into a tier does not push that tier's existing players down (§6.3.5). Auctions opened within the
/// last day are skipped so a fresh open does not slide the same day; the day the slide finds no lower
/// tier, the auction's lapsed close time expires it and the player becomes a $1 FA (§6.1.2).
#[instrument(skip(db))]
pub async fn slide_unbid_auctions_down_a_tier<C>(
    league_id: i64,
    end_of_season_year: i16,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<Vec<auction::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let opened_before = now
        .checked_sub_days(Days::new(1))
        .ok_or_else(|| eyre!("Tier slide tick timestamp underflowed: {now}"))?;
    let unbid_auctions = auction_queries::find_unbid_open_auctions(
        league_id,
        end_of_season_year,
        AuctionKind::PreseasonVeteranAuction,
        opened_before,
        db,
    )
    .await?;

    let tier_min_bid_amounts: Vec<i16> =
        auction_schedule_queries::find_min_bid_tiers(league_id, end_of_season_year, db)
            .await?
            .into_iter()
            .map(|tier| tier.min_bid_amount)
            .collect();

    let mode_deadlines = find_auction_mode_deadlines(
        AuctionKind::PreseasonVeteranAuction,
        league_id,
        end_of_season_year,
        now,
        db,
    )
    .await?;
    // An unbid auction has no bid to measure a reprieve from, so its day of clock is the ladder step.
    let next_close_at = auction_close_at(
        now,
        auction_quiet_window(now, None),
        None,
        mode_deadlines.hard_deadline,
    )?;

    let db_txn = db.begin().await?;
    let mut slid_auctions = Vec::new();
    for unbid_auction in unbid_auctions {
        // No lower tier means the ladder has run out, so the close tick expires it after this.
        if let Some(next_min_bid_amount) =
            next_lower_min_bid_amount(&tier_min_bid_amounts, unbid_auction.minimum_bid_amount)
        {
            slid_auctions.push(
                auction_queries::slide_auction_to_next_tier(
                    unbid_auction.id,
                    next_min_bid_amount,
                    next_close_at,
                    &db_txn,
                )
                .await?,
            );
        }
    }
    db_txn.commit().await?;

    Ok(slid_auctions)
}

/// The team that held the player going into free agency, i.e. the team on the previous contract.
async fn find_original_owner_team_id<C>(
    pooled_contract: &contract::Model,
    db: &C,
) -> Result<Option<i64>>
where
    C: ConnectionTrait + Debug,
{
    let Some(previous_contract_id) = pooled_contract.previous_contract_id else {
        return Ok(None);
    };
    let previous_contract = contract_queries::find_contract_by_id(previous_contract_id, db).await?;
    Ok(previous_contract.team_id)
}

const fn real_player_id(related_player: &RelatedPlayer) -> Option<i64> {
    match related_player {
        RelatedPlayer::Player(player_model) => Some(player_model.id),
        RelatedPlayer::LeaguePlayer(_) => None,
    }
}

/// Ranks are 1-based and only meaningful for the top-150 list; anything beyond `i16` is unranked.
fn rank_number(maybe_position: Option<usize>) -> Option<i16> {
    maybe_position.and_then(|position| i16::try_from(position + 1).ok())
}

/// Ranked players spread evenly over the configured tiers, best rank into the top tier (§6.3.4).
const fn tier_slot(ranked_position: usize, ranked_count: usize, tier_count: usize) -> usize {
    if ranked_count == 0 || tier_count == 0 {
        return 0;
    }
    let slot = ranked_position * tier_count / ranked_count;
    if slot >= tier_count {
        tier_count - 1
    } else {
        slot
    }
}

/// The configured tier value directly below `current_min_bid_amount`, `None` at the bottom tier.
///
/// Depends only on the auction's own current minimum, which is what makes the slide non-cascading.
fn next_lower_min_bid_amount(
    tier_min_bid_amounts: &[i16],
    current_min_bid_amount: i16,
) -> Option<i16> {
    tier_min_bid_amounts
        .iter()
        .copied()
        .filter(|min_bid_amount| *min_bid_amount < current_min_bid_amount)
        .max()
}

/// Releases are staggered a fixed number of players per day (§6.3.3).
fn release_date(first_date: Date, position: usize, players_per_day: usize) -> Result<Date> {
    let day_offset = u64::try_from(position / players_per_day.max(1))?;
    first_date
        .checked_add_days(Days::new(day_offset))
        .ok_or_else(|| eyre!("Veteran auction release date overflowed at position {position}."))
}

#[cfg(test)]
mod tests {
    use chrono::{Days, NaiveDate};
    use fbkl_entity::sea_orm::prelude::DateTimeWithTimeZone;

    use super::{
        auction_close_at, auction_quiet_window, next_lower_min_bid_amount, release_date, tier_slot,
    };

    /// A veteran auction never has an all-bid deadline, and the ladder step ignores the crunch window.
    fn ladder_step(now: DateTimeWithTimeZone) -> DateTimeWithTimeZone {
        auction_close_at(now, auction_quiet_window(now, None), None, None).unwrap()
    }

    #[test]
    fn tier_slide_is_one_step_per_auction_and_never_cascades() {
        let tiers = [20, 15, 10, 5];
        // Two auctions in the same tier both land on that tier's next-lower value, no push-down.
        assert_eq!(next_lower_min_bid_amount(&tiers, 20), Some(15));
        assert_eq!(next_lower_min_bid_amount(&tiers, 15), Some(10));
        assert_eq!(next_lower_min_bid_amount(&tiers, 15), Some(10));
        // Bottom tier stays put.
        assert_eq!(next_lower_min_bid_amount(&tiers, 5), None);
    }

    /// Rules §6.3.4-.5 + §6.1.2, with the tick order the scheduler uses: slide, then close.
    #[test]
    fn an_unbid_auction_walks_down_every_tier_before_it_expires() {
        let tiers = [20, 15, 10, 5];
        let opened_at: DateTimeWithTimeZone = "2025-09-01T12:00:00-06:00".parse().unwrap();
        let mut minimum_bid_amount = tiers[0];
        let mut close_at = ladder_step(opened_at);
        let mut minimums_walked = Vec::new();

        for day in 1..=tiers.len() {
            let tick = opened_at + Days::new(u64::try_from(day).unwrap());
            match next_lower_min_bid_amount(&tiers, minimum_bid_amount) {
                Some(next_minimum) => {
                    minimum_bid_amount = next_minimum;
                    close_at = ladder_step(tick);
                    minimums_walked.push(next_minimum);
                    // The slide moved the clock, so the close tick that follows finds nothing due.
                    assert!(close_at > tick);
                }
                // Bottom tier: nothing renews the clock, so the close tick expires it as a $1 FA.
                None => assert!(close_at <= tick),
            }
        }

        assert_eq!(minimums_walked, vec![15, 10, 5]);
        assert_eq!(minimum_bid_amount, 5);
    }

    #[test]
    fn ranked_players_spread_across_tiers_best_rank_first() {
        assert_eq!(tier_slot(0, 10, 5), 0);
        assert_eq!(tier_slot(3, 10, 5), 1);
        assert_eq!(tier_slot(9, 10, 5), 4);
        // Never past the bottom tier, even if the caller overshoots the ranked count.
        assert_eq!(tier_slot(20, 10, 5), 4);
    }

    #[test]
    fn releases_stagger_by_players_per_day() {
        let first_date = NaiveDate::from_ymd_opt(2025, 9, 1).unwrap();
        assert_eq!(release_date(first_date, 0, 15).unwrap(), first_date);
        assert_eq!(release_date(first_date, 14, 15).unwrap(), first_date);
        assert_eq!(
            release_date(first_date, 15, 15).unwrap(),
            NaiveDate::from_ymd_opt(2025, 9, 2).unwrap()
        );
    }
}
