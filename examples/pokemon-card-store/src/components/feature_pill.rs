use crate::app::StoreState;
use crate::components::palette::TEXT_BODY;
use fission::prelude::*;

pub struct FeaturePill {
    pub label: &'static str,
    pub accent: Color,
}

impl From<FeaturePill> for Widget {
    fn from(pill: FeaturePill) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(
            Text::new(pill.label)
                .size(typography.font_size_xs)
                .line_height(typography.font_size_xs * typography.line_height_snug)
                .weight(typography.font_weight_semibold)
                .color(TEXT_BODY),
        )
        .padding([
            tokens.spacing.s,
            tokens.spacing.s,
            tokens.spacing.xs,
            tokens.spacing.xs,
        ])
        .border(pill.accent.with_alpha(160), 1.0)
        .border_radius(tokens.radii.full)
        .bg(pill.accent.with_alpha(34))
        .into()
    }
}
