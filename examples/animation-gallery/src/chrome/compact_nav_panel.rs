use super::*;

pub(super) struct CompactNavPanel<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<CompactNavPanel<'_>> for Widget {
    fn from(panel: CompactNavPanel<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let mut chips: Vec<Widget> = vec![NavChip {
            ctx: panel.ctx,
            state: panel.state,
            label: "Overview",
            path: routes::OVERVIEW,
        }
        .into()];
        for item in routes::WIDGET_ROUTES {
            chips.push(
                NavChip {
                    ctx: panel.ctx,
                    state: panel.state,
                    label: item.label,
                    path: item.path,
                }
                .into(),
            );
        }
        for (label, path) in [
            ("Opacity", "/properties/opacity"),
            ("Composition", "/composition/additive"),
            ("Policy", "/policy/full"),
            ("Diagnostics", "/diagnostics/declarations"),
        ] {
            chips.push(
                NavChip {
                    ctx: panel.ctx,
                    state: panel.state,
                    label,
                    path,
                }
                .into(),
            );
        }

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: vec![
                Text::new("Routes")
                    .size(tokens.typography.font_size_xs)
                    .color(MUTED)
                    .into(),
                Wrap {
                    direction: FlexDirection::Row,
                    spacing: Some(tokens.spacing.s),
                    children: chips,
                }
                .into(),
            ],
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .padding_all(tokens.spacing.m)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
