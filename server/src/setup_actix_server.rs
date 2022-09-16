use actix_identity::IdentityMiddleware;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, dev::Server, middleware, web, App, HttpServer};
use fbkl_db::FbklPool;
use crate::handlers::{
    login::{attempt_login, login_page},
    user_registration::{confirm_registration, process_registration, register},
};
use color_eyre::Result;

pub fn generate_server_actix(pool: FbklPool) -> Result<Server> {
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
            .service(attempt_login)
    })
    .bind(("127.0.0.1", 9001))?
    .run();

    Ok(server)
}
