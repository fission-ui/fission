mod app;
mod game;
mod scene;

pub use app::{QualificationApp, QualificationState};
pub use game::{Direction, HarborGame, HarborMessage, BEACON_NODE, PLAYER_BODY};
pub use scene::{build_scene, imported_beacon};

#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop() -> anyhow::Result<()> {
    use fission::prelude::DesktopApp;

    DesktopApp::<QualificationState, _>::new(QualificationApp)
        .with_env(app::create_env()?)
        .with_title("Fission 3D Qualification — Beacon Run")
        .run()
}

#[cfg(all(target_arch = "wasm32", feature = "standalone-entry"))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    use fission::prelude::WebApp;

    console_error_panic_hook::set_once();
    WebApp::<QualificationState, _>::new(QualificationApp)
        .with_env(
            app::create_env()
                .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?,
        )
        .with_title("Fission 3D Qualification — Beacon Run")
        .mount("#fission-web-mount")
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
