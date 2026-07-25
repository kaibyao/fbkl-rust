use std::fmt::Debug;

use chrono::NaiveDate;
use color_eyre::{Result, eyre::eyre};
use fbkl_constants::league_rules::IN_SEASON_FA_MINIMUM_BID;
use fbkl_entity::{
    auction::{self, AuctionKind, AuctionStatus},
    auction_queries,
    contract::{self, ContractKind},
    contract_queries, deadline,
    sea_orm::{ConnectionTrait, TransactionTrait, prelude::DateTimeWithTimeZone},
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
    let (signed_contract_model, _, _team_update_model) = sign_auction_contract_to_team(
        &auction_model,
        &winning_bid_model,
        deadline_model,
        maybe_override_effective_date,
        &db_txn,
    )
    .await?;

    auction_queries::update_auction_status(auction_model.id, AuctionStatus::Completed, &db_txn)
        .await?;

    db_txn.commit().await?;

    Ok(signed_contract_model)
}

/// Opens a new in-season free agent auction for a player (rules §8.3).
///
/// `fixed_end` is the week's all-bid deadline (§8.2.1); bids may still roll it forward.
#[instrument]
pub async fn open_in_season_fa_auction<C>(
    league_id: i64,
    end_of_season_year: i16,
    player_id: i64,
    now: DateTimeWithTimeZone,
    fixed_end: DateTimeWithTimeZone,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    let pooled_contract =
        get_or_create_player_contract_for_fa_auction(league_id, end_of_season_year, player_id, db)
            .await?;
    let minimum_bid_amount = in_season_fa_minimum_bid(&pooled_contract, db).await?;

    auction_queries::insert_new_auction(
        pooled_contract.id,
        AuctionKind::InSeasonFreeAgent,
        minimum_bid_amount,
        now,
        Some(fixed_end),
        None,
        db,
    )
    .await
}

/// The opening bid an in-season free agent auction starts at (rules §8.3.3).
///
/// $1 unless the player was already owned this season — then their previous in-season salary is the
/// floor, RD/RDI contracts included.
#[instrument]
pub async fn in_season_fa_minimum_bid<C>(pooled_contract: &contract::Model, db: &C) -> Result<i16>
where
    C: ConnectionTrait + Debug,
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

    use super::previous_in_season_salary;

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
