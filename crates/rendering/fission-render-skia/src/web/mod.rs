//! CanvasKit lowering from the same retained Fission scene authority used by
//! native Skia.

mod compiler;
mod convert;
mod driver;
mod error;
mod host;
mod profile;
mod resources;

#[cfg(test)]
pub(crate) use compiler::compile_web_scene;
pub(crate) use compiler::compile_web_scene_with_resources;
pub use driver::{CanvasKitBackendPreference, CanvasKitDriver};
pub(crate) use error::WebCompileError;
pub use fission_skia_sys::web::ResourceHandle;
pub use host::{CanvasKitHost, CanvasKitParagraphHost, CanvasKitPixelRegion, CanvasKitReadback};
pub use profile::{CanvasKitFont, CanvasKitProfile, CanvasKitProfileError};

#[cfg(test)]
mod driver_tests;

#[cfg(test)]
mod tests;
