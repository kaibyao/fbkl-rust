//! Shared building blocks for FBKL's AWS Lambda binaries.
//!
//! The crate exposes a single process-wide database accessor tuned for Lambda's
//! execution model. Each `[[bin]]` (api, scheduler, session-gc) calls [`db`] to
//! obtain a connection that is initialized once per execution environment and
//! reused across warm invocations.

use std::time::Duration;

use fbkl_entity::sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use tokio::sync::OnceCell;

/// Process-wide connection, initialized once per Lambda execution environment.
static DB: OnceCell<DatabaseConnection> = OnceCell::const_new();

/// Return the shared SeaORM connection, initializing the pool on first call and
/// reusing it across warm invocations within the same execution environment.
///
/// `FBKL_DATABASE_URL` MUST point at Supabase's TRANSACTION pooler (port 6543,
/// Supavisor) at runtime. Bypassing the pooler (direct endpoint) risks
/// `FATAL: too many connections` with no queue. The pool is deliberately tiny
/// (one connection per execution env) so that Lambda reserved concurrency bounds
/// the worst-case number of client connections to the pooler.
pub async fn db() -> Result<&'static DatabaseConnection, DbErr> {
    DB.get_or_try_init(init_db).await
}

async fn init_db() -> Result<DatabaseConnection, DbErr> {
    let database_url = std::env::var("FBKL_DATABASE_URL").expect("FBKL_DATABASE_URL must be set");
    let mut opts = ConnectOptions::new(database_url);
    opts.max_connections(1)
        .min_connections(0)
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(600))
        .sqlx_logging(false);
    Database::connect(opts).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two `db()` calls must hand back the same shared connection — proving the
    /// pool is initialized exactly once per execution environment.
    ///
    /// Requires a reachable `FBKL_DATABASE_URL`; skips cleanly when unset so the
    /// suite passes in environments without a database.
    #[tokio::test]
    async fn db_is_initialized_once() {
        if std::env::var("FBKL_DATABASE_URL").is_err() {
            eprintln!("skipping db_is_initialized_once: FBKL_DATABASE_URL not set");
            return;
        }
        let first = db().await.expect("first db() init");
        let second = db().await.expect("second db() init");
        assert!(
            std::ptr::eq(first, second),
            "db() must return the same shared connection across calls"
        );
    }
}
