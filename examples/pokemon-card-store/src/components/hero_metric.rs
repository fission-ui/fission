use crate::app::StoreState;
use crate::components::palette::{BORDER, SURFACE, TEXT_MUTED, TEXT_PRIMARY};
use fission::prelude::*;

pub struct HeroMetric {
    pub value: &'static str,
    pub label: &'static str,
}

impl From<HeroMetric> for Widget {
    fn from(metric: HeroMetric) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new(metric.value)
                    .size(typography.font_size_xl)
                    .line_height(typography.font_size_xl * typography.line_height_heading)
                    .weight(typography.font_weight_bold)
                    .color(TEXT_PRIMARY),
                Text::new(metric.label)
                    .size(typography.font_size_xs)
                    .line_height(typography.font_size_xs * typography.line_height_snug)
                    .color(TEXT_MUTED),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
