const BEACON_GLTF_SHA256: &str = "4ef92e6f2cb0484c7589aa3dd64bb74bba87065c4c049f2e6abd7bf57832bdcf";
const BEACON_TEXTURE_SHA256: &str =
    "03065457d2d800f1f8d2103f15e9a72dcf9ecc288437850c3294dcbe95b4925f";

fn main() {
    verify(
        "assets/beacon.gltf",
        include_bytes!("assets/beacon.gltf"),
        BEACON_GLTF_SHA256,
    );
    verify(
        "assets/beacon.ppm",
        include_bytes!("assets/beacon.ppm"),
        BEACON_TEXTURE_SHA256,
    );
    println!("cargo:rustc-env=SCENE3D_BEACON_GLTF_SHA256={BEACON_GLTF_SHA256}");
    println!("cargo:rustc-env=SCENE3D_BEACON_TEXTURE_SHA256={BEACON_TEXTURE_SHA256}");
}

fn verify(path: &str, bytes: &[u8], expected: &str) {
    println!("cargo:rerun-if-changed={path}");
    fission_assets::verify_sha256(path, bytes, expected)
        .expect("scene3d qualification asset validation failed");
}
