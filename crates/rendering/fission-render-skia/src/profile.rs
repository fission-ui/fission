//! One explicitly shared Skia raster profile.
//!
//! Paragraph layout and paragraph paint must resolve through the same resource
//! registry. Keeping that ownership in this factory prevents a renderer from
//! silently constructing a second geometry or native paragraph at paint time.

use std::num::NonZeroUsize;
use std::sync::Arc;

use fission_render::backend::{BackendResult, GraphicsBackendSession};
use fission_skia_sys::ParagraphDrawData;

use crate::paragraph_draw_data::{ParagraphDrawDataBudget, ParagraphDrawDataRegistry};
use crate::{SkiaGaneshDriver, SkiaParagraphEngine, SkiaRasterDriver};

const DEFAULT_PARAGRAPH_DRAW_DATA_ENTRIES: usize = 4_096;
const DEFAULT_PARAGRAPH_DRAW_DATA_BYTES: usize = 64 * 1024 * 1024;

pub(crate) type SkiaParagraphDrawDataRegistry = ParagraphDrawDataRegistry<ParagraphDrawData>;

/// Factory for a Skia paragraph engine and raster renderer that share native
/// paint resources.
///
/// Clone the profile when layout and presentation are installed in different
/// shell components. Every clone retains the same registry; no global registry
/// or paint-time paragraph layout is involved.
#[derive(Clone)]
pub struct SkiaRasterProfile {
    paragraph_draw_data: Arc<SkiaParagraphDrawDataRegistry>,
}

impl Default for SkiaRasterProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl SkiaRasterProfile {
    pub fn new() -> Self {
        Self {
            paragraph_draw_data: new_paragraph_draw_data_registry(),
        }
    }

    /// Creates the paragraph engine paired with this profile's renderer.
    pub fn paragraph_engine(&self) -> SkiaParagraphEngine {
        SkiaParagraphEngine::with_draw_data_registry(Arc::clone(&self.paragraph_draw_data))
    }

    /// Creates a raster driver paired with this profile's paragraph engine.
    pub fn raster_driver(&self) -> BackendResult<SkiaRasterDriver> {
        SkiaRasterDriver::with_draw_data_registry(Arc::clone(&self.paragraph_draw_data))
    }

    /// Creates a graphics session paired with this profile's paragraph engine.
    pub fn create_session(&self) -> BackendResult<GraphicsBackendSession<'static>> {
        GraphicsBackendSession::new(self.raster_driver()?)
    }

    pub(crate) fn paragraph_draw_data(&self) -> Arc<SkiaParagraphDrawDataRegistry> {
        Arc::clone(&self.paragraph_draw_data)
    }
}

/// Factory for a paired SkParagraph engine and native Ganesh renderer.
///
/// The platform host retains ownership of every native display and window
/// handle supplied to the returned session for the complete attachment
/// lifetime. Clones share the exact paragraph paint-data registry.
#[derive(Clone)]
pub struct SkiaGaneshProfile {
    paragraph_draw_data: Arc<SkiaParagraphDrawDataRegistry>,
}

impl Default for SkiaGaneshProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl SkiaGaneshProfile {
    pub fn new() -> Self {
        Self {
            paragraph_draw_data: new_paragraph_draw_data_registry(),
        }
    }

    /// Creates the paragraph engine paired with this profile's renderer.
    pub fn paragraph_engine(&self) -> SkiaParagraphEngine {
        SkiaParagraphEngine::with_draw_data_registry(Arc::clone(&self.paragraph_draw_data))
    }

    /// Creates a Ganesh driver paired with this profile's paragraph engine.
    ///
    /// This fails before attachment when the linked bridge does not advertise
    /// Ganesh, Vulkan, and native-presentation support.
    pub fn ganesh_driver(&self) -> BackendResult<SkiaGaneshDriver> {
        SkiaGaneshDriver::with_draw_data_registry(Arc::clone(&self.paragraph_draw_data))
    }

    /// Creates a native Ganesh session paired with this profile's paragraph
    /// engine.
    pub fn create_session(&self) -> BackendResult<GraphicsBackendSession<'static>> {
        GraphicsBackendSession::new(self.ganesh_driver()?)
    }

    pub(crate) fn paragraph_draw_data(&self) -> Arc<SkiaParagraphDrawDataRegistry> {
        Arc::clone(&self.paragraph_draw_data)
    }
}

pub(crate) fn new_paragraph_draw_data_registry() -> Arc<SkiaParagraphDrawDataRegistry> {
    Arc::new(ParagraphDrawDataRegistry::new(
        ParagraphDrawDataBudget::new(
            NonZeroUsize::new(DEFAULT_PARAGRAPH_DRAW_DATA_ENTRIES)
                .expect("the default paragraph entry budget is nonzero"),
            NonZeroUsize::new(DEFAULT_PARAGRAPH_DRAW_DATA_BYTES)
                .expect("the default paragraph byte budget is nonzero"),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_one_paragraph_registry() {
        let profile = SkiaRasterProfile::new();
        let clone = profile.clone();

        assert!(Arc::ptr_eq(
            &profile.paragraph_draw_data(),
            &clone.paragraph_draw_data()
        ));
    }

    #[test]
    fn ganesh_clones_share_one_paragraph_registry() {
        let profile = SkiaGaneshProfile::new();
        let clone = profile.clone();

        assert!(Arc::ptr_eq(
            &profile.paragraph_draw_data(),
            &clone.paragraph_draw_data()
        ));
    }
}
