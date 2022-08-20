use crate::error::FbklError;
use actix_web::{get, http::header::ContentType, post, web, HttpResponse, Responder};
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

    let insert_user = InsertUser {
        email: form.email.clone(),
        hashed_password: form.password.clone(),
        confirmed_at: None,
        is_superadmin: false,
    };
    let (new_user, new_user_token) =
        user_queries::insert(insert_user, token.into_iter().collect(), &mut conn)?;

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
