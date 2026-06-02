pub mod app;

use anyhow::Result;
use app::{CounterApp, CounterState};
use fission::prelude::*;

#[cfg(target_os = "android")]
const ANDROID_TEST_CONTROL_PORT: u16 = 48761;

#[cfg(target_arch = "wasm32")]
fn web_app() -> WebApp<CounterState, CounterApp> {
    WebApp::<CounterState, _>::new(CounterApp)
        .with_title("Fission Web Smoke")
        .mount("#fission-web-mount")
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<CounterState, CounterApp> {
    let app = MobileApp::<CounterState, _>::new(CounterApp).with_title("Fission Web Smoke");
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> Result<()> {
    DesktopApp::<CounterState, _>::new(CounterApp)
        .with_title("Fission Web Smoke")
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

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
