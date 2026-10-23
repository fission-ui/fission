use super::*;

fission_component! {
    pub(super) Dashboard<AnimationGalleryState> |_dashboard, _state, cx| {} =>
        if cx.view.viewport_size().width >= DASHBOARD_COMPACT_BREAKPOINT {
            DashboardExpanded()
        } else {
            DashboardCompact()
        }
}
