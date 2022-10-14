use std::{sync::Arc, time::Duration};

use async_graphql::{EmptySubscription, Schema};
use async_sea_orm_session::DatabaseSessionStore;
use axum::{
    routing::{get, post},
    Extension, Router,
};
use axum_sessions::{SameSite, SessionLayer};
use color_eyre::Result;
use fbkl_entity::sea_orm::DatabaseConnection;
use tower_cookies::CookieManagerLayer;

use crate::{
    graphql::{MutationRoot, QueryRoot},
    handlers::{
        application_handlers::get_application,
        graphql_handlers::{graphiql, process_graphql},
        league_handlers::select_league,
        login_handlers::{login_page, logout, process_login},
        public_handlers::get_public_page,
        user_registration_handlers::{
            confirm_registration, get_registration_page, process_registration,
        },
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
) -> Result<Router<Arc<AppState>>> {
    let shared_state = Arc::new(AppState { db });

    // sessions
    let session_store = DatabaseSessionStore::new(shared_state.db.clone());
    let session_layer = SessionLayer::new(session_store, session_secret.as_bytes())
        .with_session_ttl(Some(Duration::from_secs(90 * 24 * 60 * 60)))
        .with_cookie_name("fbkl_id")
        .with_same_site_policy(SameSite::None)
        .with_secure(true);

    // graphql setup
    let graphql_schema = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        EmptySubscription,
    )
    .data(shared_state.db.clone())
    .limit_depth(5)
    .finish();

    Ok(Router::with_state(shared_state)
        .route("/", get(get_public_page))
        .route("/app", get(get_application))
        .route("/app/*app_path", get(get_application))
        .route("/confirm_registration", get(confirm_registration))
        .route("/gql", get(graphiql).post(process_graphql))
        .route("/login", get(login_page).post(process_login))
        .route("/logout", get(logout))
        .route(
            "/register",
            get(get_registration_page).post(process_registration),
        )
        .route(
            "/select_league",
            post(select_league)
        )
        .route(
            "/*public_path", get(get_public_page)
        )

        // Layers only apply to routes preceding them. Make sure layers are applied after all routes.
        .layer(session_layer)
        .layer(CookieManagerLayer::new())
        .layer(Extension(graphql_schema)))
}
