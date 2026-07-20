//! The scheduler half of FBKL's orchestration layer: a DB-driven poll (not cron) that runs
//! inside the long-lived `fbkl-server` process. Each tick discovers due, unprocessed
//! `deadline` rows across **all** leagues and hands them to `fbkl-transaction-processor`,
//! which owns idempotency (`job_run` claims), transactional dispatch, and outcome recording.
//!
//! Retry semantics live in the processor's claim path: a `Failed` `job_run` is reclaimed on a
//! later tick until `MAX_ATTEMPTS`, after which it stays `Failed` and must be retried manually
//! from the commissioner console.
//!
//! Note for replay/backfill (`import-data`): historical replay calls `fbkl_logic` handlers
//! directly and creates no `job_run` rows, so replayed deadlines look unprocessed to this
//! poller. Before enabling the scheduler against a database containing replayed history, mark
//! those deadlines' `job_runs` `Succeeded` (or only seed live-season deadlines).

use std::collections::BTreeMap;

use chrono::Utc;
use color_eyre::eyre::Result;
use fbkl_entity::{deadline_queries, sea_orm::DatabaseConnection};
use fbkl_transaction_processor::{ProcessOutcome, process_deadline};
use tokio::task::JoinHandle;
use tracing::{error, info, instrument};

/// How often the scheduler polls for due work.
pub const SCHEDULER_TICK_INTERVAL_SECS: u64 = 30;

/// Counts of what a single scheduler tick did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TickSummary {
    pub processed: usize,
    pub failed: usize,
    pub skipped: usize,
    /// Later deadlines in a league left unattempted because an earlier one didn't succeed.
    pub blocked: usize,
    /// Errors at the orchestration layer itself (claiming/recording), not handler failures.
    pub errors: usize,
}

/// Runs one scheduler tick: find every due deadline lacking a `Succeeded` `job_run` and process
/// each. Callable directly for tests and the commissioner console's manual trigger.
///
/// Deadlines are processed per league, strictly oldest-first, and a league's chain stops at the
/// first deadline that doesn't reach `Succeeded`. Later deadlines build on earlier ones, so a
/// failed week-2 lock must block week-3 rather than let the scheduler skip ahead into corrupt
/// state. Leagues are independent, so one stuck league never blocks another.
#[instrument(skip(db))]
pub async fn run_scheduler_tick(db: &DatabaseConnection) -> Result<TickSummary> {
    let now = Utc::now().fixed_offset();
    let due_deadlines = deadline_queries::find_due_unprocessed_deadlines(now, db).await?;

    // The query is global oldest-first; bucket per league while preserving that order.
    let mut deadlines_by_league: BTreeMap<i64, Vec<&_>> = BTreeMap::new();
    for deadline_model in &due_deadlines {
        deadlines_by_league
            .entry(deadline_model.league_id)
            .or_default()
            .push(deadline_model);
    }

    let mut summary = TickSummary::default();
    for league_deadlines in deadlines_by_league.values() {
        let mut league_blocked = false;
        for deadline_model in league_deadlines {
            if league_blocked {
                summary.blocked += 1;
                continue;
            }
            match process_deadline(db, deadline_model).await {
                Ok(ProcessOutcome::Processed { .. }) => summary.processed += 1,
                // Already done elsewhere (race) — satisfied, don't block the chain.
                Ok(ProcessOutcome::AlreadyProcessed) => summary.skipped += 1,
                // Not succeeded — block the rest of this league's chain until it does.
                Ok(ProcessOutcome::Failed { .. }) => {
                    summary.failed += 1;
                    league_blocked = true;
                }
                Ok(ProcessOutcome::AlreadyRunning | ProcessOutcome::AttemptsExhausted { .. }) => {
                    summary.skipped += 1;
                    league_blocked = true;
                }
                Err(orchestration_error) => {
                    summary.errors += 1;
                    league_blocked = true;
                    error!(
                        "Scheduler error processing deadline (id = {}): {orchestration_error:?}",
                        deadline_model.id
                    );
                }
            }
        }
    }

    // TODO(fbkl-rust-lcc, spec 01): synthesize auction sub-events (24h no-bid closes, §8.3.2
    // extension expiries) from open `auction` rows and dispatch via `process_event`.
    // TODO(fbkl-rust-1dk, spec 03): synthesize RFA 48h raise/match window expiries.

    if summary != TickSummary::default() {
        info!(
            "Scheduler tick: {} processed, {} failed, {} skipped, {} blocked, {} errors",
            summary.processed, summary.failed, summary.skipped, summary.blocked, summary.errors
        );
    }

    Ok(summary)
}

/// Spawns the scheduler loop on the tokio runtime. Tick errors are logged, never fatal —
/// the loop runs until the returned handle is aborted (server shutdown).
pub fn spawn_scheduler(db: DatabaseConnection) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(SCHEDULER_TICK_INTERVAL_SECS));
        // If a tick runs long, don't burst to catch up — just resume the cadence.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        info!(
            "Deadline scheduler started (every {}s)",
            SCHEDULER_TICK_INTERVAL_SECS
        );
        loop {
            interval.tick().await;
            if let Err(tick_error) = run_scheduler_tick(&db).await {
                error!("Scheduler tick failed: {tick_error:?}");
            }
        }
    })
}
