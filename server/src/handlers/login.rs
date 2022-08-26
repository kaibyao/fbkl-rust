use actix_web::{get, http::header::ContentType, post, web, HttpResponse, Responder};

use crate::error::FbklError;

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

// #[post("/login")]
// pub async fn attempt_login() -> Result<impl Responder, FbklError> {

// }
