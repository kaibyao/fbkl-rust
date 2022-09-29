use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    response::{Html, IntoResponse},
    Extension,
};
use axum_sessions::extractors::ReadableSession;

use crate::graphql::FbklSchema;

pub async fn process_graphql(
    schema: Extension<FbklSchema>,
    session: ReadableSession,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner().data(session)).await.into()
}

pub async fn graphiql() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("http://localhost:9001/gql")
            .finish(),
    )
}
