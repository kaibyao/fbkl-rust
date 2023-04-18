#![deny(clippy::all)]

mod error;
mod graphql;
mod handlers;
mod server;
mod session;

use color_eyre::Result;
use fbkl_auth::{encode_token, generate_token};
use fbkl_entity::sea_orm::Database;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    // DB connection pool
    let database_url = std::env::var("FBKL_DATABASE_URL").expect("FBKL_DATABASE_URL must be set");
    let db_connection = Database::connect(&database_url).await?;

    let session_secret = std::env::var("SESSION_SECRET").map_or_else(
        |_| encode_token(&generate_token().into_iter().collect()),
        |session_str| session_str,
    );
    let app = server::generate_server(db_connection, session_secret).await?;
    let server = axum::Server::bind(&"127.0.0.1:9001".parse()?).serve(app.into_make_service());

    info!("Starting fbkl/server on port 9001...");

    // TODO: Build Transaction Processor. The idea being there's a job that runs every minute to update contracts, change teams, etc.
    // TODO: Need some kind of storage for NBA dates (start of season, ASB start and end dates, MLK week early start times)
    // TODO: Maybe ping NBA API for game start times each week?
    // TODO: Reconciling end dates of different transaction types w/ when they go into effect.
    // TODO: Account for roster legalization (pre-season).
    // TODO: Account for roster legalization (weekly).
    // TODO: Do we need to account for 3 and 4-team trades? Yes, eventually. Need a M2M table for team<>trade.
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

    match server.await {
        Ok(_) => Ok(()),
        Err(e) => panic!("{}", e),
    }
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
