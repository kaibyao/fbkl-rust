use crate::{error::FbklError, server::AppState};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, Response},
    Form,
};
use fbkl_auth::{decode_token, generate_password_hash, generate_token};
use fbkl_entity::{
    sea_orm::{ActiveModelTrait, ActiveValue::NotSet, Set},
    user, user_queries,
    user_registration::{self, UserRegistrationStatus},
    user_registration_queries,
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
) -> Result<Response<String>, FbklError> {
    if form.password != form.confirm_password {
        let err_response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("PASSWORDS_NOT_MATCHING".to_string())
            .expect("PASSWORDS_NOT_MATCHING_STATUS");
        return Ok(err_response);
    }

    let token = generate_token();
    let hashed_password = generate_password_hash(&form.password)?;

    let (_new_user, _new_user_token) = user_queries::insert_user(
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

    // TODO: Separate page for user registration confirmation
//     let html = format!(
//         r#"
// <!doctype html>
// <html>
//     <head>
//         <title>User registration</title>
//     </head>
//     <body>
//         <div>user: {:?}</div>
//         <div>token: {:?}</div>
//     </body>
// </html>
//     "#,
//         new_user, new_user_token
//     );

    Ok(Response::new("ok".to_string()))
}

#[derive(Deserialize)]
pub struct TokenQuery {
    token: String,
}

pub async fn confirm_registration(
    Query(token_query): Query<TokenQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Response<String>, FbklError> {
    let token = token_query.token.trim();
    if token.is_empty() {
        let err_response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("REQUIRED_TOKEN_MISSING".to_string())?;
        return Ok(err_response);
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
                    .body("USER_REGISTRATION_NOT_FOUND".to_string())?;
                return Ok(err_response);
            }
        };

    found_user_registration.status = Set(UserRegistrationStatus::Confirmed);
    found_user_registration.update(&state.db).await?;

    Ok(Response::new("ok".to_string()))
}
