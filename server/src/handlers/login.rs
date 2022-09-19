use async_sea_orm_session::{
    prelude::{Session, SessionStore},
    DatabaseSessionStore,
};
use axum::{
    body::Full,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Form,
};
use axum_sessions::{extractors::WritableSession, SameSite};
use fbkl_auth::verify_password_against_hash;
use fbkl_entity::user_queries;
use serde::Deserialize;
use std::sync::Arc;
use tower_cookies::{Cookie, Cookies};

use crate::{error::FbklError, server::AppState};

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
    mut session: WritableSession,
    // cookies: Cookies,
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

    // create session
    session.insert("user_id", matching_user.id)?;
    // create new cookie with token
    // let mut cookie = Cookie::new("fbkl_id", session.id().to_string());
    // cookie.set_http_only(true);
    // cookie.set_secure(true);
    // cookie.set_same_site(SameSite::Strict);
    // cookies.add(cookie);

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
