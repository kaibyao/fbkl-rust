use crate::error::FbklError;
use actix_web::{get, http::header::ContentType, post, web, HttpResponse, Responder};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use db::chrono::Utc;
use db::models::user_model::UpdateUser;
use db::models::user_token_model::TokenTypeEnum;
use db::queries::user_token_queries;
use db::{models::user_model::InsertUser, queries::user_queries, FbklPool};
use rand::{rngs::OsRng, RngCore};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RegistrationFormData {
    email: String,
    password: String,
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
    let mut conn = pool.get()?;

    let mut token = [0u8; 16];
    OsRng.fill_bytes(&mut token);

    let password_bytes = form.0.password.as_bytes();
    let salt = SaltString::generate(&mut OsRng);

    let argon2 = Argon2::default();
    // Hash password to PHC string ($argon2id$v=19$...)
    let password_hash = argon2.hash_password(password_bytes, &salt)?.to_string();

    let insert_user = InsertUser {
        email: form.0.email,
        hashed_password: password_hash,
        confirmed_at: None,
        is_superadmin: false,
    };
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
        return Ok(HttpResponse::BadRequest());
    }

    let token_bytes = hex::decode(token)?;
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

    Ok(HttpResponse::Ok())
}
