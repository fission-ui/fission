use super::*;

pub(super) struct DashboardExpanded<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<DashboardExpanded<'_>> for Widget {
    fn from(dashboard: DashboardExpanded<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Row {
            gap: Some(tokens.spacing.m),
            align_items: fission::op::AlignItems::Start,
            children: vec![
                NavPanel {
                    ctx: dashboard.ctx,
                    state: dashboard.state,
                }
                .into(),
                Container::new(pages::GalleryRouter {
                    current_path: dashboard.state.current_path.clone(),
                })
                .min_width(MIN_ROUTED_CONTENT_WIDTH)
                .flex_grow(1.0)
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
