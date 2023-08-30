use std::fmt::Debug;

use color_eyre::eyre::Result;
use fbkl_entity::{
    deadline, sea_orm::ConnectionTrait, team_update::TeamUpdateStatus, team_update_queries,
};
use tracing::instrument;

use super::validate_league_rosters;

#[instrument]
pub async fn lock_rosters<C>(deadline_model: &deadline::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    validate_league_rosters(deadline_model, db).await?;

    let deadline_team_updates =
        team_update_queries::find_team_updates_for_deadline(deadline_model, db).await?;
    team_update_queries::update_team_updates_with_status(
        deadline_team_updates
            .into_iter()
            .map(|team_update_model| team_update_model.id)
            .collect(),
        TeamUpdateStatus::Done,
        db,
    )
    .await?;

    Ok(())
}
