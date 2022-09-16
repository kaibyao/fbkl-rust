use crate::{error::FbklError, AppState};
use axum::{
    body::Full,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Form,
};
use fbkl_auth::{decode_token, generate_password_hash, generate_token};
use fbkl_db::{
    chrono::Utc,
    models::{
        user_model::{InsertUser, UpdateUser},
        user_token_model::TokenTypeEnum,
    },
    queries::{user_queries, user_token_queries},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct RegistrationFormData {
    pub email: String,
    pub password: String,
    pub confirm_password: String,
}

pub async fn get_registration_page() -> Html<&'static str> {
    Html(
        r#"
<!doctype html>
<html>
    <head>
        <title>User registration</title>
    </head>
    <body>
        <form method="POST" action="/register">
            <input type="email" name="email" placeholder="Email">
            <input type="password" name="password">
            <input type="password" name="confirm_password">
            <button type="submit">Submit</button>
        </form>
    </body>
</html>
    "#,
    )
}

pub async fn process_registration(
    State(state): State<Arc<AppState>>,
    Form(form): Form<RegistrationFormData>,
) -> Result<Response, FbklError> {
    if form.password != form.confirm_password {
        let err_response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::from("PASSWORDS_NOT_MATCHING"))
            .expect("PASSWORDS_NOT_MATCHING_STATUS");
        return Ok(err_response.into_response());
    }

    let token = generate_token();
    let password_hash = generate_password_hash(&form.password)?;

    let insert_user = InsertUser {
        email: form.email,
        hashed_password: password_hash,
        confirmed_at: None,
        is_superadmin: false,
    };

    let mut conn = state.db_pool.get()?;
    let (new_user, new_user_token) =
        user_queries::insert_user(insert_user, token.into_iter().collect(), &mut conn)?;

    let html = format!(
        r#"
<!doctype html>
<html>
    <head>
        <title>User registration</title>
    </head>
    <body>
        <div>user: {:?}</div>
        <div>token: {:?}</div>
    </body>
</html>
    "#,
        new_user, new_user_token
    );

    Ok(Html(html).into_response())
}

#[derive(Deserialize)]
pub struct TokenQuery {
    token: String,
}

pub async fn confirm_registration(
    token_query: Query<TokenQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, FbklError> {
    let token = token_query.0.token.trim();
    if token.is_empty() {
        let err_response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::from("REQUIRED_TOKEN_MISSING"))
            .expect("REQUIRED_TOKEN_MISSING_STATUS");
        return Ok(err_response.into_response());
    }

    let token_bytes = decode_token(token)?;
    let mut conn = state.db_pool.get()?;

    let user_token = user_token_queries::find_by_token_and_type(
        token_bytes,
        TokenTypeEnum::RegistrationConfirm,
        &mut conn,
    )?;

    let update_user = UpdateUser {
        id: user_token.user_id,
        confirmed_at: Some(Some(Utc::now())),
        email: None,
        hashed_password: None,
        is_superadmin: None,
    };

    user_queries::update_user(update_user, &mut conn)?;

    Ok(StatusCode::OK.into_response())
}
