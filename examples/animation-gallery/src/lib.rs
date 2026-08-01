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
