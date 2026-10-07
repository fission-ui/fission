mod app;
mod game;
mod scene;

#[cfg(any(not(target_arch = "wasm32"), feature = "standalone-entry"))]
use anyhow::Result;
pub use app::QualificationApp;
use app::QualificationState;
use fission::prelude::*;
pub use game::{Direction, MovePlayer, QualificationGame, ResetGame};

#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop() -> Result<()> {
    DesktopApp::<QualificationState, _>::new(QualificationApp)
        .with_env(app::create_env()?)
        .with_title("Fission 2D qualification — Beacon Run")
        .run()
}

#[cfg(all(target_arch = "wasm32", feature = "standalone-entry"))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    WebApp::<QualificationState, _>::new(QualificationApp)
        .with_env(
            app::create_env()
                .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?,
        )
        .with_title("Fission 2D qualification — Beacon Run")
        .mount("#fission-web-mount")
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
