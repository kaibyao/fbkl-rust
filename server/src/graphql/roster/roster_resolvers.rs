//! Single-team roster moves: IR, drops, and rookie-development transitions.
//!
//! The rookie-development moves have no eligibility guards in `logic/` yet (see
//! `logic/CLAUDE.md`); fbkl-rust-22o adds them, and they belong there rather than here.

use async_graphql::{Context, Error as GraphQlError, Object, Result};
use chrono::Utc;
use color_eyre::Report;
use fbkl_entity::{
    contract,
    contract_queries::find_contract_by_id,
    deadline,
    deadline_queries::find_most_recent_deadline_by_datetime,
    sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait},
};
use fbkl_logic::{
    drop_contract::drop_contract_from_team,
    ir::{activate_contract_from_ir, move_contract_to_ir},
    rookie_development_activation::activate_rookie_development_contract,
    rookie_development_international::{
        move_rookie_development_contract_to_international,
        move_rookie_development_international_contract_to_stateside,
    },
};

use super::super::contract::Contract;
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, require_league_role,
};

#[derive(Default)]
pub struct RosterMutation;

#[Object]
impl RosterMutation {
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn move_contract_to_ir(&self, ctx: &Context<'_>, contract_id: i64) -> Result<Contract> {
        roster_op(ctx, contract_id, move_contract_to_ir).await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn activate_contract_from_ir(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
    ) -> Result<Contract> {
        roster_op(ctx, contract_id, activate_contract_from_ir).await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn drop_contract(&self, ctx: &Context<'_>, contract_id: i64) -> Result<Contract> {
        roster_op(ctx, contract_id, drop_contract_from_team).await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn activate_rookie_contract(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
    ) -> Result<Contract> {
        roster_op(ctx, contract_id, activate_rookie_development_contract).await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn move_rookie_to_international(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
    ) -> Result<Contract> {
        roster_op(
            ctx,
            contract_id,
            move_rookie_development_contract_to_international,
        )
        .await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn move_rookie_to_stateside(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
    ) -> Result<Contract> {
        roster_op(
            ctx,
            contract_id,
            move_rookie_development_international_contract_to_stateside,
        )
        .await
    }
}

/// Runs one roster move on a contract the caller's own team owns.
///
/// Ownership is re-derived from the stored contract, never from the request. Each logic fn writes a
/// contract row, a transaction, and a team update, so they share one database transaction.
async fn roster_op<F>(ctx: &Context<'_>, contract_id: i64, op: F) -> Result<Contract>
where
    F: AsyncFnOnce(
        contract::Model,
        &deadline::Model,
        &DatabaseTransaction,
    ) -> color_eyre::Result<contract::Model>,
{
    let db = ctx.data_unchecked::<DatabaseConnection>();
    let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

    let contract_model = find_contract_by_id(contract_id, db)
        .await
        .map_err(|_| code_error(ErrorCode::NotFound))?;
    if contract_model.league_id != caller_team.league_id {
        return Err(code_error(ErrorCode::NotFound));
    }
    if contract_model.team_id != Some(team_user.team_id) {
        return Err(code_error(ErrorCode::Forbidden));
    }

    let deadline_model =
        find_most_recent_deadline_by_datetime(caller_team.league_id, Utc::now().fixed_offset(), db)
            .await
            .map_err(|err| internal("failed to resolve the current deadline", &err))?;

    let db_txn = db
        .begin()
        .await
        .map_err(|err| internal("failed to start transaction", &err.into()))?;
    let updated = op(contract_model, &deadline_model, &db_txn)
        .await
        .map_err(|err| internal("roster move failed", &err))?;
    db_txn
        .commit()
        .await
        .map_err(|err| internal("failed to commit roster move", &err.into()))?;

    Contract::from_model(&updated).map_err(|_| code_error(ErrorCode::Internal))
}

fn internal(message: &str, error: &Report) -> GraphQlError {
    tracing::error!(error = ?error, message);
    code_error(ErrorCode::Internal)
}
