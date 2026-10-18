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
        let product_snapshot = state.products.clone();
        let category_snapshot = state.categories.clone();
        let selected_product = state.selected_product();

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
        let instance = if expanded { "expanded" } else { "compact" };
        let products = ProductRefresh {
            snapshot: product_snapshot,
            instance,
            status: state.refresh_status,
            pulled_extent: state.pulled_extent,
            on_pull_start: pull_started,
            on_pull_update: pull_updated,
            on_pull_cancel: pull_canceled,
            on_refresh: refresh_products,
        };
        let categories = CategoryRail {
            snapshot: category_snapshot,
            instance,
        };
        let content: Widget = if expanded {
            ProductBrowserExpanded {
                categories,
                products,
                detail: ProductDetail {
                    product: selected_product,
                    layout: ProductDetailLayout::Expanded,
                },
            }
            .into()
        } else {
            ProductBrowserCompact {
                categories,
                products,
                detail: ProductDetail {
                    product: selected_product,
                    layout: ProductDetailLayout::Compact,
                },
            }
            .into()
        };

    } =>
    Container(
        Column(
            gap: tokens.spacing.l,
            children: [
                ProductBrowserHeader {
                    on_search: search_changed,
                },
                content,
            ],
        ),
    )
    .height_length(Length::vh(100.0))
    .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
    .bg(tokens.colors.background)
}
