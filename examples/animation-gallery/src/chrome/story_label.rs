use super::*;

pub(super) struct StoryLabel<'a> {
    pub(super) step: &'a str,
    pub(super) title: &'a str,
    pub(super) caption: &'a str,
}

impl From<StoryLabel<'_>> for Widget {
    fn from(label: StoryLabel<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        Row {
            gap: Some(tokens.spacing.s),
            children: vec![
                Container::new(
                    Text::new(label.step)
                        .size(tokens.typography.font_size_xs)
                        .color(fission::op::Color::WHITE),
                )
                .width(STORY_STEP_SIZE)
                .height(STORY_STEP_SIZE)
                .padding_all(tokens.spacing.s)
                .border_radius(tokens.radii.full)
                .bg(BLUE)
                .into(),
                Column {
                    gap: Some(tokens.spacing.xs),
                    children: vec![
                        Text::new(label.title)
                            .size(tokens.typography.font_size_sm)
                            .color(BLUE)
                            .into(),
                        Text::new(label.caption)
                            .size(tokens.typography.font_size_xs)
                            .color(MUTED)
                            .into(),
                    ],
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
