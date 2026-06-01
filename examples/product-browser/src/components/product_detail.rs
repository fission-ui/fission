use crate::api::Product;
use crate::model::ProductBrowserState;
use fission::prelude::*;
use fission::IntoWidget;

#[derive(Clone, Debug)]
pub struct ProductDetail {
    pub product: Option<Product>,
}

impl Widget<ProductBrowserState> for ProductDetail {
    fn build(
        &self,
        ctx: &mut BuildCtx<ProductBrowserState>,
        view: &View<ProductBrowserState>,
    ) -> impl fission::IntoWidget<ProductBrowserState> {
        fission::core::view::internal_node_widget({
            {
                let tokens = &view.env.theme.tokens;
                let content = if let Some(product) = &self.product {
                    Column {
                        gap: Some(14.0),
                        align_items: ir_op::AlignItems::Start,
                        children: vec![
                            Image::network(product.thumbnail.clone())
                                .size(280.0, 220.0)
                                .fit(ir_op::ImageFit::Contain)
                                .into_node(),
                            Text::new(product.title.clone())
                                .size(24.0)
                                .weight(800)
                                .color(tokens.colors.text_primary)
                                .max_width(300.0)
                                .into_node(),
                            Text::new(format!("${:.2}", product.price))
                                .size(28.0)
                                .weight(800)
                                .color(tokens.colors.primary)
                                .into_node(),
                            Text::new(format!(
                                "{:.1} stars · {} in stock · {}",
                                product.rating, product.stock, product.category
                            ))
                            .size(13.0)
                            .color(tokens.colors.text_secondary)
                            .max_width(300.0)
                            .into_node(),
                            Text::new(product.description.clone())
                                .size(15.0)
                                .color(tokens.colors.text_primary)
                                .max_width(300.0)
                                .into_node(),
                            Text::new(if product.tags.is_empty() {
                                "No tags".to_string()
                            } else {
                                format!("Tags: {}", product.tags.join(", "))
                            })
                            .size(13.0)
                            .color(tokens.colors.text_secondary)
                            .max_width(300.0)
                            .into_node(),
                        ],
                        ..Default::default()
                    }
                    .into_node()
                } else {
                    Center {
                        child: Box::new(
                            Text::new("Select a product to see the details")
                                .color(tokens.colors.text_secondary)
                                .max_width(260.0)
                                .into_node(),
                        ),
                    }
                    .build(ctx, view)
                    .into_widget()
                    .lower_to_node(ctx, view)
                };

                Container::<fission::Node>::lowered(content)
                    .width(340.0)
                    .flex_shrink(0.0)
                    .padding_all(20.0)
                    .bg(tokens.colors.surface)
                    .border(tokens.colors.border, 1.0)
                    .border_radius(24.0)
                    .into_node()
            }
        })
    }
}
