use crate::api::Product;
use crate::model::{on_product_selected, ProductBrowserState, ProductSelected};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct ProductCard {
    pub product: Product,
    pub selected: bool,
    pub compact: bool,
}

impl Widget<ProductBrowserState> for ProductCard {
    fn build(
        &self,
        ctx: &mut BuildCtx<ProductBrowserState>,
        view: &View<ProductBrowserState>,
    ) -> impl fission::IntoWidget<ProductBrowserState> {
        fission::AnyWidget::from_node({
            let tokens = &view.env.theme.tokens;
            let select = with_reducer!(ctx, ProductSelected(self.product.id), on_product_selected);
            let border = if self.selected {
                tokens.colors.primary
            } else {
                tokens.colors.border
            };
            let image = Image::network(self.product.thumbnail.clone())
                .size(
                    if self.compact { 96.0 } else { 220.0 },
                    if self.compact { 96.0 } else { 160.0 },
                )
                .fit(ir_op::ImageFit::Contain)
                .into_node();

            let details = Column {
                gap: Some(6.0),
                children: vec![
                    Text::new(self.product.title.clone())
                        .size(if self.compact { 16.0 } else { 18.0 })
                        .weight(700)
                        .color(tokens.colors.text_primary)
                        .max_lines(2)
                        .into_node(),
                    Text::new(format!(
                        "{} · {:.1} stars",
                        self.product.category, self.product.rating
                    ))
                    .size(13.0)
                    .color(tokens.colors.text_secondary)
                    .max_lines(1)
                    .into_node(),
                    Text::new(format!("${:.2}", self.product.price))
                        .size(18.0)
                        .weight(700)
                        .color(tokens.colors.primary)
                        .into_node(),
                    Text::new(format!("{} in stock", self.product.stock))
                        .size(12.0)
                        .color(tokens.colors.text_secondary)
                        .into_node(),
                ],
                ..Default::default()
            }
            .into_node();

            let content = if self.compact {
                Row {
                    gap: Some(14.0),
                    children: vec![image, details],
                    ..Default::default()
                }
                .into_node()
            } else {
                Column {
                    gap: Some(12.0),
                    children: vec![image, details],
                    ..Default::default()
                }
                .into_node()
            };

            GestureDetector {
                child: Box::new(
                    Container::new(content)
                        .bg(tokens.colors.surface)
                        .border(border, if self.selected { 2.0 } else { 1.0 })
                        .border_radius(18.0)
                        .padding_all(14.0)
                        .into_node(),
                ),
                on_tap: Some(select),
                ..Default::default()
            }
            .into_node()
        })
    }
}
