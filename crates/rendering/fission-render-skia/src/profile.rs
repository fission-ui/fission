//! One explicitly shared Skia raster profile.
//!
//! Paragraph layout and paragraph paint must resolve through the same resource
//! registry. Keeping that ownership in this factory prevents a renderer from
//! silently constructing a second geometry or native paragraph at paint time.

use std::num::NonZeroUsize;
use std::sync::Arc;

use fission_render::backend::{BackendResult, GraphicsBackendSession, SurfaceMetrics};
use fission_skia_sys::ParagraphDrawData;

use crate::paragraph_draw_data::{ParagraphDrawDataBudget, ParagraphDrawDataRegistry};
use crate::paragraph_engine::NativeFontCatalog;
use crate::raster_session::SkiaRasterSession;
use crate::{SkiaGaneshDriver, SkiaParagraphEngine, SkiaRasterDriver};

const DEFAULT_PARAGRAPH_DRAW_DATA_ENTRIES: usize = 4_096;
const DEFAULT_PARAGRAPH_DRAW_DATA_BYTES: usize = 64 * 1024 * 1024;

pub(crate) type SkiaParagraphDrawDataRegistry = ParagraphDrawDataRegistry<ParagraphDrawData>;

#[derive(Clone)]
struct ProfileFonts {
    native: NativeFontCatalog,
}

/// Invalid Fission-owned font catalogue supplied to a native Skia profile.
#[derive(Debug)]
pub enum SkiaFontProfileError {
    EmptyDefaultFamily,
    MissingDefaultFamily { family: String },
    Native(fission_skia_sys::Error),
}

impl std::fmt::Display for SkiaFontProfileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDefaultFamily => {
                formatter.write_str("a native Skia profile requires a nonempty default family")
            }
            Self::MissingDefaultFamily { family } => write!(
                formatter,
                "native Skia default family {family:?} has no packaged font face"
            ),
            Self::Native(error) => write!(formatter, "native Skia font catalogue failed: {error}"),
        }
    }
}

impl std::error::Error for SkiaFontProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Native(error) => Some(error),
            _ => None,
        }
    }
}

impl From<fission_skia_sys::Error> for SkiaFontProfileError {
    fn from(error: fission_skia_sys::Error) -> Self {
        Self::Native(error)
    }
}

fn profile_fonts(
    default_family: impl Into<String>,
    faces: Vec<fission_skia_sys::ParagraphFontFace>,
) -> Result<ProfileFonts, SkiaFontProfileError> {
    let default_family = default_family.into();
    if default_family.is_empty() || default_family.trim() != default_family {
        return Err(SkiaFontProfileError::EmptyDefaultFamily);
    }
    if !faces
        .iter()
        .any(|face| face.family.eq_ignore_ascii_case(&default_family))
    {
        return Err(SkiaFontProfileError::MissingDefaultFamily {
            family: default_family,
        });
    }
    let catalog = Arc::new(fission_skia_sys::ParagraphFontCatalog::new(&faces)?);
    Ok(ProfileFonts {
        native: NativeFontCatalog::new(catalog, Arc::from(default_family)),
    })
}

/// Factory for a Skia paragraph engine and raster renderer that share native
/// paint resources.
///
/// Clone the profile when layout and presentation are installed in different
/// shell components. Every clone retains the same registry; no global registry
/// or paint-time paragraph layout is involved.
#[derive(Clone)]
pub struct SkiaRasterProfile {
    paragraph_draw_data: Arc<SkiaParagraphDrawDataRegistry>,
    fonts: Option<ProfileFonts>,
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
            fonts: None,
        }
    }

    /// Creates a paired raster profile backed by Fission-owned packaged fonts.
    pub fn try_with_fonts(
        default_family: impl Into<String>,
        faces: Vec<fission_skia_sys::ParagraphFontFace>,
    ) -> Result<Self, SkiaFontProfileError> {
        Ok(Self {
            paragraph_draw_data: new_paragraph_draw_data_registry(),
            fonts: Some(profile_fonts(default_family, faces)?),
        })
    }

    /// Creates the paragraph engine paired with this profile's renderer.
    pub fn paragraph_engine(&self) -> SkiaParagraphEngine {
        match self.fonts.as_ref() {
            Some(fonts) => SkiaParagraphEngine::with_font_catalog(
                Arc::clone(&self.paragraph_draw_data),
                fonts.native.clone(),
            ),
            None => {
                SkiaParagraphEngine::with_draw_data_registry(Arc::clone(&self.paragraph_draw_data))
            }
        }
    }

    /// Creates a raster driver paired with this profile's paragraph engine.
    pub fn raster_driver(&self) -> BackendResult<SkiaRasterDriver> {
        SkiaRasterDriver::with_draw_data_registry(Arc::clone(&self.paragraph_draw_data))
    }

    /// Creates a graphics session paired with this profile's paragraph engine.
    pub fn create_session(&self) -> BackendResult<GraphicsBackendSession<'static>> {
        GraphicsBackendSession::new(self.raster_driver()?)
    }

    /// Creates and attaches the production headless raster adapter.
    ///
    /// The returned session owns its headless target, Skia runtime, raster
    /// surface, derived-resource caches, and readback normalization. Platform
    /// hosts retain only their own upload or presentation resources and drive
    /// this session through Fission lifecycle operations.
    pub fn create_headless_session(
        &self,
        metrics: SurfaceMetrics,
    ) -> BackendResult<SkiaRasterSession> {
        SkiaRasterSession::attach(self.raster_driver()?, metrics)
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
    fonts: Option<ProfileFonts>,
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
            fonts: None,
        }
    }

    /// Creates a paired Ganesh profile backed by Fission-owned packaged fonts.
    pub fn try_with_fonts(
        default_family: impl Into<String>,
        faces: Vec<fission_skia_sys::ParagraphFontFace>,
    ) -> Result<Self, SkiaFontProfileError> {
        Ok(Self {
            paragraph_draw_data: new_paragraph_draw_data_registry(),
            fonts: Some(profile_fonts(default_family, faces)?),
        })
    }

    /// Creates the paragraph engine paired with this profile's renderer.
    pub fn paragraph_engine(&self) -> SkiaParagraphEngine {
        match self.fonts.as_ref() {
            Some(fonts) => SkiaParagraphEngine::with_font_catalog(
                Arc::clone(&self.paragraph_draw_data),
                fonts.native.clone(),
            ),
            None => {
                SkiaParagraphEngine::with_draw_data_registry(Arc::clone(&self.paragraph_draw_data))
            }
        }
    }

    /// Creates a Ganesh driver paired with this profile's paragraph engine.
    ///
    /// This fails before attachment when the linked bridge does not advertise
    /// Ganesh, native presentation, and the target platform's Vulkan, Metal,
    /// or D3D12 backend.
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

    #[test]
    fn packaged_profile_requires_its_declared_default_family() {
        let error = match SkiaRasterProfile::try_with_fonts(
            "Fission Default",
            vec![fission_skia_sys::ParagraphFontFace::new(
                "Application Sans",
                vec![1],
            )],
        ) {
            Ok(_) => panic!("missing default family must fail before native decode"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            SkiaFontProfileError::MissingDefaultFamily { ref family }
                if family == "Fission Default"
        ));
    }

    #[cfg(feature = "test-shim")]
    #[test]
    fn packaged_profile_clones_share_one_native_font_generation() {
        let profile = SkiaRasterProfile::try_with_fonts(
            "Fission Default",
            vec![fission_skia_sys::ParagraphFontFace::new(
                "Fission Default",
                vec![1, 2, 3],
            )],
        )
        .expect("test-shim font catalogue");
        let clone = profile.clone();

        assert_eq!(
            profile.fonts.as_ref().unwrap().native.generation(),
            clone.fonts.as_ref().unwrap().native.generation()
        );
    }
}
