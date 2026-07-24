use super::*;

pub(super) struct PrincipleCard<'a> {
    pub(super) title: &'a str,
    pub(super) body: &'a str,
    pub(super) mark: &'a str,
}

impl From<PrincipleCard<'_>> for Widget {
    fn from(card: PrincipleCard<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        Row {
            gap: Some(tokens.spacing.m),
            children: vec![
                Container::new(
                    Text::new(card.mark)
                        .size(tokens.typography.font_size_lg)
                        .color(BLUE),
                )
                .width(PRINCIPLE_MARK_SIZE)
                .height(PRINCIPLE_MARK_SIZE)
                .padding_all(tokens.spacing.m)
                .border_radius(tokens.radii.large)
                .border(color(199, 209, 255, 255), 1.0)
                .bg(SURFACE)
                .into(),
                Column {
                    gap: Some(tokens.spacing.xs),
                    children: vec![
                        Text::new(card.title)
                            .size(tokens.typography.font_size_sm)
                            .color(INK)
                            .into(),
                        Text::new(card.body)
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
