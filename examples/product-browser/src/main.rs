mod api;
mod components;
mod model;

use api::{fetch_categories, fetch_products, CATEGORIES_JOB, PRODUCTS_JOB};
use components::browser::ProductBrowserApp;
use fission::prelude::*;
use model::ProductBrowserState;

fn main() -> anyhow::Result<()> {
    DesktopApp::<ProductBrowserState, _>::new(ProductBrowserApp)
        .with_title("Fission Product Browser")
        .with_async(|asyncs| {
            asyncs.register_job(PRODUCTS_JOB, |request, _| async move {
                fetch_products(request).await
            });
            asyncs.register_job(CATEGORIES_JOB, |request, _| async move {
                fetch_categories(request).await
            });
        })
        .with_sync_env(|_state: &ProductBrowserState, env: &mut Env| {
            env.theme = Theme::default();
        })
        .run()
}
