use std::fmt::Debug;

use chrono::NaiveDate;
use color_eyre::{
    Result,
    eyre::{bail, ensure, eyre},
};
use fbkl_entity::{
    auction::{self, AuctionStatus},
    auction_bid, auction_queries,
    contract::{self, ContractKind},
    contract_queries,
    deadline::DeadlineKind,
    deadline_queries,
    sea_orm::{ConnectionTrait, TransactionSession, TransactionTrait},
    team_update_queries,
};
use tracing::instrument;

use super::sign_auction_contract_to_team;
use crate::deadline_processing::open_raise_window_for_closed_auction;

pub static VALID_VETERAN_AUCTION_FA_TYPES: &[ContractKind] = &[
    ContractKind::FreeAgent,
    ContractKind::RestrictedFreeAgent,
    ContractKind::UnrestrictedFreeAgentOriginalTeam,
    ContractKind::UnrestrictedFreeAgentVeteran,
];

/// What closing an auction should do with its pooled contract.
#[derive(Debug, Eq, PartialEq)]
pub enum AuctionCloseOutcome {
    /// RFA auctions close without signing; the raise/match flow completes them (rules §6.2.2.3).
    AwaitRfaResolution,
    /// Nobody bid, so the contract expires and the player returns to the free agent pool.
    Expire,
    /// Sign the winning bidder's contract.
    Sign,
}

/// Routes an auction close by pooled contract kind and whether anyone bid.
#[must_use]
pub const fn auction_close_outcome(
    pooled_contract_kind: ContractKind,
    has_winning_bid: bool,
) -> AuctionCloseOutcome {
    match (pooled_contract_kind, has_winning_bid) {
        (ContractKind::RestrictedFreeAgent, _) => AuctionCloseOutcome::AwaitRfaResolution,
        (_, false) => AuctionCloseOutcome::Expire,
        (_, true) => AuctionCloseOutcome::Sign,
    }
}

/// Ends a veteran auction and creates the associated transaction + team contract OR expires the associated contract.
#[instrument(skip(db))]
pub async fn end_veteran_auction<C>(
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
                open_raise_window_for_closed_auction(
                    &auction_model,
                    &auction_contract_model,
                    maybe_latest_bid.as_ref(),
                    &db_txn,
                )
                .await?;
                auction_contract_model
            }
            AuctionCloseOutcome::Expire => {
                // No one bid on the player; expire the contract. Player is now a free agent.
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

                sign_winning_bid(
                    &auction_model,
                    &auction_contract_model,
                    &winning_bid_model,
                    None,
                    maybe_override_effective_date,
                    &db_txn,
                )
                .await?
            }
        };

    db_txn.commit().await?;

    Ok(final_contract_model)
}

/// Hands a closed auction's pooled contract to the winning bidder, with its transaction and
/// `team_update`.
async fn sign_winning_bid<C>(
    auction_model: &auction::Model,
    auction_contract_model: &contract::Model,
    winning_bid_model: &auction_bid::Model,
    maybe_raised_bid_amount: Option<i16>,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    // A veteran signing takes effect when the preseason FA auction opens, which is when the veteran auction ends.
    let preseason_fa_auction_start_deadline_model =
        deadline_queries::find_deadline_for_season_by_type(
            auction_contract_model.league_id,
            auction_contract_model.end_of_season_year,
            DeadlineKind::PreseasonFaAuctionStart,
            db,
        )
        .await?;

    let (signed_contract_model, _, team_update_model) = sign_auction_contract_to_team(
        auction_model,
        winning_bid_model,
        &preseason_fa_auction_start_deadline_model,
        maybe_raised_bid_amount,
        None,
        db,
    )
    .await?;

    // Update the team_update's effective date + status, as they happen immediately.
    team_update_queries::update_team_update_for_auction(
        &team_update_model,
        maybe_override_effective_date,
        db,
    )
    .await?;

    auction_queries::update_auction_status(auction_model.id, AuctionStatus::Completed, db).await?;

    Ok(signed_contract_model)
}

/// Completes a closed RFA auction the way the original team declining to match completes it: the
/// winning bidder signs the player (rules §6.5).
///
/// [`end_veteran_auction`] leaves an RFA auction `Closed` with its pooled contract untouched,
/// because the raise/match exchange decides who signs. The decline branch of that exchange
/// (`fbkl_logic::deadline_processing::match_or_decline`) calls this, passing the effective bid so a
/// raised bid is what the winner pays.
///
/// `maybe_effective_bid` defaults to the winning bid when the winner never raised.
#[instrument(skip(db))]
pub async fn resolve_rfa_auction_to_winning_bid<C>(
    auction_id: i64,
    maybe_effective_bid: Option<i16>,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let auction_model = auction_queries::find_auction_by_id(auction_id, db).await?;
    let auction_contract_model = auction_model.get_contract(db).await?;
    ensure!(
        auction_contract_model.kind == ContractKind::RestrictedFreeAgent,
        "Only an RFA auction awaits resolution; auction {auction_id} pooled a {:?} contract.",
        auction_contract_model.kind
    );

    let db_txn = db.begin().await?;
    let winning_bid_model = auction_model
        .get_latest_bid(&db_txn)
        .await?
        .ok_or_else(|| eyre!("Expected a winning bid for RFA auction {auction_id}."))?;
    let signed_contract_model = sign_winning_bid(
        &auction_model,
        &auction_contract_model,
        &winning_bid_model,
        maybe_effective_bid,
        maybe_override_effective_date,
        &db_txn,
    )
    .await?;
    db_txn.commit().await?;

    Ok(signed_contract_model)
}

/// Either retrieves + validates an existing player contract that can be used for a new veteran auction, or creates one based on given arguments.
#[instrument(skip(db))]
pub async fn get_or_create_player_contract_for_veteran_auction<C>(
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
        .find(|contract_model| contract_model.player_id == Some(player_id));
    let player_contract = match maybe_existing_contract {
        None => {
            // Create new contract
            contract_queries::create_new_contract(
                contract::Model::new_contract_for_auction(league_id, end_of_season_year, player_id),
                db,
            )
            .await?
        }
        Some(existing_player_contract) => {
            if !VALID_VETERAN_AUCTION_FA_TYPES.contains(&existing_player_contract.kind) {
                // If another type of active contract exists for this player by this point, something went wrong.
                // The Keeper deadline should have caused all non-active contracts to be dropped & expired.
                bail!(
                    "Existing player contract is not a valid RFA/UFA type. Contract:\n{:#?}",
                    existing_player_contract
                );
            }
            existing_player_contract
        }
    };
    Ok(player_contract)
}

#[cfg(test)]
mod tests {
    use super::{AuctionCloseOutcome, auction_close_outcome};
    use fbkl_entity::contract::ContractKind;

    #[test]
    fn a_no_bid_auction_expires_its_contract() {
        assert_eq!(
            auction_close_outcome(ContractKind::FreeAgent, false),
            AuctionCloseOutcome::Expire
        );
        assert_eq!(
            auction_close_outcome(ContractKind::FreeAgent, true),
            AuctionCloseOutcome::Sign
        );
    }

    #[test]
    fn an_rfa_auction_closes_without_signing() {
        assert_eq!(
            auction_close_outcome(ContractKind::RestrictedFreeAgent, true),
            AuctionCloseOutcome::AwaitRfaResolution
        );
    }
}
