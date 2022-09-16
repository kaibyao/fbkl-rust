use std::sync::Arc;

use axum::{routing::get, Router};
use axum_sessions::{async_session::MemoryStore, SessionLayer};
use fbkl_auth::generate_token;
use fbkl_db::FbklPool;
use tower_cookies::CookieManagerLayer;

use crate::{
    handlers::{
        login::{login_page, process_login},
        user_registration::{confirm_registration, get_registration_page, process_registration},
    },
    AppState,
};

pub fn generate_server(db_pool: FbklPool) -> Router<Arc<AppState>> {
    let shared_state = Arc::new(AppState { db_pool });

    // sessions
    let session_store = MemoryStore::new();
    let secret = generate_token();
    let session_layer = SessionLayer::new(session_store, &secret);

    Router::with_state(shared_state)
        .route(
            "/register",
            get(get_registration_page).post(process_registration),
        )
        .route("/confirm_registration", get(confirm_registration))
        .route("/login", get(login_page).post(process_login))

        // Layers only apply to routes preceding them. Make sure layers are applied after all routes.
        .layer(session_layer)
        .layer(CookieManagerLayer::new())
}
