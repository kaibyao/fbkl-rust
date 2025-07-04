use std::{collections::HashMap, fmt::Debug};

use color_eyre::{eyre::eyre, Result};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
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
