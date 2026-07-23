use crate::api::Product;
use crate::components::layout::{
    PRODUCT_CARD_BORDER_WIDTH, PRODUCT_CARD_HOVER_OPACITY, PRODUCT_CARD_PRESSED_SCALE,
    PRODUCT_CARD_SELECTED_BORDER_WIDTH, PRODUCT_CARD_TRANSITION_MS, PRODUCT_IMAGE_COMFORTABLE,
    PRODUCT_IMAGE_COMPACT,
};
use crate::model::{on_product_selected, ProductBrowserState, ProductSelected};
use fission::motion::MotionTransition;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct ProductCard {
    pub product: Product,
    pub selected: bool,
    pub density: ProductCardDensity,
    pub instance: String,
}

#[derive(Clone, Debug)]
pub enum ProductCardDensity {
    Compact,
    Comfortable,
}

impl ProductCard {
    fn is_compact(&self) -> bool {
        matches!(self.density, ProductCardDensity::Compact)
    }
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

        let compact = component.is_compact();

        let image_size = if compact {
            PRODUCT_IMAGE_COMPACT
        } else {
            PRODUCT_IMAGE_COMFORTABLE
        };

        let image = Image::network(component.product.thumbnail.clone())
            .size(image_size, image_size)
            .fit(ir_op::ImageFit::Contain)
            .into();

        let details = Column {
            gap: Some(if compact {
                tokens.spacing.xs
            } else {
                tokens.spacing.s
            }),
            children: vec![
                Text::new(component.product.title.clone())
                    .size(if compact {
                        tokens.typography.label_large_size
                    } else {
                        tokens.typography.body_large_size
                    })
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_primary)
                    .max_lines(2)
                    .into(),
                Text::new(format!(
                    "{} · {:.1} stars",
                    component.product.category, component.product.rating
                ))
                .size(tokens.typography.body_medium_size)
                .color(tokens.colors.text_secondary)
                .max_lines(1)
                .into(),
                Text::new(format!("${:.2}", component.product.price))
                    .size(tokens.typography.body_large_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.primary)
                    .into(),
                Text::new(format!("{} in stock", component.product.stock))
                    .size(tokens.typography.font_size_base)
                    .color(tokens.colors.text_secondary)
                    .into(),
            ],
            ..Default::default()
        }
        .into();

        let content: Widget = if compact {
            Row {
                gap: Some(tokens.spacing.m),
                children: vec![image, details],
                ..Default::default()
            }
            .into()
        } else {
            Column {
                gap: Some(tokens.spacing.m),
                children: vec![image, details],
                ..Default::default()
            }
            .into()
        };

        let identifier = format!(
            "product-browser.product.{}.{}",
            component.instance, component.product.id
        );

        Pressable::new(
            Container::new(content)
                .bg(tokens.colors.surface)
                .border(
                    border,
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
        .on_press(select)
        .hover(PressableStyle {
            opacity: Some(PRODUCT_CARD_HOVER_OPACITY),
            ..Default::default()
        })
        .pressed(PressableStyle {
            scale: Some(PRODUCT_CARD_PRESSED_SCALE),
            ..Default::default()
        })
        .transition(MotionTransition::ease_out(PRODUCT_CARD_TRANSITION_MS))
        .into()
    }
}
