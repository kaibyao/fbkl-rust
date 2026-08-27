//! Single-team roster moves: IR, drops, and rookie-development transitions.
//!
//! The rookie-development moves have no eligibility guards in `logic/` yet (see
//! `logic/CLAUDE.md`); fbkl-rust-22o adds them, and they belong there rather than here.

use std::collections::HashSet;

use async_graphql::{Context, Error as GraphQlError, Object, Result};
use chrono::Utc;
use color_eyre::Report;
use fbkl_entity::{
    contract,
    contract_queries::{find_active_contracts_for_team, find_contract_by_id},
    deadline,
    deadline_queries::{find_deadline_by_id, find_most_recent_deadline_by_datetime},
    sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, TransactionTrait},
    team_queries::find_team_by_id_in_league,
    team_update::TeamUpdateStatus,
    team_update_queries::{find_team_updates_by_team, update_team_update_sequences},
    team_user::LeagueRole,
};
use fbkl_logic::{
    deadline_processing::roster_lock::validate_league_rosters,
    drop_contract::drop_contract_from_team,
    ir::{activate_contract_from_ir, move_contract_to_ir},
    rookie_development_activation::activate_rookie_development_contract,
    rookie_development_international::{
        move_rookie_development_contract_to_international,
        move_rookie_development_international_contract_to_stateside,
    },
};

use super::super::{contract::Contract, team::TeamUpdate};
use super::{RosterMove, RosterMoveKind, RosterRuleLegality, TeamWeek};
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, graphql_error, require_league_role,
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

    /// Saves the owner's chosen order for their team's moves (rules §13.1.1).
    ///
    /// Order is presentational and for the audit log only: the same set of moves stays legal or
    /// illegal whatever order it is put in, so nothing is re-validated here.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn reorder_weekly_moves(
        &self,
        ctx: &Context<'_>,
        team_id: i64,
        ordered_team_update_ids: Vec<i64>,
    ) -> Result<Vec<TeamUpdate>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, _) = require_league_role(ctx, RoleRequirement::Member).await?;
        if team_user.team_id != team_id {
            return Err(code_error(ErrorCode::Forbidden));
        }

        let requested_ids: HashSet<i64> = ordered_team_update_ids.iter().copied().collect();
        if requested_ids.len() != ordered_team_update_ids.len() {
            return Err(graphql_error(
                ErrorCode::BadRequest,
                "a move cannot be listed twice in an order".to_owned(),
            ));
        }

        let team_update_models = find_team_updates_by_team(team_id, None, None, db)
            .await
            .map_err(|err| internal("failed to load the team's moves", &err))?;
        let owned_ids: HashSet<i64> = team_update_models.iter().map(|model| model.id).collect();
        if !requested_ids.is_subset(&owned_ids) {
            return Err(code_error(ErrorCode::NotFound));
        }

        let db_txn = db
            .begin()
            .await
            .map_err(|err| internal("failed to start transaction", &err.into()))?;
        update_team_update_sequences(&ordered_team_update_ids, &db_txn)
            .await
            .map_err(|err| internal("failed to save the move order", &err))?;
        let reordered = find_team_updates_by_team(team_id, None, None, &db_txn)
            .await
            .map_err(|err| internal("failed to reload the team's moves", &err))?;
        db_txn
            .commit()
            .await
            .map_err(|err| internal("failed to commit the move order", &err.into()))?;

        let reordered: Vec<_> = reordered
            .into_iter()
            .filter(|model| requested_ids.contains(&model.id))
            .collect();
        Ok(TeamWeek::in_owner_order(&reordered))
    }

    /// Applies a batch of roster moves in one database transaction for the season-start wizard.
    ///
    /// The whole batch is rejected when the team's roster is still illegal after it, so a
    /// mid-batch state that breaks a rule is fine as long as the end state does not.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn legalize_roster(
        &self,
        ctx: &Context<'_>,
        team_id: i64,
        moves: Vec<RosterMove>,
    ) -> Result<Vec<Contract>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        if team_user.team_id != team_id {
            return Err(code_error(ErrorCode::Forbidden));
        }

        let deadline_model = find_most_recent_deadline_by_datetime(
            caller_team.league_id,
            Utc::now().fixed_offset(),
            db,
        )
        .await
        .map_err(|err| internal("failed to resolve the current deadline", &err))?;

        let db_txn = db
            .begin()
            .await
            .map_err(|err| internal("failed to start transaction", &err.into()))?;

        let mut updated_contracts = Vec::with_capacity(moves.len());
        for roster_move in moves {
            let contract_model = find_contract_by_id(roster_move.contract_id, &db_txn)
                .await
                .map_err(|_| code_error(ErrorCode::NotFound))?;
            if contract_model.league_id != caller_team.league_id {
                return Err(code_error(ErrorCode::NotFound));
            }
            if contract_model.team_id != Some(team_id) {
                return Err(code_error(ErrorCode::Forbidden));
            }

            let updated = match roster_move.kind {
                RosterMoveKind::Drop => {
                    drop_contract_from_team(contract_model, &deadline_model, &db_txn).await
                }
                RosterMoveKind::MoveToIr => {
                    move_contract_to_ir(contract_model, &deadline_model, &db_txn).await
                }
                RosterMoveKind::ActivateFromIr => {
                    activate_contract_from_ir(contract_model, &deadline_model, &db_txn).await
                }
                RosterMoveKind::ActivateRookie => {
                    activate_rookie_development_contract(contract_model, &deadline_model, &db_txn)
                        .await
                }
            }
            .map_err(|err| internal("roster move failed", &err))?;

            updated_contracts
                .push(Contract::from_model(&updated).map_err(|_| code_error(ErrorCode::Internal))?);
        }

        let illegal_rules = team_rule_legality(team_id, &deadline_model, &db_txn)
            .await?
            .into_iter()
            .filter_map(|rule| rule.message)
            .collect::<Vec<_>>();
        if !illegal_rules.is_empty() {
            return Err(graphql_error(
                ErrorCode::RosterIllegal,
                illegal_rules.join("\n"),
            ));
        }

        db_txn
            .commit()
            .await
            .map_err(|err| internal("failed to commit the roster batch", &err.into()))?;

        Ok(updated_contracts)
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

#[derive(Default)]
pub struct RosterQuery;

#[Object]
impl RosterQuery {
    /// A team's roster for one deadline, with that week's pending moves and a flag per roster rule.
    ///
    /// Reads only. Rules 13.1.1 make move order presentational, so nothing here reads it.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn team_week(
        &self,
        ctx: &Context<'_>,
        team_id: i64,
        deadline_id: i64,
    ) -> Result<TeamWeek> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        if team_user.team_id != team_id && team_user.league_role != LeagueRole::LeagueCommissioner {
            return Err(code_error(ErrorCode::Forbidden));
        }

        let deadline_model = find_deadline_by_id(deadline_id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;
        if deadline_model.league_id != caller_team.league_id {
            return Err(code_error(ErrorCode::NotFound));
        }
        find_team_by_id_in_league(team_id, caller_team.league_id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        let contract_models = find_active_contracts_for_team(team_id, db)
            .await
            .map_err(|err| internal("failed to load the team's contracts", &err))?;
        let contracts = contract_models
            .iter()
            .map(Contract::from_model)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| code_error(ErrorCode::Internal))?;

        let pending_move_models = find_team_updates_by_team(
            team_id,
            Some(TeamUpdateStatus::Pending),
            Some(deadline_id),
            db,
        )
        .await
        .map_err(|err| internal("failed to load this week's pending moves", &err))?;

        let rule_legality = team_rule_legality(team_id, &deadline_model, db).await?;
        let is_legal = rule_legality.iter().all(|rule| rule.is_legal);

        Ok(TeamWeek {
            team_id,
            deadline_id,
            contracts,
            pending_moves: TeamWeek::in_owner_order(&pending_move_models),
            rule_legality,
            is_legal,
        })
    }
}

/// Runs the roster-lock rules for one team without mutating anything.
///
/// The keeper window has rules of its own (`validate_team_keepers`), so no roster-lock rule
/// applies there and the list comes back empty.
async fn team_rule_legality<C>(
    team_id: i64,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<Vec<RosterRuleLegality>>
where
    C: ConnectionTrait,
{
    if deadline_model.is_preseason_keeper_or_before() {
        return Ok(vec![]);
    }

    let violations = validate_league_rosters(deadline_model, db)
        .await
        .map_err(|err| internal("failed to validate the team's roster", &err))?;

    Ok(TeamWeek::rule_legality_for_team(team_id, &violations))
}
