//! What the team that held the player at the keeper deadline does with him (rules §15.3).
//!
//! Two decision points share that owner and the discount right recorded in
//! `rfa_resolution.original_owner_team_id`:
//!
//! * somebody bid, so the owner matches the effective bid at a discount or declines and takes a
//!   draft pick instead (§15.3.2, §15.2);
//! * nobody bid, so the owner re-signs at the standard 4th-year salary or lets the player go to the
//!   regular free agent auction (§15.3.5).

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use fbkl_entity::{
    auction::{AuctionKind, AuctionStatus},
    auction_queries,
    contract::{self, FreeAgentException},
    contract_queries, deadline, draft_pick, draft_pick_queries,
    league_event::LeagueEventKind,
    rfa_resolution::{self, RfaResolutionStatus},
    rfa_resolution_queries,
    sea_orm::{
        ActiveModelTrait, ActiveValue, ConnectionTrait, TransactionSession, TransactionTrait,
        prelude::DateTimeWithTimeZone,
    },
    team_update::DraftPickUpdateType,
};
use tracing::instrument;

use crate::{
    auction::resolve_rfa_auction_to_winning_bid,
    roster::{SalarySnapshot, calculate_team_contract_salary_at_datetime},
};

use super::rfa_league_event::{
    find_rfa_handshake_deadline, insert_compensation_pick_team_update, insert_rfa_league_event,
    insert_rfa_resign_team_update,
};

/// What the original owner does with a winning bid on his restricted free agent (rules §15.3.2).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RfaMatchDecision {
    /// Re-sign the player at the effective bid less the discount.
    Match,
    /// Let the winner sign him and take a draft pick as compensation.
    Decline,
}

/// What the original owner does with a restricted free agent nobody bid on (rules §15.3.5).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnbidRfaDecision {
    /// Re-sign at the standard 4th-year salary the RFA contract already carries.
    Resign,
    /// Pass, sending the player to the regular free agent auction.
    ReleaseToAuction,
}

/// Settles the original owner's 48h window (rules §15.3.2).
///
/// On a decline the winner signs through the shared auction path and the pick his bid named
/// changes hands. A match leaves that pick where it is.
#[instrument(skip(db))]
pub async fn match_or_decline<C>(
    rfa_resolution_id: i64,
    original_owner_team_id: i64,
    decision: RfaMatchDecision,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let rfa_resolution_model =
        rfa_resolution_queries::find_rfa_resolution_by_id(rfa_resolution_id, db).await?;
    ensure!(
        rfa_resolution_model.status == RfaResolutionStatus::AwaitingMatch,
        "The match window for RFA resolution {rfa_resolution_id} is not open (status: {:?}).",
        rfa_resolution_model.status
    );
    ensure!(
        rfa_resolution_model.original_owner_team_id == original_owner_team_id,
        "Only the original owner may match or decline RFA resolution {rfa_resolution_id}."
    );
    let effective_bid = rfa_resolution_model
        .effective_bid()
        .ok_or_else(|| eyre!("RFA resolution {rfa_resolution_id} has no bid to match."))?;

    let deadline_model = find_rfa_handshake_deadline(&rfa_resolution_model, db).await?;
    let db_txn = db.begin().await?;
    match decision {
        RfaMatchDecision::Match => {
            resign_to_original_owner(
                &rfa_resolution_model,
                effective_bid,
                FreeAgentException::Held,
                &deadline_model,
                &db_txn,
            )
            .await?;
            if let Some(auction_id) = rfa_resolution_model.auction_id {
                auction_queries::update_auction_status(
                    auction_id,
                    AuctionStatus::Completed,
                    &db_txn,
                )
                .await?;
            }
        }
        RfaMatchDecision::Decline => {
            forfeit_pick_to_original_owner(
                &rfa_resolution_model,
                effective_bid,
                &deadline_model,
                &db_txn,
            )
            .await?;
        }
    }
    let final_status = match decision {
        RfaMatchDecision::Match => RfaResolutionStatus::Resolved,
        RfaMatchDecision::Decline => RfaResolutionStatus::Declined,
    };
    let finished_rfa_resolution = rfa_resolution_queries::finish_rfa_resolution(
        rfa_resolution_id,
        final_status,
        now,
        &db_txn,
    )
    .await?;
    db_txn.commit().await?;

    Ok(finished_rfa_resolution)
}

/// Settles a restricted free agent nobody bid on (rules §15.3.5). Call it once the veteran auction
/// has ended, which is when a resolution still sitting in `AwaitingAuction` is known to be unbid.
#[instrument(skip(db))]
pub async fn resolve_unbid_rfa<C>(
    rfa_resolution_id: i64,
    original_owner_team_id: i64,
    decision: UnbidRfaDecision,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let rfa_resolution_model =
        rfa_resolution_queries::find_rfa_resolution_by_id(rfa_resolution_id, db).await?;
    ensure!(
        rfa_resolution_model.status == RfaResolutionStatus::AwaitingAuction
            && rfa_resolution_model.auction_id.is_none(),
        "RFA resolution {rfa_resolution_id} drew a bid, so it settles through the raise/match windows (status: {:?}).",
        rfa_resolution_model.status
    );
    ensure!(
        rfa_resolution_model.original_owner_team_id == original_owner_team_id,
        "Only the original owner may settle unbid RFA resolution {rfa_resolution_id}."
    );

    let deadline_model = find_rfa_handshake_deadline(&rfa_resolution_model, db).await?;
    let db_txn = db.begin().await?;
    let rfa_contract_model = find_rfa_contract(&rfa_resolution_model, &db_txn).await?;
    let rfa_player_id = rfa_contract_model.player_id.ok_or_else(|| {
        eyre!(
            "RFA contract {} has no NBA player, so its auction cannot be found.",
            rfa_contract_model.id
        )
    })?;
    let (final_status, rfa_auction_status) = match decision {
        UnbidRfaDecision::Resign => {
            // Nobody bid, so the 10% discount comes off the carry salary itself (rules §15.3.5).
            resign_to_original_owner(
                &rfa_resolution_model,
                rfa_contract_model.salary,
                FreeAgentException::HeldNoBid,
                &deadline_model,
                &db_txn,
            )
            .await?;
            (RfaResolutionStatus::NoBidResigned, AuctionStatus::Completed)
        }
        UnbidRfaDecision::ReleaseToAuction => {
            // Expiring is how a player rejoins the free agent pool (rules §15.3.5).
            contract_queries::expire_contract(rfa_contract_model, &db_txn).await?;
            (RfaResolutionStatus::NoBidToAuction, AuctionStatus::Expired)
        }
    };
    settle_unbid_rfa_auction(
        &rfa_resolution_model,
        rfa_player_id,
        rfa_auction_status,
        &db_txn,
    )
    .await?;
    let finished_rfa_resolution = rfa_resolution_queries::finish_rfa_resolution(
        rfa_resolution_id,
        final_status,
        now,
        &db_txn,
    )
    .await?;
    db_txn.commit().await?;

    Ok(finished_rfa_resolution)
}

/// Signs the player back to the team that held him at the keeper deadline, at the discount that
/// team's contract earns (rules §15.3.2, §15.3.5).
///
/// `fa_exception` says which discount: `Held` floors a matched bid at the carry salary, `HeldNoBid`
/// discounts the carry salary itself.
async fn resign_to_original_owner<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    signing_amount: i16,
    fa_exception: FreeAgentException,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait,
{
    let rfa_contract_model = find_rfa_contract(rfa_resolution_model, db).await?;
    let original_owner_team_id = rfa_resolution_model.original_owner_team_id;

    let SalarySnapshot {
        salary: previous_salary,
        cap: previous_salary_cap,
    } = calculate_team_contract_salary_at_datetime(
        rfa_resolution_model.league_id,
        original_owner_team_id,
        deadline_model.date_time,
        db,
    )
    .await?;
    let signed_contract_model = contract_queries::sign_rfa_or_ufa_contract_to_team(
        rfa_contract_model,
        original_owner_team_id,
        signing_amount,
        fa_exception,
        db,
    )
    .await?;

    let league_event_model = insert_rfa_league_event(
        rfa_resolution_model,
        LeagueEventKind::RfaResign,
        Some(signed_contract_model.id),
        deadline_model,
        db,
    )
    .await?;
    insert_rfa_resign_team_update(
        &signed_contract_model,
        deadline_model,
        (previous_salary, previous_salary_cap),
        league_event_model.id,
        db,
    )
    .await?;

    Ok(signed_contract_model)
}

/// Hands the player to the winning bidder and one of the winner's draft picks to the original owner
/// (rules §15.2, §15.3.2).
async fn forfeit_pick_to_original_owner<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    effective_bid: i16,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<draft_pick::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let rfa_resolution_id = rfa_resolution_model.id;
    let auction_id = rfa_resolution_model.auction_id.ok_or_else(|| {
        eyre!("RFA resolution {rfa_resolution_id} has no auction, so nothing can be declined.")
    })?;
    let winning_team_id = rfa_resolution_model
        .winning_team_id
        .ok_or_else(|| eyre!("RFA resolution {rfa_resolution_id} has no winning bidder."))?;

    // The winning bid is what names the pick, so a decline only spends what is already there.
    let forfeited_draft_pick_id =
        rfa_resolution_queries::find_rfa_compensation_pick_for_resolution(rfa_resolution_id, db)
            .await?
            .ok_or_else(|| {
                eyre!("RFA resolution {rfa_resolution_id} has no compensation pick to forfeit.")
            })?
            .forfeited_draft_pick_id;
    let forfeited_draft_pick_model =
        draft_pick_queries::find_draft_pick_by_id(forfeited_draft_pick_id, db).await?;
    ensure!(
        forfeited_draft_pick_model.current_owner_team_id == winning_team_id,
        "Draft pick {forfeited_draft_pick_id} left team {winning_team_id} after it was named as compensation for RFA resolution {rfa_resolution_id}."
    );

    resolve_rfa_auction_to_winning_bid(auction_id, Some(effective_bid), None, db).await?;

    let mut draft_pick_to_move: draft_pick::ActiveModel = forfeited_draft_pick_model.into();
    draft_pick_to_move.current_owner_team_id =
        ActiveValue::Set(rfa_resolution_model.original_owner_team_id);
    let moved_draft_pick_model = draft_pick_to_move.update(db).await?;

    let league_event_model = insert_rfa_league_event(
        rfa_resolution_model,
        LeagueEventKind::RfaDeclineAndForfeit,
        None,
        deadline_model,
        db,
    )
    .await?;
    insert_compensation_pick_team_update(
        winning_team_id,
        &moved_draft_pick_model,
        DraftPickUpdateType::ForfeitedAsRfaCompensation,
        deadline_model,
        league_event_model.id,
        db,
    )
    .await?;
    insert_compensation_pick_team_update(
        rfa_resolution_model.original_owner_team_id,
        &moved_draft_pick_model,
        DraftPickUpdateType::AddViaRfaCompensation,
        deadline_model,
        league_event_model.id,
        db,
    )
    .await?;

    Ok(moved_draft_pick_model)
}

/// Closes out the RFA-week auction an unbid resolution leaves parked in `Closed`.
///
/// `end_veteran_auction` parks every RFA auction there for the handshake, but an unbid one never
/// writes its id into the resolution row, so it is found by player - a trade replaces the pooled
/// contract row and the resolution keeps pointing at the older one.
async fn settle_unbid_rfa_auction<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    rfa_player_id: i64,
    new_status: AuctionStatus,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    let maybe_auction_model = auction_queries::find_auction_for_player_in_season(
        rfa_resolution_model.league_id,
        rfa_resolution_model.end_of_season_year,
        rfa_player_id,
        AuctionKind::PreseasonVeteranAuction,
        db,
    )
    .await?;
    if let Some(auction_model) = maybe_auction_model
        && auction_model.status == AuctionStatus::Closed
    {
        auction_queries::update_auction_status(auction_model.id, new_status, db).await?;
    }
    Ok(())
}

async fn find_rfa_contract<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait,
{
    // A trade during the auction replaces the contract row, leaving the resolution pointing at the older one.
    let rfa_contract_model =
        contract_queries::find_contract_by_id(rfa_resolution_model.rfa_contract_id, db)
            .await?
            .get_latest_in_chain(db)
            .await?;
    ensure!(
        rfa_contract_model.status == contract::ContractStatus::Active,
        "RFA contract {} is no longer active, so RFA resolution {} cannot be settled from it.",
        rfa_contract_model.id,
        rfa_resolution_model.id
    );
    Ok(rfa_contract_model)
}
