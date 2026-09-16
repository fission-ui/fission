mod api;
mod components;
mod model;
mod translations;

pub use api::{fetch_categories, fetch_products, ProductCategory, CATEGORIES_JOB, PRODUCTS_JOB};
pub use components::browser::ProductBrowserApp;
pub use model::ProductBrowserState;
pub use translations::translation_bundles;

/// The browser as another app mounts it, such as the example showcase. The host
/// already names the example, so the header drops the app title.
pub fn embedded_state() -> ProductBrowserState {
    ProductBrowserState {
        embedded: true,
        ..ProductBrowserState::default()
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    let mut env = fission::core::Env::default();
    for bundle in translation_bundles() {
        env.i18n.add_bundle(bundle);
    }
    env.locale = fission::i18n::Locale::from("en-US");
    fission::prelude::DesktopApp::<ProductBrowserState, _>::new(ProductBrowserApp)
        .with_env(env)
        .with_title("Fission Product Browser")
        .with_async(|asyncs| {
            asyncs.register_job(PRODUCTS_JOB, |request, _| async move {
                fetch_products(request).await
            });
            asyncs.register_job(CATEGORIES_JOB, |request, _| async move {
                fetch_categories(request).await
            });
        })
        .with_sync_env(
            |_state: &ProductBrowserState, env: &mut fission::core::Env| {
                env.theme = fission::theme::Theme::default();
            },
        )
        .run()
}
