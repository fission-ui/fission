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

#[derive(Clone)]
pub struct StoreHomePage;

impl From<StoreHomePage> for Widget {
    fn from(_: StoreHomePage) -> Self {
        let (ctx, view) = fission::build::current::<StoreState>();
        let catalog_loaded = with_reducer!(ctx, CatalogLoaded, on_catalog_loaded);
        let catalog_failed = with_reducer!(ctx, CatalogFailed, on_catalog_failed);
        ctx.register::<AddToCart, _>(reduce_with!(on_add_to_cart));
        let request = CatalogRequest { generation: 1 };
        let snapshot = view.state().catalog.clone();
        let gap = view.env().theme.tokens.spacing.l;

        ctx.with_resources(|resources| {
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

        StoreShell {
            child: Column {
                gap: Some(gap),
                children: widgets![
                    Hero,
                    CartSummary,
                    CardGrid { snapshot },
                    BrowserRuntimePanel,
                ],
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}
