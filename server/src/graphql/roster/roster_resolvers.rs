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
    deadline_queries::{find_deadline_by_id, find_sorted_deadlines_for_league_season},
    sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, TransactionTrait},
    team_queries::find_team_by_id_in_league,
    team_update::TeamUpdateStatus,
    team_update_queries::{find_team_updates_by_team, update_team_update_sequences},
    team_user::LeagueRole,
};
use fbkl_logic::{
    deadline_processing::roster_lock::{TeamRosterViolation, validate_team_roster},
    drop_contract::drop_contract_from_team,
    ir::{activate_contract_from_ir, move_contract_to_ir},
    rookie_development_activation::activate_rookie_development_contract,
    rookie_development_international::{
        move_rookie_development_contract_to_international,
        move_rookie_development_international_contract_to_stateside,
    },
};

use super::super::{contract::Contract, team::TeamUpdate};
use super::{RosterMove, RosterMoveKind, TeamWeek, roster_illegal_error};
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, graphql_error, require_league_role,
};

#[derive(Default)]
pub struct RosterMutation;

#[Object]
impl RosterMutation {
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn move_contract_to_ir(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
        deadline_id: i64,
    ) -> Result<Contract> {
        roster_op(ctx, contract_id, deadline_id, move_contract_to_ir).await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn activate_contract_from_ir(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
        deadline_id: i64,
    ) -> Result<Contract> {
        roster_op(ctx, contract_id, deadline_id, activate_contract_from_ir).await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn drop_contract(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
        deadline_id: i64,
    ) -> Result<Contract> {
        roster_op(ctx, contract_id, deadline_id, drop_contract_from_team).await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn activate_rookie_contract(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
        deadline_id: i64,
    ) -> Result<Contract> {
        roster_op(
            ctx,
            contract_id,
            deadline_id,
            activate_rookie_development_contract,
        )
        .await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn move_rookie_to_international(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
        deadline_id: i64,
    ) -> Result<Contract> {
        roster_op(
            ctx,
            contract_id,
            deadline_id,
            move_rookie_development_contract_to_international,
        )
        .await
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn move_rookie_to_stateside(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
        deadline_id: i64,
    ) -> Result<Contract> {
        roster_op(
            ctx,
            contract_id,
            deadline_id,
            move_rookie_development_international_contract_to_stateside,
        )
        .await
    }

    /// Saves the owner's chosen order for one week's moves (rules §13.1.1).
    ///
    /// Order is presentational and for the audit log only: the same set of moves stays legal or
    /// illegal whatever order it is put in, so nothing is re-validated here.
    ///
    /// The order covers one week, named by its lock deadline, and has to list that week's moves and
    /// no others. Sequences are positions in the list, so a partial list or a move from another
    /// week would write positions that clash with the ones already stored for other weeks.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn reorder_weekly_moves(
        &self,
        ctx: &Context<'_>,
        team_id: i64,
        deadline_id: i64,
        ordered_team_update_ids: Vec<i64>,
    ) -> Result<Vec<TeamUpdate>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        if team_user.team_id != team_id {
            return Err(code_error(ErrorCode::Forbidden));
        }

        resolve_roster_lock(deadline_id, caller_team.league_id, db).await?;

        let week_move_models = find_team_updates_by_team(team_id, None, Some(deadline_id), db)
            .await
            .map_err(|err| internal("failed to load this week's moves", &err))?;
        let week_move_ids: HashSet<i64> = week_move_models.iter().map(|model| model.id).collect();
        let requested_ids: HashSet<i64> = ordered_team_update_ids.iter().copied().collect();
        if requested_ids.len() != ordered_team_update_ids.len() || requested_ids != week_move_ids {
            return Err(graphql_error(
                ErrorCode::BadRequest,
                "an order has to list each of this week's moves once and nothing else".to_owned(),
            ));
        }

        let db_txn = db
            .begin()
            .await
            .map_err(|err| internal("failed to start transaction", &err.into()))?;
        update_team_update_sequences(&ordered_team_update_ids, &db_txn)
            .await
            .map_err(|err| internal("failed to save the move order", &err))?;
        let reordered = find_team_updates_by_team(team_id, None, Some(deadline_id), &db_txn)
            .await
            .map_err(|err| internal("failed to reload this week's moves", &err))?;
        db_txn
            .commit()
            .await
            .map_err(|err| internal("failed to commit the move order", &err.into()))?;

        Ok(TeamWeek::in_owner_order(&reordered))
    }

    /// Applies a batch of roster moves in one database transaction for the season-start wizard.
    ///
    /// The whole batch is rejected when the team's roster is still illegal after it, so a
    /// mid-batch state that breaks a rule is fine as long as the end state does not.
    ///
    /// `deadline_id` is the lock the owner is legalizing towards, normally the upcoming one. It has
    /// to be given rather than read off the clock: owners work in the window before the lock fires,
    /// where the last passed deadline is the previous one and carries the wrong rules.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn legalize_roster(
        &self,
        ctx: &Context<'_>,
        team_id: i64,
        deadline_id: i64,
        moves: Vec<RosterMove>,
    ) -> Result<Vec<Contract>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        if team_user.team_id != team_id {
            return Err(code_error(ErrorCode::Forbidden));
        }

        let deadline_model =
            resolve_upcoming_roster_lock(deadline_id, caller_team.league_id, db).await?;

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

        let violations = team_roster_violations(team_id, &deadline_model, &db_txn).await?;
        if !violations.is_empty() {
            return Err(roster_illegal_error(&violations));
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
///
/// `deadline_id` names the lock the move counts towards, the same argument `legalize_roster` takes
/// and validated the same way: a single move and a batched one have to agree on which week they
/// belong to, or the same-week guards and the limits read a different period for each.
async fn roster_op<F>(
    ctx: &Context<'_>,
    contract_id: i64,
    deadline_id: i64,
    op: F,
) -> Result<Contract>
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
        resolve_upcoming_roster_lock(deadline_id, caller_team.league_id, db).await?;

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

/// Resolves the roster-lock deadline a request names, in the caller's own league.
///
/// Deadlines are league-scoped rows, so an id from another league reads as not-found rather than
/// forbidden. Only locks are legal arguments: the other kinds carry another period's rules — drops
/// are penalty-free at the keeper deadline (rules §8.3.3) and the auction-end kinds resolve the
/// post-season cap (§4.2.3) — so a move naming one would run under the wrong rules.
async fn resolve_roster_lock<C>(deadline_id: i64, league_id: i64, db: &C) -> Result<deadline::Model>
where
    C: ConnectionTrait,
{
    let deadline_model = find_deadline_by_id(deadline_id, db)
        .await
        .map_err(|_| code_error(ErrorCode::NotFound))?;
    if deadline_model.league_id != league_id {
        return Err(code_error(ErrorCode::NotFound));
    }
    if !deadline_model.is_roster_lock() {
        return Err(graphql_error(
            ErrorCode::BadRequest,
            "roster moves count towards a roster lock, and this deadline is not one".to_owned(),
        ));
    }

    Ok(deadline_model)
}

/// Resolves the lock a roster move counts towards: the caller's league's next one still to fire.
///
/// The lock has to be named rather than read off the clock, because owners work in the window
/// before it fires where the last passed deadline is the previous one. Naming it is not the same as
/// choosing it, though: a lock that has already fired belongs to a settled week and a later one is
/// a period not yet in effect, so running a move against either applies the wrong limits, skips the
/// same-week guards, and files the move under the wrong week.
///
/// The season comes off the named deadline, so last season's rows fail the same check — none of
/// that season's locks is still to fire.
async fn resolve_upcoming_roster_lock<C>(
    deadline_id: i64,
    league_id: i64,
    db: &C,
) -> Result<deadline::Model>
where
    C: ConnectionTrait,
{
    let deadline_model = resolve_roster_lock(deadline_id, league_id, db).await?;

    let now = Utc::now().fixed_offset();
    let season_deadlines =
        find_sorted_deadlines_for_league_season(league_id, deadline_model.end_of_season_year, db)
            .await
            .map_err(|err| internal("failed to load the league's deadlines", &err))?;
    let upcoming_lock_id = season_deadlines
        .iter()
        .find(|candidate| candidate.date_time >= now && candidate.is_roster_lock())
        .map(|lock| lock.id);
    if upcoming_lock_id != Some(deadline_id) {
        return Err(graphql_error(
            ErrorCode::BadRequest,
            "roster moves count towards the upcoming roster lock, and this is not it".to_owned(),
        ));
    }

    Ok(deadline_model)
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

        let violations = team_roster_violations(team_id, &deadline_model, db).await?;
        let rule_legality = TeamWeek::rule_legality_for_team(team_id, &violations);
        let is_legal = violations.is_empty();

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
/// applies there and the list comes back empty. Only the named team is read: the rest of the
/// league's legality is nobody's business on a single-team request.
async fn team_roster_violations<C>(
    team_id: i64,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<Vec<TeamRosterViolation>>
where
    C: ConnectionTrait,
{
    if deadline_model.is_preseason_keeper_or_before() {
        return Ok(vec![]);
    }

    validate_team_roster(team_id, deadline_model, db)
        .await
        .map_err(|err| internal("failed to validate the team's roster", &err))
}
