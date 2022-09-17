use crate::{error::FbklError, server::AppState};
use axum::{
    body::Full,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Form,
};
use fbkl_auth::{decode_token, generate_password_hash, generate_token};
use fbkl_entity::{
    user, user_queries,
    user_registration::{self, UserRegistrationStatus},
    user_registration_queries,
};
use migration::sea_orm::{ActiveValue::NotSet, Set};
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
    let hashed_password = generate_password_hash(&form.password)?;

    let (new_user, new_user_token) = user_queries::insert_user(
        user::ActiveModel {
            id: NotSet,
            email: Set(form.email),
            hashed_password: Set(hashed_password),
            ..Default::default()
        },
        token.into_iter().collect(),
        &state.db,
    )
    .await?;

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
            .body(Full::from("REQUIRED_TOKEN_MISSING"))?;
        return Ok(err_response.into_response());
    }

    let token_bytes = decode_token(token)?;

    let mut found_user_registration: user_registration::ActiveModel =
        match user_registration_queries::find_user_registration_by_token(token_bytes, &state.db)
            .await?
        {
            Some(user_registration) => user_registration.into(),
            None => {
                let err_response = Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Full::from("USER_REGISTRATION_NOT_FOUND"))?;
                return Ok(err_response.into_response());
            }
        };

    found_user_registration.status = Set(UserRegistrationStatus::Confirmed);
    user_registration_queries::update_user_registration(found_user_registration, &state.db).await?;

    Ok(StatusCode::OK.into_response())
}
