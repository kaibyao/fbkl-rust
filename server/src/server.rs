use std::sync::Arc;

use async_sea_orm_session::DatabaseSessionStore;
use axum::{http::StatusCode, routing::get, Router};
use axum_sessions::{extractors::ReadableSession, SameSite, SessionLayer};
use color_eyre::Result;
use fbkl_entity::sea_orm::DatabaseConnection;
use tower_cookies::CookieManagerLayer;

use crate::handlers::{
    application::get_application,
    login::{login_page, logout, process_login},
    public::get_public_page,
    user_registration::{confirm_registration, get_registration_page, process_registration},
};

/// Application state.
pub struct AppState {
    /// SeaORM database connection.
    pub db: DatabaseConnection,
}

/// Generates the Axum server that allows the user to interact with the application.
pub async fn generate_server(
    db: DatabaseConnection,
    session_secret: String,
) -> Result<Router<Arc<AppState>>> {
    let shared_state = Arc::new(AppState { db });

    // sessions
    let session_store = DatabaseSessionStore::new(shared_state.db.clone());
    let session_layer = SessionLayer::new(session_store, session_secret.as_bytes())
        .with_cookie_name("fbkl_id")
        .with_same_site_policy(SameSite::Strict)
        .with_secure(true);

    Ok(Router::with_state(shared_state)
        .route("/", get(get_public_page))
        .route("/app", get(get_application))
        .route("/confirm_registration", get(confirm_registration))
        .route("/login", get(login_page).post(process_login))
        .route("/logout", get(logout))
        .route(
            "/register",
            get(get_registration_page).post(process_registration),
        )

        // Layers only apply to routes preceding them. Make sure layers are applied after all routes.
        .layer(session_layer)
        .layer(CookieManagerLayer::new()))
}

/// Used within a handler that checks if a user is currently logged in and if not, return an error.
pub fn enforce_logged_in(session: ReadableSession) -> Result<i64, StatusCode> {
    match dbg!(session).get("user_id") {
        Some(user_id) => Ok(user_id),
        None => Err(StatusCode::UNAUTHORIZED),
    }
}
