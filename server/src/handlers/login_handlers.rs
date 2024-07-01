use axum::{
    extract::State, http::StatusCode, response::{Html, IntoResponse, Response}, Form
};
use fbkl_auth::verify_password_against_hash;
use fbkl_entity::user_queries;
use serde::Deserialize;
use std::sync::Arc;
use time::Duration;
use tower_sessions::{Expiry, Session};

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
    session: Session,
    Form(form): Form<LoginFormData>,
) -> Result<Response<String>, FbklError> {
    let email = form.email;

    let matching_user = match user_queries::find_user_by_email(&email, &state.db).await? {
        Some(matching_user) => matching_user,
        None => {
            let err_response = Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body("USER_NOT_FOUND".to_string())?;
            return Ok(err_response);
        }
    };

    verify_password_against_hash(&form.password, &matching_user.hashed_password)?;

    // create session
    session.cycle_id().await?;
    session.set_expiry(Some(Expiry::OnInactivity(Duration::days(90)))); // 90 days
    session.insert("user_id", matching_user.id).await?;

    // TODO: Separate page for login success

//     let html = r#"
// <!doctype html>
// <html>
//     <head>
//         <title>Login successful</title>
//     </head>
//     <body>
//         OK!
//     </body>
// </html>
//     "#;

    Ok(Response::new("ok".to_string()))
}

pub async fn logout(session: Session) -> Result<Response, FbklError> {
    session.flush().await?;

    Ok(StatusCode::OK.into_response())
}
