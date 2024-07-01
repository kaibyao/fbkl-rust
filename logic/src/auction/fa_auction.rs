use std::fmt::Debug;

use chrono::NaiveDate;
use color_eyre::{eyre::eyre, Result};
use fbkl_entity::{
    auction_queries,
    contract::{self, ContractKind},
    contract_queries, deadline,
    sea_orm::{ConnectionTrait, TransactionTrait},
};
use tracing::instrument;

use super::sign_auction_contract_to_team;

/// Ends a free agent auction and creates the associated transaction + team contract OR expires the associated contract.
#[instrument]
pub async fn end_fa_auction<C>(
    deadline_model: &deadline::Model,
    auction_id: i64,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let auction_model = auction_queries::find_auction_by_id(auction_id, db).await?;

    // Create contract for player <--> team
    let db_txn = db.begin().await?;

    let winning_bid_model = auction_model
        .get_latest_bid(&db_txn)
        .await?
        .ok_or_else(|| {
            eyre!(
                "Expected a bid to exist for FA auction (auction_id = {})",
                auction_model.id
            )
        })?;

    // Find preseason FA auction start deadline model, as that only starts at the end of the veteran auction
    let (signed_contract_model, _, _team_update_model) =
        sign_auction_contract_to_team(&auction_model, &winning_bid_model, deadline_model, &db_txn)
            .await?;

    db_txn.commit().await?;

    Ok(signed_contract_model)
}

/// Either retrieves + validates an existing player contract that can be used for a new free agent auction, or creates one based on given arguments.
#[instrument]
pub async fn get_or_create_player_contract_for_fa_auction<C>(
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
