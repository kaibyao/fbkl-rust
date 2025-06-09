use std::fmt::Debug;

use color_eyre::eyre::{eyre, Result};
use sea_orm::{ConnectionTrait, EntityTrait};
use tracing::instrument;

use crate::real_team;

pub async fn find_real_team_by_id<C>(real_team_id: i64, db: &C) -> Result<real_team::Model>
where
    C: ConnectionTrait,
{
    let real_team_model = real_team::Entity::find_by_id(real_team_id).one(db).await?;
    real_team_model.ok_or_else(|| eyre!("Real Team not found"))
}

#[instrument]
pub async fn get_all_real_teams<C>(db: &C) -> Result<Vec<real_team::Model>>
where
    C: ConnectionTrait + Debug,
{
    let real_team_models = real_team::Entity::find().all(db).await?;
    Ok(real_team_models)
}
