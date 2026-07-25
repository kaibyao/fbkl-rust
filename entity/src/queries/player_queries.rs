use std::collections::HashSet;

use color_eyre::{Result, eyre::eyre};
use sea_orm::sea_query::Expr;
use sea_orm::{ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::player;

pub async fn find_player_by_id<C>(id: i64, db: &C) -> Result<player::Model>
where
    C: ConnectionTrait,
{
    let player = player::Entity::find_by_id(id).one(db).await?;
    player.ok_or_else(|| eyre!("Player not found"))
}

/// Case- and accent-insensitive substring search on real (NBA) player names, for the player
/// search box. `limit` bounds the result set — the real-player table spans every NBA season.
pub async fn search_players_by_name<C>(
    name_query: &str,
    limit: u64,
    db: &C,
) -> Result<Vec<player::Model>>
where
    C: ConnectionTrait,
{
    let condition = Expr::cust_with_values(
        "unaccent(name) ILIKE unaccent($1)",
        [format!("%{name_query}%")],
    );

    let player_models = player::Entity::find()
        .filter(condition)
        .order_by_asc(player::Column::Name)
        .limit(limit)
        .all(db)
        .await?;
    Ok(player_models)
}

// internal query API; generic hasher flexibility not needed
#[allow(clippy::implicit_hasher)]
pub async fn find_players_by_name<C>(
    player_names: HashSet<&str>,
    db: &C,
) -> Result<Vec<player::Model>>
where
    C: ConnectionTrait,
{
    let player_names_vec: Vec<String> = player_names
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let condition = Expr::cust_with_values("unaccent(name) = ANY($1)", [player_names_vec]);

    let player_models = player::Entity::find().filter(condition).all(db).await?;
    Ok(player_models)
}
