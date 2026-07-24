use super::*;

pub(super) struct Dashboard<'a> {
    pub(super) ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub(super) state: &'a AnimationGalleryState,
}

impl From<Dashboard<'_>> for Widget {
    fn from(dashboard: Dashboard<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();

        if view.viewport_size().width >= DASHBOARD_COMPACT_BREAKPOINT {
            DashboardExpanded {
                ctx: dashboard.ctx,
                state: dashboard.state,
            }
            .into()
        } else {
            DashboardCompact {
                ctx: dashboard.ctx,
                state: dashboard.state,
            }
            .into()
        }
    }
}
