mod error;
mod graphql;
mod handlers;
mod server;
mod session;

pub use graphql::*;
pub use server::*;

use std::sync::Arc;

use async_graphql::{EmptySubscription, Schema};
use axum::{Extension, Router};
use color_eyre::Result;
use fbkl_auth::{encode_token, generate_token};
use fbkl_entity::sea_orm::{Database, DatabaseConnection, DbErr};
use sha2::{Digest, Sha512};
use time::Duration as TimeDuration;
use tokio::signal;
use tokio::task::AbortHandle;
use tower_cookies::{CookieManagerLayer, Key, cookie::SameSite};
use tower_sessions::{Expiry, SessionManagerLayer, service::PrivateCookie};
use tower_sessions_sqlx_store::PostgresStore;
use tracing_subscriber::EnvFilter;

/// The async-graphql schema type shared by every entrypoint (local bin + Lambdas).
pub type AppSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

/// Connect to the SeaORM database pool from `FBKL_DATABASE_URL`.
///
/// Lambdas must point this at Supabase's transaction pooler (6543); the local
/// bin uses a direct connection.
pub async fn init_db() -> Result<DatabaseConnection, DbErr> {
    let database_url = std::env::var("FBKL_DATABASE_URL").expect("FBKL_DATABASE_URL must be set");
    Database::connect(&database_url).await
}

/// Build the async-graphql schema with the configured complexity/depth limits.
pub fn build_graphql_schema(db: DatabaseConnection) -> AppSchema {
    Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        EmptySubscription,
    )
    .data(db)
    .limit_complexity(50) // If this ever gets to 100, we should probably consider loader patterns.
    .limit_depth(10)
    .finish()
}

/// Build the Postgres-backed tower-sessions store over the shared connection pool.
///
/// Does NOT run `session_store.migrate()` — that is a one-time deploy/init step,
/// not a per-request or per-cold-start operation. Shared by the session layer
/// and the standalone session-GC Lambda.
pub fn build_session_store(db: &DatabaseConnection) -> PostgresStore {
    PostgresStore::new(db.get_postgres_connection_pool().clone())
}

/// Build the tower-sessions layer backed by a Postgres store.
pub fn build_session_layer(
    db: &DatabaseConnection,
) -> SessionManagerLayer<PostgresStore, PrivateCookie> {
    let session_store = build_session_store(db);
    let session_secret = std::env::var("SESSION_SECRET")
        .unwrap_or_else(|_| encode_token(&generate_token().into_iter().collect()));
    // `Key::from` requires >= 64 bytes; derive a fixed 64-byte key from the secret
    // via SHA-512 so any-length SESSION_SECRET is accepted.
    let key_bytes = Sha512::digest(session_secret.as_bytes());
    let key = Key::from(&key_bytes);
    SessionManagerLayer::new(session_store)
        .with_private(key)
        .with_secure(true)
        .with_name("fbkl_id")
        .with_expiry(Expiry::OnInactivity(TimeDuration::seconds(
            90 * 24 * 60 * 60,
        )))
        .with_same_site(SameSite::None)
}

/// Compose the full Axum router: routes + state + session/cookie layers + graphql schema.
///
/// Layers only apply to routes preceding them, so they are applied after all routes.
pub fn build_router(
    state: Arc<AppState>,
    session_layer: SessionManagerLayer<PostgresStore, PrivateCookie>,
    schema: AppSchema,
) -> Router {
    setup_server_router()
        .with_state(state)
        .layer(session_layer)
        .layer(CookieManagerLayer::new())
        .layer(Extension(schema))
}

/// Initialize color-eyre + tracing + dotenv for a local/long-running process.
///
/// Lambda binaries initialize their own subscriber instead of calling this.
pub fn setup() -> Result<()> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        // SAFETY: called once during single-threaded startup before any threads spawn.
        unsafe { std::env::set_var("RUST_LIB_BACKTRACE", "1") }
    }
    // Empty theme: errors are captured into strings (DB, logs), so ANSI codes would be literal noise.
    color_eyre::config::HookBuilder::default()
        .theme(color_eyre::config::Theme::new())
        .install()?;

    if std::env::var("RUST_LOG").is_err() {
        // SAFETY: called once during single-threaded startup before any threads spawn.
        unsafe { std::env::set_var("RUST_LOG", "info") }
    }
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    dotenv::dotenv().ok();

    Ok(())
}

/// Wait for Ctrl-C / SIGTERM, then abort the given background tasks. Used by the
/// local bin for graceful shutdown.
pub async fn shutdown_signal(background_task_abort_handles: Vec<AbortHandle>) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    for abort_handle in background_task_abort_handles {
        abort_handle.abort();
    }
}
