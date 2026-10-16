mod app;
mod catalog;
mod components;
mod i18n;
#[cfg(test)]
mod interaction_tests;
mod mounted_example;
mod pages;
#[cfg(test)]
mod preview_theme_tests;
mod previews;
mod router;
mod semantics;
mod state;

use anyhow::Result;
use app::ShowcaseApp;
use fission::prelude::*;
use i18n::create_env;
use state::ShowcaseState;

include!(concat!(env!("OUT_DIR"), "/showcase_design_system.rs"));

#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop() -> Result<()> {
    DesktopApp::<ShowcaseState, _>::new(ShowcaseApp)
        .with_title("Fission Example Showcase")
        .with_env(create_env().expect("showcase translations must parse"))
        .with_design_system::<fission::theme::FissionDefaultDesignSystem>(DesignMode::Light)
        .with_sync_env(|state: &ShowcaseState, env: &mut Env| {
            env.locale = state.locale.clone();
            env.theme = state::shell_theme(state.theme_mode);
        })
        .run()
}

#[cfg(target_arch = "wasm32")]
fn web_app() -> WebApp<ShowcaseState, ShowcaseApp> {
    WebApp::<ShowcaseState, _>::new(ShowcaseApp)
        .mount("#fission-web-mount")
        .with_title("Fission Example Showcase")
        .with_env(create_env().expect("showcase translations must parse"))
        .with_design_system::<fission::theme::FissionDefaultDesignSystem>(DesignMode::Light)
        .with_sync_env(|state: &ShowcaseState, env: &mut Env| {
            env.locale = state.locale.clone();
            env.theme = state::shell_theme(state.theme_mode);
        })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
