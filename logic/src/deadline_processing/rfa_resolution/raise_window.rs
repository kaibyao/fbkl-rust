//! The winning bidder's 48h window after an RFA auction closes (rules §15.3.2.1).
//!
//! Closing the auction signs nobody. It hands the bid facts to the resolution row seeded at the
//! keeper deadline and starts this window, in which the winner may raise its own bid once. Raising
//! or standing pat both end the window and open the original owner's 48h window.
//!
//! A raise moves the price into a possibly stricter compensation tier, so it names the pick it
//! would forfeit the same way the original bid did (rules §15.3.3).

use chrono::TimeDelta;
use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use fbkl_constants::league_rules::{
    RFA_MATCH_WINDOW_HOURS, RFA_RAISE_WINDOW_HOURS, compensation_round_for_bid,
};
use fbkl_entity::{
    auction, auction_bid, contract,
    rfa_resolution::{self, RfaResolutionStatus},
    rfa_resolution_queries::{self, ClosedRfaAuctionResult},
    sea_orm::{
        ConnectionTrait, TransactionSession, TransactionTrait, prelude::DateTimeWithTimeZone,
    },
    transaction::TransactionKind,
};
use tracing::{instrument, warn};

use super::{
    find_eligible_compensation_pick, name_compensation_pick,
    rfa_transaction::{find_rfa_handshake_deadline, insert_rfa_transaction},
};

/// Fills the seeded resolution in from the auction that just closed and starts the raise window.
///
/// Returns `None` when there is no handshake to run: nobody bid (rules §15.3.5 covers that through
/// `resolve_unbid_rfa`), the contract was unowned at the keeper deadline so no resolution was
/// seeded, or the window is already open.
#[instrument(skip(db))]
pub async fn open_raise_window_for_closed_auction<C>(
    auction_model: &auction::Model,
    auction_contract_model: &contract::Model,
    maybe_winning_bid_model: Option<&auction_bid::Model>,
    db: &C,
) -> Result<Option<rfa_resolution::Model>>
where
    C: ConnectionTrait,
{
    let Some(winning_bid_model) = maybe_winning_bid_model else {
        return Ok(None);
    };
    let Some(rfa_resolution_model) =
        rfa_resolution_queries::find_rfa_resolution_for_contract(auction_contract_model.id, db)
            .await?
    else {
        warn!(
            contract_id = auction_contract_model.id,
            "Closed RFA auction has no resolution row, so no raise window opens for it."
        );
        return Ok(None);
    };
    if rfa_resolution_model.status != RfaResolutionStatus::AwaitingAuction {
        warn!(
            rfa_resolution_id = rfa_resolution_model.id,
            status = ?rfa_resolution_model.status,
            "RFA resolution has already moved past its auction, so the raise window stays as it is."
        );
        return Ok(None);
    }

    let raise_deadline_at = auction_model
        .close_at_timestamp
        .checked_add_signed(TimeDelta::hours(RFA_RAISE_WINDOW_HOURS))
        .ok_or_else(|| {
            eyre!(
                "Raise deadline for auction {} overflowed.",
                auction_model.id
            )
        })?;
    let winning_team_model = winning_bid_model.get_team(db).await?;

    let opened_rfa_resolution = rfa_resolution_queries::open_rfa_raise_window(
        rfa_resolution_model.id,
        ClosedRfaAuctionResult {
            auction_id: auction_model.id,
            winning_team_id: winning_team_model.id,
            final_bid: winning_bid_model.bid_amount,
            final_bid_at: winning_bid_model.created_at,
            raise_deadline_at,
        },
        db,
    )
    .await?;
    Ok(Some(opened_rfa_resolution))
}

/// The winner raises its own winning bid (rules §15.3.2.1), which opens the original owner's window.
///
/// The higher price may owe a better pick, so the raise names the pick it would forfeit. Rules
/// §15.3.3 forbids a raise into a tier the winner cannot pay, which is what naming an ineligible
/// pick means here.
#[instrument(skip(db))]
pub async fn raise_bid<C>(
    rfa_resolution_id: i64,
    raising_team_id: i64,
    new_bid: i16,
    compensation_draft_pick_id: i64,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let rfa_resolution_model =
        find_raisable_rfa_resolution(rfa_resolution_id, raising_team_id, db).await?;
    let final_bid = rfa_resolution_model
        .final_bid
        .ok_or_else(|| eyre!("RFA resolution {rfa_resolution_id} has no winning bid to raise."))?;
    ensure!(
        new_bid > final_bid,
        "A raise must beat the winning bid of ${final_bid}; ${new_bid} does not."
    );

    let required_round = compensation_round_for_bid(new_bid);
    let eligible_draft_pick = find_eligible_compensation_pick(
        &rfa_resolution_model,
        raising_team_id,
        required_round,
        compensation_draft_pick_id,
        db,
    )
    .await?
    .ok_or_else(|| {
        eyre!(
            "Raising to ${new_bid} owes a round {required_round} or better pick, and draft pick {compensation_draft_pick_id} cannot settle it (rules §15.3.3)."
        )
    })?;

    let deadline_model = find_rfa_handshake_deadline(&rfa_resolution_model, db).await?;
    let db_txn = db.begin().await?;
    name_compensation_pick(
        &rfa_resolution_model,
        raising_team_id,
        required_round,
        &eligible_draft_pick,
        &db_txn,
    )
    .await?;
    insert_rfa_transaction(
        &rfa_resolution_model,
        TransactionKind::RfaRaiseBid,
        None,
        &deadline_model,
        &db_txn,
    )
    .await?;
    let updated_rfa_resolution = rfa_resolution_queries::open_rfa_match_window(
        rfa_resolution_id,
        Some(new_bid),
        match_deadline_from(now)?,
        &db_txn,
    )
    .await?;
    db_txn.commit().await?;

    Ok(updated_rfa_resolution)
}

/// The winner stands pat (rules §15.3.2.1).
///
/// That opens the original owner's window straight away instead of waiting the full 48h out.
/// Standing pat writes no transaction: nothing changed, and the pick the winning bid named stands.
#[instrument(skip(db))]
pub async fn decline_to_raise<C>(
    rfa_resolution_id: i64,
    raising_team_id: i64,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait,
{
    find_raisable_rfa_resolution(rfa_resolution_id, raising_team_id, db).await?;
    rfa_resolution_queries::open_rfa_match_window(
        rfa_resolution_id,
        None,
        match_deadline_from(now)?,
        db,
    )
    .await
}

/// Reads the resolution and checks that this team may act on it right now.
async fn find_raisable_rfa_resolution<C>(
    rfa_resolution_id: i64,
    raising_team_id: i64,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait,
{
    let rfa_resolution_model =
        rfa_resolution_queries::find_rfa_resolution_by_id(rfa_resolution_id, db).await?;
    ensure!(
        rfa_resolution_model.status == RfaResolutionStatus::AwaitingRaise,
        "The raise window for RFA resolution {rfa_resolution_id} is not open (status: {:?}).",
        rfa_resolution_model.status
    );
    ensure!(
        rfa_resolution_model.winning_team_id == Some(raising_team_id),
        "Only the winning bidder may act in the raise window for RFA resolution {rfa_resolution_id}."
    );
    Ok(rfa_resolution_model)
}

fn match_deadline_from(now: DateTimeWithTimeZone) -> Result<DateTimeWithTimeZone> {
    now.checked_add_signed(TimeDelta::hours(RFA_MATCH_WINDOW_HOURS))
        .ok_or_else(|| eyre!("Match deadline overflowed from {now}."))
}
