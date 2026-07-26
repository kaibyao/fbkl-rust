//! Team reads for the selected league.
//!
//! Every team lookup is scoped to the league in the caller's session, so a team id
//! from another league resolves to `NOT_FOUND` rather than leaking across leagues.
//! There is deliberately no `selected_team_id` session key — the caller's own team
//! comes from their `team_user` row in the active league.

use async_graphql::{Context, Object, Result};
use fbkl_entity::{
    sea_orm::DatabaseConnection,
    team_queries::{find_team_by_id_in_league, find_teams_in_league},
};

use super::Team;
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, require_league_role,
};

#[derive(Default)]
pub struct TeamQuery;

#[Object]
impl TeamQuery {
    /// Every team in the caller's selected league.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn teams(&self, ctx: &Context<'_>) -> Result<Vec<Team>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let team_models = find_teams_in_league(caller_team.league_id, db)
            .await
            .map_err(|db_err| {
                tracing::error!(error = ?db_err, "failed to list teams in league");
                code_error(ErrorCode::Internal)
            })?;

        Ok(team_models.into_iter().map(Team::from_model).collect())
    }

    /// A single team, scoped to the caller's selected league.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn team(&self, ctx: &Context<'_>, id: i64) -> Result<Team> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let team_model = find_team_by_id_in_league(id, caller_team.league_id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        Ok(Team::from_model(team_model))
    }
}
