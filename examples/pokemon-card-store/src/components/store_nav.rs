use crate::app::StoreState;
use crate::components::feature_pill::FeaturePill;
use crate::components::nav_link::NavLink;
use fission::prelude::*;

pub struct StoreNav;

impl From<StoreNav> for Widget {
    fn from(_: StoreNav) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Row {
            gap: Some(tokens.spacing.m),
            wrap: ir_op::FlexWrap::Wrap,
            children: widgets![
                Text::new("Fission Card Market")
                    .size(typography.font_size_xl)
                    .line_height(typography.font_size_xl * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.text_primary)
                    .semantics_identifier("site-route:/"),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                NavLink {
                    label: "Catalogue",
                    href: "/",
                },
                FeaturePill {
                    label: "Session cart",
                    accent: tokens.colors.success,
                },
                FeaturePill {
                    label: "Worker filters",
                    accent: tokens.colors.primary,
                },
                FeaturePill {
                    label: "Cart island",
                    accent: tokens.colors.secondary,
                },
            ],
            align_items: ir_op::AlignItems::Center,
            ..Default::default()
        }
        .into()
    }
}
