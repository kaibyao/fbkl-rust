use color_eyre::Result;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::position;

pub async fn find_position_by_id<C>(position_id: i32, db: &C) -> Result<position::Model>
where
    C: ConnectionTrait,
{
    let position_model = position::Entity::find()
        .filter(player::Column::Name.is_in(player_names))
        .all(db)
        .await?;
    Ok(player_models)
}
