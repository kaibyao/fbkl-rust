use color_eyre::Result;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, TransactionTrait};

use crate::contract;

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

/// Inserts the new/advanced contract and sets the status of the old one appropriately.
pub async fn advance_contract<C>(
    current_contract_model: contract::Model,
    advanced_contract_model: contract::ActiveModel,
    db: &C,
) -> Result<(contract::Model, contract::Model)>
where
    C: ConnectionTrait + TransactionTrait,
{
    let inserted_advanced_contract = advanced_contract_model.insert(db).await?;

    let mut original_contract_model_to_update: contract::ActiveModel =
        current_contract_model.into();
    original_contract_model_to_update.status = ActiveValue::Set(contract::ContractStatus::Replaced);
    let updated_original_contract = original_contract_model_to_update.update(db).await?;

    Ok((updated_original_contract, inserted_advanced_contract))
}
