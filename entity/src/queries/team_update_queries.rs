use color_eyre::Result;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};

use crate::team_update;

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
