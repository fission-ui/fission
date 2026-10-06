# fission-assets

Typed provenance and licence metadata for assets shipped with Fission games
and applications. The contracts let build and release tooling validate that
each packaged asset has an auditable source and distribution policy.

This crate is an **alpha API**. Pin `fission-assets = "=0.1.0-alpha.1"` when
depending on it directly. It does not download, decode, or render media.

Use `verify_sha256` from `build.rs` to bind checked-in bytes to the digest
recorded in a runtime asset bundle:

```rust
const EXPECTED: &str = "…64 lowercase hexadecimal digits…";

fn main() {
    println!("cargo:rerun-if-changed=assets/player.png");
    fission_assets::verify_sha256(
        "assets/player.png",
        include_bytes!("assets/player.png"),
        EXPECTED,
    )
    .expect("player asset validation failed");
}
```
