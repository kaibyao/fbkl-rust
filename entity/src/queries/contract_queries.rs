use std::{collections::HashMap, fmt::Debug};

use color_eyre::{eyre::bail, Result};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, JoinType, ModelTrait,
    QueryFilter, QuerySelect, RelationTrait, TransactionTrait,
};
use tracing::instrument;

use crate::{
    contract::{self, ContractStatus},
    league_player, player, team,
};

/// Inserts the new/advanced contract and sets the status of the old one appropriately.
#[instrument]
pub async fn advance_contract<C>(
    current_contract_model: contract::Model,
    db: &C,
) -> Result<(contract::Model, contract::Model)>
where
    C: ConnectionTrait + TransactionTrait + Debug,
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
    C: ConnectionTrait + TransactionTrait + Debug,
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
) -> Result<(contract::Model, contract::Model)>
where
    C: ConnectionTrait + TransactionTrait + Debug,
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
pub async fn expire_contract<C>(model: contract::Model, db: &C) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let mut model_to_update: contract::ActiveModel = model.into();
    model_to_update.status = ActiveValue::Set(contract::ContractStatus::Expired);

    let updated_model = model_to_update.update(db).await?;
    Ok(updated_model)
}

/// Retrieves all active contracts for a given team.
#[instrument]
pub async fn find_active_contracts_for_team<C>(
    team: &team::Model,
    db: &C,
) -> Result<Vec<contract::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let active_team_contracts = team
        .find_related(contract::Entity)
        .filter(contract::Column::Status.eq(ContractStatus::Active))
        .all(db)
        .await?;
    Ok(active_team_contracts)
}

/// Retrieves all contracts currently active in a league. Note that this includes Free Agent contracts where the player had been signed onto a team at some point but ended the season as a free agent.
#[instrument]
pub async fn find_active_contracts_in_league<C>(
    league_id: i64,
    db: &C,
) -> Result<Vec<contract::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
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

/// Retrieves all contracts currently in the given league that match the given list of player names.
#[instrument]
pub async fn find_active_league_contracts_by_player_names<C>(
    player_names: &[&str],
    league_id: i64,
    db: &C,
) -> Result<HashMap<String, contract::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
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

/// Used to replace an existing contract with a new one. The new one refers to the original as its original_contract_id, and the old one's status is set to `Replaced`.
#[instrument]
async fn add_replacement_contract_to_chain<C>(
    current_contract_model: contract::Model,
    replacement_contract_model: contract::ActiveModel,
    db: &C,
) -> Result<(contract::Model, contract::Model)>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let mut original_contract_model_to_update: contract::ActiveModel =
        current_contract_model.into();
    original_contract_model_to_update.status = ActiveValue::Set(contract::ContractStatus::Replaced);
    let updated_original_contract = original_contract_model_to_update.update(db).await?;

    let inserted_replacement_contract = replacement_contract_model.insert(db).await?;

    Ok((updated_original_contract, inserted_replacement_contract))
}
