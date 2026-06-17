use std::fmt::Debug;

use chrono::{Duration, Utc};
use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ActiveEnum, ActiveValue, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter,
    sea_query::{Expr, OnConflict},
};
use tracing::instrument;

use crate::{
    deadline,
    job_run::{self, JobEventKind, JobRunStatus},
};

/// Maximum number of times a failed job run is retried before it stays `Failed` and is
/// surfaced to the commissioner console instead of silently retried forever.
pub const MAX_ATTEMPTS: i16 = 5;

/// A `Running` job run whose `updated_at` is older than this is considered abandoned
/// (e.g. the process crashed mid-run) and may be reclaimed by a later tick.
pub const STALE_RUNNING_TIMEOUT_MINUTES: i64 = 10;

/// Arguments for claiming a job run before dispatching its handler.
#[derive(Debug, Clone)]
pub struct NewJobRun {
    pub league_id: i64,
    pub end_of_season_year: i16,
    pub deadline_id: Option<i64>,
    pub event_kind: JobEventKind,
    pub dispatch_target: String,
    pub idempotency_key: String,
}

/// The result of attempting to claim a job run for processing.
#[derive(Debug, Clone)]
pub enum ClaimOutcome {
    /// This caller owns the run and must dispatch the handler, then record the outcome.
    Claimed(job_run::Model),
    /// The event was already processed successfully — no-op.
    AlreadySucceeded,
    /// Another (non-stale) tick is currently processing this event — skip.
    AlreadyRunning,
    /// The run failed `MAX_ATTEMPTS` times and requires manual intervention.
    AttemptsExhausted(job_run::Model),
}

/// Stable idempotency key for a deadline row: `(league_id, end_of_season_year, kind, id)`.
/// Weekly locks share a kind across distinct rows, so the deadline id disambiguates. Single
/// source of truth shared by the live scheduler (`fbkl-transaction-processor`) and historical
/// replay (`import-data`) so both compute the same key for a given deadline.
pub fn deadline_idempotency_key(deadline_model: &deadline::Model) -> String {
    format!(
        "{}:{}:{:?}:deadline-{}",
        deadline_model.league_id,
        deadline_model.end_of_season_year,
        deadline_model.kind,
        deadline_model.id
    )
}

/// Records an already-completed deadline as a `Succeeded` job run. Historical replay
/// (`import-data`) runs `fbkl_logic` handlers directly instead of going through the scheduler;
/// without this, replayed deadlines look unprocessed and the live scheduler reprocesses them on
/// every tick. Idempotent: if a job run for this deadline already exists, it is left untouched.
#[instrument]
pub async fn record_succeeded_deadline_job_run<C>(
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let insert_result = job_run::Entity::insert(job_run::ActiveModel {
        id: ActiveValue::NotSet,
        league_id: ActiveValue::Set(deadline_model.league_id),
        end_of_season_year: ActiveValue::Set(deadline_model.end_of_season_year),
        deadline_id: ActiveValue::Set(Some(deadline_model.id)),
        event_kind: ActiveValue::Set(JobEventKind::Deadline),
        dispatch_target: ActiveValue::Set(format!("{:?}", deadline_model.kind)),
        status: ActiveValue::Set(JobRunStatus::Succeeded),
        attempts: ActiveValue::Set(1),
        idempotency_key: ActiveValue::Set(deadline_idempotency_key(deadline_model)),
        transaction_id: ActiveValue::Set(None),
        error: ActiveValue::Set(None),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    })
    .on_conflict(
        OnConflict::column(job_run::Column::IdempotencyKey)
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await;

    match insert_result {
        Ok(_) | Err(DbErr::RecordNotInserted) => Ok(()),
        Err(other) => Err(other.into()),
    }
}

/// Atomically claims a job run for the given idempotency key. The unique index on
/// `idempotency_key` guarantees concurrent ticks cannot both claim the same event; the
/// loser of the insert race observes the winner's row and skips.
#[instrument]
pub async fn claim_job_run<C>(new_job_run: NewJobRun, db: &C) -> Result<ClaimOutcome>
where
    C: ConnectionTrait + Debug,
{
    let insert_result = job_run::Entity::insert(job_run::ActiveModel {
        id: ActiveValue::NotSet,
        league_id: ActiveValue::Set(new_job_run.league_id),
        end_of_season_year: ActiveValue::Set(new_job_run.end_of_season_year),
        deadline_id: ActiveValue::Set(new_job_run.deadline_id),
        event_kind: ActiveValue::Set(new_job_run.event_kind),
        dispatch_target: ActiveValue::Set(new_job_run.dispatch_target.clone()),
        status: ActiveValue::Set(JobRunStatus::Running),
        attempts: ActiveValue::Set(1),
        idempotency_key: ActiveValue::Set(new_job_run.idempotency_key.clone()),
        transaction_id: ActiveValue::Set(None),
        error: ActiveValue::Set(None),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    })
    .on_conflict(
        OnConflict::column(job_run::Column::IdempotencyKey)
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await;

    match insert_result {
        Ok(inserted) => {
            let job_run_model = job_run::Entity::find_by_id(inserted.last_insert_id)
                .one(db)
                .await?
                .ok_or_else(|| eyre!("Job run disappeared immediately after insert"))?;
            Ok(ClaimOutcome::Claimed(job_run_model))
        }
        Err(DbErr::RecordNotInserted) => {
            claim_existing_job_run(&new_job_run.idempotency_key, db).await
        }
        Err(other) => Err(other.into()),
    }
}

/// Handles the conflict path of [`claim_job_run`]: a row for this idempotency key already
/// exists, so decide based on its status whether it can be (re)claimed.
async fn claim_existing_job_run<C>(idempotency_key: &str, db: &C) -> Result<ClaimOutcome>
where
    C: ConnectionTrait + Debug,
{
    let existing = find_job_run_by_idempotency_key(idempotency_key, db)
        .await?
        .ok_or_else(|| {
            eyre!(
                "Job run insert conflicted but no row found for idempotency key: {}",
                idempotency_key
            )
        })?;

    match existing.status {
        JobRunStatus::Succeeded => Ok(ClaimOutcome::AlreadySucceeded),
        JobRunStatus::Running => {
            let stale_cutoff = Utc::now() - Duration::minutes(STALE_RUNNING_TIMEOUT_MINUTES);
            if existing.updated_at > stale_cutoff.fixed_offset() {
                return Ok(ClaimOutcome::AlreadyRunning);
            }
            // Abandoned run (crashed worker) — reclaim it. The conditional filter ensures only
            // one of multiple concurrent reclaimers wins.
            reclaim_with_filter(
                &existing,
                job_run::Column::Status
                    .eq(JobRunStatus::Running)
                    .and(job_run::Column::UpdatedAt.lte(stale_cutoff.fixed_offset())),
                db,
            )
            .await
        }
        JobRunStatus::Failed if existing.attempts >= MAX_ATTEMPTS => {
            Ok(ClaimOutcome::AttemptsExhausted(existing))
        }
        JobRunStatus::Pending | JobRunStatus::Failed => {
            reclaim_with_filter(
                &existing,
                job_run::Column::Status.is_in([JobRunStatus::Pending, JobRunStatus::Failed]),
                db,
            )
            .await
        }
    }
}

/// Conditionally flips an existing job run back to `Running` (incrementing `attempts`).
/// Returns `AlreadyRunning` if another claimer won the conditional update race.
async fn reclaim_with_filter<C>(
    existing: &job_run::Model,
    status_filter: sea_orm::sea_query::SimpleExpr,
    db: &C,
) -> Result<ClaimOutcome>
where
    C: ConnectionTrait + Debug,
{
    let update_result = job_run::Entity::update_many()
        .col_expr(
            job_run::Column::Status,
            Expr::value(JobRunStatus::Running.as_enum()),
        )
        .col_expr(
            job_run::Column::Attempts,
            Expr::col(job_run::Column::Attempts).add(1),
        )
        .filter(job_run::Column::Id.eq(existing.id))
        .filter(status_filter)
        .exec(db)
        .await?;

    if update_result.rows_affected == 0 {
        return Ok(ClaimOutcome::AlreadyRunning);
    }

    let reclaimed = job_run::Entity::find_by_id(existing.id)
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Job run (id = {}) disappeared during reclaim", existing.id))?;
    Ok(ClaimOutcome::Claimed(reclaimed))
}

#[instrument]
pub async fn find_job_run_by_idempotency_key<C>(
    idempotency_key: &str,
    db: &C,
) -> Result<Option<job_run::Model>>
where
    C: ConnectionTrait + Debug,
{
    let maybe_job_run = job_run::Entity::find()
        .filter(job_run::Column::IdempotencyKey.eq(idempotency_key))
        .one(db)
        .await?;
    Ok(maybe_job_run)
}

#[instrument]
pub async fn mark_job_run_succeeded<C>(
    job_run_id: i64,
    transaction_id: Option<i64>,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    job_run::Entity::update_many()
        .col_expr(
            job_run::Column::Status,
            Expr::value(JobRunStatus::Succeeded.as_enum()),
        )
        .col_expr(job_run::Column::TransactionId, Expr::value(transaction_id))
        .col_expr(job_run::Column::Error, Expr::value(Option::<String>::None))
        .filter(job_run::Column::Id.eq(job_run_id))
        .exec(db)
        .await?;
    Ok(())
}

#[instrument]
pub async fn mark_job_run_failed<C>(job_run_id: i64, error: &str, db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    job_run::Entity::update_many()
        .col_expr(
            job_run::Column::Status,
            Expr::value(JobRunStatus::Failed.as_enum()),
        )
        .col_expr(job_run::Column::Error, Expr::value(Some(error.to_string())))
        .filter(job_run::Column::Id.eq(job_run_id))
        .exec(db)
        .await?;
    Ok(())
}

/// Lists job runs for a league season, most recent first — the commissioner console's audit view.
#[instrument]
pub async fn find_job_runs_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<job_run::Model>>
where
    C: ConnectionTrait + Debug,
{
    use sea_orm::QueryOrder;

    let job_runs = job_run::Entity::find()
        .filter(
            job_run::Column::LeagueId
                .eq(league_id)
                .and(job_run::Column::EndOfSeasonYear.eq(end_of_season_year)),
        )
        .order_by_desc(job_run::Column::CreatedAt)
        .all(db)
        .await?;
    Ok(job_runs)
}
