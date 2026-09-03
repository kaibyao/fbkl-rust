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
    deadline_queries::{find_deadline_by_id, find_upcoming_roster_lock},
    roster_lock_violation_queries::find_violations_for_league,
    sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, TransactionTrait},
    team_queries::find_team_by_id_in_league,
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
    roster::RosterMoveRejection,
    trade::MISSING_ROSTER_LOCK_ADVICE,
};

use super::super::{contract::Contract, team::TeamUpdate};
use super::{RosterLockViolation, RosterMove, RosterMoveKind, TeamWeek, roster_illegal_error};
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
            .map_err(|err| internal("failed to start league_event", &err.into()))?;
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
            .map_err(|err| internal("failed to start league_event", &err.into()))?;

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
            .map_err(|err| roster_move_error(&err))?;

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
/// contract row, a league event, and a team update, so they share one database transaction.
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
        .map_err(|err| internal("failed to start league_event", &err.into()))?;
    let updated = op(contract_model, &deadline_model, &db_txn)
        .await
        .map_err(|err| roster_move_error(&err))?;
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

    let upcoming_lock = find_upcoming_roster_lock(
        league_id,
        deadline_model.end_of_season_year,
        Utc::now().fixed_offset(),
        db,
    )
    .await
    .map_err(|err| internal("failed to load the league's deadlines", &err))?;
    match upcoming_lock {
        // In-roster moves stay legal through the playoff weeks, so weekly locks run to `SeasonEnd`
        // and a season with none left to fire is missing deadline rows rather than closed for moves.
        None => Err(graphql_error(
            ErrorCode::BadRequest,
            format!(
                "this league season has no roster lock still to fire, so a move has no week to be \
                 judged in; {MISSING_ROSTER_LOCK_ADVICE}"
            ),
        )),
        Some(lock) if lock.id != deadline_id => Err(graphql_error(
            ErrorCode::BadRequest,
            "roster moves count towards the upcoming roster lock, and this is not it".to_owned(),
        )),
        Some(_) => Ok(deadline_model),
    }
}

/// A move a league rule refuses is the caller's fault and keeps the rule message; the rest is ours.
fn roster_move_error(error: &Report) -> GraphQlError {
    let Some(rejection) = error.downcast_ref::<RosterMoveRejection>() else {
        return internal("roster move failed", error);
    };

    let code = match rejection {
        // A stale contract row is a refetch-and-retry for the client, not a rule it broke.
        RosterMoveRejection::NotLatestInChain { .. } => ErrorCode::NotLatestInChain,
        _ => ErrorCode::RosterMoveRejected,
    };

    graphql_error(code, rejection.to_string())
}

fn internal(message: &str, error: &Report) -> GraphQlError {
    tracing::error!(error = ?error, message);
    code_error(ErrorCode::Internal)
}

#[derive(Default)]
pub struct RosterQuery;

#[Object]
impl RosterQuery {
    /// A team's roster for one deadline, with every move recorded for that week and a flag per roster rule.
    ///
    /// Reads only. The move list is the set `reorderWeeklyMoves` accepts, in the order the owner
    /// chose (rules 13.1.1); order is presentational, so no rule here reads it.
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

        let week_move_models = find_team_updates_by_team(team_id, None, Some(deadline_id), db)
            .await
            .map_err(|err| internal("failed to load this week's moves", &err))?;

        let violations = team_roster_violations(team_id, &deadline_model, db).await?;
        let rule_legality = TeamWeek::rule_legality_for_team(team_id, &violations);
        let is_legal = violations.is_empty();

        Ok(TeamWeek {
            team_id,
            deadline_id,
            contracts,
            moves: TeamWeek::in_owner_order(&week_move_models),
            rule_legality,
            is_legal,
        })
    }

    /// Every roster-lock failure the league has recorded, newest deadline first (rules §13.1.2, §13.2).
    ///
    /// Commissioner-only: an illegal roster keeps its moves Pending, and this is where the
    /// commissioner sees which rule stopped which team without reading the scheduler's logs.
    /// `deadline_id` narrows the read to one lock.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Commissioner)")]
    async fn roster_lock_violations(
        &self,
        ctx: &Context<'_>,
        deadline_id: Option<i64>,
    ) -> Result<Vec<RosterLockViolation>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Commissioner).await?;

        let violation_models = find_violations_for_league(caller_team.league_id, deadline_id, db)
            .await
            .map_err(|err| internal("failed to load the league's roster-lock violations", &err))?;
        Ok(violation_models
            .into_iter()
            .map(RosterLockViolation::from_model)
            .collect())
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

#[cfg(test)]
mod tests {
    use fbkl_entity::{contract::ContractStatus, deadline::DeadlineKind};

    use super::*;

    fn code_of(error: &GraphQlError) -> Option<async_graphql::Value> {
        error
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .cloned()
    }

    #[test]
    fn rule_rejections_carry_a_client_code_and_the_rule_message() {
        let cases = [
            (
                RosterMoveRejection::StraightToIr {
                    contract_id: 7,
                    deadline_kind: DeadlineKind::Week1RosterLock,
                },
                "straight to IR",
            ),
            (
                RosterMoveRejection::DropSameWeekAdd {
                    contract_id: 7,
                    violations: "roster is over the 22-man limit".to_owned(),
                },
                "added this week",
            ),
            (
                RosterMoveRejection::ContractNotActive {
                    contract_id: 7,
                    status: ContractStatus::Replaced,
                },
                "Replaced",
            ),
        ];

        for (rejection, expected_phrase) in cases {
            let error = roster_move_error(&Report::new(rejection));

            assert_eq!(code_of(&error), Some("ROSTER_MOVE_REJECTED".into()));
            assert!(
                error.message.contains(expected_phrase),
                "the owner should be told which rule stopped the move: {error:?}"
            );
        }
    }

    #[test]
    fn a_stale_contract_gets_the_chain_code_instead_of_a_rule_rejection() {
        let error = roster_move_error(&Report::new(RosterMoveRejection::NotLatestInChain {
            contract_id: 7,
        }));

        assert_eq!(code_of(&error), Some("NOT_LATEST_IN_CHAIN".into()));
        assert!(error.message.contains("latest in its chain"), "{error:?}");
    }

    #[test]
    fn a_database_fault_stays_internal_and_says_nothing() {
        let error = roster_move_error(&color_eyre::eyre::eyre!("connection reset"));

        assert_eq!(code_of(&error), Some("INTERNAL".into()));
        assert_eq!(error.message, "internal server error");
    }
}
