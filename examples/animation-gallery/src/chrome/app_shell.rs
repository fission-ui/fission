use super::*;

fission_component! {
    pub AppShell<AnimationGalleryState> |_shell, _state, cx| {} =>
        if cx.view.viewport_size().width >= APP_COMPACT_BREAKPOINT {
            DesktopShell()
        } else {
            CompactShell()
        }
}
