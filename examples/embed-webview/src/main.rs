fn main() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::new(embed_webview::WebViewEmbedApp)
        .with_title("Fission WebView Embed")
        .run()
}
