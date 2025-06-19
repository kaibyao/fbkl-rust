use std::collections::HashSet;

use color_eyre::{eyre::eyre, Result};
use sea_orm::sea_query::Expr;
use sea_orm::{ConnectionTrait, EntityTrait, QueryFilter};

use crate::player;

pub async fn find_player_by_id<C>(id: i64, db: &C) -> Result<player::Model>
where
    C: ConnectionTrait,
{
    let player = player::Entity::find_by_id(id).one(db).await?;
    player.ok_or_else(|| eyre!("Player not found"))
}

pub async fn find_players_by_name<C>(
    player_names: HashSet<&str>,
    db: &C,
) -> Result<Vec<player::Model>>
where
    C: ConnectionTrait,
{
    let player_names_vec: Vec<String> = player_names.iter().map(|s| s.to_string()).collect();
    let condition = Expr::cust_with_values("unaccent(name) = ANY($1)", [player_names_vec]);

    let player_models = player::Entity::find().filter(condition).all(db).await?;
    Ok(player_models)
}
