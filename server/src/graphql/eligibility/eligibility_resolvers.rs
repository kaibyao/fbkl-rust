//! Eligibility reads + commissioner corrections (spec 10).
//!
//! Every classification rule lives in `fbkl_logic::eligibility`; these resolvers only fetch,
//! authorize, and map to GraphQL types. The two mutations stay deliberately separate:
//! `setPlayerNbaRosterStatus` corrects the underlying *fact*, `overridePlayerEligibility`
//! overrides the *derived classification*.

use async_graphql::{Context, Object, Result, SimpleObject};
use fbkl_entity::{
    contract::RelatedPlayer,
    eligibility_queries::{
        set_league_player_eligibility_override, set_league_player_nba_roster_status,
        set_player_eligibility_override, set_player_nba_roster_status,
    },
    league_player_queries::find_league_player_by_id,
    player::EligibilityClassification,
    player_queries::find_player_by_id,
    sea_orm::DatabaseConnection,
};
use fbkl_logic::eligibility;

use super::super::player::{LeagueOrRealPlayer, PlayerSearchKind};
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, current_season, graphql_error,
    require_league_role,
};

/// §6.2.2 — RFAs are auctioned in the first week only and their original owner may not bid, so the
/// pool is exposed pre-partitioned rather than as one flat list.
#[derive(SimpleObject)]
pub struct VeteranAuctionPool {
    pub restricted: Vec<LeagueOrRealPlayer>,
    pub unrestricted: Vec<LeagueOrRealPlayer>,
    pub free_agents: Vec<LeagueOrRealPlayer>,
}

#[derive(Default)]
pub struct EligibilityQuery;

#[Object]
impl EligibilityQuery {
    /// Players eligible for the preseason veteran auction, partitioned per §6.2.2.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn veteran_auction_pool(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<VeteranAuctionPool> {
        let (league_id, season) = league_and_season(ctx, end_of_season_year).await?;
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let pool = eligibility::build_veteran_auction_pool(league_id, season, db)
            .await
            .map_err(|err| internal("failed to build the veteran auction pool", &err))?;

        Ok(VeteranAuctionPool {
            restricted: to_graphql(pool.restricted_free_agents),
            unrestricted: to_graphql(pool.unrestricted_free_agents),
            free_agents: to_graphql(pool.free_agents),
        })
    }

    /// Players eligible for the rookie draft. Prior league draft/ownership is irrelevant (§7.5.3).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn rookie_draft_eligible_pool(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<Vec<LeagueOrRealPlayer>> {
        let (league_id, season) = league_and_season(ctx, end_of_season_year).await?;
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let pool = eligibility::build_rookie_draft_eligible_pool(league_id, season, db)
            .await
            .map_err(|err| internal("failed to build the rookie draft pool", &err))?;

        Ok(to_graphql(pool))
    }

    /// Players signable in-season: both eligible pools minus current rosters (§8.4).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn in_season_free_agent_pool(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<Vec<LeagueOrRealPlayer>> {
        let (league_id, season) = league_and_season(ctx, end_of_season_year).await?;
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let pool = eligibility::build_in_season_fa_pool(league_id, season, db)
            .await
            .map_err(|err| internal("failed to build the in-season free agent pool", &err))?;

        Ok(to_graphql(pool))
    }
}

#[derive(Default)]
pub struct EligibilityMutation;

#[Object]
impl EligibilityMutation {
    /// Commissioner correction of the NBA-roster *fact* the classification derives from.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Commissioner)")]
    async fn set_player_nba_roster_status(
        &self,
        ctx: &Context<'_>,
        kind: PlayerSearchKind,
        id: i64,
        has_been_on_nba_roster: bool,
    ) -> Result<LeagueOrRealPlayer> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let related_player = load_player_for_commissioner(ctx, kind, id).await?;

        let updated = match related_player {
            RelatedPlayer::LeaguePlayer(model) => RelatedPlayer::LeaguePlayer(
                set_league_player_nba_roster_status(model, has_been_on_nba_roster, db)
                    .await
                    .map_err(|err| internal("failed to set the NBA roster status", &err))?,
            ),
            RelatedPlayer::Player(model) => RelatedPlayer::Player(
                set_player_nba_roster_status(model, has_been_on_nba_roster, db)
                    .await
                    .map_err(|err| internal("failed to set the NBA roster status", &err))?,
            ),
        };

        Ok(LeagueOrRealPlayer::from_related_player(updated))
    }

    /// Commissioner override of the *derived* classification. `classification: null` clears it.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Commissioner)")]
    async fn override_player_eligibility(
        &self,
        ctx: &Context<'_>,
        kind: PlayerSearchKind,
        id: i64,
        classification: Option<EligibilityClassification>,
        reason: String,
    ) -> Result<LeagueOrRealPlayer> {
        if reason.trim().is_empty() {
            return Err(graphql_error(
                ErrorCode::BadRequest,
                "an eligibility override needs a reason",
            ));
        }

        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, _) = require_league_role(ctx, RoleRequirement::Commissioner).await?;
        let related_player = load_player_for_commissioner(ctx, kind, id).await?;

        let updated = match related_player {
            RelatedPlayer::LeaguePlayer(model) => RelatedPlayer::LeaguePlayer(
                set_league_player_eligibility_override(
                    model,
                    classification,
                    reason,
                    team_user.id,
                    db,
                )
                .await
                .map_err(|err| internal("failed to override eligibility", &err))?,
            ),
            RelatedPlayer::Player(model) => RelatedPlayer::Player(
                set_player_eligibility_override(model, classification, reason, team_user.id, db)
                    .await
                    .map_err(|err| internal("failed to override eligibility", &err))?,
            ),
        };

        Ok(LeagueOrRealPlayer::from_related_player(updated))
    }
}

/// Loads the mutation target, rejecting a league player from another league.
async fn load_player_for_commissioner(
    ctx: &Context<'_>,
    kind: PlayerSearchKind,
    id: i64,
) -> Result<RelatedPlayer> {
    let db = ctx.data_unchecked::<DatabaseConnection>();
    let (_, caller_team) = require_league_role(ctx, RoleRequirement::Commissioner).await?;

    match kind {
        PlayerSearchKind::League => {
            let model = find_league_player_by_id(id, db)
                .await
                .map_err(|_| code_error(ErrorCode::NotFound))?;
            if model.league_id != caller_team.league_id {
                return Err(code_error(ErrorCode::NotFound));
            }
            Ok(RelatedPlayer::LeaguePlayer(model))
        }
        PlayerSearchKind::Real => {
            let model = find_player_by_id(id, db)
                .await
                .map_err(|_| code_error(ErrorCode::NotFound))?;
            Ok(RelatedPlayer::Player(model))
        }
    }
}

/// The caller's league plus the season to build pools for, defaulting to the current one.
async fn league_and_season(
    ctx: &Context<'_>,
    end_of_season_year: Option<i16>,
) -> Result<(i64, i16)> {
    let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
    let season = match end_of_season_year {
        Some(year) => year,
        None => current_season(ctx, caller_team.league_id).await?,
    };
    Ok((caller_team.league_id, season))
}

fn to_graphql(members: Vec<RelatedPlayer>) -> Vec<LeagueOrRealPlayer> {
    members
        .into_iter()
        .map(LeagueOrRealPlayer::from_related_player)
        .collect()
}

fn internal(message: &str, error: &color_eyre::Report) -> async_graphql::Error {
    tracing::error!(error = ?error, message);
    code_error(ErrorCode::Internal)
}
