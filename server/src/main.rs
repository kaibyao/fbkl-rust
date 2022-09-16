#![deny(clippy::all)]

mod error;
mod handlers;
mod server;

use color_eyre::Result;
use fbkl_db::{create_pool, FbklPool};
// use migration::{Migrator, MigratorTrait};
use tracing::info;
use tracing_subscriber::EnvFilter;

pub struct AppState {
    pub db_pool: FbklPool,
}

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    // DB connection pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_pool = create_pool(database_url);

    let app = server::generate_server(db_pool);
    let server = axum::Server::bind(&"127.0.0.1:9001".parse()?).serve(app.into_make_service());

    info!("Starting fbkl/server on port 9001...");

    // TODO: Switch from Diesel to SeaORM
    // TODO: Save session ID to cookie on browser side (/login) + DB using https://docs.rs/async-sqlx-session/0.4.0/async_sqlx_session/struct.PostgresSessionStore.html
    // TODO: "Secure" + "HttpOnly" cookie attributes
    // TODO: Separate out "public" from "application"
    // TODO: Get front-end build pipeline working
    // TODO: eventually convert to GraphQL, but let's just focus on shipping / making progress instead of codewriter's block.
    // TODO: login/registration needs validation (password length, email is correct, etc.)
    // TODO: Handle errors with actual HTTP status codes
    // TODO: Check if user confirmation already happened
    // TODO: hook up user registration to sendgrid/similar.

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
