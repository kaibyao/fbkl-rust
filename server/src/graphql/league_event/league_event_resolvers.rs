//! The league's read-only audit feed. League events are only ever written as a side effect of the
//! trade / auction / draft / keeper / roster engines, so this module exposes no mutations.

use async_graphql::{Context, Object, Result};
use fbkl_entity::{
    league_event_queries::{find_league_event_by_id, find_league_events_in_league},
    sea_orm::DatabaseConnection,
};

use super::{LeagueEvent, LeagueEventFilter, PagedLeagueEvents};
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, require_league_role,
};

/// The feed spans a league's whole history (2014-15 onwards), so a page is always bounded.
const MAX_PAGE_SIZE: u64 = 100;

#[derive(Default)]
pub struct LeagueEventQuery;

#[Object]
impl LeagueEventQuery {
    /// One page of the caller's league's league event history, newest first.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn league_events(
        &self,
        ctx: &Context<'_>,
        filter: Option<LeagueEventFilter>,
        #[graphql(default = 0)] page: u64,
        #[graphql(default = 25)] page_size: u64,
    ) -> Result<PagedLeagueEvents> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let (maybe_team_id, maybe_kind) = filter.map_or((None, None), |f| (f.team_id, f.kind));

        let paged = find_league_events_in_league(
            caller_team.league_id,
            maybe_team_id,
            maybe_kind,
            page,
            page_size.min(MAX_PAGE_SIZE),
            db,
        )
        .await
        .map_err(|db_err| {
            tracing::error!(error = ?db_err, league_id = caller_team.league_id, "failed to load league_events");
            code_error(ErrorCode::Internal)
        })?;

        Ok(PagedLeagueEvents {
            items: paged.items.iter().map(LeagueEvent::from_model).collect(),
            total_items: paged.total_items,
        })
    }

    /// A single league event, scoped to the caller's selected league.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn league_event(&self, ctx: &Context<'_>, id: i64) -> Result<LeagueEvent> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let model = find_league_event_by_id(id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        if model.league_id != caller_team.league_id {
            return Err(code_error(ErrorCode::NotFound));
        }

        Ok(LeagueEvent::from_model(&model))
    }
}
