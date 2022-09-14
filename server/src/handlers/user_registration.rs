use crate::error::FbklError;
use actix_web::{get, http::header::ContentType, post, web, HttpResponse, Responder};
use fbkl_auth::{decode_token, generate_password_hash, generate_token};
use fbkl_db::{
    chrono::Utc,
    models::{
        user_model::{InsertUser, UpdateUser},
        user_token_model::TokenTypeEnum,
    },
    queries::{user_queries, user_token_queries},
    FbklPool,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RegistrationFormData {
    email: String,
    password: String,
    confirm_password: String,
}

#[get("/register")]
pub async fn register() -> impl Responder {
    let html = r#"
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
    "#;

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(html)
}

#[post("/register")]
pub async fn process_registration(
    form: web::Form<RegistrationFormData>,
    pool: web::Data<FbklPool>,
) -> Result<impl Responder, FbklError> {
    if form.0.password != form.0.confirm_password {
        return Ok(HttpResponse::BadRequest()
            .reason("PASSWORDS_NOT_MATCHING")
            .finish());
    }

    let token = generate_token();
    let password_hash = generate_password_hash(&form.0.password)?;

    let insert_user = InsertUser {
        email: form.0.email,
        hashed_password: password_hash,
        confirmed_at: None,
        is_superadmin: false,
    };

    let mut conn = pool.get()?;
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

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(html))
}

#[derive(Deserialize)]
pub struct TokenQuery {
    token: String,
}

#[get("/confirm_registration")]
pub async fn confirm_registration(
    token_query: web::Query<TokenQuery>,
    pool: web::Data<FbklPool>,
) -> Result<impl Responder, FbklError> {
    let token = token_query.0.token.trim();
    if token.is_empty() {
        return Ok(HttpResponse::BadRequest()
            .reason("REQUIRED_TOKEN_MISSING")
            .finish());
    }

    let token_bytes = decode_token(token)?;
    let mut conn = pool.get()?;

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

    Ok(HttpResponse::Ok().finish())
}
