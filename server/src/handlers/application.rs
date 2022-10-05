use axum::response::Html;
use axum_sessions::extractors::ReadableSession;

use crate::{error::FbklError, server::enforce_logged_in};

pub async fn get_application(session: ReadableSession) -> Result<Html<&'static str>, FbklError> {
    enforce_logged_in(&session)?;

    Ok(Html(
        r#"
<!doctype html>
<html>
    <head>
        <title>FBKL</title>
    </head>
    <body>
        <div id="fbkl-application"></div>

        <script type="module">
        import RefreshRuntime from 'http://localhost:3100/@react-refresh'
        RefreshRuntime.injectIntoGlobalHook(window)
        window.$RefreshReg$ = () => {}
        window.$RefreshSig$ = () => (type) => type
        window.__vite_plugin_react_preamble_installed__ = true
        </script>
        <script type="module" src="http://localhost:3100/@vite/client"></script>
        <script type="module" src="http://localhost:3100/src/main.tsx"></script>
    </body>
</html>
"#,
    ))
}
