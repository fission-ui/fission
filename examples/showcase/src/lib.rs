mod app;
mod catalog;
mod components;
mod i18n;
mod mounted_example;
mod pages;
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

macro_rules! register_example_jobs {
    ($asyncs:expr) => {{
        $asyncs.register_job(
            product_browser_example::PRODUCTS_JOB,
            |request, _| async move { product_browser_example::fetch_products(request).await },
        );
        $asyncs.register_job(
            product_browser_example::CATEGORIES_JOB,
            |request, _| async move { product_browser_example::fetch_categories(request).await },
        );
        $asyncs.register_job(
            field_inspector_example::WEATHER_JOB,
            |request, _| async move { field_inspector_example::fetch_weather(request).await },
        );
        $asyncs.register_job(
            field_inspector_example::STREAM_BYTES_JOB,
            |request, ctx| async move {
                field_inspector_example::collect_stream_bytes(request, ctx).await
            },
        );
        $asyncs.register_job(editor_example::TREE_SCAN_JOB, |request, _| async move {
            editor_example::run_tree_scan(request)
        });
        $asyncs.register_job(editor_example::GIT_STATUS_JOB, |request, _| async move {
            editor_example::run_git_status(request)
        });
        $asyncs.register_job(
            pokemon_card_store_example::CATALOG_JOB,
            |_request, _| async move {
                Ok::<_, pokemon_card_store_example::StoreError>(
                    pokemon_card_store_example::catalog_response(),
                )
            },
        );
    }};
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop() -> Result<()> {
    DesktopApp::<ShowcaseState, _>::new(ShowcaseApp)
        .with_title("Fission Example Showcase")
        .with_env(create_env().expect("showcase translations must parse"))
        .with_design_system::<ShowcaseDesignSystem>(DesignMode::Light)
        .with_async(|asyncs| register_example_jobs!(asyncs))
        .with_sync_env(|state: &ShowcaseState, env: &mut Env| {
            env.locale = state.locale.clone();
            env.theme = ShowcaseDesignSystem::theme(state.theme_mode);
        })
        .run()
}

#[cfg(target_arch = "wasm32")]
fn web_app() -> WebApp<ShowcaseState, ShowcaseApp> {
    WebApp::<ShowcaseState, _>::new(ShowcaseApp)
        .mount("#fission-web-mount")
        .with_title("Fission Example Showcase")
        .with_env(create_env().expect("showcase translations must parse"))
        .with_design_system::<ShowcaseDesignSystem>(DesignMode::Light)
        .with_async(|asyncs| register_example_jobs!(asyncs))
        .with_sync_env(|state: &ShowcaseState, env: &mut Env| {
            env.locale = state.locale.clone();
            env.theme = ShowcaseDesignSystem::theme(state.theme_mode);
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
