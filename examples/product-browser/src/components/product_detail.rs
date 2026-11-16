use crate::api::Product;
use crate::components::layout::{
    DETAIL_EMPTY_TEXT_WIDTH, DETAIL_MEDIA_HEIGHT, DETAIL_MEDIA_WIDTH, DETAIL_TEXT_WIDTH,
    DETAIL_WIDTH,
};
use crate::model::ProductBrowserState;
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub enum ProductDetailLayout {
    Compact,
    Expanded,
}

fission_component! {
    pub ProductDetail<ProductBrowserState> {
        product: Option<Product>,
        layout: ProductDetailLayout,
    } |component, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            if let Some(product) = &component.product {
                Column(
                    gap: tokens.spacing.m,
                    align_items: ir_op::AlignItems::Start,
                    children: [
                        Image::network(product.thumbnail.clone())
                            .size(DETAIL_MEDIA_WIDTH, DETAIL_MEDIA_HEIGHT)
                            .fit(ir_op::ImageFit::Contain),
                        Text(product.title.clone())
                            .size(tokens.typography.font_size_xl)
                            .line_height(
                                tokens.typography.font_size_xl
                                    * tokens.typography.line_height_heading,
                            )
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.text_primary)
                            .max_width(DETAIL_TEXT_WIDTH),
                        Text(format!("${:.2}", product.price))
                            .size(tokens.typography.heading_size)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.primary),
                        Text(
                            cx.view
                                .tr("product_browser.detail.facts")
                                .replace("{rating}", &format!("{:.1}", product.rating))
                                .replace("{stock}", &product.stock.to_string())
                                .replace("{category}", &product.category),
                        )
                        .size(tokens.typography.font_size_sm)
                        .color(tokens.colors.text_secondary)
                        .max_width(DETAIL_TEXT_WIDTH),
                        Text(product.description.clone())
                            .size(tokens.typography.body_medium_size)
                            .line_height(
                                tokens.typography.body_medium_size
                                    * tokens.typography.line_height_normal,
                            )
                            .color(tokens.colors.text_primary)
                            .max_width(DETAIL_TEXT_WIDTH),
                        Text(if product.tags.is_empty() {
                            cx.view.tr("product_browser.detail.no_tags")
                        } else {
                            cx.view
                                .tr("product_browser.detail.tags")
                                .replace("{tags}", &product.tags.join(", "))
                        })
                        .size(tokens.typography.font_size_sm)
                        .color(tokens.colors.text_secondary)
                        .max_width(DETAIL_TEXT_WIDTH),
                    ],
                )
            } else {
                Center(
                    Text(cx.view.tr("product_browser.detail.empty"))
                        .color(tokens.colors.text_secondary)
                        .max_width(DETAIL_EMPTY_TEXT_WIDTH),
                )
            },
        )
        .padding_all(tokens.spacing.l)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.xxl)
        .width_length(match component.layout {
            ProductDetailLayout::Compact => Length::percent(100.0),
            ProductDetailLayout::Expanded => Length::points(DETAIL_WIDTH),
        })
        .flex_shrink(if matches!(component.layout, ProductDetailLayout::Expanded) {
            0.0
        } else {
            1.0
        })
}
