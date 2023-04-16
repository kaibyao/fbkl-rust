use std::fmt::Debug;

use chrono::{DateTime, FixedOffset};
use color_eyre::{eyre::bail, Result};
use fbkl_entity::{
    auction::{self, AuctionType},
    auction_queries,
    contract::{self, ContractType},
    contract_queries,
    sea_orm::{ActiveModelTrait, ConnectionTrait, TransactionTrait},
};
use tracing::instrument;

static VALID_VETERAN_AUCTION_FA_TYPES: &[ContractType] = &[
    ContractType::RestrictedFreeAgent,
    ContractType::UnrestrictedFreeAgentOriginalTeam,
    ContractType::UnrestrictedFreeAgentVeteran,
];

/// Ends a veteran auction and creates the associated transaction + team contract OR expires the associated contract..
#[instrument]
pub async fn end_veteran_auction<C>(auction_id: i64, db: &C) -> Result<auction::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let auction_model = auction_queries::find_auction_by_id(auction_id, db).await?;
    let auction_contract_model = auction_model.get_contract(db).await?;
    let maybe_latest_bid = auction_model.get_latest_bid(db).await?;

    // Create contract for player <--> team
    match maybe_latest_bid {
        None => {
            // No one bid on the player; expire the contract. Player is now a free agent.
            let _expired_contract =
                contract_queries::expire_contract(auction_contract_model, db).await?;
        }
        Some(_winning_bid_model) => {
            // Sign contract to team
            let _signed_contract =
                contract_queries::sign_auction_contract_to_team(&auction_model, db).await?;
        }
    }

    // Create transaction

    todo!()
}

/// Creates a new veteran auction for a given player + league.
#[instrument]
pub async fn start_new_veteran_auction_for_nba_player<C>(
    league_id: i64,
    end_of_season_year: i16,
    player_id: i64,
    start_timestamp: DateTime<FixedOffset>,
    starting_bid_amount: i16,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let player_contract = get_or_create_player_contract_for_veteran_auction(
        league_id,
        end_of_season_year,
        player_id,
        db,
    )
    .await?;

    // Create the auction for it.
    let inserted_auction = auction_queries::insert_new_auction(
        player_contract.id,
        AuctionType::PreseasonVeteranAuction,
        starting_bid_amount,
        start_timestamp,
        None,
        db,
    )
    .await?;

    Ok(inserted_auction)
}

/// Either retrieves + validates an existing player contract that can be used for a new veteran auction, or creates one based on given arguments.
#[instrument]
async fn get_or_create_player_contract_for_veteran_auction<C>(
    league_id: i64,
    end_of_season_year: i16,
    player_id: i64,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
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
            contract::Model::new_contract_for_veteran_auction(
                league_id,
                end_of_season_year,
                player_id,
            )
            .insert(db)
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
