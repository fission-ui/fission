use crate::app::StoreState;
use crate::components::hero_metric::HeroMetric;
use crate::components::palette::{TEXT_BODY, TEXT_PRIMARY};
use fission::prelude::*;

pub struct HeroCopy;

impl From<HeroCopy> for Widget {
    fn from(_: HeroCopy) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                Text::new("Server-rendered collector commerce")
                    .size(typography.display_sm_size)
                    .line_height(
                        typography.display_sm_size * typography.line_height_display
                    )
                    .weight(typography.font_weight_bold)
                    .color(TEXT_PRIMARY),
                Text::new("A Fission web store selling Pokemon cards with server-rendered product pages, route-local enhancement workers, and a session-backed cart.")
                    .size(typography.body_large_size)
                    .line_height(
                        typography.body_large_size * typography.line_height_normal
                    )
                    .color(TEXT_BODY),
                Row {
                    gap: Some(tokens.spacing.s),
                    wrap: ir_op::FlexWrap::Wrap,
                    children: widgets![
                        HeroMetric {
                            value: "6",
                            label: "cards",
                        },
                        HeroMetric {
                            value: "1",
                            label: "session cart",
                        },
                        HeroMetric {
                            value: "2",
                            label: "browser artifacts",
                        },
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .flex_grow(1.0)
        .into()
    }
}
