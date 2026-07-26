//! Read-only rookie draft surface. `makePick` / `passPick` / `lotteryResults` land with
//! fbkl-rust-2jq (spec 02), which builds the selection logic they need.

use async_graphql::{Context, Object, Result, SimpleObject};
use chrono::Utc;
use fbkl_entity::{
    deadline_queries::find_most_recent_deadline_by_datetime,
    draft_pick,
    draft_pick_queries::get_draft_picks_for_league_season,
    rookie_draft_selection::{self, RookieDraftSelectionStatus},
    rookie_draft_selection_queries::find_rookie_draft_selections_for_league_season,
    sea_orm::DatabaseConnection,
};

use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, require_league_role,
};

#[derive(SimpleObject)]
pub struct DraftPick {
    pub id: i64,
    pub round: i16,
    pub end_of_season_year: i16,
    pub league_id: i64,
    pub current_owner_team_id: i64,
    pub original_owner_team_id: i64,
}

impl DraftPick {
    const fn from_model(model: &draft_pick::Model) -> Self {
        Self {
            id: model.id,
            round: model.round,
            end_of_season_year: model.end_of_season_year,
            league_id: model.league_id,
            current_owner_team_id: model.current_owner_team_id,
            original_owner_team_id: model.original_owner_team_id,
        }
    }
}

/// A used or skipped pick. `contractId` is the signed rookie contract when a player was selected.
#[derive(SimpleObject)]
pub struct RookieDraftSelection {
    pub id: i64,
    pub order: i16,
    pub status: RookieDraftSelectionStatus,
    pub contract_id: Option<i64>,
    pub draft_pick_id: i64,
}

impl RookieDraftSelection {
    const fn from_model(model: &rookie_draft_selection::Model) -> Self {
        Self {
            id: model.id,
            order: model.order,
            status: model.status,
            contract_id: model.contract_id,
            draft_pick_id: model.draft_pick_id,
        }
    }
}

/// A season's draft: every pick plus the selections made so far.
#[derive(SimpleObject)]
pub struct DraftBoard {
    pub end_of_season_year: i16,
    pub picks: Vec<DraftPick>,
    pub selections: Vec<RookieDraftSelection>,
}

#[derive(Default)]
pub struct DraftQuery;

#[Object]
impl DraftQuery {
    /// The season's picks and the selections made against them. Defaults to the current season.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn draft_board(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<DraftBoard> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        let season = season_or_current(ctx, caller_team.league_id, end_of_season_year).await?;

        let picks = get_draft_picks_for_league_season(caller_team.league_id, season, db)
            .await
            .map_err(|err| internal("failed to load draft picks", &err))?;
        let selections =
            find_rookie_draft_selections_for_league_season(caller_team.league_id, season, db)
                .await
                .map_err(|err| internal("failed to load rookie draft selections", &err))?;

        Ok(DraftBoard {
            end_of_season_year: season,
            picks: picks.iter().map(DraftPick::from_model).collect(),
            selections: selections
                .iter()
                .map(RookieDraftSelection::from_model)
                .collect(),
        })
    }

    /// The season's picks ordered by round, then pick id. Defaults to the current season.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn draft_order(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<Vec<DraftPick>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        let season = season_or_current(ctx, caller_team.league_id, end_of_season_year).await?;

        let picks = get_draft_picks_for_league_season(caller_team.league_id, season, db)
            .await
            .map_err(|err| internal("failed to load draft picks", &err))?;

        Ok(picks.iter().map(DraftPick::from_model).collect())
    }
}

async fn season_or_current(
    ctx: &Context<'_>,
    league_id: i64,
    end_of_season_year: Option<i16>,
) -> Result<i16> {
    if let Some(year) = end_of_season_year {
        return Ok(year);
    }

    let db = ctx.data_unchecked::<DatabaseConnection>();
    let deadline = find_most_recent_deadline_by_datetime(league_id, Utc::now().fixed_offset(), db)
        .await
        .map_err(|err| internal("failed to resolve the current season", &err))?;

    Ok(deadline.end_of_season_year)
}

fn internal(message: &str, error: &color_eyre::Report) -> async_graphql::Error {
    tracing::error!(error = ?error, message);
    code_error(ErrorCode::Internal)
}
