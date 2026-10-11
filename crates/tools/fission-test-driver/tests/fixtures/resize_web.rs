//! A Web entrypoint for the freshly initialized resize-web test fixture.
pub mod app;
use app::{CounterApp, CounterState};
use fission::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop() -> anyhow::Result<()> {
    DesktopApp::<CounterState, _>::new(CounterApp).run()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    WebApp::<CounterState, _>::new(CounterApp)
        .with_mount_selector("#fission-web-mount")
        .with_navigation(
            WebNavigationConfig::new(WebRouteStrategy::Path)
                .base_path(option_env!("RESIZE_BASE_PATH").unwrap_or("/")),
        )
        .with_route_handler(|state, action, _| {
            state.route = action.location.pathname.trim_end_matches('/').to_string();
            if state.route.is_empty() {
                state.route = "/".into();
            }
        })
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
