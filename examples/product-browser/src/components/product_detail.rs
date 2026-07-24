use crate::api::Product;
use crate::components::layout::{
    DETAIL_EMPTY_TEXT_WIDTH, DETAIL_MEDIA_HEIGHT, DETAIL_MEDIA_WIDTH, DETAIL_TEXT_WIDTH,
    DETAIL_WIDTH,
};
use crate::model::ProductBrowserState;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct ProductDetail {
    pub product: Option<Product>,
    pub layout: ProductDetailLayout,
}

#[derive(Clone, Copy, Debug)]
pub enum ProductDetailLayout {
    Compact,
    Expanded,
}

impl From<ProductDetail> for Widget {
    fn from(component: ProductDetail) -> Self {
        let (_ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
        let content: Widget = if let Some(product) = &component.product {
            Column {
                gap: Some(tokens.spacing.m),
                align_items: ir_op::AlignItems::Start,
                children: vec![
                    Image::network(product.thumbnail.clone())
                        .size(DETAIL_MEDIA_WIDTH, DETAIL_MEDIA_HEIGHT)
                        .fit(ir_op::ImageFit::Contain)
                        .into(),
                    Text::new(product.title.clone())
                        .size(tokens.typography.font_size_xl)
                        .line_height(
                            tokens.typography.font_size_xl * tokens.typography.line_height_heading,
                        )
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.text_primary)
                        .max_width(DETAIL_TEXT_WIDTH)
                        .into(),
                    Text::new(format!("${:.2}", product.price))
                        .size(tokens.typography.heading_size)
                        .weight(tokens.typography.font_weight_bold)
                        .color(tokens.colors.primary)
                        .into(),
                    Text::new(format!(
                        "{:.1} stars · {} in stock · {}",
                        product.rating, product.stock, product.category
                    ))
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary)
                    .max_width(DETAIL_TEXT_WIDTH)
                    .into(),
                    Text::new(product.description.clone())
                        .size(tokens.typography.body_medium_size)
                        .line_height(
                            tokens.typography.body_medium_size
                                * tokens.typography.line_height_normal,
                        )
                        .color(tokens.colors.text_primary)
                        .max_width(DETAIL_TEXT_WIDTH)
                        .into(),
                    Text::new(if product.tags.is_empty() {
                        "No tags".to_string()
                    } else {
                        format!("Tags: {}", product.tags.join(", "))
                    })
                    .size(tokens.typography.font_size_sm)
                    .color(tokens.colors.text_secondary)
                    .max_width(DETAIL_TEXT_WIDTH)
                    .into(),
                ],
                ..Default::default()
            }
            .into()
        } else {
            Center {
                child: Text::new("Select a product to see the details")
                    .color(tokens.colors.text_secondary)
                    .max_width(DETAIL_EMPTY_TEXT_WIDTH)
                    .into(),
            }
            .into()
        };

        let detail = Container::new(content)
            .padding_all(tokens.spacing.l)
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.xxl);

        match component.layout {
            ProductDetailLayout::Compact => detail.width_length(Length::percent(100.0)).into(),
            ProductDetailLayout::Expanded => detail.width(DETAIL_WIDTH).flex_shrink(0.0).into(),
        }
    }
}
