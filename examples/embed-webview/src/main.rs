fn main() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<embed_webview::WebViewEmbedState, _>::new(
        embed_webview::WebViewEmbedApp,
    )
    .with_title("Fission WebView Embed")
    .run()
}
