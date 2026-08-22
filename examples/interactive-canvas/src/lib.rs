mod app;
mod canvas_node_card;
mod canvas_panel;
mod example_header;
mod example_layout;
mod state;
mod viewer_panel;
mod viewer_scene;

#[cfg(any(not(target_arch = "wasm32"), feature = "standalone-entry"))]
use anyhow::Result;
pub use app::InteractiveCanvasExample;
use fission::prelude::*;
pub use state::CanvasExampleState;

#[cfg(target_os = "android")]
const ANDROID_TEST_CONTROL_PORT: u16 = 48763;

#[cfg(all(target_arch = "wasm32", feature = "standalone-entry"))]
fn web_app() -> WebApp<CanvasExampleState, InteractiveCanvasExample> {
    WebApp::<CanvasExampleState, _>::new(InteractiveCanvasExample)
        .with_title("Fission Interactive Canvas")
        .mount("#fission-web-mount")
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<CanvasExampleState, InteractiveCanvasExample> {
    let app = MobileApp::<CanvasExampleState, _>::new(InteractiveCanvasExample)
        .with_title("Fission Interactive Canvas");
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> Result<()> {
    DesktopApp::<CanvasExampleState, _>::new(InteractiveCanvasExample)
        .with_title("Fission Interactive Canvas")
        .run()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn run_mobile() -> Result<()> {
    mobile_app().run()
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app_handle: AndroidApp) {
    let _ = mobile_app().run_with_android_app(app_handle);
}

#[cfg(all(target_arch = "wasm32", feature = "standalone-entry"))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
