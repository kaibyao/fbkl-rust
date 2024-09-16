#![deny(clippy::all)]

mod error;
mod graphql;
mod handlers;
mod server;
mod session;

use std::sync::Arc;
use std::time::Duration;

use async_graphql::{EmptySubscription, Schema};
use axum::{serve, Extension};
use color_eyre::Result;
use fbkl_auth::{encode_token, generate_token};
use fbkl_entity::sea_orm::Database;
use server::AppState;
use time::Duration as TimeDuration;
use tokio::{signal, task::AbortHandle};
use tower_cookies::{cookie::SameSite, CookieManagerLayer, Key};
use tower_sessions::{session_store::ExpiredDeletion, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::graphql::{MutationRoot, QueryRoot};

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    // DB connection pool
    let database_url = std::env::var("FBKL_DATABASE_URL").expect("FBKL_DATABASE_URL must be set");
    let db_connection = Database::connect(&database_url).await?;
    let shared_state = Arc::new(AppState {
        db: db_connection.clone(),
    });

    // Server endpoints
    let router = server::setup_server_router();

    // Sessions
    let session_store = PostgresStore::new(db_connection.get_postgres_connection_pool().clone());
    session_store.migrate().await?;
    let session_deletion_task = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(Duration::from_secs(60)),
    );
    let session_secret = std::env::var("SESSION_SECRET")
        .unwrap_or_else(|_| encode_token(&generate_token().into_iter().collect()));
    let key = Key::from(session_secret.as_bytes());
    let session_layer = SessionManagerLayer::new(session_store)
        .with_private(key)
        .with_secure(true)
        .with_name("fbkl_id")
        .with_expiry(Expiry::OnInactivity(TimeDuration::seconds(
            90 * 24 * 60 * 60,
        )))
        .with_same_site(SameSite::None);

    // graphql setup
    let graphql_schema = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        EmptySubscription,
    )
    .data(shared_state.db.clone())
    .limit_depth(5)
    .finish();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9001").await?;
    let server = serve(
        listener,
        router
        .with_state(shared_state)
        // Layers only apply to routes preceding them. Make sure layers are applied after all routes.
        .layer(session_layer)
        .layer(CookieManagerLayer::new())
        .layer(Extension(graphql_schema)).into_make_service(),
    )
    .with_graceful_shutdown(shutdown_signal(session_deletion_task.abort_handle()));

    info!("Starting fbkl/server on port 9001...");

    // TODO: GQL: Add argument for only active team users in GetLeague
    // TODO: GQL: Add roster players w/ stats to GetLeague's teams
    // TODO: Build Transaction Processor. The idea being there's a job that runs every minute to update contracts, change teams, etc.
    // TODO: Something that automatically creates team updates. This might just be the same thing as the transaction processor.
    // TODO: Need to update players db table with new players from the NBA API.
    // TODO: Need some kind of storage for NBA dates (start of season, ASB start and end dates, MLK week early start times)
    // TODO: Maybe ping NBA API for game start times each week?
    // TODO: Reconciling end dates of different transaction types w/ when they go into effect.
    // TODO: Players change names.
    // TODO: Account for roster legalization (pre-season).
    // TODO: Account for roster legalization (weekly).
    // TODO: Configuration for rookie draft + end-of-season standings
    // TODO: import data (transactions)
    // TODO: Rest of DB migrations (incl. FK relations)
    // TODO: Auto-drop logic for weekly locks to ensure teams are legal.
    // TODO: login/registration needs validation (password length, email is correct, etc.)
    // TODO: Handle errors with actual HTTP status codes + logging (test w/ graphql errors)
    // TODO: Check if user confirmation already happened
    // TODO: Add CSP header: https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP
    // TODO: hook up user registration to sendgrid/similar.
    // TODO: Use Next.JS for public path? Turbopack seems interesting.
    // TODO: Possibly use https://github.com/casbin/casbin-rs for access control?
    // TODO: Create league config data structure to hold deadlines for each season.
    // TODO: advancing contracts should take into account custom players created for a league (merging them w/ nba/espn data if they exist in official datasets)
    // TODO: process for associating a league player with a real player and updating contracts related to league_player.

    server.await?;
    session_deletion_task.await??;

    Ok(())
}

fn setup() -> Result<()> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    dotenv::dotenv().ok();

    Ok(())
}

async fn shutdown_signal(deletion_task_abort_handle: AbortHandle) {
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
        _ = ctrl_c => { deletion_task_abort_handle.abort() },
        _ = terminate => { deletion_task_abort_handle.abort() },
    }
}
