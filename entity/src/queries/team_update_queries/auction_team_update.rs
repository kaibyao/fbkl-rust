use chrono::{NaiveDate, Utc};
use color_eyre::Result;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait};
use std::fmt::Debug;
use tracing::instrument;

use crate::team_update::{self, TeamUpdateStatus};

/// Updates the given team_update (generated via veteran auction processing) to be finished, along with an optional effective date (defaults to `now()` otherwise).
#[instrument]
pub async fn update_team_update_for_preseason_veteran_auction<C>(
    team_update_model: &team_update::Model,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut update_team_update_date_and_status: team_update::ActiveModel =
        team_update_model.clone().into();
    update_team_update_date_and_status.status = ActiveValue::Set(TeamUpdateStatus::Done);
    update_team_update_date_and_status.effective_date =
        ActiveValue::Set(maybe_override_effective_date.unwrap_or_else(|| Utc::now().date_naive()));

    let updated_model = update_team_update_date_and_status.update(db).await?;
    Ok(updated_model)
}
