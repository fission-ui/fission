use crate::api::{ApiError, ProductPage};
use crate::components::layout::{
    GRID_BREAKPOINT, PRODUCT_GRID_MIN_TRACK, PRODUCT_LIST_ITEM_HEIGHT,
};
use crate::components::product_card::{ProductCard, ProductCardDensity};
use crate::model::ProductBrowserState;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct ProductResults {
    pub snapshot: AsyncSnapshot<ProductPage, ApiError>,
    pub instance: &'static str,
}

impl From<ProductResults> for Widget {
    fn from(component: ProductResults) -> Self {
        let (_ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
        let instance = component.instance;
        if component.snapshot.connection_state == AsyncConnectionState::Waiting {
            return Center {
                child: Column {
                    gap: Some(tokens.spacing.s),
                    children: vec![
                        CircularProgress {
                            id: WidgetId::explicit(&format!("product-browser.loading.{instance}")),
                            ..Default::default()
                        }
                        .into(),
                        Text::new("Loading products...")
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                    ..Default::default()
                }
                .into(),
            }
            .into();
        }

        if let Some(error) = component.snapshot.error() {
            return Center {
                child: Column {
                    gap: Some(tokens.spacing.m),
                    children: vec![
                        Text::new("Products could not be loaded")
                            .size(tokens.typography.font_size_xl)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.text_primary)
                            .into(),
                        Text::new(error.message.clone())
                            .color(tokens.colors.text_secondary)
                            .size(tokens.typography.body_medium_size)
                            .into(),
                    ],
                    ..Default::default()
                }
                .into(),
            }
            .into();
        }

        let Some(page) = component.snapshot.data() else {
            return Spacer {
                flex_grow: 1.0,
                ..Default::default()
            }
            .into();
        };

        if page.products.is_empty() {
            return Center {
                child: Text::new("No products match the current filters")
                    .color(tokens.colors.text_secondary)
                    .into(),
            }
            .into();
        }

        let selected_product = view.state().selected_product_id;
        if view.viewport_size().width >= GRID_BREAKPOINT {
            Scroll {
                id: Some(WidgetId::explicit(&format!(
                    "product-browser.results.{instance}.grid"
                ))),
                child: Some(
                    Grid {
                        columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                            GridTrack::Points(PRODUCT_GRID_MIN_TRACK),
                            GridTrack::Fr(1.0),
                        ))],
                        rows: vec![GridTrack::Auto],
                        column_gap: Some(tokens.spacing.m),
                        row_gap: Some(tokens.spacing.m),
                        padding: [
                            tokens.spacing.s,
                            tokens.spacing.m,
                            tokens.spacing.s,
                            tokens.spacing.m,
                        ],
                        children: page
                            .products
                            .iter()
                            .map(|product| {
                                ProductCard {
                                    product: product.clone(),
                                    selected: Some(product.id) == selected_product,
                                    density: ProductCardDensity::Comfortable,
                                    instance: format!("{instance}.grid"),
                                }
                                .into()
                            })
                            .collect(),
                        ..Default::default()
                    }
                    .into(),
                ),
                flex_grow: 1.0,
                ..Default::default()
            }
            .into()
        } else {
            LazyColumn {
                id: Some(WidgetId::explicit(&format!(
                    "product-browser.results.{instance}.list"
                ))),
                item_height: PRODUCT_LIST_ITEM_HEIGHT,
                children: page
                    .products
                    .iter()
                    .map(|product| {
                        ProductCard {
                            product: product.clone(),
                            selected: Some(product.id) == selected_product,
                            density: ProductCardDensity::Compact,
                            instance: format!("{instance}.list"),
                        }
                        .into()
                    })
                    .collect(),
            }
            .into()
        }
    }
}
