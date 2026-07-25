use std::collections::HashSet;

use color_eyre::{Result, eyre::eyre};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

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

/// Real players that could land in any eligibility pool (spec 10).
///
/// Currently-active players that either have NBA-roster history, carry the draft-eligible flag, or
/// have a commissioner override. Everyone else classifies `Ineligible`, so filtering them in SQL
/// keeps the whole all-seasons `player` table out of memory. Callers classify and filter roster
/// status themselves.
pub async fn find_eligibility_candidate_players<C>(db: &C) -> Result<Vec<player::Model>>
where
    C: ConnectionTrait,
{
    let players = player::Entity::find()
        .filter(player::Column::Status.eq(player::PlayerStatus::Active))
        .filter(
            player::Column::HasBeenOnNbaRoster
                .eq(true)
                .or(player::Column::IsRdiEligible.eq(true))
                .or(player::Column::EligibilityOverride.is_not_null()),
        )
        .order_by_asc(player::Column::Name)
        .all(db)
        .await?;
    Ok(players)
}

/// Batch fetch for the GraphQL player `DataLoader`.
pub async fn find_players_by_ids<C>(ids: Vec<i64>, db: &C) -> Result<Vec<player::Model>>
where
    C: ConnectionTrait,
{
    let players = player::Entity::find()
        .filter(player::Column::Id.is_in(ids))
        .all(db)
        .await?;
    Ok(players)
}
