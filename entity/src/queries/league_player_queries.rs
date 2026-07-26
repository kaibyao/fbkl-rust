use std::{collections::HashMap, fmt::Debug};

use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, sea_query::Expr,
};
use tracing::instrument;

use crate::league_player;

#[instrument]
pub async fn find_all_league_players_in_league<C>(
    league_id: i64,
    db: &C,
) -> Result<HashMap<String, league_player::Model>>
where
    C: ConnectionTrait + Debug,
{
    let league_players_by_name = league_player::Entity::find()
        .filter(league_player::Column::LeagueId.eq(league_id))
        .all(db)
        .await?
        .into_iter()
        .map(|league_player_model| (league_player_model.name.clone(), league_player_model))
        .collect();
    Ok(league_players_by_name)
}

/// Case- and accent-insensitive substring search on league player names, scoped to one league.
#[instrument]
pub async fn search_league_players_by_name<C>(
    name_query: &str,
    league_id: i64,
    limit: u64,
    db: &C,
) -> Result<Vec<league_player::Model>>
where
    C: ConnectionTrait + Debug,
{
    let name_condition = Expr::cust_with_values(
        "unaccent(name) ILIKE unaccent($1)",
        [format!("%{name_query}%")],
    );

    let league_player_models = league_player::Entity::find()
        .filter(league_player::Column::LeagueId.eq(league_id))
        .filter(name_condition)
        .order_by_asc(league_player::Column::Name)
        .limit(limit)
        .all(db)
        .await?;
    Ok(league_player_models)
}

#[instrument]
pub async fn find_league_player_by_id<C>(id: i64, db: &C) -> Result<league_player::Model>
where
    C: ConnectionTrait + Debug,
{
    let league_player = league_player::Entity::find_by_id(id).one(db).await?;
    league_player.ok_or_else(|| eyre!("League player not found"))
}

#[instrument]
pub async fn insert_league_player_with_name<C>(
    name: String,
    league_id: i64,
    db: &C,
) -> Result<league_player::Model>
where
    C: ConnectionTrait + Debug,
{
    let league_player_to_insert = league_player::ActiveModel {
        name: ActiveValue::Set(name),
        league_id: ActiveValue::Set(league_id),
        is_rdi_eligible: ActiveValue::Set(true),
        ..Default::default()
    };
    let inserted_league_player = league_player_to_insert.insert(db).await?;
    Ok(inserted_league_player)
}

/// Batch fetch for the GraphQL league-player `DataLoader`.
pub async fn find_league_players_by_ids<C>(
    ids: Vec<i64>,
    db: &C,
) -> Result<Vec<league_player::Model>>
where
    C: ConnectionTrait,
{
    let league_players = league_player::Entity::find()
        .filter(league_player::Column::Id.is_in(ids))
        .all(db)
        .await?;
    Ok(league_players)
}
