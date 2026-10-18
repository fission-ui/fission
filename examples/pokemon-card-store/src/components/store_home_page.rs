use crate::app::{
    on_add_to_cart, on_catalog_failed, on_catalog_loaded, AddToCart, CatalogFailed, CatalogLoaded,
    StoreState,
};
use crate::components::browser_runtime_panel::BrowserRuntimePanel;
use crate::components::card_grid::CardGrid;
use crate::components::cart_summary::CartSummary;
use crate::components::hero::Hero;
use crate::components::shell::StoreShell;
use crate::data::{CatalogRequest, CATALOG_JOB};
use fission::core::{JobResource, ResourceKey};
use fission::prelude::*;

fission_component! {
    pub StoreHomePage<StoreState>
    |_page, state, cx| {
        let catalog_loaded = with_reducer!(cx.build, CatalogLoaded, on_catalog_loaded);
        let catalog_failed = with_reducer!(cx.build, CatalogFailed, on_catalog_failed);
        cx.build.register::<AddToCart, _>(reduce_with!(on_add_to_cart));
        let request = CatalogRequest { generation: 1 };
        let snapshot = state.catalog.clone();
        let gap = cx.view.env().theme.tokens.spacing.l;

        cx.build.with_resources(|resources| {
            resources.job(
                JobResource::new(
                    ResourceKey::new("pokemon-card-store.catalog"),
                    CATALOG_JOB,
                    request.clone(),
                )
                .deps(request)
                .on_ok(catalog_loaded)
                .on_err(catalog_failed),
            );
        });

    } =>
    StoreShell(
        Column(
            gap: gap,
            children: [
                Hero,
                CartSummary,
                CardGrid { snapshot },
                BrowserRuntimePanel,
            ],
        ),
    )
}
