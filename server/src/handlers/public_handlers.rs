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

        <script type="module">
        import RefreshRuntime from 'http://localhost:3200/@react-refresh'
        RefreshRuntime.injectIntoGlobalHook(window)
        window.$RefreshReg$ = () => {}
        window.$RefreshSig$ = () => (type) => type
        window.__vite_plugin_react_preamble_installed__ = true
        </script>
        <script type="module" src="http://localhost:3200/@vite/client"></script>
        <script type="module" src="http://localhost:3200/src/main.tsx"></script>
    </body>
</html>
    "#,
    )
}
