use chrono::{Datelike, Days, NaiveDate};
use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use fbkl_constants::{
    date::{LEAGUE_TIME_ZONE, league_wall_clock},
    league_rules::{
        IN_SEASON_FA_ALL_BID_DEADLINE_HOUR_MINUTE, IN_SEASON_FA_MINIMUM_BID,
        IN_SEASON_FA_OPENING_BID_DEADLINE_HOUR_MINUTE,
    },
};
use fbkl_entity::{
    auction::{self, AuctionKind, AuctionStatus},
    auction_queries::{self, NewAuction},
    contract::{self, ContractKind},
    contract_queries,
    deadline::{self, DeadlineKind},
    deadline_queries,
    sea_orm::{
        ConnectionTrait, TransactionSession, TransactionTrait, prelude::DateTimeWithTimeZone,
    },
    team_update_queries,
};
use tracing::instrument;

use super::{
    AuctionCloseOutcome, auction_close_at, auction_close_outcome, auction_quiet_window,
    find_auction_mode_deadlines, sign_auction_contract_to_team,
};

/// Ends a free agent auction and creates the associated transaction + team contract OR expires the associated contract.
#[instrument(skip(db))]
pub async fn end_fa_auction<C>(
    deadline_model: &deadline::Model,
    auction_id: i64,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let auction_model = auction_queries::find_auction_by_id(auction_id, db).await?;
    let auction_contract_model = auction_model.get_contract(db).await?;

    // Create contract for player <--> team
    let db_txn = db.begin().await?;

    let maybe_latest_bid = auction_model.get_latest_bid(&db_txn).await?;
    let final_contract_model =
        match auction_close_outcome(auction_contract_model.kind, maybe_latest_bid.is_some()) {
            AuctionCloseOutcome::AwaitRfaResolution => {
                auction_queries::update_auction_status(
                    auction_model.id,
                    AuctionStatus::Closed,
                    &db_txn,
                )
                .await?;
                auction_contract_model
            }
            AuctionCloseOutcome::Expire => {
                // No one bid on the player; expire the contract. Player is now a free agent again.
                auction_queries::update_auction_status(
                    auction_model.id,
                    AuctionStatus::Expired,
                    &db_txn,
                )
                .await?;
                contract_queries::expire_contract(auction_contract_model, &db_txn).await?
            }
            AuctionCloseOutcome::Sign => {
                let winning_bid_model = maybe_latest_bid.ok_or_else(|| {
                    eyre!("Expected a winning bid for auction {}", auction_model.id)
                })?;

                let (signed_contract_model, _, team_update_model) = sign_auction_contract_to_team(
                    &auction_model,
                    &winning_bid_model,
                    deadline_model,
                    maybe_override_effective_date,
                    &db_txn,
                )
                .await?;

                // Stamp the team_update the same way the veteran path does; the signing is immediate.
                team_update_queries::update_team_update_for_auction(
                    &team_update_model,
                    maybe_override_effective_date,
                    &db_txn,
                )
                .await?;

                auction_queries::update_auction_status(
                    auction_model.id,
                    AuctionStatus::Completed,
                    &db_txn,
                )
                .await?;

                signed_contract_model
            }
        };

    db_txn.commit().await?;

    Ok(final_contract_model)
}

/// Opens a new in-season free agent auction for a player (rules §8.3).
///
/// Only allowed up to the week's Friday opening-bid deadline (§8.2); the auction's all-bid
/// deadline is that week's Sunday 8pm CT, which bids may still roll forward (§8.3.2).
#[instrument(skip(db))]
pub async fn open_in_season_fa_auction<C>(
    league_id: i64,
    end_of_season_year: i16,
    player_id: i64,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait,
{
    let (opening_bid_deadline, all_bid_deadline) = fa_auction_week_deadlines(now)?;
    ensure!(
        now <= opening_bid_deadline,
        "New free agent auctions cannot be opened after the week's opening-bid deadline ({opening_bid_deadline}) per rules §8.2."
    );
    let fa_auction_end = deadline_queries::find_deadline_for_season_by_type(
        league_id,
        end_of_season_year,
        DeadlineKind::FreeAgentAuctionEnd,
        db,
    )
    .await?;
    ensure!(
        now <= fa_auction_end.date_time,
        "The season's free agent auction period ended at {}.",
        fa_auction_end.date_time
    );

    let pooled_contract =
        get_or_create_player_contract_for_fa_auction(league_id, end_of_season_year, player_id, db)
            .await?;
    let minimum_bid_amount = in_season_fa_minimum_bid(&pooled_contract, db).await?;

    let mode_deadlines = find_auction_mode_deadlines(
        AuctionKind::InSeasonFreeAgent,
        league_id,
        end_of_season_year,
        now,
        db,
    )
    .await?;

    auction_queries::insert_new_auction(
        NewAuction {
            contract_id: pooled_contract.id,
            kind: AuctionKind::InSeasonFreeAgent,
            minimum_bid_amount,
            start_timestamp: now,
            close_at_timestamp: auction_close_at(
                now,
                auction_quiet_window(now, mode_deadlines.crunch_window_start),
                Some(all_bid_deadline),
                mode_deadlines.hard_deadline,
            )?,
            all_bid_deadline_timestamp: Some(all_bid_deadline),
            original_owner_team_id: None,
        },
        db,
    )
    .await
}

/// The week's §8.2 free agent auction deadlines as `(opening-bid, all-bid)`.
///
/// Weeks run Monday-Sunday (the league's matchup week): new auctions may be nominated until Friday
/// 11:59pm CT, and every auction in that week ends at Sunday 8pm CT — so auctions opened before
/// Friday keep taking bids after new nominations freeze (§8.2.1).
pub fn fa_auction_week_deadlines(
    now: DateTimeWithTimeZone,
) -> Result<(DateTimeWithTimeZone, DateTimeWithTimeZone)> {
    let league_now = now.with_timezone(&LEAGUE_TIME_ZONE);
    let monday =
        league_now.date_naive() - Days::new(u64::from(league_now.weekday().num_days_from_monday()));

    let deadline_at = |day_offset: u64, (hour, minute): (u32, u32)| {
        (monday + Days::new(day_offset))
            .and_hms_opt(hour, minute, 0)
            .ok_or_else(|| eyre!("Could not build the §8.2 weekly deadline for {monday}."))
            .and_then(league_wall_clock)
    };

    Ok((
        deadline_at(4, IN_SEASON_FA_OPENING_BID_DEADLINE_HOUR_MINUTE)?,
        deadline_at(6, IN_SEASON_FA_ALL_BID_DEADLINE_HOUR_MINUTE)?,
    ))
}

/// The opening bid an in-season free agent auction starts at (rules §8.3.3).
///
/// $1 unless the player was already owned this season — then their previous in-season salary is the
/// floor, RD/RDI contracts included.
#[instrument(skip(db))]
pub async fn in_season_fa_minimum_bid<C>(pooled_contract: &contract::Model, db: &C) -> Result<i16>
where
    C: ConnectionTrait,
{
    let contract_chain = contract_queries::find_contract_chain(pooled_contract.id, db).await?;
    Ok(
        previous_in_season_salary(&contract_chain, pooled_contract.end_of_season_year)
            .unwrap_or(IN_SEASON_FA_MINIMUM_BID),
    )
}

/// Salary of the latest contract in the chain that a team actually owned during the season.
fn previous_in_season_salary(
    contract_chain: &[contract::Model],
    end_of_season_year: i16,
) -> Option<i16> {
    contract_chain
        .iter()
        .filter(|contract_model| {
            contract_model.end_of_season_year == end_of_season_year
                && contract_model.team_id.is_some()
        })
        .max_by_key(|contract_model| contract_model.id)
        .map(|contract_model| contract_model.salary)
}

/// Either retrieves + validates an existing player contract that can be used for a new free agent auction, or creates one based on given arguments.
#[instrument(skip(db))]
pub async fn get_or_create_player_contract_for_fa_auction<C>(
    league_id: i64,
    end_of_season_year: i16,
    player_id: i64,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait,
{
    let maybe_existing_contract = contract_queries::find_active_contracts_in_league(league_id, db)
        .await?
        .into_iter()
        .find(|contract_model| {
            (contract_model.player_id == Some(player_id))
                && contract_model.kind == ContractKind::FreeAgent
        });
    let player_contract = match maybe_existing_contract {
        None => {
            // Create new contract
            contract_queries::create_new_contract(
                contract::Model::new_contract_for_auction(league_id, end_of_season_year, player_id),
                db,
            )
            .await?
        }
        Some(existing_player_contract) => existing_player_contract,
    };
    Ok(player_contract)
}

#[cfg(test)]
mod tests {
    use fbkl_entity::contract::{ContractKind, ContractStatus, Model};

    use fbkl_entity::sea_orm::prelude::DateTimeWithTimeZone;

    use super::{fa_auction_week_deadlines, previous_in_season_salary};

    #[test]
    fn weekly_free_agent_deadlines_freeze_new_auctions_on_friday_but_take_bids_until_sunday() {
        // 2026-11-04 is a Wednesday; 10am CT = 16:00 UTC.
        let wednesday = DateTimeWithTimeZone::parse_from_rfc3339("2026-11-04T16:00:00Z").unwrap();
        let (opening_bid_deadline, all_bid_deadline) =
            fa_auction_week_deadlines(wednesday).unwrap();
        assert_eq!(
            opening_bid_deadline.to_rfc3339(),
            "2026-11-06T23:59:00-06:00"
        );
        assert_eq!(all_bid_deadline.to_rfc3339(), "2026-11-08T20:00:00-06:00");

        // Saturday sits past the opening-bid deadline but inside the same bidding week.
        let saturday = DateTimeWithTimeZone::parse_from_rfc3339("2026-11-07T16:00:00Z").unwrap();
        let (saturday_opening, saturday_all_bid) = fa_auction_week_deadlines(saturday).unwrap();
        assert_eq!(saturday_opening, opening_bid_deadline);
        assert_eq!(saturday_all_bid, all_bid_deadline);
        assert!(saturday > saturday_opening);
        assert!(saturday < saturday_all_bid);
    }

    #[test]
    fn weekly_deadlines_hold_their_central_wall_clock_across_both_dst_transitions() {
        // Mar 2-8 2026 straddles spring forward: Friday is still CST, Sunday is already CDT.
        let spring = DateTimeWithTimeZone::parse_from_rfc3339("2026-03-04T16:00:00Z").unwrap();
        let (spring_opening, spring_all_bid) = fa_auction_week_deadlines(spring).unwrap();
        assert_eq!(spring_opening.to_rfc3339(), "2026-03-06T23:59:00-06:00");
        assert_eq!(spring_all_bid.to_rfc3339(), "2026-03-08T20:00:00-05:00");

        // Oct 26 - Nov 1 2026 straddles fall back the other way: Friday CDT, Sunday CST.
        let fall = DateTimeWithTimeZone::parse_from_rfc3339("2026-10-28T16:00:00Z").unwrap();
        let (fall_opening, fall_all_bid) = fa_auction_week_deadlines(fall).unwrap();
        assert_eq!(fall_opening.to_rfc3339(), "2026-10-30T23:59:00-05:00");
        assert_eq!(fall_all_bid.to_rfc3339(), "2026-11-01T20:00:00-06:00");
    }

    fn contract(id: i64, end_of_season_year: i16, team_id: Option<i64>, salary: i16) -> Model {
        Model {
            id,
            year_number: 1,
            kind: ContractKind::Veteran,
            is_ir: false,
            salary,
            end_of_season_year,
            status: ContractStatus::Replaced,
            league_id: 1,
            league_player_id: None,
            player_id: Some(1),
            previous_contract_id: None,
            original_contract_id: Some(1),
            team_id,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn previously_owned_player_opens_at_their_last_in_season_salary() {
        let chain = [
            contract(1, 2024, Some(7), 12),
            contract(2, 2025, Some(7), 9),
            contract(3, 2025, None, 9),
        ];
        assert_eq!(previous_in_season_salary(&chain, 2025), Some(9));
    }

    #[test]
    fn never_owned_this_season_has_no_previous_salary() {
        let chain = [contract(1, 2024, Some(7), 12), contract(2, 2025, None, 12)];
        assert_eq!(previous_in_season_salary(&chain, 2025), None);
    }
}
