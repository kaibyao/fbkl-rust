//! `fbkl-session-gc` Lambda: deletes expired session rows on a 5-minute
//! `EventBridge` schedule.
//!
//! Replaces the in-process `continuously_delete_expired(60s)` loop with a
//! one-shot `PostgresStore::delete_expired()`. Assumes the `tower_sessions`
//! schema already exists (created by the one-time `session_store.migrate()`
//! deploy/init step).

use fbkl_lambdas::db;
use fbkl_server::build_session_store;
use lambda_runtime::{Error, LambdaEvent, run, service_fn, tracing};
use serde_json::Value;
use tower_sessions::session_store::ExpiredDeletion;

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
    let store = build_session_store(db);
    store.delete_expired().await?;

    tracing::info!("expired sessions deleted");

    Ok(())
}
