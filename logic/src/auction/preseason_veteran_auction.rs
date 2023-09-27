use std::fmt::Debug;

use chrono::NaiveDate;
use color_eyre::{eyre::bail, Result};
use fbkl_entity::{
    auction_queries,
    contract::{self, ContractType},
    contract_queries,
    deadline::DeadlineType,
    deadline_queries,
    sea_orm::{ConnectionTrait, TransactionTrait},
    team_update_queries,
};
use tracing::instrument;

use super::sign_auction_contract_to_team;

pub static VALID_VETERAN_AUCTION_FA_TYPES: &[ContractType] = &[
    ContractType::RestrictedFreeAgent,
    ContractType::UnrestrictedFreeAgentOriginalTeam,
    ContractType::UnrestrictedFreeAgentVeteran,
];

/// Ends a veteran auction and creates the associated transaction + team contract OR expires the associated contract.
#[instrument]
pub async fn end_veteran_auction<C>(
    auction_id: i64,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let auction_model = auction_queries::find_auction_by_id(auction_id, db).await?;
    let auction_contract_model = auction_model.get_contract(db).await?;

    // Create contract for player <--> team
    let db_txn = db.begin().await?;

    let maybe_latest_bid = auction_model.get_latest_bid(&db_txn).await?;
    let final_contract_model = match maybe_latest_bid {
        None => {
            // No one bid on the player; expire the contract. Player is now a free agent.
            contract_queries::expire_contract(auction_contract_model, &db_txn).await?
        }
        Some(winning_bid_model) => {
            // Find preseason FA auction start deadline model, as that only starts at the end of the veteran auction
            let preseason_fa_auction_start_deadline_model =
                deadline_queries::find_deadline_for_season_by_type(
                    auction_contract_model.league_id,
                    auction_contract_model.end_of_season_year,
                    DeadlineType::PreseasonFaAuctionStart,
                    &db_txn,
                )
                .await?;

            let (signed_contract_model, _, team_update_model) = sign_auction_contract_to_team(
                &auction_model,
                &winning_bid_model,
                &preseason_fa_auction_start_deadline_model,
                &db_txn,
            )
            .await?;

            // Update the team_update's effective date + status, as they happen immediately.
            team_update_queries::update_team_update_for_preseason_veteran_auction(
                &team_update_model,
                maybe_override_effective_date,
                &db_txn,
            )
            .await?;

            signed_contract_model
        }
    };

    db_txn.commit().await?;

    Ok(final_contract_model)
}

/// Either retrieves + validates an existing player contract that can be used for a new veteran auction, or creates one based on given arguments.
#[instrument]
pub async fn get_or_create_player_contract_for_veteran_auction<C>(
    league_id: i64,
    end_of_season_year: i16,
    player_id: i64,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let maybe_existing_contract = contract_queries::find_active_contracts_in_league(league_id, db)
        .await?
        .into_iter()
        .find(|contract_model| {
            contract_model
                .player_id
                .map_or(false, |contract_player_id| contract_player_id == player_id)
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
        Some(existing_player_contract) => {
            if !VALID_VETERAN_AUCTION_FA_TYPES.contains(&existing_player_contract.contract_type) {
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
