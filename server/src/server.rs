use std::{sync::Arc, time::Duration};

use axum::{
    routing::{get, post},
    Router,
};
use color_eyre::Result;
use fbkl_entity::sea_orm::DatabaseConnection;
use time::Duration as TimeDuration;
use tower_cookies::{
    cookie::{Key, SameSite},
    CookieManagerLayer,
};
use tower_sessions::{session_store::ExpiredDeletion, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;

use crate::handlers::{
    application_handlers::get_application,
    league_handlers::select_league,
    login_handlers::{login_page, logout, process_login},
    public_handlers::get_public_page,
    user_registration_handlers::{
        confirm_registration, get_registration_page, process_registration
    },
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
) -> Result<Router> {
    let shared_state = Arc::new(AppState { db });

    // sessions
    let session_store = PostgresStore::new(shared_state.db.get_postgres_connection_pool().clone());
    session_store.migrate().await?;

    let deletion_task = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(Duration::from_secs(60)),
    );
    deletion_task.await??;

    let key = Key::from(session_secret.as_bytes());
    let session_layer = SessionManagerLayer::new(session_store)
        .with_private(key)
        .with_secure(true)
        .with_name("fbkl_id")
        .with_expiry(Expiry::OnInactivity(TimeDuration::seconds(
            90 * 24 * 60 * 60,
        )))
        .with_same_site(SameSite::None);

    Ok(Router::new()
        .route("/", get(get_public_page))
        .route(
            "/api/select_league",
            post(select_league)
        )
        .route("/app", get(get_application))
        .route("/app/*app_path", get(get_application))
        .route("/confirm_registration", get(confirm_registration))
        .route("/login", get(login_page).post(process_login))
        .route("/logout", get(logout))
        .route(
            "/register",
            get(get_registration_page).post(process_registration),
        )
        .route(
            "/*public_path", get(get_public_page)
        )
        .with_state(shared_state)

        // Layers only apply to routes preceding them. Make sure layers are applied after all routes.
        .layer(session_layer)
        .layer(CookieManagerLayer::new())
        )
}
