use super::*;

pub(super) struct CompactBrand;

impl From<CompactBrand> for Widget {
    fn from(_: CompactBrand) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: vec![
                Row {
                    gap: Some(tokens.spacing.m),
                    children: vec![
                        Container::new(
                            Text::new("F")
                                .size(typography.font_size_xl)
                                .color(fission::op::Color::WHITE),
                        )
                        .width(COMPACT_BRAND_MARK_SIZE)
                        .height(COMPACT_BRAND_MARK_SIZE)
                        .padding_all(tokens.spacing.m)
                        .border_radius(tokens.radii.large)
                        .bg(color(6, 17, 57, 255))
                        .into(),
                        Column {
                            gap: Some(tokens.spacing.xs),
                            children: vec![
                                Text::new("Fission")
                                    .size(typography.font_size_xl)
                                    .color(INK)
                                    .into(),
                                Text::new("Animation Gallery")
                                    .size(typography.font_size_lg)
                                    .color(BLUE)
                                    .into(),
                            ],
                            ..Default::default()
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                Text::new("Calm, explicit, deterministic motion for Fission apps.")
                    .size(typography.font_size_sm)
                    .color(MUTED)
                    .into(),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
