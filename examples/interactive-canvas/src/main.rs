#[cfg(target_os = "android")]
fn main() {}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_os = "ios")]
fn main() -> anyhow::Result<()> {
    interactive_canvas_example::run_mobile()
}

#[cfg(not(any(target_arch = "wasm32", target_os = "ios", target_os = "android")))]
fn main() -> anyhow::Result<()> {
    interactive_canvas_example::run_desktop()
}
