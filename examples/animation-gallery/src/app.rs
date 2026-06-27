use crate::chrome;
use crate::state::AnimationGalleryState;
use crate::style::BACKGROUND;
use fission::{build, Container, DesktopApp, Widget};

#[derive(Clone)]
pub struct AnimationGalleryApp;

impl From<AnimationGalleryApp> for Widget {
    fn from(_component: AnimationGalleryApp) -> Self {
        let (ctx, view) = build::current::<AnimationGalleryState>();
        let state = view.state().clone();
        let viewport = view.viewport_size();
        Container::new(chrome::AppShell {
            ctx: &ctx,
            state: &state,
            viewport_width: viewport.width,
        })
        .bg(BACKGROUND)
        .into()
    }
}

pub fn run() -> anyhow::Result<()> {
    DesktopApp::<AnimationGalleryState, _>::new(AnimationGalleryApp)
        .with_title("Fission Animation Gallery")
        .with_route_handler(crate::state::on_shell_route_changed)
        .run()
}
