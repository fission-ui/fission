use crate::api::{ApiError, ProductPage};
use crate::components::product_card::ProductCard;
use crate::model::ProductBrowserState;
use fission::prelude::*;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ProductResults {
    pub snapshot: AsyncSnapshot<ProductPage, ApiError>,
    pub use_grid: bool,
}

impl Widget<ProductBrowserState> for ProductResults {
    fn build(
        &self,
        ctx: &mut BuildCtx<ProductBrowserState>,
        view: &View<ProductBrowserState>,
    ) -> impl fission::IntoWidget<ProductBrowserState> {
        fission::AnyWidget::from_node({
            let tokens = &view.env.theme.tokens;
            if self.snapshot.connection_state == AsyncConnectionState::Waiting {
                return fission::AnyWidget::from_node(
                    Center {
                        child: Box::new(
                            Column {
                                gap: Some(12.0),
                                children: vec![
                                    CircularProgress {
                                        id: WidgetNodeId::explicit("product-browser.loading"),
                                        ..Default::default()
                                    }
                                    .build_node(ctx, view),
                                    Text::new("Loading products...")
                                        .color(tokens.colors.text_secondary)
                                        .into_node(),
                                ],
                                ..Default::default()
                            }
                            .into_node(),
                        ),
                    }
                    .build_node(ctx, view),
                );
            }

            if let Some(error) = self.snapshot.error() {
                return fission::AnyWidget::from_node(
                    Center {
                        child: Box::new(
                            Column {
                                gap: Some(12.0),
                                children: vec![
                                    Text::new("Products could not be loaded")
                                        .size(22.0)
                                        .weight(700)
                                        .color(tokens.colors.text_primary)
                                        .into_node(),
                                    Text::new(error.message.clone())
                                        .color(tokens.colors.text_secondary)
                                        .max_width(520.0)
                                        .into_node(),
                                ],
                                ..Default::default()
                            }
                            .into_node(),
                        ),
                    }
                    .build_node(ctx, view),
                );
            }

            let Some(page) = self.snapshot.data() else {
                return fission::AnyWidget::from_node(
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                );
            };

            if page.products.is_empty() {
                return fission::AnyWidget::from_node(
                    Center {
                        child: Box::new(
                            Text::new("No products match the current filters")
                                .color(tokens.colors.text_secondary)
                                .into_node(),
                        ),
                    }
                    .build_node(ctx, view),
                );
            }

            if self.use_grid {
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
                        GridItem::new(
                            ProductCard {
                                product: product.clone(),
                                selected: Some(product.id) == view.state.selected_product_id,
                                compact: false,
                            }
                            .build_node(ctx, view),
                        )
                        .cell(row, col)
                        .into_node()
                    })
                    .collect();

                Scroll {
                    child: Some(Box::new(
                        Grid {
                            columns: vec![ir_op::GridTrack::Fr(1.0); columns],
                            rows: vec![ir_op::GridTrack::Auto; rows],
                            column_gap: Some(16.0),
                            row_gap: Some(16.0),
                            padding: [4.0, 16.0, 4.0, 24.0],
                            children: items,
                            ..Default::default()
                        }
                        .into_node(),
                    )),
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node()
            } else {
                let items = page
                    .products
                    .iter()
                    .map(|product| {
                        ProductCard {
                            product: product.clone(),
                            selected: Some(product.id) == view.state.selected_product_id,
                            compact: true,
                        }
                        .build_node(ctx, view)
                    })
                    .collect();

                LazyColumn {
                    id: None,
                    children: Arc::new(items),
                    item_height: 138.0,
                }
                .into_node()
            }
        })
    }
}
