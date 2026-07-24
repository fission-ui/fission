use super::*;

pub struct AppShell<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<AppShell<'_>> for Widget {
    fn from(shell: AppShell<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();

        if view.viewport_size().width >= APP_COMPACT_BREAKPOINT {
            DesktopShell {
                ctx: shell.ctx,
                state: shell.state,
            }
            .into()
        } else {
            CompactShell {
                ctx: shell.ctx,
                state: shell.state,
            }
            .into()
        }
    }
}
