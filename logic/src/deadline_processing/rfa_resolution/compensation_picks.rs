//! Which Rookie-Draft picks a winning owner may forfeit when the original owner declines (rules §15.2).
//!
//! The tier table turns the bid into the worst round that will settle the debt; everything else here
//! is subtraction. The winner chooses from what is left, so this returns the whole set rather than
//! picking one.
//!
//! Rules §15.3.3 also bars a bid or raise the bidder could not pay for, and a team may owe several
//! picks at once. [`find_unpayable_rfa_obligation`] answers that question for every live debt
//! together, so bid time, raise time and (later) trade time all use one rule.

use std::collections::HashSet;

use color_eyre::{Result, eyre::eyre};
use fbkl_constants::league_rules::compensation_round_for_bid;
use fbkl_entity::{
    auction_queries, draft_pick, draft_pick_queries, rfa_resolution,
    rfa_resolution::RfaResolutionStatus,
    rfa_resolution_queries,
    sea_orm::{ConnectionTrait, prelude::DateTimeWithTimeZone},
};
use tracing::instrument;

/// The picks the winning owner may hand to the original owner, best round first.
///
/// A pick qualifies when the winner still holds it, its round is no worse than the tier the bid
/// earns (an earlier round always settles a later one, rules §15.2.1), and the winner did not
/// acquire it after the winning bid was announced (rules §15.2.2).
///
/// An empty result means the winner owes a pick he cannot pay, which rules §15.3.3 is meant to
/// prevent at bid and raise time.
#[instrument(skip(db))]
pub async fn compute_eligible_compensation_picks<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait,
{
    let resolution_id = rfa_resolution_model.id;
    let winning_team_id = rfa_resolution_model.winning_team_id.ok_or_else(|| {
        eyre!("Nobody won RFA resolution {resolution_id}, so no pick is owed for it.")
    })?;
    let effective_bid = rfa_resolution_model
        .effective_bid()
        .ok_or_else(|| eyre!("RFA resolution {resolution_id} has no bid to price a pick from."))?;
    let final_bid_at = rfa_resolution_model.final_bid_at.ok_or_else(|| {
        eyre!("RFA resolution {resolution_id} is missing the time its winning bid was announced.")
    })?;

    eligible_compensation_picks(
        rfa_resolution_model.league_id,
        rfa_resolution_model.end_of_season_year,
        winning_team_id,
        effective_bid,
        Some(final_bid_at),
        db,
    )
    .await
}

/// The same eligible set as [`compute_eligible_compensation_picks`], for a bid with no resolution
/// row filled in yet.
///
/// `maybe_announced_at` is NULL while the auction is still open: no winning bid has been announced,
/// so rules §15.2.2 excludes nothing.
#[instrument(skip(db))]
pub async fn eligible_compensation_picks<C>(
    league_id: i64,
    end_of_season_year: i16,
    team_id: i64,
    bid_amount: i16,
    maybe_announced_at: Option<DateTimeWithTimeZone>,
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait,
{
    let required_round = compensation_round_for_bid(bid_amount);
    let picks_acquired_after_bid = match maybe_announced_at {
        Some(announced_at) => {
            draft_pick_queries::find_draft_pick_ids_acquired_by_team_after(
                team_id,
                announced_at,
                db,
            )
            .await?
        }
        None => HashSet::new(),
    };
    let season_picks =
        draft_pick_queries::get_draft_picks_for_league_season(league_id, end_of_season_year, db)
            .await?;

    Ok(season_picks
        .into_iter()
        .filter(|season_pick| {
            season_pick.current_owner_team_id == team_id
                && season_pick.round <= required_round
                && !picks_acquired_after_bid.contains(&season_pick.id)
        })
        .collect())
}

/// One RFA compensation debt a team carries right now: a bid it leads, or a handshake it won.
#[derive(Clone, Copy, Debug)]
pub struct RfaObligation {
    pub rfa_resolution_id: i64,
    /// The amount the tier is priced off: the live high bid, or the effective bid once won.
    pub bid_amount: i16,
    /// When the winning bid was announced (rules §15.2.2); NULL while the auction is still open.
    pub announced_at: Option<DateTimeWithTimeZone>,
}

/// Every live compensation debt `team_id` carries in the season.
///
/// A settled resolution owes nothing: a match hands the player back, and a decline has already
/// taken the pick. A released RFA (`NoBidToAuction`) is a plain free agent again, so its old
/// resolution is skipped here too.
#[instrument(skip(db))]
pub async fn find_rfa_obligations_for_team<C>(
    league_id: i64,
    end_of_season_year: i16,
    team_id: i64,
    db: &C,
) -> Result<Vec<RfaObligation>>
where
    C: ConnectionTrait,
{
    let winning_bids =
        auction_queries::find_winning_bids_for_team(team_id, league_id, end_of_season_year, db)
            .await?;

    let mut rfa_obligations = Vec::new();
    for (auction_id, bid_amount) in winning_bids {
        let auction_model = auction_queries::find_auction_by_id(auction_id, db).await?;
        let Some(rfa_resolution_model) =
            rfa_resolution_queries::find_rfa_resolution_for_contract(auction_model.contract_id, db)
                .await?
        else {
            continue;
        };
        if matches!(
            rfa_resolution_model.status,
            RfaResolutionStatus::AwaitingAuction
                | RfaResolutionStatus::AwaitingRaise
                | RfaResolutionStatus::AwaitingMatch
        ) {
            rfa_obligations.push(RfaObligation {
                rfa_resolution_id: rfa_resolution_model.id,
                bid_amount,
                announced_at: rfa_resolution_model.final_bid_at,
            });
        }
    }
    Ok(rfa_obligations)
}

/// The compensation round `team_id` could not pay once `proposed_obligation` joins its other live
/// RFA debts, or `None` when every debt can be settled (rules §15.3.3).
///
/// Two debts may not be paid with one pick, so this is a matching, not a per-bid check: the
/// strictest tier picks first and spends the weakest pick that will do, which leaves the earlier
/// rounds for the tiers that need them.
///
/// `proposed_obligation` replaces the team's existing debt on the same resolution — a raise or a
/// re-bid moves the price, it does not add a second player.
#[instrument(skip(db))]
pub async fn find_unpayable_rfa_obligation<C>(
    league_id: i64,
    end_of_season_year: i16,
    team_id: i64,
    proposed_obligation: RfaObligation,
    db: &C,
) -> Result<Option<i16>>
where
    C: ConnectionTrait,
{
    let mut rfa_obligations =
        find_rfa_obligations_for_team(league_id, end_of_season_year, team_id, db).await?;
    rfa_obligations.retain(|rfa_obligation| {
        rfa_obligation.rfa_resolution_id != proposed_obligation.rfa_resolution_id
    });
    rfa_obligations.push(proposed_obligation);

    let mut eligible_per_obligation = Vec::with_capacity(rfa_obligations.len());
    for rfa_obligation in rfa_obligations {
        let eligible_picks = eligible_compensation_picks(
            league_id,
            end_of_season_year,
            team_id,
            rfa_obligation.bid_amount,
            rfa_obligation.announced_at,
            db,
        )
        .await?;
        eligible_per_obligation.push((
            compensation_round_for_bid(rfa_obligation.bid_amount),
            eligible_picks,
        ));
    }
    eligible_per_obligation.sort_by_key(|(required_round, _)| *required_round);

    let mut spent_pick_ids = HashSet::new();
    for (required_round, eligible_picks) in eligible_per_obligation {
        // the eligible set runs best round first, so the last unspent pick is the weakest that pays
        let Some(pick_to_spend) = eligible_picks
            .iter()
            .rev()
            .find(|eligible_pick| !spent_pick_ids.contains(&eligible_pick.id))
        else {
            return Ok(Some(required_round));
        };
        spent_pick_ids.insert(pick_to_spend.id);
    }
    Ok(None)
}
