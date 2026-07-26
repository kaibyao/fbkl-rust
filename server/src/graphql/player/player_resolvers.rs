//! Player reads: name search plus single lookups for real (NBA) and league players.
//!
//! League players are scoped to the caller's selected league; real players are global NBA
//! rows and therefore not league-scoped. `playerEligibility(leaguePlayerId)` from spec 06
//! lands on fbkl-rust-22o (spec 10), which adds the eligibility guard fns in `logic/`.

use async_graphql::{Context, Enum, Object, Result};
use fbkl_entity::{
    league_player_queries::{find_league_player_by_id, search_league_players_by_name},
    player_queries::{find_player_by_id, search_players_by_name},
    sea_orm::DatabaseConnection,
};

use super::{LeagueOrRealPlayer, LeaguePlayer, RealPlayer};
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, require_league_role,
};

/// Bounds each search leg — the real-player table spans every NBA season.
const SEARCH_LIMIT: u64 = 25;

#[derive(Copy, Clone, Debug, Enum, Eq, PartialEq)]
pub enum PlayerSearchKind {
    League,
    Real,
}

#[derive(Default)]
pub struct PlayerQuery;

#[Object]
impl PlayerQuery {
    /// Name search across league and/or real players. Omitting `kind` searches both.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn search_players(
        &self,
        ctx: &Context<'_>,
        query: String,
        kind: Option<PlayerSearchKind>,
    ) -> Result<Vec<LeagueOrRealPlayer>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let mut results = vec![];

        if kind != Some(PlayerSearchKind::Real) {
            let models =
                search_league_players_by_name(&query, caller_team.league_id, SEARCH_LIMIT, db)
                    .await
                    .map_err(|db_err| {
                        tracing::error!(error = ?db_err, "failed to search league players");
                        code_error(ErrorCode::Internal)
                    })?;
            results.extend(
                models
                    .into_iter()
                    .map(|model| LeagueOrRealPlayer::LeaguePlayer(LeaguePlayer::from_model(model))),
            );
        }

        if kind != Some(PlayerSearchKind::League) {
            let models = search_players_by_name(&query, SEARCH_LIMIT, db)
                .await
                .map_err(|db_err| {
                    tracing::error!(error = ?db_err, "failed to search real players");
                    code_error(ErrorCode::Internal)
                })?;
            results.extend(
                models
                    .into_iter()
                    .map(|model| LeagueOrRealPlayer::RealPlayer(RealPlayer::from_model(model))),
            );
        }

        Ok(results)
    }

    /// A single real (NBA) player.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn real_player(&self, ctx: &Context<'_>, id: i64) -> Result<RealPlayer> {
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let model = find_player_by_id(id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        Ok(RealPlayer::from_model(model))
    }

    /// A single league player, scoped to the caller's selected league.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn league_player(&self, ctx: &Context<'_>, id: i64) -> Result<LeaguePlayer> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let model = find_league_player_by_id(id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        if model.league_id != caller_team.league_id {
            return Err(code_error(ErrorCode::NotFound));
        }

        Ok(LeaguePlayer::from_model(model))
    }
}
