use actix_web::{get, http::header::ContentType, post, web, HttpResponse, Responder};
use fbkl_auth::verify_password_against_hash;
use fbkl_db::{queries::user_queries, FbklPool};
use serde::Deserialize;

use crate::error::FbklError;

#[derive(Debug, Deserialize)]
pub struct LoginFormData {
    email: String,
    password: String,
}

#[get("/login")]
pub async fn login_page() -> impl Responder {
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

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(html)
}

#[post("/login")]
pub async fn attempt_login(
    form: web::Form<LoginFormData>,
    pool: web::Data<FbklPool>,
) -> Result<impl Responder, FbklError> {
    let email = form.0.email;

    let mut conn = pool.get()?;
    let matching_user = user_queries::find_user_by_email(email, &mut conn)?;

    verify_password_against_hash(&form.0.password, &matching_user.hashed_password)?;

    Ok(HttpResponse::Ok())
}