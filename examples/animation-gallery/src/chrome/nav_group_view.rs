use super::*;

pub(super) struct NavGroupView<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
    pub(super) group: &'a NavGroup,
}

impl From<NavGroupView<'_>> for Widget {
    fn from(group: NavGroupView<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let mut children = vec![Text::new(group.group.title)
            .size(tokens.typography.font_size_xs)
            .color(MUTED)
            .into()];
        for item in group.group.items {
            children.push(
                NavButton {
                    ctx: group.ctx,
                    state: group.state,
                    item,
                }
                .into(),
            );
        }
        Column {
            gap: Some(tokens.spacing.xs),
            children,
            ..Default::default()
        }
        .into()
    }
}
