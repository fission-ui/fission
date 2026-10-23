use crate::app::StoreState;
use crate::components::feature_pill::FeaturePill;
use crate::components::nav_link::NavLink;
use fission::prelude::*;

fission_component! {
    pub StoreNav<StoreState> {}
    |_nav, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Row(
            gap: tokens.spacing.m,
            wrap: ir_op::FlexWrap::Wrap,
            children: [
                Text::new("Fission Card Market")
                    .size(typography.font_size_xl)
                    .line_height(typography.font_size_xl * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.text_primary)
                    .semantics_identifier("site-route:/"),
                Spacer(flex_grow: 1.0),
                NavLink("Catalogue", "/"),
                FeaturePill("Session cart", tokens.colors.success),
                FeaturePill("Worker filters", tokens.colors.primary),
                FeaturePill("Cart island", tokens.colors.secondary),
            ],
            align_items: ir_op::AlignItems::Center,
        )
}
