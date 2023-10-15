use std::{collections::HashMap, fmt::Debug};

use color_eyre::{
    eyre::{bail, ensure},
    Result,
};
use multimap::MultiMap;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, JoinType, ModelTrait,
    QueryFilter, QuerySelect, RelationTrait,
};
use tracing::instrument;

use crate::{
    auction,
    contract::{self, ContractStatus, ContractType},
    deadline::{self, DeadlineType},
    league_player, player,
    transaction::{self, TransactionType},
};

/// Moves a contract to IR and returns the new contract in the contract chain
pub async fn activate_contract_from_ir<C>(
    contract_model: contract::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_to_insert = contract_model.activate_from_ir();
    let updated_contract =
        add_replacement_contract_to_chain(contract_model, contract_to_insert, db).await?;

    Ok(updated_contract)
}

/// Activates a rookie development (+ international) contract
#[instrument]
pub async fn activate_rookie_development_contract<C>(
    contract_model: contract::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let activated_contract_to_insert = contract_model.activate_rookie()?;
    let activated_contract =
        add_replacement_contract_to_chain(contract_model, activated_contract_to_insert, db).await?;

    Ok(activated_contract)
}

/// Inserts the new/advanced contract and sets the status of the old one appropriately.
#[instrument]
pub async fn advance_contract<C>(
    current_contract_model: contract::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_to_advance = current_contract_model.create_annual_contract_advancement()?;
    add_replacement_contract_to_chain(current_contract_model, contract_to_advance, db).await
}

/// This is needed in order to set the `original_contract_id` after creating a new contract.
#[instrument]
pub async fn create_new_contract<C>(
    contract_to_insert: contract::ActiveModel,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let inserted_contract = contract_to_insert.insert(db).await?;
    let inserted_contract_id = inserted_contract.id;

    let mut model_to_update: contract::ActiveModel = inserted_contract.into();
    model_to_update.original_contract_id = ActiveValue::Set(Some(inserted_contract_id));

    let updated_contract = model_to_update.update(db).await?;
    Ok(updated_contract)
}

/// Inserts the "Dropped" contract as the next contract in the contract chain.
#[instrument]
pub async fn drop_contract<C>(
    current_contract_model: contract::Model,
    is_before_pre_season_keeper_deadline: bool,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_to_drop = if is_before_pre_season_keeper_deadline {
        current_contract_model.create_dropped_contract_before_preseason_keeper_deadline()?
    } else {
        current_contract_model.create_dropped_contract_after_preseason_keeper_deadline()?
    };

    add_replacement_contract_to_chain(current_contract_model, contract_to_drop, db).await
}

/// Expires the given contract.
#[instrument]
pub async fn expire_contract<C>(contract_model: contract::Model, db: &C) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_to_expire = contract_model.create_expired_contract()?;
    add_replacement_contract_to_chain(contract_model, contract_to_expire, db).await
}

/// Retrieves all contracts currently active in a league. Note that this includes Free Agent contracts where the player had been signed onto a team at some point but ended the season as a free agent.
#[instrument]
pub async fn find_active_contracts_in_league<C>(
    league_id: i64,
    db: &C,
) -> Result<Vec<contract::Model>>
where
    C: ConnectionTrait + Debug,
{
    let contracts = contract::Entity::find()
        .filter(
            contract::Column::LeagueId
                .eq(league_id)
                .and(contract::Column::Status.eq(contract::ContractStatus::Active)),
        )
        .all(db)
        .await?;

    Ok(contracts)
}

/// Finds active contracts that belong to the given teams.
#[instrument]
pub async fn find_active_contracts_by_teams<C>(
    team_ids: Vec<i64>,
    db: &C,
) -> Result<MultiMap<i64, contract::Model>>
where
    C: ConnectionTrait + Debug,
{
    let all_contracts = contract::Entity::find()
        .filter(
            contract::Column::TeamId
                .is_in(team_ids)
                .and(contract::Column::Status.eq(contract::ContractStatus::Active)),
        )
        .all(db)
        .await?;

    let mut active_contracts_by_team_id = MultiMap::new();
    for active_contract in all_contracts {
        active_contracts_by_team_id.insert(active_contract.team_id.expect("A contract model retrieved by filtering on team_id should have a non-empty team_id value."), active_contract);
    }

    Ok(active_contracts_by_team_id)
}

#[instrument]
pub async fn find_active_team_contract_by_player_name<C>(
    team_id: i64,
    player_name: &str,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut matching_contracts = contract::Entity::find()
        .join(JoinType::LeftJoin, contract::Relation::Player.def())
        .join(JoinType::LeftJoin, contract::Relation::LeaguePlayer.def())
        .filter(contract::Column::Status.eq(ContractStatus::Active))
        .filter(contract::Column::TeamId.eq(team_id))
        .filter(
            player::Column::Name
                .eq(player_name)
                .or(league_player::Column::Name.eq(player_name)),
        )
        .all(db)
        .await?;

    ensure!(
        matching_contracts.len() == 1,
        "Found more (or less) than 1 contract for player, '{}', for team with id = {}. Contract ids: {:?}",
        player_name,
        team_id,
        matching_contracts.iter().map(|c| c.id).collect::<Vec<_>>()
    );

    Ok(matching_contracts.swap_remove(0))
}

/// Retrieves all contracts currently in the given league that match the given list of player names.
#[instrument]
pub async fn find_active_league_contracts_by_player_names<C>(
    player_names: &[&str],
    league_id: i64,
    db: &C,
) -> Result<HashMap<String, contract::Model>>
where
    C: ConnectionTrait + Debug,
{
    let contracts_and_player_models = contract::Entity::find()
        .join(JoinType::LeftJoin, contract::Relation::Player.def())
        .join(JoinType::LeftJoin, contract::Relation::LeaguePlayer.def())
        .filter(
            contract::Column::LeagueId
                .eq(league_id)
                .and(contract::Column::Status.eq(contract::ContractStatus::Active))
                .and(
                    player::Column::Name
                        .is_in(
                            player_names
                                .iter()
                                .map(|player_name| player_name.to_string()),
                        )
                        .or(league_player::Column::Name.is_in(
                            player_names
                                .iter()
                                .map(|player_name| player_name.to_string()),
                        )),
                ),
        )
        .all(db)
        .await?;
    let mut contracts_by_player_name: HashMap<String, contract::Model> = HashMap::new();
    for contract_model in contracts_and_player_models {
        let maybe_related_player_model =
            contract_model.find_related(player::Entity).one(db).await?;
        let player_name = if let Some(related_player_model) = maybe_related_player_model {
            related_player_model.name
        } else {
            let maybe_related_league_player_model = contract_model
                .find_related(league_player::Entity)
                .one(db)
                .await?;
            match maybe_related_league_player_model {
                None => bail!(
                    "Could not find player or league_player related to contract id {}",
                    contract_model.id
                ),
                Some(related_league_player_model) => related_league_player_model.name,
            }
        };

        contracts_by_player_name.insert(player_name, contract_model);
    }

    Ok(contracts_by_player_name)
}

#[instrument]
pub async fn find_contracts_by_ids<C>(
    contract_ids: Vec<i64>,
    db: &C,
) -> Result<Vec<contract::Model>>
where
    C: ConnectionTrait + Debug,
{
    let contracts = contract::Entity::find()
        .filter(contract::Column::Id.is_in(contract_ids))
        .all(db)
        .await?;
    Ok(contracts)
}

#[instrument]
pub async fn find_contracts_dropped_by_team_in_regular_season<C>(
    team_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<contract::Model>>
where
    C: ConnectionTrait + Debug,
{
    let dropped_team_contracts = contract::Entity::find()
        .join(
            JoinType::LeftJoin,
            contract::Relation::DroppedContractTransaction.def(),
        )
        .join(JoinType::LeftJoin, transaction::Relation::Deadline.def())
        .filter(
            contract::Column::TeamId
                .eq(team_id)
                .and(contract::Column::EndOfSeasonYear.eq(end_of_season_year))
                .and(
                    transaction::Column::TransactionType
                        .eq(TransactionType::TeamUpdateDropContract),
                )
                .and(
                    deadline::Column::DeadlineType
                        .is_not_in([DeadlineType::PreseasonStart, DeadlineType::PreseasonKeeper]),
                ),
        )
        .all(db)
        .await?;
    Ok(dropped_team_contracts)
}

/// Moves a contract to IR and returns the new contract in the contract chain
pub async fn move_contract_to_ir<C>(
    contract_model: contract::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_to_insert = contract_model.move_to_ir();
    let updated_contract =
        add_replacement_contract_to_chain(contract_model, contract_to_insert, db).await?;

    Ok(updated_contract)
}

/// Moves a rookie development contract to international and returns the new contract in the contract chain.
pub async fn move_rd_contract_to_rdi<C>(
    rd_contract_model: contract::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_to_insert = rd_contract_model.move_rd_to_international(db).await?;
    let updated_contract =
        add_replacement_contract_to_chain(rd_contract_model, contract_to_insert, db).await?;

    Ok(updated_contract)
}

/// Moves a rookie development international contract to (stateside) rookie development and returns the new contract in the contract chain.
pub async fn move_rdi_contract_to_rd<C>(
    rdi_contract_model: contract::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_to_insert = rdi_contract_model.move_rdi_to_rd()?;
    let updated_contract =
        add_replacement_contract_to_chain(rdi_contract_model, contract_to_insert, db).await?;

    Ok(updated_contract)
}

/// Signs a contract to a team as a result of an auction ending (either the pre-season veteran auction or in-season FA auction).
pub async fn sign_auction_contract_to_team<C>(
    auction_model: &auction::Model,
    winning_bid_amount: i16,
    winning_team_id: i64,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_model = auction_model.get_contract(db).await?;

    let signed_contract_model_to_insert = match contract_model.contract_type {
        ContractType::RestrictedFreeAgent | ContractType::UnrestrictedFreeAgentOriginalTeam | ContractType::UnrestrictedFreeAgentVeteran => contract_model.sign_rfa_or_ufa_contract_to_team(winning_team_id, winning_bid_amount)?,
        ContractType::Veteran | ContractType::FreeAgent => contract_model.sign_veteran_contract_to_team(winning_team_id, winning_bid_amount)?,
        _ => bail!("Cannot sign a contract won via auction to a team if the contract was not a valid free agent contract type. (auction_id = {}, contract_id = {})", auction_model.id, contract_model.id),
    };

    add_replacement_contract_to_chain(contract_model, signed_contract_model_to_insert, db).await
}

/// Trades a contract to a new team
pub async fn trade_contract_to_team<C>(
    contract_model: contract::Model,
    new_team_id: i64,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let new_contract: contract::ActiveModel = contract_model.trade_contract_to_team(new_team_id);
    add_replacement_contract_to_chain(contract_model, new_contract, db).await
}

/// Used to replace an existing contract with a new one. The new one refers to the original as its original_contract_id, and the old one's status is set to `Replaced`.
/// Returns the new replacement contract.
#[instrument]
async fn add_replacement_contract_to_chain<C>(
    current_contract_model: contract::Model,
    replacement_contract_model: contract::ActiveModel,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut original_contract_model_to_update: contract::ActiveModel =
        current_contract_model.into();
    original_contract_model_to_update.status = ActiveValue::Set(contract::ContractStatus::Replaced);
    let _updated_original_contract = original_contract_model_to_update.update(db).await?;

    let inserted_replacement_contract = replacement_contract_model.insert(db).await?;

    Ok(inserted_replacement_contract)
}

#[instrument]
pub async fn validate_contract_is_latest_in_chain<C>(
    contract_model: &contract::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let is_latest = contract_model.is_latest_in_chain(db).await?;

    ensure!(
        is_latest,
        "contract_model with id ({}) is not the latest in its chain.",
        contract_model.id
    );

    Ok(())
}
