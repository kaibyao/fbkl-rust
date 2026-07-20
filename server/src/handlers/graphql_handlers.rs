use std::sync::Arc;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    Extension,
    extract::State,
    response::{Html, IntoResponse},
};
use tower_sessions::Session;

use crate::{graphql::FbklSchema, server::AppState, session::get_current_user};

/// This handler is the endpoint for all graphql queries.
pub async fn process_graphql(
    schema: Extension<FbklSchema>,
    session: Session,
    State(state): State<Arc<AppState>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let user_model = match get_current_user(session.clone(), &state.db).await {
        Ok(user_model) => user_model,
        // Fail closed: a session/DB outage must not be served as "not logged in".
        Err(e) => {
            tracing::error!(error = ?e, "failed to load current user");
            return async_graphql::Response::from_errors(vec![async_graphql::ServerError::new(
                "internal server error",
                None,
            )])
            .into();
        }
    };

    schema
        .execute(
            req.into_inner()
                .data(session)
                .data(user_model)
                .data(state.db.clone()),
        )
        .await
        .into()
}

pub async fn graphiql() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("http://localhost:9001/api/gql")
            .finish(),
    )
}
