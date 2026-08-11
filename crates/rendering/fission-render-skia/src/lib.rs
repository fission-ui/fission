#![doc = include_str!("../README.md")]

mod api;
mod capabilities;
mod compiler;
mod driver;
mod error;
mod ganesh_api;
mod ganesh_driver;
mod ganesh_native;
mod image;
mod native;
mod paragraph_caret;
mod paragraph_draw_data;
mod paragraph_engine;
mod picture;
mod profile;
mod svg;
mod thread_owner;
mod web;

pub use capabilities::{skia_ganesh_capabilities, skia_raster_capabilities};
pub use driver::SkiaRasterDriver;
pub use ganesh_driver::SkiaGaneshDriver;
pub use paragraph_engine::SkiaParagraphEngine;
pub use profile::{SkiaGaneshProfile, SkiaRasterProfile};

use fission_render::backend::{BackendResult, GraphicsBackendSession};

/// Construct a Fission-owned graphics session backed by Skia raster.
///
/// Backend-native objects remain private to this crate. Callers submit only
/// Fission [`fission_render::frame::InteractiveFrame`] values through the
/// returned session.
pub fn create_raster_session() -> BackendResult<GraphicsBackendSession<'static>> {
    GraphicsBackendSession::new(SkiaRasterDriver::new()?)
}
