//! Resolving "which season is it?" for resolvers whose arguments leave it implicit.
//!
//! The league's deadline schedule is the source of truth: the most recent deadline that has
//! already passed names the current `end_of_season_year`.

use async_graphql::{Context, Result};
use chrono::Utc;
use fbkl_entity::{
    deadline_queries::find_most_recent_deadline_by_datetime, sea_orm::DatabaseConnection,
};

use super::error::{ErrorCode, code_error};

pub async fn current_season(ctx: &Context<'_>, league_id: i64) -> Result<i16> {
    let db = ctx.data_unchecked::<DatabaseConnection>();
    let deadline = find_most_recent_deadline_by_datetime(league_id, Utc::now().fixed_offset(), db)
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, league_id, "failed to resolve the current season");
            code_error(ErrorCode::Internal)
        })?;

    Ok(deadline.end_of_season_year)
}
