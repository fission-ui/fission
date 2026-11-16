mod app;
mod chrome;
mod layout;
mod pages;
mod routes;
mod state;
mod style;
mod ui;
mod widgets;

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub use app::run;
pub use app::AnimationGalleryApp;
pub use state::AnimationGalleryState;

/// The gallery as another app mounts it, such as the example showcase. The host
/// already names the example, so the gallery drops its brand and hero.
pub fn embedded_state() -> AnimationGalleryState {
    AnimationGalleryState {
        embedded: true,
        ..AnimationGalleryState::default()
    }
}
