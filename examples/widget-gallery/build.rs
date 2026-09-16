//! Raises the main-thread stack reserve for Windows desktop builds.
//!
//! `FissionDefaultDesignSystem::theme_ref` builds its `ComponentTheme` as one
//! very large nested struct literal in generated code. Unoptimized, that single
//! function frame asks for about 1.02 MiB, which is more than the 1 MiB stack
//! Windows reserves for the main thread by default. The process therefore dies
//! with `STATUS_STACK_OVERFLOW` while the shell builds its first `Env`, before
//! any window exists.
//!
//! macOS and Linux hand the main thread 8 MiB, which is why this only shows up
//! on Windows. The reserve is address space and is committed on demand, so
//! raising it costs nothing at runtime. `cargo-fission` raises its own worker
//! stack for the same reason.

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "windows" && target_env == "msvc" {
        // 16 MiB, matching the reserve the Fission CLI gives its worker thread.
        println!("cargo:rustc-link-arg=/STACK:16777216");
    }
}
