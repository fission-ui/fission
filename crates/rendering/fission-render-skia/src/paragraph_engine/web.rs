//! CanvasKit SkParagraph adapter paired with the browser paint profile.

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

#[cfg(test)]
use fission_layout::ParagraphCapability;
use fission_layout::{
    ParagraphCapabilities, ParagraphDescription, ParagraphEngine, ParagraphError, ParagraphResult,
};
use fission_skia_sys::web::{
    decode_paragraph_response, encode_paragraph_request, ResourceHandle, WebParagraphFont,
    WebParagraphRequest,
};

use super::cache_key::paragraph_cache_key;
use super::native::{map_capabilities, native_request, packed_output};
use super::request::PackedParagraphRequest;
use crate::paragraph_draw_data::{ParagraphDrawDataBudget, ParagraphDrawDataRegistry};

const DEFAULT_PARAGRAPH_DRAW_DATA_ENTRIES: usize = 4_096;
const DEFAULT_PARAGRAPH_DRAW_DATA_BYTES: usize = 64 * 1024 * 1024;
const COMPLETE_CAPABILITY_BITS: u64 = fission_skia_sys::ParagraphCapabilities::BIDIRECTIONAL_TEXT
    | fission_skia_sys::ParagraphCapabilities::VARIABLE_FONTS
    | fission_skia_sys::ParagraphCapabilities::FONT_FEATURES
    | fission_skia_sys::ParagraphCapabilities::INLINE_OBJECTS
    | fission_skia_sys::ParagraphCapabilities::CLUSTER_MAPPING
    | fission_skia_sys::ParagraphCapabilities::HIT_TESTING
    | fission_skia_sys::ParagraphCapabilities::CARET_GEOMETRY
    | fission_skia_sys::ParagraphCapabilities::SELECTION_GEOMETRY
    | fission_skia_sys::ParagraphCapabilities::UNRESOLVED_GLYPHS;

/// Object-safe browser boundary used by paragraph layout and retained handle
/// destruction. Implementations own the JavaScript executor and copy every
/// packet across the Wasm boundary.
pub(crate) trait CanvasKitParagraphBridge: Send + Sync {
    fn layout_paragraph(&self, request: Vec<u8>) -> Result<Vec<u8>, String>;
    fn destroy_paragraph(&self, handle: ResourceHandle) -> Result<(), String>;
}

#[derive(Debug, Clone)]
struct InstalledFontCatalog {
    fonts: Vec<WebParagraphFont>,
}

/// Profile-local font authority. The immutable family order and generation
/// are known before layout; handles become available only after the browser
/// acknowledges the matching resource batch.
pub(crate) struct CanvasKitFontState {
    generation: u64,
    default_family: Box<str>,
    fallback_families: Box<[Box<str>]>,
    installed: Mutex<Option<InstalledFontCatalog>>,
}

impl CanvasKitFontState {
    pub(crate) fn new(
        generation: u64,
        default_family: String,
        fallback_families: Vec<String>,
    ) -> Self {
        debug_assert_ne!(generation, 0);
        Self {
            generation,
            default_family: default_family.into_boxed_str(),
            fallback_families: fallback_families
                .into_iter()
                .map(String::into_boxed_str)
                .collect(),
            installed: Mutex::new(None),
        }
    }

    pub(crate) fn install(&self, fonts: Vec<WebParagraphFont>) {
        *self.installed.lock().unwrap() = Some(InstalledFontCatalog { fonts });
    }

    pub(crate) fn clear(&self) {
        *self.installed.lock().unwrap() = None;
    }

    fn snapshot(
        &self,
        request: &PackedParagraphRequest,
    ) -> Result<InstalledFontCatalog, ParagraphError> {
        let installed = self.installed.lock().unwrap();
        let installed = installed.as_ref().ok_or_else(|| {
            ParagraphError::backend(
                "skia-canvaskit-skparagraph",
                "the CanvasKit font batch has not been acknowledged for the active session",
            )
        })?;
        let mut families = Vec::<&str>::new();
        for family in request
            .style_runs
            .iter()
            .filter_map(|run| run.font_family.as_deref())
            .chain(request.fallback_families.iter().map(Box::as_ref))
            .chain(std::iter::once(self.default_family.as_ref()))
        {
            if !families
                .iter()
                .any(|current| current.eq_ignore_ascii_case(family))
            {
                families.push(family);
            }
        }
        let fonts = installed
            .fonts
            .iter()
            .filter(|font| {
                families
                    .iter()
                    .any(|family| family.eq_ignore_ascii_case(&font.family))
            })
            .cloned()
            .collect::<Vec<_>>();
        if fonts.is_empty() && !request.text.is_empty() {
            return Err(ParagraphError::backend(
                "skia-canvaskit-skparagraph",
                "the active CanvasKit font catalog contains no requested or default family",
            ));
        }
        Ok(InstalledFontCatalog { fonts })
    }

    fn apply_to(&self, request: &mut PackedParagraphRequest) {
        request.font_catalog_generation = self.generation;
        let mut fallback_families = request.fallback_families.to_vec();
        for family in std::iter::once(&self.default_family).chain(&self.fallback_families) {
            if !fallback_families
                .iter()
                .any(|current| current.eq_ignore_ascii_case(family))
            {
                fallback_families.push(family.clone());
            }
        }
        request.fallback_families = fallback_families.into_boxed_slice();
    }
}

pub(crate) type CanvasKitParagraphDrawDataRegistry =
    ParagraphDrawDataRegistry<CanvasKitParagraphDrawData>;

pub(crate) fn new_canvaskit_paragraph_registry() -> Arc<CanvasKitParagraphDrawDataRegistry> {
    Arc::new(ParagraphDrawDataRegistry::new(
        ParagraphDrawDataBudget::new(
            NonZeroUsize::new(DEFAULT_PARAGRAPH_DRAW_DATA_ENTRIES)
                .expect("the default CanvasKit paragraph entry budget is nonzero"),
            NonZeroUsize::new(DEFAULT_PARAGRAPH_DRAW_DATA_BYTES)
                .expect("the default CanvasKit paragraph byte budget is nonzero"),
        ),
    ))
}

/// One retained CanvasKit Paragraph handle produced alongside authoritative
/// geometry. Its final owner releases the browser object; paint only reuses the
/// handle and never reshapes text.
pub(crate) struct CanvasKitParagraphDrawData {
    handle: ResourceHandle,
    bridge: Arc<dyn CanvasKitParagraphBridge>,
}

impl CanvasKitParagraphDrawData {
    pub(crate) const fn handle(&self) -> ResourceHandle {
        self.handle
    }
}

impl fmt::Debug for CanvasKitParagraphDrawData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanvasKitParagraphDrawData")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

impl Drop for CanvasKitParagraphDrawData {
    fn drop(&mut self) {
        let _ = self.bridge.destroy_paragraph(self.handle);
    }
}

/// SkParagraph engine paired with one CanvasKit graphics profile.
///
/// The copied result geometry is authoritative for layout, hit testing,
/// selection, caret placement, and IME. The returned draw-data identifier
/// names the exact browser paragraph that produced those values.
pub struct CanvasKitParagraphEngine {
    bridge: Arc<dyn CanvasKitParagraphBridge>,
    fonts: Arc<CanvasKitFontState>,
    draw_data: Arc<CanvasKitParagraphDrawDataRegistry>,
    capabilities: ParagraphCapabilities,
}

impl CanvasKitParagraphEngine {
    pub(crate) fn new(
        bridge: Arc<dyn CanvasKitParagraphBridge>,
        fonts: Arc<CanvasKitFontState>,
        draw_data: Arc<CanvasKitParagraphDrawDataRegistry>,
    ) -> Self {
        let capabilities = map_capabilities(COMPLETE_CAPABILITY_BITS)
            .expect("the CanvasKit capability inventory contains only known capabilities");
        Self {
            bridge,
            fonts,
            draw_data,
            capabilities,
        }
    }
}

impl ParagraphEngine for CanvasKitParagraphEngine {
    fn capabilities(&self) -> ParagraphCapabilities {
        self.capabilities
    }

    fn layout(
        &self,
        description: &ParagraphDescription,
    ) -> Result<ParagraphResult, ParagraphError> {
        description.validate()?;
        self.capabilities
            .require_all(description.required_capabilities())
            .map_err(ParagraphError::UnsupportedCapability)?;

        let mut packed = PackedParagraphRequest::from_description(description)?;
        self.fonts.apply_to(&mut packed);
        let installed = self.fonts.snapshot(&packed)?;
        let cache_key = paragraph_cache_key(&packed);
        let request = WebParagraphRequest {
            paragraph: native_request(packed.clone()).map_err(paragraph_bridge_error)?,
            fonts: installed.fonts,
        };
        let encoded = encode_paragraph_request(&request).map_err(|error| {
            ParagraphError::backend(
                "skia-canvaskit-skparagraph",
                format!("paragraph request encoding failed: {error}"),
            )
        })?;
        let response = self
            .bridge
            .layout_paragraph(encoded)
            .map_err(|error| ParagraphError::backend("skia-canvaskit-skparagraph", error))?;
        let response = decode_paragraph_response(&response).map_err(|error| {
            ParagraphError::backend(
                "skia-canvaskit-skparagraph",
                format!("paragraph response decoding failed: {error}"),
            )
        })?;
        // From this point onward every failure path retires the browser handle.
        let draw_data = Arc::new(CanvasKitParagraphDrawData {
            handle: response.handle,
            bridge: Arc::clone(&self.bridge),
        });
        let returned = map_capabilities(response.output.capabilities.bits())
            .map_err(paragraph_bridge_error)?;
        if returned != self.capabilities {
            return Err(ParagraphError::backend(
                "skia-canvaskit-skparagraph",
                format!(
                    "CanvasKit returned paragraph capabilities {returned:?}, expected {:?}",
                    self.capabilities
                ),
            ));
        }
        let decoded = packed_output(response.output)
            .map_err(paragraph_bridge_error)?
            .decode(&description.text)?;
        let result = ParagraphResult::new(
            description,
            cache_key,
            self.capabilities,
            decoded.geometry,
            decoded.unresolved_glyphs,
        )?;
        let approximate_bytes = usize::try_from(response.approximate_bytes).map_err(|_| {
            ParagraphError::backend(
                "skia-canvaskit-skparagraph",
                "CanvasKit paragraph byte accounting does not fit this target",
            )
        })?;
        let draw_data = self
            .draw_data
            .register(cache_key, draw_data, approximate_bytes)
            .map_err(|error| {
                ParagraphError::backend("skia-canvaskit-skparagraph", error.to_string())
            })?;
        Ok(result.with_draw_data(draw_data))
    }
}

fn paragraph_bridge_error(error: impl fmt::Display) -> ParagraphError {
    ParagraphError::backend("skia-canvaskit-skparagraph", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_ir::op::{Color, TextParagraphStyle, TextStyle};
    use fission_layout::{ParagraphStyleRun, Utf8Range};

    #[test]
    fn complete_web_profile_includes_interactive_geometry() {
        let capabilities = map_capabilities(COMPLETE_CAPABILITY_BITS).unwrap();

        for capability in [
            ParagraphCapability::HitTesting,
            ParagraphCapability::CaretGeometry,
            ParagraphCapability::SelectionGeometry,
            ParagraphCapability::ClusterMapping,
        ] {
            assert!(capabilities.supports(capability));
        }
    }

    #[test]
    fn profile_catalog_families_extend_request_fallbacks_deterministically() {
        let range = Utf8Range::from_byte_offsets(0, 4).unwrap();
        let description = ParagraphDescription::new(
            "text",
            vec![ParagraphStyleRun::new(
                range,
                TextStyle {
                    font_size: 16.0,
                    color: Color::BLACK,
                    underline: false,
                    font_family: Some("Author Font".into()),
                    locale: None,
                    font_weight: 400,
                    font_style: Default::default(),
                    line_height: None,
                    letter_spacing: 0.0,
                    background_color: None,
                },
            )],
            TextParagraphStyle::default(),
            Some(200.0),
        );
        let mut request = PackedParagraphRequest::from_description(&description).unwrap();
        let fonts = CanvasKitFontState::new(
            42,
            "Fission Default".into(),
            vec![
                "Fission Default".into(),
                "Noto Sans".into(),
                "Noto Sans".into(),
            ],
        );

        fonts.apply_to(&mut request);

        assert_eq!(request.font_catalog_generation, 42);
        assert_eq!(
            request
                .fallback_families
                .iter()
                .map(Box::as_ref)
                .collect::<Vec<_>>(),
            vec!["Fission Default", "Noto Sans"]
        );
    }
}
