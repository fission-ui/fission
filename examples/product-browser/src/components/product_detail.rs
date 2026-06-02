use crate::api::Product;
use crate::model::ProductBrowserState;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct ProductDetail {
    pub product: Option<Product>,
}

impl From<ProductDetail> for Widget {
    fn from(component: ProductDetail) -> Self {
        let (_ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
        let content: Widget = if let Some(product) = &component.product {
            Column {
                gap: Some(14.0),
                align_items: ir_op::AlignItems::Start,
                children: vec![
                    Image::network(product.thumbnail.clone())
                        .size(280.0, 220.0)
                        .fit(ir_op::ImageFit::Contain)
                        .into(),
                    Text::new(product.title.clone())
                        .size(24.0)
                        .weight(800)
                        .color(tokens.colors.text_primary)
                        .max_width(300.0)
                        .into(),
                    Text::new(format!("${:.2}", product.price))
                        .size(28.0)
                        .weight(800)
                        .color(tokens.colors.primary)
                        .into(),
                    Text::new(format!(
                        "{:.1} stars · {} in stock · {}",
                        product.rating, product.stock, product.category
                    ))
                    .size(13.0)
                    .color(tokens.colors.text_secondary)
                    .max_width(300.0)
                    .into(),
                    Text::new(product.description.clone())
                        .size(15.0)
                        .color(tokens.colors.text_primary)
                        .max_width(300.0)
                        .into(),
                    Text::new(if product.tags.is_empty() {
                        "No tags".to_string()
                    } else {
                        format!("Tags: {}", product.tags.join(", "))
                    })
                    .size(13.0)
                    .color(tokens.colors.text_secondary)
                    .max_width(300.0)
                    .into(),
                ],
                ..Default::default()
            }
            .into()
        } else {
            Center {
                child: Text::new("Select a product to see the details")
                    .color(tokens.colors.text_secondary)
                    .max_width(260.0)
                    .into(),
            }
            .into()
        };

        Container::new(content)
            .width(340.0)
            .flex_shrink(0.0)
            .padding_all(20.0)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(24.0)
            .into()
    }
}
