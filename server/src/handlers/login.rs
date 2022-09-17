use axum::{
    body::Full,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Form,
};
use fbkl_auth::verify_password_against_hash;
use fbkl_entity::user_queries;
use serde::Deserialize;
use std::sync::Arc;

use crate::{error::FbklError, AppState};

#[derive(Debug, Deserialize)]
pub struct LoginFormData {
    email: String,
    password: String,
}

pub async fn login_page() -> Html<&'static str> {
    let html = r#"
<!doctype html>
<html>
    <head>
        <title>User login</title>
    </head>
    <body>
        <form method="POST" action="/login">
            <input type="email" name="email" placeholder="Email">
            <input type="password" name="password">
            <button type="submit">Submit</button>
        </form>
    </body>
</html>
    "#;

    Html(html)
}

pub async fn process_login(
    State(state): State<Arc<AppState>>,
    Form(form): Form<LoginFormData>,
) -> Result<Response, FbklError> {
    let email = form.email;

    let matching_user = match user_queries::find_user_by_email(&email, &state.db).await? {
        Some(matching_user) => matching_user,
        None => {
            let err_response = Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::from("USER_NOT_FOUND"))?;
            return Ok(err_response.into_response());
        }
    };

    verify_password_against_hash(&form.password, &matching_user.hashed_password)?;

    let html = r#"
<!doctype html>
<html>
    <head>
        <title>Login successful</title>
    </head>
    <body>
        OK!
    </body>
</html>
    "#;

    Ok(Html(html).into_response())
}
