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

#[derive(Clone)]
pub struct ProductBrowserApp;

impl From<ProductBrowserApp> for Widget {
    fn from(_component: ProductBrowserApp) -> Self {
        let (ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
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

        let products_request = view.state().product_request();
        let categories_request = view.state().categories_request();
        let product_snapshot = view.state().products.clone();
        let category_snapshot = view.state().categories.clone();
        let selected_product = view.state().selected_product();

        let category_node = FutureBuilder::<ProductBrowserState, _>::new(
            ResourceKey::new("product-browser.categories"),
            CATEGORIES_JOB,
            categories_request.clone(),
            category_snapshot.clone(),
            |_, _, snapshot| {
                CategoryRail {
                    snapshot: snapshot.clone(),
                }
                .into()
            },
        )
        .deps(categories_request)
        .on_ok(categories_loaded)
        .on_err(categories_failed)
        .into();

        let product_node: Widget = FutureBuilder::<ProductBrowserState, _>::new(
            ResourceKey::new("product-browser.products"),
            PRODUCTS_JOB,
            products_request.clone(),
            product_snapshot.clone(),
            move |_, _, snapshot| {
                ProductResults {
                    snapshot: snapshot.clone(),
                    use_grid: grid,
                }
                .into()
            },
        )
        .deps(products_request)
        .on_ok(products_loaded)
        .on_err(products_failed)
        .into();

        let refreshed_products: Widget = RefreshIndicator::new(product_node)
            .status(view.state().refresh_status)
            .pulled_extent(view.state().pulled_extent)
            .trigger_distance(80.0)
            .displacement(64.0)
            .on_pull_start(pull_started)
            .on_pull_update(pull_updated)
            .on_pull_cancel(pull_canceled)
            .on_refresh(refresh_products)
            .id(WidgetId::explicit("product-browser.refresh"))
            .into();

        let product_area = Container::new(refreshed_products)
            .flex_grow(1.0)
            .bg(tokens.colors.background)
            .into();

        let content = if wide {
            let detail_panel = Column {
                gap: Some(0.0),
                children: vec![
                    ProductDetail {
                        product: selected_product.clone(),
                    }
                    .into(),
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into();

            Row {
                gap: Some(18.0),
                flex_grow: 1.0,
                align_items: ir_op::AlignItems::Stretch,
                children: vec![category_node, product_area, detail_panel],
                ..Default::default()
            }
            .into()
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
                    .into(),
                ],
                ..Default::default()
            }
            .into()
        };

        Container::new(Column {
            gap: Some(18.0),
            children: vec![
                Header {
                    on_search: search_changed,
                }
                .into(),
                content,
            ],
            ..Default::default()
        })
        .height(viewport.height.max(1.0))
        .padding_all(24.0)
        .bg(tokens.colors.background)
        .into()
    }
}
struct Header {
    on_search: ActionEnvelope,
}

impl From<Header> for Widget {
    fn from(component: Header) -> Self {
        let (_ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
        let summary = match view.state().products.data() {
            Some(page) if page.total > page.products.len() as u32 => {
                format!(
                    "{} shown from {} matching products",
                    page.products.len(),
                    page.total
                )
            }
            Some(page) => format!("{} products shown", page.products.len()),
            None if view.state().products.has_error() => "Product service unavailable".to_string(),
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
                    .into(),
                Text::new(summary)
                    .size(14.0)
                    .line_height(20.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
            ],
            ..Default::default()
        }
        .into();

        let search = TextInput {
            value: view.state().query.clone(),
            placeholder: Some("Search products".into()),
            on_change: Some(component.on_search.clone()),
            width: Some(if view.viewport_size().width >= 720.0 {
                320.0
            } else {
                (view.viewport_size().width - 48.0).max(240.0)
            }),
            ..Default::default()
        }
        .into();

        if view.viewport_size().width >= 720.0 {
            Row {
                gap: Some(18.0),
                children: vec![
                    title,
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into(),
                    search,
                ],
                ..Default::default()
            }
            .into()
        } else {
            Column {
                gap: Some(14.0),
                children: vec![title, search],
                ..Default::default()
            }
            .into()
        }
    }
}
