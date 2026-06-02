fn main() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<embed_video::VideoEmbedState, _>::new(embed_video::VideoEmbedApp)
        .with_title("Fission Video Embed")
        .run()
}
