//! `fbkl-scheduler` Lambda: runs one `jobs::run_scheduler_tick` per invocation,
//! driven by `EventBridge` Scheduler (1-minute cadence).
//!
//! This replaces the in-process 30s poll loop. We deliberately call the one-shot
//! tick — never `spawn_scheduler` (a forever loop Lambda can't run). Idempotency
//! is owned by the transaction-processor's `job_run` claims, so `EventBridge`
//! double-fires (and retries) are safe: an already-processed deadline is a no-op.

use fbkl_jobs::run_scheduler_tick;
use fbkl_lambdas::db;
use lambda_runtime::{Error, LambdaEvent, run, service_fn, tracing};
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Emits JSON when Lambda sets AWS_LAMBDA_LOG_FORMAT=JSON, which the
    // function's `logging_config` block in infra/lambdas.tf turns on. Text
    // format falls back to the ANSI-colored tracing formatter.
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}

async fn handler(_event: LambdaEvent<Value>) -> Result<(), Error> {
    let db = db().await?;
    // color_eyre's Report is not a std::error::Error, so it cannot become a
    // lambda_runtime::Error directly. The runtime logs whatever we return, so the
    // detail goes in the structured event and the returned error stays short.
    let summary = run_scheduler_tick(db).await.map_err(|tick_error| {
        tracing::error!(error = ?tick_error, "scheduler tick failed");
        "scheduler tick failed"
    })?;

    tracing::info!(
        processed = summary.processed,
        failed = summary.failed,
        skipped = summary.skipped,
        blocked = summary.blocked,
        errors = summary.errors,
        "scheduler tick complete"
    );

    Ok(())
}
