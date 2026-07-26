use color_eyre::{Result, eyre::eyre};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::position;

pub async fn find_position_by_id<C>(position_id: i32, db: &C) -> Result<position::Model>
where
    C: ConnectionTrait,
{
    let position_model = position::Entity::find_by_id(position_id).one(db).await?;
    position_model.ok_or_else(|| eyre!("Position not found"))
}

/// Batch fetch for the GraphQL position `DataLoader`.
pub async fn find_positions_by_ids<C>(ids: Vec<i32>, db: &C) -> Result<Vec<position::Model>>
where
    C: ConnectionTrait,
{
    let positions = position::Entity::find()
        .filter(position::Column::Id.is_in(ids))
        .all(db)
        .await?;
    Ok(positions)
}
