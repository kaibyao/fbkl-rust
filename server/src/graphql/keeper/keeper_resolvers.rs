//! Keeper declarations for the preseason keeper deadline.
//!
//! Processing the deadline itself (`process_keeper_deadline_transaction`) stays deadline-driven and
//! is deliberately not exposed as a mutation.

use async_graphql::{Context, Error as GraphQlError, Object, Result, SimpleObject};
use chrono::Utc;
use color_eyre::Report;
use fbkl_entity::{
    contract,
    contract_queries::find_contract_by_id,
    deadline_queries::find_most_recent_deadline_by_datetime,
    sea_orm::{DatabaseConnection, TransactionTrait},
    team,
    team_update_queries::find_team_updates_by_transaction,
    team_user::LeagueRole,
    transaction_queries::find_keeper_deadline_transaction,
};
use fbkl_logic::deadline_processing::keeper_deadline::{
    KeeperValidationError, save_keeper_team_update, validate_team_keepers,
};

use super::super::team::TeamUpdate;
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, graphql_error, require_league_role,
};

/// Result of a keeper dry-run: whether the submission would be accepted, and why not if it wouldn't.
#[derive(SimpleObject)]
pub struct KeeperValidation {
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct KeeperQuery;

#[Object]
impl KeeperQuery {
    /// Keeper declarations for this season. Owners see their own team; the commissioner sees all,
    /// or one team when `team_id` is given.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn keeper_declarations(
        &self,
        ctx: &Context<'_>,
        team_id: Option<i64>,
    ) -> Result<Vec<TeamUpdate>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        let is_commissioner = team_user.league_role == LeagueRole::LeagueCommissioner;

        let visible_team_id = match team_id {
            Some(requested) if requested != team_user.team_id && !is_commissioner => {
                return Err(code_error(ErrorCode::Forbidden));
            }
            Some(requested) => Some(requested),
            None if is_commissioner => None,
            None => Some(team_user.team_id),
        };

        let end_of_season_year = current_season(ctx).await?;
        let Some(keeper_transaction) =
            find_keeper_deadline_transaction(caller_team.league_id, end_of_season_year, db)
                .await
                .map_err(|err| internal("failed to load the keeper transaction", &err))?
        else {
            return Ok(vec![]);
        };

        let team_updates = find_team_updates_by_transaction(keeper_transaction.id, db)
            .await
            .map_err(|err| internal("failed to load keeper declarations", &err))?;

        Ok(team_updates
            .iter()
            .filter(|team_update| visible_team_id.is_none_or(|id| team_update.team_id == id))
            .map(TeamUpdate::from_model)
            .collect())
    }

    /// Dry run of the keeper rules — same validation as `declareKeepers`, nothing persisted.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn validate_keepers(
        &self,
        ctx: &Context<'_>,
        contract_ids: Vec<i64>,
    ) -> Result<KeeperValidation> {
        let (_, contracts) = load_own_keeper_contracts(ctx, &contract_ids).await?;

        Ok(match validate_team_keepers(&contracts) {
            Ok(()) => KeeperValidation {
                valid: true,
                error: None,
            },
            Err(err) => KeeperValidation {
                valid: false,
                error: Some(err.message),
            },
        })
    }
}

#[derive(Default)]
pub struct KeeperMutation;

#[Object]
impl KeeperMutation {
    /// Records the caller's team's keepers for this season, replacing any earlier declaration.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn declare_keepers(
        &self,
        ctx: &Context<'_>,
        contract_ids: Vec<i64>,
    ) -> Result<TeamUpdate> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (caller_team, contracts) = load_own_keeper_contracts(ctx, &contract_ids).await?;
        let end_of_season_year = current_season(ctx).await?;

        let db_txn = db
            .begin()
            .await
            .map_err(|err| internal("failed to start transaction", &err.into()))?;
        let team_update =
            save_keeper_team_update(&caller_team, contracts, end_of_season_year, &db_txn)
                .await
                .map_err(|err| keeper_save_error(&err))?;
        db_txn
            .commit()
            .await
            .map_err(|err| internal("failed to commit keeper declaration", &err.into()))?;

        Ok(TeamUpdate::from_model(&team_update))
    }
}

/// Loads the submitted contracts, rejecting anything the caller's own team does not hold.
async fn load_own_keeper_contracts(
    ctx: &Context<'_>,
    contract_ids: &[i64],
) -> Result<(team::Model, Vec<contract::Model>)> {
    let db = ctx.data_unchecked::<DatabaseConnection>();
    let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

    let mut contracts = Vec::with_capacity(contract_ids.len());
    for &contract_id in contract_ids {
        let contract_model = find_contract_by_id(contract_id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;
        if contract_model.league_id != caller_team.league_id {
            return Err(code_error(ErrorCode::NotFound));
        }
        if contract_model.team_id != Some(team_user.team_id) {
            return Err(code_error(ErrorCode::Forbidden));
        }
        contracts.push(contract_model);
    }

    Ok((caller_team, contracts))
}

async fn current_season(ctx: &Context<'_>) -> Result<i16> {
    let db = ctx.data_unchecked::<DatabaseConnection>();
    let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

    let deadline =
        find_most_recent_deadline_by_datetime(caller_team.league_id, Utc::now().fixed_offset(), db)
            .await
            .map_err(|err| internal("failed to resolve the current deadline", &err))?;

    Ok(deadline.end_of_season_year)
}

/// A broken keeper rule is the client's fault; anything else is a server fault.
fn keeper_save_error(error: &Report) -> GraphQlError {
    error.downcast_ref::<KeeperValidationError>().map_or_else(
        || internal("failed to save keepers", error),
        |validation_error| {
            graphql_error(
                ErrorCode::KeeperValidationFailed,
                validation_error.message.clone(),
            )
        },
    )
}

fn internal(message: &str, error: &Report) -> GraphQlError {
    tracing::error!(error = ?error, message);
    code_error(ErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_keeper_rules_are_a_typed_client_error() {
        let error = keeper_save_error(&Report::new(KeeperValidationError {
            message: "too many keepers".to_owned(),
        }));

        assert_eq!(error.message, "too many keepers");
        assert_eq!(
            error.extensions.as_ref().and_then(|ext| ext.get("code")),
            Some(&"KEEPER_VALIDATION_FAILED".into())
        );
    }

    #[test]
    fn other_failures_stay_internal() {
        let error = keeper_save_error(&color_eyre::eyre::eyre!("db exploded"));

        assert_eq!(
            error.extensions.as_ref().and_then(|ext| ext.get("code")),
            Some(&"INTERNAL".into())
        );
    }
}
