use axum::response::Html;

pub async fn get_public_page() -> Html<&'static str> {
    Html(
        r#"
<!doctype html>
<html>
    <head>
        <title>Franchise Basketball Keepers League</title>
    </head>
    <body>
        <div id="fbkl-public"></div>
    </body>
</html>
    "#,
    )
}
