#![deny(clippy::all)]

mod error;
mod handlers;

use actix_identity::IdentityMiddleware;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, dev::Server, middleware, web, App, HttpServer};
use color_eyre::Result;
use db::create_pool;
use handlers::{
    login::login_page,
    user_registration::{confirm_registration, process_registration, register},
};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    // DB connection pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let server = generate_server(database_url)?;

    info!("Starting fbkl/server on port 9001...");

    // TODO: Save session ID to cookie on browser side (/login)
    // TODO: "Secure" cookie
    // TODO: break out user auth steps into its own crate
    // TODO: Switch from Actix to Axum
    // TODO: Separate out "public" from "application"
    // TODO: Get front-end build pipeline working
    // TODO: eventually convert to GraphQL, but let's just focus on shipping / making progress instead of codewriter's block.
    // TODO: maybe break out user token generation into its own file + fn or even its own crate. or its own scheduled job.
    // TODO: Actual user confirmation.
    // TODO: Handle errors with actual HTTP status codes
    // TODO: Check if user confirmation already happened

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

fn generate_server<S>(database_url: S) -> Result<Server>
where
    S: Into<String>,
{
    let pool = create_pool(database_url);

    // session tokens
    let secret_key = Key::generate();

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            // Install the identity framework
            .wrap(IdentityMiddleware::default())
            // The identity system is built on top of sessions. You must install the session
            // middleware to leverage `actix-identity`. The session middleware must be mounted
            // AFTER the identity middleware: `actix-web` invokes middleware in the OPPOSITE
            // order of registration when it receives an incoming request.
            .wrap(SessionMiddleware::new(
                CookieSessionStore::default(),
                secret_key.clone(),
            ))
            .wrap(middleware::Logger::default())
            .service(register)
            .service(process_registration)
            .service(confirm_registration)
            .service(login_page)
    })
    .bind(("127.0.0.1", 9001))?
    .run();

    Ok(server)
}
