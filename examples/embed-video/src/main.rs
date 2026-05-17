fn main() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::new(embed_video::VideoEmbedApp)
        .with_title("Fission Video Embed")
        .run()
}
