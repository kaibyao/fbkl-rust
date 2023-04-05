use color_eyre::Result;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DeleteResult, EntityTrait,
    QueryFilter, TransactionTrait,
};

use crate::{
    contract, team_update,
    team_update_contract::{self, UpdateType},
};

pub async fn find_team_updates_by_transaction<C>(
    transaction_id: i64,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait + TransactionTrait,
{
    let team_updates = team_update::Entity::find()
        .filter(team_update::Column::TransactionId.eq(transaction_id))
        .all(db)
        .await?;
    Ok(team_updates)
}

pub async fn delete_team_update_contracts<C>(team_update_id: i64, db: &C) -> Result<DeleteResult>
where
    C: ConnectionTrait + TransactionTrait,
{
    let team_update_contract_deletion_result = team_update_contract::Entity::delete_many()
        .filter(team_update_contract::Column::TeamUpdateId.eq(team_update_id))
        .exec(db)
        .await?;

    Ok(team_update_contract_deletion_result)
}

pub async fn insert_team_update_contracts<C>(
    team_update_id: i64,
    contract_ids: &[i64],
    db: &C,
) -> Result<Vec<team_update_contract::Model>>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut team_update_contracts = vec![];

    for contract_id in contract_ids {
        let inserted_team_update_contract = team_update_contract::ActiveModel {
            id: ActiveValue::NotSet,
            update_type: ActiveValue::Set(UpdateType::Keeper),
            team_update_id: ActiveValue::Set(team_update_id),
            contract_id: ActiveValue::Set(*contract_id),
        }
        .insert(db)
        .await?;

        team_update_contracts.push(inserted_team_update_contract);
    }

    Ok(team_update_contracts)
}
