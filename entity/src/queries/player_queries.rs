use std::collections::HashSet;

use color_eyre::Result;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};

use crate::player;

pub async fn find_players_by_name<C>(
    player_names: HashSet<&str>,
    db: &C,
) -> Result<Vec<player::Model>>
where
    C: ConnectionTrait + TransactionTrait,
{
    let player_models = player::Entity::find()
        .filter(player::Column::Name.is_in(player_names))
        .all(db)
        .await?;
    Ok(player_models)
}
