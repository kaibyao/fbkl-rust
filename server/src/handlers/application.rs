use axum::response::Html;
use axum_sessions::extractors::ReadableSession;

use crate::{error::FbklError, server::enforce_logged_in};

pub async fn get_application(session: ReadableSession) -> Result<Html<&'static str>, FbklError> {
    enforce_logged_in(session)?;

    Ok(Html(
        r#"
<!doctype html>
<html>
    <head>
        <title>FBKL</title>
    </head>
    <body>
        <div id="fbkl-application"></div>
    </body>
</html>
"#,
    ))
}
