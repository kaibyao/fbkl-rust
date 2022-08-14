#![deny(clippy::all)]

use actix_identity::IdentityMiddleware;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, get, middleware, web, App, HttpServer, Responder};
use color_eyre::Result;
use db::create_pool;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello {name}!")
}

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    // DB connection pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = create_pool(database_url);

    // session tokens
    let secret_key = Key::generate();

    info!("Starting fbkl/server on port 9001...");

    match HttpServer::new(move || {
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
            .route("/hello", web::get().to(|| async { "Hello World!" }))
            .service(greet)
    })
    .bind(("127.0.0.1", 9001))?
    .run()
    .await
    {
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
