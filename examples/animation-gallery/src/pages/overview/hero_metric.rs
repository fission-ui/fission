use crate::state::AnimationGalleryState;
use crate::style::{BLUE, BORDER, MUTED, SURFACE};
use fission::prelude::*;

pub(super) struct HeroMetric<'a> {
    pub value: &'a str,
    pub label: &'a str,
}

impl From<HeroMetric<'_>> for Widget {
    fn from(metric: HeroMetric<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new(metric.value)
                    .size(typography.font_size_xl)
                    .color(BLUE),
                Text::new(metric.label)
                    .size(typography.font_size_sm)
                    .color(MUTED),
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
