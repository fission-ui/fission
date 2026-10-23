use crate::chrome::AppShell;
use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;
#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
use fission::DesktopApp;

fission_component! {
    pub AnimationGalleryApp<AnimationGalleryState>
    |_app, _state, _cx| =>
    Container(AppShell())
    .bg(style::background())
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run() -> anyhow::Result<()> {
    DesktopApp::<AnimationGalleryState, _>::new(AnimationGalleryApp)
        .with_title("Fission Animation Gallery")
        .with_route_handler(crate::state::on_shell_route_changed)
        .run()
}
