use std::sync::Arc;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    Extension,
};
use axum_sessions::extractors::ReadableSession;

use crate::{
    graphql::FbklSchema,
    server::{get_current_user, AppState},
};

/// This handler is the endpoint for all graphql queries.
pub async fn process_graphql(
    schema: Extension<FbklSchema>,
    session: ReadableSession,
    State(state): State<Arc<AppState>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let user_model = get_current_user(&session, &state.db).await;

    dbg!(&session);
    dbg!(&user_model);

    schema
        .execute(req.into_inner().data(session).data(user_model).data(state))
        .await
        .into()
}

pub async fn graphiql() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("http://localhost:9001/gql")
            .finish(),
    )
}
