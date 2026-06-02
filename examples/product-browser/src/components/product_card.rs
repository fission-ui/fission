use crate::api::Product;
use crate::model::{on_product_selected, ProductBrowserState, ProductSelected};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct ProductCard {
    pub product: Product,
    pub selected: bool,
    pub compact: bool,
}

impl From<ProductCard> for Widget {
    fn from(component: ProductCard) -> Self {
        let (ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
        let select = with_reducer!(
            ctx,
            ProductSelected(component.product.id),
            on_product_selected
        );
        let border = if component.selected {
            tokens.colors.primary
        } else {
            tokens.colors.border
        };
        let image = Image::network(component.product.thumbnail.clone())
            .size(
                if component.compact { 96.0 } else { 220.0 },
                if component.compact { 96.0 } else { 160.0 },
            )
            .fit(ir_op::ImageFit::Contain)
            .into();

        let details = Column {
            gap: Some(6.0),
            children: vec![
                Text::new(component.product.title.clone())
                    .size(if component.compact { 16.0 } else { 18.0 })
                    .weight(700)
                    .color(tokens.colors.text_primary)
                    .max_lines(2)
                    .into(),
                Text::new(format!(
                    "{} · {:.1} stars",
                    component.product.category, component.product.rating
                ))
                .size(13.0)
                .color(tokens.colors.text_secondary)
                .max_lines(1)
                .into(),
                Text::new(format!("${:.2}", component.product.price))
                    .size(18.0)
                    .weight(700)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(format!("{} in stock", component.product.stock))
                    .size(12.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
            ],
            ..Default::default()
        }
        .into();

        let content: Widget = if component.compact {
            Row {
                gap: Some(14.0),
                children: vec![image, details],
                ..Default::default()
            }
            .into()
        } else {
            Column {
                gap: Some(12.0),
                children: vec![image, details],
                ..Default::default()
            }
            .into()
        };

        GestureDetector {
            child: Container::new(content)
                .bg(tokens.colors.surface)
                .border(border, if component.selected { 2.0 } else { 1.0 })
                .border_radius(18.0)
                .padding_all(14.0)
                .into(),
            on_tap: Some(select),
            ..Default::default()
        }
        .into()
    }
}
