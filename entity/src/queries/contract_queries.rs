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
