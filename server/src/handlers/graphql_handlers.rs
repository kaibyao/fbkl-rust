use std::sync::Arc;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    Extension,
};
use axum_sessions::extractors::ReadableSession;

use crate::{graphql::FbklSchema, server::AppState, session::get_current_user};

/// This handler is the endpoint for all graphql queries.
pub async fn process_graphql(
    schema: Extension<FbklSchema>,
    read_session: ReadableSession,
    State(state): State<Arc<AppState>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let user_model = get_current_user(&read_session, &state.db).await;

    schema
        .execute(
            req.into_inner()
                .data(read_session)
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
