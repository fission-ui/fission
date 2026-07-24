use crate::chrome;
use crate::state::AnimationGalleryState;
use crate::style::BACKGROUND;
#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
use fission::DesktopApp;
use fission::{build, Container, Widget};

#[derive(Clone)]
pub struct AnimationGalleryApp;

impl From<AnimationGalleryApp> for Widget {
    fn from(_component: AnimationGalleryApp) -> Self {
        let (ctx, view) = build::current::<AnimationGalleryState>();
        let state = view.state().clone();
        Container::new(chrome::AppShell {
            ctx: &ctx,
            state: &state,
        })
        .bg(BACKGROUND)
        .into()
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run() -> anyhow::Result<()> {
    DesktopApp::<AnimationGalleryState, _>::new(AnimationGalleryApp)
        .with_title("Fission Animation Gallery")
        .with_route_handler(crate::state::on_shell_route_changed)
        .run()
}
