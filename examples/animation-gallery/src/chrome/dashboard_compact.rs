use super::*;

pub(super) struct DashboardCompact<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<DashboardCompact<'_>> for Widget {
    fn from(dashboard: DashboardCompact<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Column {
            gap: Some(tokens.spacing.m),
            children: vec![
                CompactNavPanel {
                    ctx: dashboard.ctx,
                    state: dashboard.state,
                }
                .into(),
                Container::new(pages::GalleryRouter {
                    current_path: dashboard.state.current_path.clone(),
                })
                .width_length(Length::percent(100.0))
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
