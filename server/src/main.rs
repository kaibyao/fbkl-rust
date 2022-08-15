#![deny(clippy::all)]

use actix_identity::IdentityMiddleware;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, dev::Server, get, middleware, web, App, HttpServer, Responder, post, HttpRequest, Result as ActixResult, HttpResponse, http::header::ContentType};
use color_eyre::Result;
use db::create_pool;
use rand::{rngs::OsRng, RngCore};
use serde::Deserialize;
use tracing::info;
use tracing_subscriber::EnvFilter;

// #[get("/hello/{name}")]
// async fn greet(name: web::Path<String>) -> impl Responder {
//     format!("Hello {name}!")
// }

#[derive(Debug, Deserialize)]
struct RegistrationFormData {
    email: String,
    password: String
}

#[get("/register")]
async fn register() -> impl Responder {
    let html = r#"
<!doctype html>
<html>
    <head>
        <title>User registration</title>
    </head>
    <body>
        <form method="POST" action="/register">
            <input type="email" name="email" placeholder="Email">
            <input type="password" name="password">
            <button type="submit">Submit</button>
        </form>
    </body>
</html>
    "#;

    HttpResponse::Ok().content_type(ContentType::html()).body(html)
}

#[post("/register")]
async fn process_registration(form: web::Form<RegistrationFormData>) -> ActixResult<impl Responder> {
    let mut token = [0u8; 16];
    OsRng.fill_bytes(&mut token);

    dbg!(token);

    let html = format!(r#"
<!doctype html>
<html>
    <head>
        <title>User registration</title>
    </head>
    <body>
        <div>email: {}</div>
        <div>password: {}</div>
        <div>token: {}</div>
    </body>
</html>
    "#,
    form.email,
    form.password,
    token.iter().map(|byte| byte.to_string()).collect::<Vec<String>>().join(" "));

    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(html))
}

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    // DB connection pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let server = generate_server(database_url)?;

    info!("Starting fbkl/server on port 9001...");

    // TODO: Session ID generated needs to be 128 bits, or 16 bytes
    // TODO: Save session ID to cookie on browser side
    // TODO: insert Row into user_token table
    // TODO: User registration
    // TODO: "Secure" cookie

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
    })
    .bind(("127.0.0.1", 9001))?
    .run();

    Ok(server)
}
