use crate::api::{CATEGORIES_JOB, PRODUCTS_JOB};
use crate::components::categories::CategoryRail;
use crate::components::product_detail::ProductDetail;
use crate::components::product_results::ProductResults;
use crate::model::{
    on_categories_failed, on_categories_loaded, on_products_failed, on_products_loaded,
    on_pull_canceled, on_pull_started, on_pull_updated, on_refresh_products, on_search_changed,
    CategoriesFailed, CategoriesLoaded, ProductBrowserState, ProductsFailed, ProductsLoaded,
    PullCanceled, PullStarted, PullUpdated, RefreshProducts, SearchChanged,
};
use fission::core::ResourceKey;
use fission::prelude::*;

pub struct ProductBrowserApp;

impl Widget<ProductBrowserState> for ProductBrowserApp {
    fn build(
        &self,
        ctx: &mut BuildCtx<ProductBrowserState>,
        view: &View<ProductBrowserState>,
    ) -> Node {
        let tokens = &view.env.theme.tokens;
        let viewport = view.viewport_size();
        let wide = viewport.width >= 980.0;
        let grid = viewport.width >= 760.0;

        let products_loaded = with_reducer!(ctx, ProductsLoaded, on_products_loaded);
        let products_failed = with_reducer!(ctx, ProductsFailed, on_products_failed);
        let categories_loaded = with_reducer!(ctx, CategoriesLoaded, on_categories_loaded);
        let categories_failed = with_reducer!(ctx, CategoriesFailed, on_categories_failed);
        let search_changed = with_reducer!(ctx, SearchChanged(String::new()), on_search_changed);
        let pull_started = with_reducer!(ctx, PullStarted, on_pull_started);
        let pull_updated = with_reducer!(ctx, PullUpdated, on_pull_updated);
        let pull_canceled = with_reducer!(ctx, PullCanceled, on_pull_canceled);
        let refresh_products = with_reducer!(ctx, RefreshProducts, on_refresh_products);

        let products_request = view.state.product_request();
        let categories_request = view.state.categories_request();
        let product_snapshot = view.state.products.clone();
        let category_snapshot = view.state.categories.clone();
        let selected_product = view.state.selected_product();

        let category_node = FutureBuilder::new(
            ResourceKey::new("product-browser.categories"),
            CATEGORIES_JOB,
            categories_request.clone(),
            category_snapshot,
            |ctx, view, snapshot| {
                CategoryRail {
                    snapshot: snapshot.clone(),
                }
                .build(ctx, view)
            },
        )
        .deps(categories_request)
        .on_ok(categories_loaded)
        .on_err(categories_failed)
        .build(ctx, view);

        let product_node = FutureBuilder::new(
            ResourceKey::new("product-browser.products"),
            PRODUCTS_JOB,
            products_request.clone(),
            product_snapshot,
            move |ctx, view, snapshot| {
                ProductResults {
                    snapshot: snapshot.clone(),
                    use_grid: grid,
                }
                .build(ctx, view)
            },
        )
        .deps(products_request)
        .on_ok(products_loaded)
        .on_err(products_failed)
        .build(ctx, view);

        let refreshed_products = RefreshIndicator::new(product_node)
            .id(WidgetNodeId::explicit("product-browser.refresh"))
            .status(view.state.refresh_status)
            .pulled_extent(view.state.pulled_extent)
            .trigger_distance(80.0)
            .displacement(64.0)
            .on_pull_start(pull_started)
            .on_pull_update(pull_updated)
            .on_pull_cancel(pull_canceled)
            .on_refresh(refresh_products)
            .build(ctx, view);

        let product_area = Container::new(refreshed_products)
            .flex_grow(1.0)
            .bg(tokens.colors.background)
            .into_node();

        let content = if wide {
            let detail_panel = Column {
                gap: Some(0.0),
                children: vec![
                    ProductDetail {
                        product: selected_product.clone(),
                    }
                    .build(ctx, view),
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node();

            Row {
                gap: Some(18.0),
                flex_grow: 1.0,
                align_items: ir_op::AlignItems::Stretch,
                children: vec![category_node, product_area, detail_panel],
                ..Default::default()
            }
            .into_node()
        } else {
            Column {
                gap: Some(16.0),
                flex_grow: 1.0,
                children: vec![
                    category_node,
                    product_area,
                    ProductDetail {
                        product: selected_product,
                    }
                    .build(ctx, view),
                ],
                ..Default::default()
            }
            .into_node()
        };

        Container::new(
            Column {
                gap: Some(18.0),
                children: vec![
                    Header {
                        on_search: search_changed,
                    }
                    .build(ctx, view),
                    content,
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .height(viewport.height.max(1.0))
        .padding_all(24.0)
        .bg(tokens.colors.background)
        .into_node()
    }
}

struct Header {
    on_search: ActionEnvelope,
}

impl Widget<ProductBrowserState> for Header {
    fn build(
        &self,
        _ctx: &mut BuildCtx<ProductBrowserState>,
        view: &View<ProductBrowserState>,
    ) -> Node {
        let tokens = &view.env.theme.tokens;
        let summary = match view.state.products.data() {
            Some(page) if page.total > page.products.len() as u32 => {
                format!(
                    "{} shown from {} matching products",
                    page.products.len(),
                    page.total
                )
            }
            Some(page) => format!("{} products shown", page.products.len()),
            None if view.state.products.has_error() => "Product service unavailable".to_string(),
            None => "Loading product catalog".to_string(),
        };
        let title = Column {
            gap: Some(6.0),
            children: vec![
                Text::new("Product Browser")
                    .size(34.0)
                    .line_height(42.0)
                    .weight(800)
                    .color(tokens.colors.text_primary)
                    .into_node(),
                Text::new(summary)
                    .size(14.0)
                    .line_height(20.0)
                    .color(tokens.colors.text_secondary)
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node();

        let search = TextInput {
            value: view.state.query.clone(),
            placeholder: Some("Search products".into()),
            on_change: Some(self.on_search.clone()),
            width: Some(if view.viewport_size().width >= 720.0 {
                320.0
            } else {
                (view.viewport_size().width - 48.0).max(240.0)
            }),
            ..Default::default()
        }
        .into_node();

        if view.viewport_size().width >= 720.0 {
            Row {
                gap: Some(18.0),
                children: vec![
                    title,
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                    search,
                ],
                ..Default::default()
            }
            .into_node()
        } else {
            Column {
                gap: Some(14.0),
                children: vec![title, search],
                ..Default::default()
            }
            .into_node()
        }
    }
}
