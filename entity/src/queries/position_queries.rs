use color_eyre::{eyre::eyre, Result};
use sea_orm::{ConnectionTrait, EntityTrait};

use crate::position;

pub async fn find_position_by_id<C>(position_id: i32, db: &C) -> Result<position::Model>
where
    C: ConnectionTrait,
{
    let position_model = position::Entity::find_by_id(position_id).one(db).await?;
    position_model.ok_or_else(|| eyre!("Position not found"))
}
