pub mod app;

use anyhow::Result;
use app::CounterApp;
#[cfg(target_arch = "wasm32")]
use app::CounterState;
use fission::prelude::*;

#[cfg(target_arch = "wasm32")]
fn web_app() -> WebApp<CounterState, CounterApp> {
    WebApp::new(CounterApp).with_title("Fission Web Smoke")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop() -> Result<()> {
    DesktopApp::new(CounterApp)
        .with_title("Fission Web Smoke")
        .run()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
