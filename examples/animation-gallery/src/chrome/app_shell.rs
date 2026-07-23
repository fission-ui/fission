use super::*;

pub struct AppShell<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<AppShell<'_>> for Widget {
    fn from(shell: AppShell<'_>) -> Self {
        Responsive::new(CompactShell {
            ctx: shell.ctx,
            state: shell.state,
        })
        .id(WidgetId::explicit("animation-gallery.app-shell.responsive"))
        .case(ResponsiveCase::min_width(
            APP_COMPACT_BREAKPOINT,
            DesktopShell {
                ctx: shell.ctx,
                state: shell.state,
            },
        ))
        .into()
    }
}
