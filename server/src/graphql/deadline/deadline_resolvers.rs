//! League deadline calendar plus the commissioner's manual fire path.
//!
//! `triggerDeadline` goes through `fbkl_transaction_processor::process_deadline` — the same
//! dispatch (and `job_run` idempotency) the spec-05 scheduler uses, so a manual fire cannot
//! diverge from the automatic one, and re-firing a processed deadline is a no-op.

use async_graphql::{Context, Enum, Error as GraphQlError, Object, Result, SimpleObject};
use chrono::Utc;
use color_eyre::Report;
use fbkl_entity::{
    deadline::{self, DeadlineKind},
    deadline_queries::{
        find_deadline_by_id, find_most_recent_deadline_by_datetime,
        find_sorted_deadlines_for_league_season,
    },
    sea_orm::DatabaseConnection,
};
use fbkl_transaction_processor::{ProcessOutcome, process_deadline};

use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, require_league_role,
};

/// A dated league event (roster lock, keeper deadline, auction boundary, …).
#[derive(SimpleObject)]
pub struct Deadline {
    pub id: i64,
    pub date_time: String,
    pub kind: DeadlineKind,
    pub name: String,
    pub end_of_season_year: i16,
    pub league_id: i64,
}

impl Deadline {
    fn from_model(model: &deadline::Model) -> Self {
        Self {
            id: model.id,
            date_time: model.date_time.to_rfc3339(),
            kind: model.kind,
            name: model.name.clone(),
            end_of_season_year: model.end_of_season_year,
            league_id: model.league_id,
        }
    }
}

/// What the processor did with a manually fired deadline.
#[derive(Copy, Clone, Debug, Enum, Eq, PartialEq)]
pub enum DeadlineTriggerOutcome {
    Processed,
    AlreadyProcessed,
    AlreadyRunning,
    AttemptsExhausted,
    Failed,
}

#[derive(SimpleObject)]
pub struct DeadlineTriggerResult {
    pub outcome: DeadlineTriggerOutcome,
    pub job_run_id: Option<i64>,
    /// Handler failure detail; only set when `outcome` is `FAILED`.
    pub error: Option<String>,
}

impl From<ProcessOutcome> for DeadlineTriggerResult {
    fn from(outcome: ProcessOutcome) -> Self {
        let (outcome, job_run_id, error) = match outcome {
            ProcessOutcome::Processed { job_run_id } => {
                (DeadlineTriggerOutcome::Processed, Some(job_run_id), None)
            }
            ProcessOutcome::AlreadyProcessed => {
                (DeadlineTriggerOutcome::AlreadyProcessed, None, None)
            }
            ProcessOutcome::AlreadyRunning => (DeadlineTriggerOutcome::AlreadyRunning, None, None),
            ProcessOutcome::AttemptsExhausted { job_run_id } => (
                DeadlineTriggerOutcome::AttemptsExhausted,
                Some(job_run_id),
                None,
            ),
            ProcessOutcome::Failed { job_run_id, error } => (
                DeadlineTriggerOutcome::Failed,
                Some(job_run_id),
                Some(error),
            ),
        };

        Self {
            outcome,
            job_run_id,
            error,
        }
    }
}

#[derive(Default)]
pub struct DeadlineQuery;

#[Object]
impl DeadlineQuery {
    /// The league's deadline calendar for a season, oldest first. Defaults to the current season.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn deadlines(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<Vec<Deadline>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let season = match end_of_season_year {
            Some(year) => year,
            None => {
                find_most_recent_deadline_by_datetime(
                    caller_team.league_id,
                    Utc::now().fixed_offset(),
                    db,
                )
                .await
                .map_err(|err| internal("failed to resolve the current deadline", &err))?
                .end_of_season_year
            }
        };

        let deadlines = find_sorted_deadlines_for_league_season(caller_team.league_id, season, db)
            .await
            .map_err(|err| internal("failed to load deadlines", &err))?;

        Ok(deadlines.iter().map(Deadline::from_model).collect())
    }
}

#[derive(Default)]
pub struct DeadlineMutation;

#[Object]
impl DeadlineMutation {
    /// Fires a deadline now, using the scheduler's dispatch and idempotency.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Commissioner)")]
    async fn trigger_deadline(
        &self,
        ctx: &Context<'_>,
        deadline_id: i64,
    ) -> Result<DeadlineTriggerResult> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Commissioner).await?;

        let deadline_model = find_deadline_by_id(deadline_id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;
        if deadline_model.league_id != caller_team.league_id {
            return Err(code_error(ErrorCode::NotFound));
        }

        let outcome = process_deadline(db, &deadline_model)
            .await
            .map_err(|err| internal("failed to process the deadline", &err))?;

        Ok(outcome.into())
    }
}

fn internal(message: &str, error: &Report) -> GraphQlError {
    tracing::error!(error = ?error, message);
    code_error(ErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handler_failure_is_reported_as_a_result_not_an_error() {
        let result: DeadlineTriggerResult = ProcessOutcome::Failed {
            job_run_id: 7,
            error: "roster lock blew up".to_owned(),
        }
        .into();

        assert_eq!(result.outcome, DeadlineTriggerOutcome::Failed);
        assert_eq!(result.job_run_id, Some(7));
        assert_eq!(result.error.as_deref(), Some("roster lock blew up"));
    }

    #[test]
    fn refiring_a_processed_deadline_is_a_no_op() {
        let result: DeadlineTriggerResult = ProcessOutcome::AlreadyProcessed.into();

        assert_eq!(result.outcome, DeadlineTriggerOutcome::AlreadyProcessed);
        assert!(result.job_run_id.is_none());
    }
}
