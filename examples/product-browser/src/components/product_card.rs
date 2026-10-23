use crate::api::Product;
use crate::components::layout::{
    PRODUCT_CARD_BORDER_WIDTH, PRODUCT_CARD_HOVER_OPACITY, PRODUCT_CARD_PRESSED_SCALE,
    PRODUCT_CARD_SELECTED_BORDER_WIDTH, PRODUCT_CARD_TRANSITION_MS, PRODUCT_IMAGE_COMFORTABLE,
    PRODUCT_IMAGE_COMPACT,
};
use crate::model::{on_product_selected, ProductBrowserState, ProductSelected};
use fission::motion::MotionTransition;
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub enum ProductCardDensity {
    Compact,
    Comfortable,
}

fission_component! {
    pub ProductCard<ProductBrowserState> {
        product: Product,
        selected: bool,
        density: ProductCardDensity,
        instance: String,
    } |component, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let compact = matches!(component.density, ProductCardDensity::Compact);
        let image_size = if compact {
            PRODUCT_IMAGE_COMPACT
        } else {
            PRODUCT_IMAGE_COMFORTABLE
        };
        let identifier = format!(
            "product-browser.product.{}.{}",
            component.instance,
            component.product.id,
        );
    } =>
        Pressable(
            Container(
                if compact {
                    Row(
                        gap: tokens.spacing.m,
                        children: [
                            ProductImage(component.product.thumbnail.clone(), image_size),
                            ProductCardDetails(component.product.clone(), true),
                        ],
                    )
                } else {
                    Column(
                        gap: tokens.spacing.m,
                        children: [
                            ProductImage(component.product.thumbnail.clone(), image_size),
                            ProductCardDetails(component.product.clone(), false),
                        ],
                    )
                },
            )
            .bg(tokens.colors.surface)
            .border(
                if component.selected {
                    tokens.colors.primary
                } else {
                    tokens.colors.border
                },
                if component.selected {
                    PRODUCT_CARD_SELECTED_BORDER_WIDTH
                } else {
                    PRODUCT_CARD_BORDER_WIDTH
                },
            )
            .border_radius(tokens.radii.large)
            .padding_all(tokens.spacing.m),
        )
        .id(WidgetId::explicit(&identifier))
        .semantics_identifier(identifier)
        .label(format!("Select {}", component.product.title))
        .role(PressableRole::Button)
        .on_press(with_reducer!(
            cx.build,
            ProductSelected(component.product.id),
            on_product_selected,
        ))
        .hover(PressableStyle {
            opacity: Some(PRODUCT_CARD_HOVER_OPACITY),
            ..Default::default()
        })
        .pressed(PressableStyle {
            scale: Some(PRODUCT_CARD_PRESSED_SCALE),
            ..Default::default()
        })
        .transition(MotionTransition::ease_out(PRODUCT_CARD_TRANSITION_MS))
}

fission_component! {
    ProductImage<ProductBrowserState> { source: String, size: f32 }
    |image, _state, _cx| {} =>
        Image::network(image.source.clone())
            .size(image.size, image.size)
            .fit(ir_op::ImageFit::Contain)
}

fission_component! {
    ProductCardDetails<ProductBrowserState> { product: Product, compact: bool }
    |details, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: if details.compact {
                tokens.spacing.xs
            } else {
                tokens.spacing.s
            },
            children: [
                Text(details.product.title.clone())
                    .size(if details.compact {
                        tokens.typography.label_large_size
                    } else {
                        tokens.typography.body_large_size
                    })
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_primary)
                    .max_lines(2),
                Text(format!(
                    "{} · {:.1} stars",
                    details.product.category,
                    details.product.rating,
                ))
                .size(tokens.typography.body_medium_size)
                .color(tokens.colors.text_secondary)
                .max_lines(1),
                Text(format!("${:.2}", details.product.price))
                    .size(tokens.typography.body_large_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary),
                Text(format!("{} in stock", details.product.stock))
                    .size(tokens.typography.font_size_base)
                    .color(tokens.colors.text_secondary),
            ],
        )
}
