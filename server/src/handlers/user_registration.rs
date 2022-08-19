use actix_web::{get, web, Responder, post, Result as ActixResult, HttpResponse, http::header::ContentType, Error as ActixError};
use rand::{rngs::OsRng, RngCore};
use serde::Deserialize;
use db::{queries::user_queries, FbklPool, models::user_model::InsertUser};

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
pub async fn process_registration(form: web::Form<RegistrationFormData>, pool: web::Data<FbklPool>) -> ActixResult<impl Responder> {
    let conn = match pool.get() {
        Ok(conn) => {

        },
        Err(e) => return Err(e.into())
    };

    let mut token = [0u8; 16];
    OsRng.fill_bytes(&mut token);

    let insert_user = InsertUser {
        email: form.email,
        hashed_password: form.password,
        confirmed_at: None,
        is_superadmin: false
    };
    let (new_user, new_user_token) = user_queries::insert(insert_user, token.iter().collect(), conn)?;

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
