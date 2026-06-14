//! `fbkl-api` Lambda: serves the full Axum HTTP API (GraphQL + login/registration
//! + public routes) behind a Lambda Function URL.
//!
//! The Axum `Router` built by `fbkl_server::build_router` implements
//! `Service<Request>`, so `lambda_http::run` drives it directly — the session,
//! cookie, and graphql layers ride along unchanged. Cookies are
//! `SameSite=None; Secure`, which works because the Function URL is HTTPS.

use std::sync::Arc;

use fbkl_lambdas::db;
use fbkl_server::{AppState, build_graphql_schema, build_router, build_session_layer};
use lambda_http::{Error, run, tracing};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // JSON, no ANSI — CloudWatch-friendly structured logs.
    tracing::init_default_subscriber();

    let db = db().await?.clone();
    let state = Arc::new(AppState { db: db.clone() });
    let session_layer = build_session_layer(&db);
    let schema = build_graphql_schema(db);

    let app = build_router(state, session_layer, schema);

    run(app).await
}
