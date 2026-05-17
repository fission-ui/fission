fn main() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::new(embed_3d::Scene3DEmbedApp)
        .with_title("Fission 3D Embed")
        .run()
}
