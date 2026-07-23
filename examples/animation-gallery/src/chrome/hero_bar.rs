use super::*;

pub(super) struct HeroBar<'a> {
    pub(super) state: &'a AnimationGalleryState,
}

impl From<HeroBar<'_>> for Widget {
    fn from(bar: HeroBar<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let title = if bar.state.current_path == routes::OVERVIEW {
            "Motion Workbench"
        } else {
            "Fission Motion Workbench"
        };
        Container::new(Row {
            gap: Some(tokens.spacing.m),
            children: vec![
                Column {
                    gap: Some(tokens.spacing.xs),
                    children: vec![
                        Text::new(title)
                            .size(typography.heading_size)
                            .color(INK)
                            .into(),
                        Text::new("Preview real widgets, compose motion atoms, inspect lowered tracks, and validate deterministic behavior.")
                            .size(typography.font_size_sm)
                            .color(MUTED)
                            .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                Container::new(
                    Text::new("explicit / deterministic / testable")
                        .size(typography.font_size_xs)
                        .color(BLUE),
                )
                    .width(HERO_BADGE_WIDTH)
                    .padding_all(tokens.spacing.m)
                    .border(BORDER, 1.0)
                    .border_radius(tokens.radii.full)
                    .bg(SURFACE)
                    .into(),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.xxl)
        .border(BORDER, 1.0)
        .bg(color(247, 250, 255, 255))
        .into()
    }
}
