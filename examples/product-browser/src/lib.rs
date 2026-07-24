mod api;
mod components;
mod model;

pub use api::{fetch_categories, fetch_products, CATEGORIES_JOB, PRODUCTS_JOB};
pub use components::browser::ProductBrowserApp;
pub use model::ProductBrowserState;

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    fission::prelude::DesktopApp::<ProductBrowserState, _>::new(ProductBrowserApp)
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
