use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    TransactionTrait,
};
use tracing::instrument;

use crate::league_player;

#[instrument]
pub async fn find_all_league_players_in_league<C>(
    league_id: i64,
    db: &C,
) -> Result<Vec<league_player::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let league_player_models = league_player::Entity::find()
        .filter(league_player::Column::LeagueId.eq(league_id))
        .all(db)
        .await?;
    Ok(league_player_models)
}

#[instrument]
pub async fn insert_league_player_with_name<C>(
    name: String,
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<league_player::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let league_player_to_insert = league_player::ActiveModel {
        name: ActiveValue::Set(name),
        league_id: ActiveValue::Set(league_id),
        end_of_season_year: ActiveValue::Set(end_of_season_year),
        ..Default::default()
    };
    let inserted_league_player = league_player_to_insert.insert(db).await?;
    Ok(inserted_league_player)
}
