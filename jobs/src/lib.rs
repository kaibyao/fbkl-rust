//! The scheduler half of FBKL's orchestration layer: a DB-driven poll (not cron) that runs
//! inside the long-lived `fbkl-server` process. Each tick discovers due, unprocessed
//! `deadline` rows across **all** leagues and hands them to `fbkl-transaction-processor`,
//! which owns idempotency (`job_run` claims), transactional dispatch, and outcome recording.
//!
//! Retry semantics live in the processor's claim path: a `Failed` job_run is reclaimed on a
//! later tick until `MAX_ATTEMPTS`, after which it stays `Failed` and must be retried manually
//! from the commissioner console.
//!
//! Note for replay/backfill (`import-data`): historical replay calls `fbkl_logic` handlers
//! directly and creates no `job_run` rows, so replayed deadlines look unprocessed to this
//! poller. Before enabling the scheduler against a database containing replayed history, mark
//! those deadlines' job_runs `Succeeded` (or only seed live-season deadlines).

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
    /// Errors at the orchestration layer itself (claiming/recording), not handler failures.
    pub errors: usize,
}

/// Runs one scheduler tick: find every due deadline lacking a `Succeeded` job_run and process
/// each. Callable directly for tests and the commissioner console's manual trigger.
#[instrument(skip(db))]
pub async fn run_scheduler_tick(db: &DatabaseConnection) -> Result<TickSummary> {
    let now = Utc::now().fixed_offset();
    let due_deadlines = deadline_queries::find_due_unprocessed_deadlines(now, db).await?;

    let mut summary = TickSummary::default();
    for deadline_model in &due_deadlines {
        match process_deadline(db, deadline_model).await {
            Ok(ProcessOutcome::Processed { .. }) => summary.processed += 1,
            Ok(ProcessOutcome::Failed { .. }) => summary.failed += 1,
            Ok(
                ProcessOutcome::AlreadyProcessed
                | ProcessOutcome::AlreadyRunning
                | ProcessOutcome::AttemptsExhausted { .. },
            ) => summary.skipped += 1,
            Err(orchestration_error) => {
                // A claim/record failure for one deadline shouldn't abort the whole tick.
                summary.errors += 1;
                error!(
                    "Scheduler error processing deadline (id = {}): {orchestration_error:?}",
                    deadline_model.id
                );
            }
        }
    }

    // TODO(fbkl-rust-lcc, spec 01): synthesize auction sub-events (24h no-bid closes, §8.3.2
    // extension expiries) from open `auction` rows and dispatch via `process_event`.
    // TODO(fbkl-rust-1dk, spec 03): synthesize RFA 48h raise/match window expiries.

    if summary != TickSummary::default() {
        info!(
            "Scheduler tick: {} processed, {} failed, {} skipped, {} errors",
            summary.processed, summary.failed, summary.skipped, summary.errors
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
