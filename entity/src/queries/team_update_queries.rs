use color_eyre::Result;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    TransactionTrait,
};

use crate::{contract, team, team_update, transaction};

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

/// Inserts & returns a new team update containing keeper contracts for a specific team.
pub async fn create_keeper_team_update<C>(
    team: &team::Model,
    keeper_contracts: &[contract::Model],
    keeper_transaction: &transaction::Model,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let team_update_data = team_update::TeamUpdateData::create_roster_update_from_contracts(
        keeper_contracts,
        team_update::ContractUpdateType::Keeper,
    )?;

    let team_update_to_insert = team_update::ActiveModel {
        update_type: ActiveValue::Set(team_update::TeamUpdateType::Roster),
        data: ActiveValue::Set(team_update_data.as_bytes()?),
        effective_date: ActiveValue::Set(
            keeper_transaction
                .get_deadline(db)
                .await?
                .date_time
                .date_naive(),
        ),
        status: ActiveValue::Set(team_update::TeamUpdateStatus::Pending),
        team_id: ActiveValue::Set(team.id),
        transaction_id: ActiveValue::Set(Some(keeper_transaction.id)),
        ..Default::default()
    };
    let team_update = team_update_to_insert.insert(db).await?;

    Ok(team_update)
}
