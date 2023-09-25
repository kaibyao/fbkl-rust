use std::fmt::Debug;

use color_eyre::eyre::Result;
use sea_orm::{ConnectionTrait, EntityTrait};
use tracing::instrument;

use crate::real_team;

#[instrument]
pub async fn get_all_real_teams<C>(db: &C) -> Result<Vec<real_team::Model>>
where
    C: ConnectionTrait + Debug,
{
    let real_team_models = real_team::Entity::find().all(db).await?;
    Ok(real_team_models)
}
