use super::*;

pub(super) struct NavPanel<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<NavPanel<'_>> for Widget {
    fn from(panel: NavPanel<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let mut children = vec![Text::new("Fission")
            .size(tokens.typography.font_size_lg)
            .color(INK)
            .into()];
        for group in routes::NAV_GROUPS {
            children.push(
                NavGroupView {
                    ctx: panel.ctx,
                    state: panel.state,
                    group,
                }
                .into(),
            );
        }

        Container::new(Column {
            gap: Some(tokens.spacing.m),
            children,
            ..Default::default()
        })
        .width(NAV_PANEL_WIDTH)
        .padding_all(tokens.spacing.m)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
