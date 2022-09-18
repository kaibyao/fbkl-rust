use std::sync::Arc;

use async_sea_orm_session::DatabaseSessionStore;
use axum::{routing::get, Router};
use axum_sessions::{SameSite, SessionLayer};
use color_eyre::Result;
use fbkl_auth::generate_token;
use fbkl_entity::sea_orm::DatabaseConnection;
use tower_cookies::CookieManagerLayer;

use crate::handlers::{
    login::{login_page, process_login},
    user_registration::{confirm_registration, get_registration_page, process_registration},
};

pub struct AppState {
    pub db: DatabaseConnection,
}

pub async fn generate_server(db: DatabaseConnection) -> Result<Router<Arc<AppState>>> {
    let shared_state = Arc::new(AppState { db });

    // sessions
    let session_store = DatabaseSessionStore::new(shared_state.db.clone());
    let secret = generate_token();
    let session_layer = SessionLayer::new(session_store, &secret)
        .with_cookie_name("fbkl_id")
        .with_same_site_policy(SameSite::Strict)
        .with_secure(true);

    Ok(Router::with_state(shared_state)
        .route(
            "/register",
            get(get_registration_page).post(process_registration),
        )
        .route("/confirm_registration", get(confirm_registration))
        .route("/login", get(login_page).post(process_login))

        // Layers only apply to routes preceding them. Make sure layers are applied after all routes.
        .layer(session_layer)
        .layer(CookieManagerLayer::new()))
}
