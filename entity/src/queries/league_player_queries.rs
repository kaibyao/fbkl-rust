use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, TransactionTrait};
use tracing::instrument;

use crate::league_player;

#[instrument]
pub async fn insert_league_player_with_name<C>(
    name: String,
    league_id: i64,
    season_end_year: i16,
    db: &C,
) -> Result<league_player::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let league_player_to_insert = league_player::ActiveModel {
        name: ActiveValue::Set(name),
        league_id: ActiveValue::Set(league_id),
        season_end_year: ActiveValue::Set(season_end_year),
        ..Default::default()
    };
    let inserted_league_player = league_player_to_insert.insert(db).await?;
    Ok(inserted_league_player)
}
