use actix_web::{get, web, App, HttpServer, Responder};
use color_eyre::{Result};

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello {name}!")
}

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    match HttpServer::new(|| {
        println!("Starting fbkl/server on port 5432...");

        App::new()
            .route("/hello", web::get().to(|| async { "Hello World!" }))
            .service(greet)
    })
    .bind(("127.0.0.1", 5432))?
    .run()
    .await {
        Ok(_) => Ok(()),
        Err(e) => panic!("{}", e)
    }
}

fn setup() -> Result<()> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    Ok(())
}
