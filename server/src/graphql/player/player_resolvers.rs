//! Player reads: name search plus single lookups for real (NBA) and league players.
//!
//! League players are scoped to the caller's selected league; real players are global NBA
//! rows and therefore not league-scoped. `playerEligibility` answers which acquisition pool a
//! league player belongs to by calling `fbkl_logic::eligibility`, never by re-deriving it here.
//!
//! Eligibility is per-season, so these resolvers classify for the caller league's current season —
//! resolved once per call, not per row. `playerEligibility` takes an explicit season to override it.

use async_graphql::{Context, Enum, Object, Result};
use fbkl_entity::{
    league_player_queries::{find_league_player_by_id, search_league_players_by_name},
    player::EligibilityClassification,
    player_queries::{find_player_by_id, search_players_by_name},
    sea_orm::DatabaseConnection,
};
use fbkl_logic::eligibility::{PlayerEligibilityFacts, classify_player};

use super::{LeagueOrRealPlayer, LeaguePlayer, RealPlayer};
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, current_season, require_league_role,
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
        let season = current_season(ctx, caller_team.league_id).await?;

        let mut results = vec![];

        if kind != Some(PlayerSearchKind::Real) {
            let models =
                search_league_players_by_name(&query, caller_team.league_id, SEARCH_LIMIT, db)
                    .await
                    .map_err(|db_err| {
                        tracing::error!(error = ?db_err, "failed to search league players");
                        code_error(ErrorCode::Internal)
                    })?;
            results.extend(models.into_iter().map(|model| {
                LeagueOrRealPlayer::LeaguePlayer(LeaguePlayer::from_model(model, season))
            }));
        }

        if kind != Some(PlayerSearchKind::League) {
            let models = search_players_by_name(&query, SEARCH_LIMIT, db)
                .await
                .map_err(|db_err| {
                    tracing::error!(error = ?db_err, "failed to search real players");
                    code_error(ErrorCode::Internal)
                })?;
            results.extend(models.into_iter().map(|model| {
                LeagueOrRealPlayer::RealPlayer(RealPlayer::from_model(model, season))
            }));
        }

        Ok(results)
    }

    /// A single real (NBA) player.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn real_player(&self, ctx: &Context<'_>, id: i64) -> Result<RealPlayer> {
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        let season = current_season(ctx, caller_team.league_id).await?;

        let model = find_player_by_id(id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        Ok(RealPlayer::from_model(model, season))
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

        let season = current_season(ctx, caller_team.league_id).await?;
        Ok(LeaguePlayer::from_model(model, season))
    }

    /// Which acquisition pool a league player belongs to, override included. Defaults to the
    /// league's current season; pass `endOfSeasonYear` to ask about a past one.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn player_eligibility(
        &self,
        ctx: &Context<'_>,
        league_player_id: i64,
        end_of_season_year: Option<i16>,
    ) -> Result<EligibilityClassification> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let model = find_league_player_by_id(league_player_id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        if model.league_id != caller_team.league_id {
            return Err(code_error(ErrorCode::NotFound));
        }

        let season = match end_of_season_year {
            Some(year) => year,
            None => current_season(ctx, caller_team.league_id).await?,
        };
        Ok(classify_player(
            PlayerEligibilityFacts::from(&model),
            season,
        ))
    }
}
