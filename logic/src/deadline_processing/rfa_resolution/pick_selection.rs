//! The winning bidder's 24h window to name the pick he would forfeit (rules §15.2.2).
//!
//! Rules §15.2.2 gives the choice to the winner, but the original owner has to know what declining
//! would earn him before he decides. So the choice gets a window of its own between the two 48h
//! ones: the raise window settles, the winner names a pick, and only then does the owner's match
//! window open. The named pick is written to `rfa_compensation_pick` straight away and changes
//! hands later, and only if the owner declines.

use chrono::TimeDelta;
use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use fbkl_constants::league_rules::{RFA_MATCH_WINDOW_HOURS, compensation_round_for_bid};
use fbkl_entity::{
    draft_pick,
    rfa_resolution::{self, RfaResolutionStatus},
    rfa_resolution_queries::{self, NewRfaCompensationPick},
    sea_orm::{
        ConnectionTrait, TransactionSession, TransactionTrait, prelude::DateTimeWithTimeZone,
    },
};
use tracing::{instrument, warn};

use crate::rookie_draft::find_season_draft_pick_order;

use super::compute_eligible_compensation_picks;

/// The winner names the pick he would forfeit, which opens the original owner's window straight
/// away instead of waiting the full 24h out (rules §15.2.2).
#[instrument(skip(db))]
pub async fn select_compensation_pick<C>(
    rfa_resolution_id: i64,
    selecting_team_id: i64,
    draft_pick_id: i64,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let rfa_resolution_model = find_selectable_rfa_resolution(rfa_resolution_id, db).await?;
    ensure!(
        rfa_resolution_model.winning_team_id == Some(selecting_team_id),
        "Only the winning bidder may name the compensation pick for RFA resolution {rfa_resolution_id}."
    );

    let chosen_draft_pick_model = compute_eligible_compensation_picks(&rfa_resolution_model, db)
        .await?
        .into_iter()
        .find(|eligible_pick| eligible_pick.id == draft_pick_id)
        .ok_or_else(|| {
            eyre!("Draft pick {draft_pick_id} cannot settle RFA resolution {rfa_resolution_id}.")
        })?;

    settle_pick_selection(&rfa_resolution_model, &chosen_draft_pick_model, now, db).await
}

/// Nobody named a pick inside the 24h, so the league names the one that costs the winner least
/// (rules §15.2.2) and the original owner's window opens.
#[instrument(skip(db))]
pub async fn expire_pick_selection_window<C>(
    rfa_resolution_id: i64,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let rfa_resolution_model = find_selectable_rfa_resolution(rfa_resolution_id, db).await?;
    let eligible_draft_picks =
        compute_eligible_compensation_picks(&rfa_resolution_model, db).await?;
    let chosen_draft_pick_model =
        lowest_ranked_eligible_pick(&rfa_resolution_model, &eligible_draft_picks, db).await?;

    settle_pick_selection(&rfa_resolution_model, chosen_draft_pick_model, now, db).await
}

/// The eligible pick that costs the winner least: the one latest in rookie-draft order (§7.2.1).
///
/// Intra-round order comes from the season's draft slate or, before the draft starts, from the
/// lottery draw. A season whose lottery has not been drawn yet has no order to rank by, so the
/// highest round number stands in for it.
async fn lowest_ranked_eligible_pick<'a, C>(
    rfa_resolution_model: &rfa_resolution::Model,
    eligible_draft_picks: &'a [draft_pick::Model],
    db: &C,
) -> Result<&'a draft_pick::Model>
where
    C: ConnectionTrait,
{
    let maybe_draft_pick_order = find_season_draft_pick_order(
        rfa_resolution_model.league_id,
        rfa_resolution_model.end_of_season_year,
        db,
    )
    .await?;
    if maybe_draft_pick_order.is_none() {
        warn!(
            rfa_resolution_id = rfa_resolution_model.id,
            "No rookie draft lottery has been drawn, so the highest-round eligible pick is forfeited."
        );
    }

    // Unranked picks key on `None` and `max_by_key` keeps the last of those, the highest round.
    eligible_draft_picks
        .iter()
        .max_by_key(|eligible_pick| {
            maybe_draft_pick_order.as_ref().and_then(|draft_pick_order| {
                draft_pick_order
                    .iter()
                    .position(|ordered_pick_id| *ordered_pick_id == eligible_pick.id)
            })
        })
        .ok_or_else(|| {
            eyre!(
                "The winner of RFA resolution {} holds no draft pick that can settle it (rules §15.3.3).",
                rfa_resolution_model.id
            )
        })
}

/// Records the named pick and opens the original owner's 48h window.
async fn settle_pick_selection<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    chosen_draft_pick_model: &draft_pick::Model,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let rfa_resolution_id = rfa_resolution_model.id;
    let winning_team_id = rfa_resolution_model
        .winning_team_id
        .ok_or_else(|| eyre!("RFA resolution {rfa_resolution_id} has no winning bidder."))?;
    let effective_bid = rfa_resolution_model.effective_bid().ok_or_else(|| {
        eyre!("RFA resolution {rfa_resolution_id} has no bid to price a pick from.")
    })?;
    let match_deadline_at = now
        .checked_add_signed(TimeDelta::hours(RFA_MATCH_WINDOW_HOURS))
        .ok_or_else(|| eyre!("Match deadline overflowed from {now}."))?;

    let db_txn = db.begin().await?;
    rfa_resolution_queries::insert_rfa_compensation_pick(
        NewRfaCompensationPick {
            rfa_resolution_id,
            required_round: compensation_round_for_bid(effective_bid),
            forfeited_draft_pick_id: Some(chosen_draft_pick_model.id),
            to_team_id: rfa_resolution_model.original_owner_team_id,
            from_team_id: winning_team_id,
        },
        &db_txn,
    )
    .await?;
    let updated_rfa_resolution = rfa_resolution_queries::open_rfa_match_window(
        rfa_resolution_id,
        match_deadline_at,
        &db_txn,
    )
    .await?;
    db_txn.commit().await?;

    Ok(updated_rfa_resolution)
}

/// Reads the resolution and checks its pick-selection window is the one that is open.
async fn find_selectable_rfa_resolution<C>(
    rfa_resolution_id: i64,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait,
{
    let rfa_resolution_model =
        rfa_resolution_queries::find_rfa_resolution_by_id(rfa_resolution_id, db).await?;
    ensure!(
        rfa_resolution_model.status == RfaResolutionStatus::AwaitingPickSelection,
        "The pick selection window for RFA resolution {rfa_resolution_id} is not open (status: {:?}).",
        rfa_resolution_model.status
    );
    Ok(rfa_resolution_model)
}
