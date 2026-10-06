const SCOUT_SHA256: &str = "c8f7e002bc807665ca7f456a794be2a785a27f61d1244c0d23a7ab3d0bee1d6f";

fn main() {
    println!("cargo:rerun-if-changed=assets/scout-sheet.svg");
    fission_assets::verify_sha256(
        "assets/scout-sheet.svg",
        include_bytes!("assets/scout-sheet.svg"),
        SCOUT_SHA256,
    )
    .expect("scene2d qualification asset validation failed");
    println!("cargo:rustc-env=SCENE2D_SCOUT_SHA256={SCOUT_SHA256}");
}
