//! League-role authorization shared by every resolver.
//!
//! Roles come from the caller's `team_user` row in the session's selected league,
//! so nothing about authorization is client-supplied. The declarative half (is the
//! caller a member / a commissioner?) is an async-graphql [`Guard`]; the
//! caller-team-owns-this-asset half stays in the resolver, because it needs the
//! resolved asset.

use async_graphql::{Context, Guard, Result};
use fbkl_entity::{
    sea_orm::DatabaseConnection,
    team,
    team_user::{self, LeagueRole},
    team_user_queries::get_team_user_by_user_and_league,
};
use tower_sessions::Session;

use super::error::{ErrorCode, code_error, graphql_error};

/// The league role a field requires of its caller.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RoleRequirement {
    /// Any active member of the selected league.
    Member,
    /// The league's commissioner.
    Commissioner,
}

const fn role_satisfies(role: LeagueRole, requirement: RoleRequirement) -> bool {
    match requirement {
        RoleRequirement::Member => !matches!(role, LeagueRole::Inactive),
        RoleRequirement::Commissioner => matches!(role, LeagueRole::LeagueCommissioner),
    }
}

/// Resolve the caller's `team_user` + team in the selected league, rejecting
/// anyone who does not meet `requirement`.
pub async fn require_league_role(
    ctx: &Context<'_>,
    requirement: RoleRequirement,
) -> Result<(team_user::Model, team::Model)> {
    let session = ctx.data_unchecked::<Session>();
    let db = ctx.data_unchecked::<DatabaseConnection>();

    let Some(user_id) = session_value(session, "user_id").await? else {
        return Err(code_error(ErrorCode::Unauthenticated));
    };
    let Some(league_id) = session_value(session, "selected_league_id").await? else {
        return Err(graphql_error(ErrorCode::BadRequest, "no league selected"));
    };

    let Some((team_user, maybe_team)) = get_team_user_by_user_and_league(&user_id, &league_id, db)
        .await
        .map_err(|db_err| {
            tracing::error!(error = ?db_err, "failed to load team_user for authz");
            code_error(ErrorCode::Internal)
        })?
    else {
        return Err(code_error(ErrorCode::Forbidden));
    };

    if !role_satisfies(team_user.league_role, requirement) {
        return Err(code_error(ErrorCode::Forbidden));
    }

    // The join always has a team; a missing one means the row is corrupt, not that the caller is at fault.
    let team = maybe_team.ok_or_else(|| {
        tracing::error!(team_user_id = team_user.id, "team_user has no team");
        code_error(ErrorCode::Internal)
    })?;

    Ok((team_user, team))
}

async fn session_value(session: &Session, key: &str) -> Result<Option<i64>> {
    session.get::<i64>(key).await.map_err(|session_err| {
        tracing::error!(error = ?session_err, key, "failed to read session");
        code_error(ErrorCode::Internal)
    })
}

/// Field guard for the declarative role requirement.
pub struct LeagueRoleGuard(pub RoleRequirement);

impl Guard for LeagueRoleGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        require_league_role(ctx, self.0).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_are_every_active_role() {
        assert!(role_satisfies(
            LeagueRole::TeamOwner,
            RoleRequirement::Member
        ));
        assert!(role_satisfies(
            LeagueRole::LeagueCommissioner,
            RoleRequirement::Member
        ));
        assert!(!role_satisfies(
            LeagueRole::Inactive,
            RoleRequirement::Member
        ));
    }

    #[test]
    fn only_the_commissioner_satisfies_commissioner() {
        assert!(role_satisfies(
            LeagueRole::LeagueCommissioner,
            RoleRequirement::Commissioner
        ));
        assert!(!role_satisfies(
            LeagueRole::TeamOwner,
            RoleRequirement::Commissioner
        ));
        assert!(!role_satisfies(
            LeagueRole::Inactive,
            RoleRequirement::Commissioner
        ));
    }
}
