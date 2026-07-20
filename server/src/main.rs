#![deny(clippy::all)]

use std::sync::Arc;
use std::time::Duration;

use axum::serve;
use color_eyre::Result;
use fbkl_server::{
    AppState, build_graphql_schema, build_router, build_session_layer, init_db, setup,
    shutdown_signal,
};
use tower_sessions::session_store::ExpiredDeletion;
use tower_sessions_sqlx_store::PostgresStore;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    // DB connection pool
    info!("Connecting to database...");
    let db_connection = init_db().await?;
    let shared_state = Arc::new(AppState {
        db: db_connection.clone(),
    });

    // Session store: migrate schema + spawn the expired-session deletion loop.
    // On Lambda these move to a one-time deploy step + a dedicated session-gc Lambda.
    info!("Migrating session store + starting session deletion loop...");
    let session_store = PostgresStore::new(db_connection.get_postgres_connection_pool().clone());
    session_store.migrate().await?;
    let session_deletion_task = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(Duration::from_mins(1)),
    );

    info!("Building session layer + graphql schema + router...");
    let session_layer = build_session_layer(&db_connection);
    let graphql_schema = build_graphql_schema(db_connection.clone());
    let router = build_router(shared_state, session_layer, graphql_schema);

    // Deadline scheduler: polls for due deadlines across all leagues and dispatches them
    // to the transaction processor (see notes/implementation-specs/05).
    info!("Starting scheduler...");
    let scheduler_task = fbkl_jobs::spawn_scheduler(db_connection.clone());

    info!("Starting server...");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9001").await?;
    let server =
        serve(listener, router.into_make_service()).with_graceful_shutdown(shutdown_signal(vec![
            session_deletion_task.abort_handle(),
            scheduler_task.abort_handle(),
        ]));

    info!("Starting fbkl/server on port 9001...");

    // TODO: Configuration for rookie draft + end-of-season standings
    // TODO: Functionality for draft picks
    // TODO: Functionality for trades
    // TODO: GQL: Add argument for only active team users in GetLeague
    // TODO: Need to update players db table with new players from the NBA API.
    // TODO: Players change names.
    // TODO: Need some kind of storage for NBA dates (start of season, ASB start and end dates, MLK week early start times)
    // TODO: Maybe ping NBA API for game start times each week?
    // TODO: Reconciling end dates of different transaction types w/ when they go into effect.
    // TODO: Account for roster legalization (pre-season).
    // TODO: Account for roster legalization (weekly).
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
