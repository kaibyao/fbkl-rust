use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use fbkl_entity::sea_orm::DatabaseConnection;

use crate::handlers::{
    application_handlers::get_application,
    graphql_handlers::{graphiql, process_graphql},
    league_handlers::select_league,
    login_handlers::{login_page, logout, process_login},
    public_handlers::get_public_page,
    user_registration_handlers::{
        confirm_registration, get_registration_page, process_registration,
    },
};

/// Application state.
pub struct AppState {
    /// SeaORM database connection.
    pub db: DatabaseConnection,
}

pub fn setup_server_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_public_page))
        .route("/api/gql", get(graphiql).post(process_graphql))
        .route("/api/select_league", post(select_league))
        .route("/app", get(get_application))
        .route("/app/", get(get_application))
        .route("/app/*app_path", get(get_application))
        .route("/confirm_registration", get(confirm_registration))
        .route("/login", get(login_page).post(process_login))
        .route("/logout", get(logout))
        .route(
            "/register",
            get(get_registration_page).post(process_registration),
        )
        .route("/*public_path", get(get_public_page))
}
