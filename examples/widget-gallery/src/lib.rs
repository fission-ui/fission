mod colour_picker_section;
mod data_section;
mod display_section;
mod drag_drop;
mod feedback_section;
mod form_section;
mod foundations_section;
mod gallery_app;
mod input_section;
mod layout_section;
mod navigation_section;
mod overlay_section;
mod pages;
mod picker_section;
mod quality_gallery;
mod state;

pub use gallery_app::GalleryApp;
pub use pages::GalleryPage;
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
    fission::prelude::DesktopApp::<GalleryState, _>::new(GalleryApp)
        .with_sync_env(|state: &GalleryState, env: &mut fission::prelude::Env| {
            use fission::theme::DesignSystem;
            let mode = if state.dark_mode {
                fission::theme::DesignMode::Dark
            } else {
                fission::theme::DesignMode::Light
            };
            let theme = if state.native_look {
                // Preview another platform's native look from this machine.
                let platform = match state.native_preview {
                    1 => fission::theme::HostPlatform::MacOs,
                    2 => fission::theme::HostPlatform::Android,
                    3 => fission::theme::HostPlatform::Windows,
                    _ => env.host_platform,
                };
                env.platform_look = fission::theme::PlatformLook::Native;
                fission::theme::PlatformLook::Native.theme_ref(platform, mode)
            } else {
                env.platform_look = fission::theme::PlatformLook::Unified;
                match state.look {
                    1 => fission::theme::FissionGraphiteDesignSystem::theme_ref(mode),
                    2 => fission::theme::FissionEmberDesignSystem::theme_ref(mode),
                    _ => fission::theme::FissionDefaultDesignSystem::theme_ref(mode),
                }
            };
            env.theme = theme.with_density(state.density);
        })
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
