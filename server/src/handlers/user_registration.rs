use actix_web::{get, web, Responder, post, Result as ActixResult, HttpResponse, http::header::ContentType};
use rand::{rngs::OsRng, RngCore};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RegistrationFormData {
    email: String,
    password: String
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

    HttpResponse::Ok().content_type(ContentType::html()).body(html)
}

#[post("/register")]
pub async fn process_registration(form: web::Form<RegistrationFormData>) -> ActixResult<impl Responder> {
    let mut token = [0u8; 16];
    OsRng.fill_bytes(&mut token);

    dbg!(token);

    let html = format!(r#"
<!doctype html>
<html>
    <head>
        <title>User registration</title>
    </head>
    <body>
        <div>email: {}</div>
        <div>password: {}</div>
        <div>token: {}</div>
    </body>
</html>
    "#,
    form.email,
    form.password,
    token.iter().map(|byte| byte.to_string()).collect::<Vec<String>>().join(" "));

    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(html))
}
