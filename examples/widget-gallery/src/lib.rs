mod colour_picker_section;
mod data_section;
mod display_section;
mod drag_drop;
mod feedback_section;
mod gallery_app;
mod gallery_header;
mod gallery_section;
mod input_section;
mod navigation_section;
mod overlay_section;
mod quality_gallery;
mod state;

pub use gallery_app::GalleryApp;
pub use quality_gallery::QualityGalleryApp;
pub use state::GalleryState;

#[cfg(target_arch = "wasm32")]
fn web_app() -> fission::prelude::WebApp<GalleryState, QualityGalleryApp> {
    fission::prelude::WebApp::<GalleryState, _>::new(QualityGalleryApp)
        .with_title("Fission UI Quality Gallery")
        .with_env(quality_gallery::initial_env())
        .with_design_system::<fission::theme::FissionDefaultDesignSystem>(
            quality_gallery::initial_design_mode(),
        )
        .with_state_init(quality_gallery::configure_initial_state)
        .mount("#fission-web-mount")
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<GalleryState, _>::new(GalleryApp).run()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
