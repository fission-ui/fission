#![doc = include_str!("../README.md")]

mod api;
mod capabilities;
mod compiler;
mod driver;
mod error;
mod native;
mod thread_owner;

pub use capabilities::skia_raster_capabilities;
pub use driver::SkiaRasterDriver;

use fission_render::backend::{BackendResult, GraphicsBackendSession};

/// Construct a Fission-owned graphics session backed by Skia raster.
///
/// Backend-native objects remain private to this crate. Callers submit only
/// Fission [`fission_render::frame::InteractiveFrame`] values through the
/// returned session.
pub fn create_raster_session() -> BackendResult<GraphicsBackendSession<'static>> {
    GraphicsBackendSession::new(SkiaRasterDriver::new()?)
}
