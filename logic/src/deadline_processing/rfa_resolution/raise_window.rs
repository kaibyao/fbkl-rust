//! The winning bidder's 48h window after an RFA auction closes (rules §15.3.2.1).
//!
//! Closing the auction signs nobody. It hands the bid facts to the resolution row seeded at the
//! keeper deadline and starts this window, in which the winner may raise its own bid once. Raising
//! or standing pat both end the window and open the winner's 24h pick-selection window.

use chrono::TimeDelta;
use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use fbkl_constants::league_rules::{RFA_PICK_SELECTION_WINDOW_HOURS, RFA_RAISE_WINDOW_HOURS};
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
    RfaObligation, find_unpayable_rfa_obligation,
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

/// The winner raises its own winning bid (rules §15.3.2.1), which moves the handshake on to naming
/// the compensation pick that the higher price now owes.
///
/// Rules §15.3.3 forbids a raise into a compensation tier the winner cannot pay, so a raise that
/// would leave no forfeitable pick is rejected before anything is written.
#[instrument(skip(db))]
pub async fn raise_bid<C>(
    rfa_resolution_id: i64,
    raising_team_id: i64,
    new_bid: i16,
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

    let maybe_unpayable_round = find_unpayable_rfa_obligation(
        rfa_resolution_model.league_id,
        rfa_resolution_model.end_of_season_year,
        raising_team_id,
        RfaObligation {
            rfa_resolution_id,
            bid_amount: new_bid,
            announced_at: rfa_resolution_model.final_bid_at,
        },
        db,
    )
    .await?;
    ensure!(
        maybe_unpayable_round.is_none(),
        "Raising to ${new_bid} would owe a draft pick that team {raising_team_id} cannot forfeit (rules §15.3.3)."
    );

    let deadline_model = find_rfa_handshake_deadline(&rfa_resolution_model, db).await?;
    let db_txn = db.begin().await?;
    insert_rfa_transaction(
        &rfa_resolution_model,
        TransactionKind::RfaRaiseBid,
        None,
        &deadline_model,
        &db_txn,
    )
    .await?;
    let updated_rfa_resolution = rfa_resolution_queries::open_rfa_pick_selection_window(
        rfa_resolution_id,
        Some(new_bid),
        pick_selection_deadline_from(now)?,
        &db_txn,
    )
    .await?;
    db_txn.commit().await?;

    Ok(updated_rfa_resolution)
}

/// The winner stands pat, which opens the pick-selection window straight away instead of waiting
/// the full 48h out (rules §15.3.2.1). Standing pat writes no transaction: nothing changed.
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
    rfa_resolution_queries::open_rfa_pick_selection_window(
        rfa_resolution_id,
        None,
        pick_selection_deadline_from(now)?,
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

fn pick_selection_deadline_from(now: DateTimeWithTimeZone) -> Result<DateTimeWithTimeZone> {
    now.checked_add_signed(TimeDelta::hours(RFA_PICK_SELECTION_WINDOW_HOURS))
        .ok_or_else(|| eyre!("Pick selection deadline overflowed from {now}."))
}
