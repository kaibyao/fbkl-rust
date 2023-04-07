use color_eyre::Result;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    TransactionTrait,
};

use crate::contract;

/// Inserts the new/advanced contract and sets the status of the old one appropriately.
pub async fn advance_contract<C>(
    current_contract_model: contract::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let advanced_contract_to_insert = current_contract_model.create_contract_year_advancement(
        contract::ContractYearAdvancementType::AdvanceYearOnly,
        None,
    )?;
    let inserted_advanced_contract = advanced_contract_to_insert.insert(db).await?;

    let mut original_contract_model_to_update: contract::ActiveModel =
        current_contract_model.into();
    original_contract_model_to_update.status = ActiveValue::Set(contract::ContractStatus::Replaced);
    let _updated_original_contract = original_contract_model_to_update.update(db).await?;

    Ok(inserted_advanced_contract)
}

/// Expires the given contract
pub async fn expire_contract<C>(model: contract::Model, db: &C) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut model_to_update: contract::ActiveModel = model.into();
    model_to_update.status = ActiveValue::Set(contract::ContractStatus::Expired);

    let updated_model = model_to_update.update(db).await?;
    Ok(updated_model)
}

/// This is needed in order to set the `original_contract_id` after creating a new contract.
pub async fn create_new_contract<C>(model: contract::ActiveModel, db: &C) -> Result<contract::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut model_to_update_after_insert = model.clone();

    let inserted = model.insert(db).await?;

    model_to_update_after_insert.original_contract_id = ActiveValue::Set(Some(inserted.id));
    let updated = model_to_update_after_insert.update(db).await?;

    Ok(updated)
}

/// Retrieves all contracts currently active in a league. Note that this includes Free Agent contracts where the player had been signed onto a team at some point but ended the season as a free agent.
pub async fn find_active_contracts_in_league<C>(
    league_id: i64,
    db: &C,
) -> Result<Vec<contract::Model>>
where
    C: ConnectionTrait + TransactionTrait,
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
