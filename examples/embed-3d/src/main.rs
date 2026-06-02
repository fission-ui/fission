fn main() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<embed_3d::Scene3DEmbedState, _>::new(embed_3d::Scene3DEmbedApp)
        .with_title("Fission 3D Embed")
        .run()
}
