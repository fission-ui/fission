use crate::app::StoreState;
use crate::components::hero_metric::HeroMetric;
use fission::prelude::*;

fission_component! {
    pub HeroCopy<StoreState> {}
    |_copy, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Container(Column(
            gap: tokens.spacing.m,
            children: [
                Text::new("Server-rendered collector commerce")
                    .size(typography.display_sm_size)
                    .line_height(
                        typography.display_sm_size * typography.line_height_display
                    )
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                Text::new("A Fission web store selling Pokemon cards with server-rendered product pages, route-local enhancement workers, and a session-backed cart.")
                    .size(typography.body_large_size)
                    .line_height(
                        typography.body_large_size * typography.line_height_normal
                    )
                    .color(tokens.colors.text_secondary),
                Row(
                    gap: tokens.spacing.s,
                    wrap: ir_op::FlexWrap::Wrap,
                    children: [
                        HeroMetric("6", "cards"),
                        HeroMetric("1", "session cart"),
                        HeroMetric("2", "browser artifacts"),
                    ],
                ),
            ],
        ))
        .flex_grow(1.0)
}
