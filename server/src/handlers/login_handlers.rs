use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Form, Json,
};
use fbkl_auth::verify_password_against_hash;
use fbkl_entity::user_queries;
use fbkl_logic::team_ownership::get_team_user_access_for_user_in_league;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::Duration;
use tower_sessions::{Expiry, Session};
use tracing::instrument;

use crate::{error::FbklError, server::AppState, session::get_current_user};

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

#[instrument(skip_all, fields(email = %form.email))]
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

#[derive(Serialize)]
#[serde(untagged)]
pub enum LoggedInResponse {
    LoggedIn {
        id: i64,
        email: String,
        selected_league_id: Option<i64>,
        /// The ID of the team in the selected league of which the user is a current owner.
        selected_league_owner_team_id: Option<i64>,
    },
    NotLoggedIn {},
}

pub async fn logged_in_data(
    session: Session,
    State(state): State<Arc<AppState>>,
) -> Result<Json<LoggedInResponse>, FbklError> {
    let user_model = match get_current_user(session.clone(), &state.db).await {
        None => return Ok(Json(LoggedInResponse::NotLoggedIn {})),
        Some(model) => model,
    };

    let selected_league_id = session.get::<i64>("selected_league_id").await?;

    let maybe_user_team_id = match selected_league_id {
        Some(league_id) => {
            match get_team_user_access_for_user_in_league(user_model.id, league_id, &state.db).await
            {
                Ok(maybe_team) => maybe_team.map(|team| team.id),
                Err(e) => {
                    tracing::error!("Error getting team user access for user in league: {}", e);
                    None
                }
            }
        }
        None => None,
    };

    Ok(Json(LoggedInResponse::LoggedIn {
        id: user_model.id,
        email: user_model.email,
        selected_league_id,
        selected_league_owner_team_id: maybe_user_team_id,
    }))
}
