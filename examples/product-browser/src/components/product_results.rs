use crate::api::{ApiError, ProductPage};
use crate::components::product_card::ProductCard;
use crate::model::ProductBrowserState;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct ProductResults {
    pub snapshot: AsyncSnapshot<ProductPage, ApiError>,
    pub use_grid: bool,
}

impl From<ProductResults> for Widget {
    fn from(component: ProductResults) -> Self {
        let (_ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
        if component.snapshot.connection_state == AsyncConnectionState::Waiting {
            return Center {
                child: Column {
                    gap: Some(12.0),
                    children: vec![
                        CircularProgress {
                            id: WidgetId::explicit("product-browser.loading"),
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
                    gap: Some(12.0),
                    children: vec![
                        Text::new("Products could not be loaded")
                            .size(22.0)
                            .weight(700)
                            .color(tokens.colors.text_primary)
                            .into(),
                        Text::new(error.message.clone())
                            .color(tokens.colors.text_secondary)
                            .max_width(520.0)
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

        if component.use_grid {
            let columns: usize = if view.viewport_size().width >= 1280.0 {
                3
            } else {
                2
            };
            let rows = (page.products.len() + columns - 1) / columns;
            let items = page
                .products
                .iter()
                .enumerate()
                .map(|(index, product)| {
                    let row = (index / columns) as i16 + 1;
                    let col = (index % columns) as i16 + 1;
                    GridItem::new(ProductCard {
                        product: product.clone(),
                        selected: Some(product.id) == view.state().selected_product_id,
                        compact: false,
                    })
                    .cell(row, col)
                    .into()
                })
                .collect();

            Scroll {
                child: Some(
                    Grid {
                        columns: vec![ir_op::GridTrack::Fr(1.0); columns],
                        rows: vec![ir_op::GridTrack::Auto; rows],
                        column_gap: Some(16.0),
                        row_gap: Some(16.0),
                        padding: [4.0, 16.0, 4.0, 24.0],
                        children: items,
                        ..Default::default()
                    }
                    .into(),
                ),
                flex_grow: 1.0,
                ..Default::default()
            }
            .into()
        } else {
            let items = page
                .products
                .iter()
                .map(|product| {
                    ProductCard {
                        product: product.clone(),
                        selected: Some(product.id) == view.state().selected_product_id,
                        compact: true,
                    }
                    .into()
                })
                .collect();

            LazyColumn {
                id: None,
                children: items,
                item_height: 138.0,
            }
            .into()
        }
    }
}
