use crate::api::{CATEGORIES_JOB, PRODUCTS_JOB};
use crate::components::browser_compact::ProductBrowserCompact;
use crate::components::browser_expanded::ProductBrowserExpanded;
use crate::components::categories::CategoryRail;
use crate::components::header::ProductBrowserHeader;
use crate::components::layout::EXPANDED_BREAKPOINT;
use crate::components::product_detail::{ProductDetail, ProductDetailLayout};
use crate::components::product_refresh::ProductRefresh;
use crate::model::{
    on_categories_failed, on_categories_loaded, on_products_failed, on_products_loaded,
    on_pull_canceled, on_pull_started, on_pull_updated, on_refresh_products, on_search_changed,
    CategoriesFailed, CategoriesLoaded, ProductBrowserState, ProductsFailed, ProductsLoaded,
    PullCanceled, PullStarted, PullUpdated, RefreshProducts, SearchChanged,
};
use fission::core::{JobResource, ResourceKey};
use fission::prelude::*;

fission_component! {
    pub ProductBrowserApp<ProductBrowserState>
    |_app, state, cx| {
        let tokens = &cx.view.env().theme.tokens;

        let products_loaded = with_reducer!(cx.build, ProductsLoaded, on_products_loaded);
        let products_failed = with_reducer!(cx.build, ProductsFailed, on_products_failed);
        let categories_loaded = with_reducer!(cx.build, CategoriesLoaded, on_categories_loaded);
        let categories_failed = with_reducer!(cx.build, CategoriesFailed, on_categories_failed);
        let search_changed = with_reducer!(cx.build, SearchChanged, on_search_changed);
        let pull_started = with_reducer!(cx.build, PullStarted, on_pull_started);
        let pull_updated = with_reducer!(cx.build, PullUpdated, on_pull_updated);
        let pull_canceled = with_reducer!(cx.build, PullCanceled, on_pull_canceled);
        let refresh_products = with_reducer!(cx.build, RefreshProducts, on_refresh_products);

        let products_request = state.product_request();
        let categories_request = state.categories_request();
        cx.build.with_resources(|resources| {
            resources.job(
                JobResource::new(
                    ResourceKey::new("product-browser.categories"),
                    CATEGORIES_JOB,
                    categories_request.clone(),
                )
                .deps(categories_request)
                .on_ok(categories_loaded)
                .on_err(categories_failed),
            );
            resources.job(
                JobResource::new(
                    ResourceKey::new("product-browser.products"),
                    PRODUCTS_JOB,
                    products_request.clone(),
                )
                .deps(products_request)
                .on_ok(products_loaded)
                .on_err(products_failed),
            );
        });

        let expanded = cx.view.viewport_size().width >= EXPANDED_BREAKPOINT;
    } =>
    Container(
        Column(
            gap: tokens.spacing.l,
            children: [
                ProductBrowserHeader(search_changed),
                if expanded {
                    ProductBrowserExpanded(
                        CategoryRail(state.categories.clone(), "expanded"),
                        ProductRefresh(
                            state.products.clone(),
                            "expanded",
                            state.refresh_status,
                            state.pulled_extent,
                            pull_started.clone(),
                            pull_updated.clone(),
                            pull_canceled.clone(),
                            refresh_products.clone(),
                        ),
                        ProductDetail(
                            state.selected_product(),
                            ProductDetailLayout::Expanded,
                        ),
                    )
                } else {
                    ProductBrowserCompact(
                        CategoryRail(state.categories.clone(), "compact"),
                        ProductRefresh(
                            state.products.clone(),
                            "compact",
                            state.refresh_status,
                            state.pulled_extent,
                            pull_started,
                            pull_updated,
                            pull_canceled,
                            refresh_products,
                        ),
                        ProductDetail(
                            state.selected_product(),
                            ProductDetailLayout::Compact,
                        ),
                    )
                },
            ],
        ),
    )
    .height_length(Length::vh(100.0))
    .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
    .bg(tokens.colors.background)
}
