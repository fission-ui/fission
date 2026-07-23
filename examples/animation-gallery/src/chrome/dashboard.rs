use super::*;

pub(super) struct Dashboard<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<Dashboard<'_>> for Widget {
    fn from(dashboard: Dashboard<'_>) -> Self {
        Responsive::new(DashboardCompact {
            ctx: dashboard.ctx,
            state: dashboard.state,
        })
        .id(WidgetId::explicit("animation-gallery.dashboard.responsive"))
        .case(ResponsiveCase::min_width(
            DASHBOARD_COMPACT_BREAKPOINT,
            DashboardExpanded {
                ctx: dashboard.ctx,
                state: dashboard.state,
            },
        ))
        .into()
    }
}
