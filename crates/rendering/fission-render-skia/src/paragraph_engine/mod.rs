//! SkParagraph's backend-neutral, owned paragraph adapter.
//!
//! The native seam is deliberately expressed as one owned request and one
//! owned result. That keeps Skia pointers, thread affinity, and object lifetime
//! out of [`fission_layout::ParagraphResult`].

mod cache_key;
mod native;
mod output;
mod request;

use std::fmt;
use std::sync::Arc;

#[cfg(test)]
use fission_layout::ParagraphCapability;
use fission_layout::{
    ParagraphCapabilities, ParagraphDescription, ParagraphEngine, ParagraphError, ParagraphResult,
};

use self::cache_key::paragraph_cache_key;
#[cfg(not(feature = "test-shim"))]
use self::native::NativeParagraphApi;
use self::output::PackedParagraphOutput;
use self::request::PackedParagraphRequest;
use crate::profile::{new_paragraph_draw_data_registry, SkiaParagraphDrawDataRegistry};

#[cfg(test)]
mod tests;

#[cfg(test)]
const COMPLETE_PARAGRAPH_CAPABILITIES: ParagraphCapabilities = ParagraphCapabilities::NONE
    .with(ParagraphCapability::BidirectionalText)
    .with(ParagraphCapability::VariableFonts)
    .with(ParagraphCapability::FontFeatures)
    .with(ParagraphCapability::InlineObjects)
    .with(ParagraphCapability::ClusterMapping)
    .with(ParagraphCapability::HitTesting)
    .with(ParagraphCapability::CaretGeometry)
    .with(ParagraphCapability::SelectionGeometry)
    .with(ParagraphCapability::UnresolvedGlyphDiagnostics);

/// Safe renderer-side SkParagraph adapter.
///
/// The concrete batched API is injected internally. Production builds use the
/// safe, owned `fission-skia-sys` paragraph engine. Test-shim builds keep their
/// lack of native paragraph support explicit.
pub struct SkiaParagraphEngine {
    api: Arc<dyn BatchedParagraphApi>,
    draw_data: Arc<SkiaParagraphDrawDataRegistry>,
}

impl Default for SkiaParagraphEngine {
    fn default() -> Self {
        Self::with_draw_data_registry(new_paragraph_draw_data_registry())
    }
}

#[cfg(feature = "test-shim")]
fn default_api() -> Arc<dyn BatchedParagraphApi> {
    Arc::new(UnavailableParagraphApi::new(BatchedParagraphError::new(
        "layout",
        "the native SkParagraph ABI has not been installed for the test shim",
    )))
}

#[cfg(not(feature = "test-shim"))]
fn default_api() -> Arc<dyn BatchedParagraphApi> {
    match NativeParagraphApi::new() {
        Ok(api) => Arc::new(api),
        Err(error) => Arc::new(UnavailableParagraphApi::new(error)),
    }
}

impl SkiaParagraphEngine {
    pub(crate) fn with_draw_data_registry(draw_data: Arc<SkiaParagraphDrawDataRegistry>) -> Self {
        Self {
            api: default_api(),
            draw_data,
        }
    }

    #[cfg(test)]
    fn with_api(api: impl BatchedParagraphApi + 'static) -> Self {
        Self {
            api: Arc::new(api),
            draw_data: new_paragraph_draw_data_registry(),
        }
    }
}

impl ParagraphEngine for SkiaParagraphEngine {
    fn capabilities(&self) -> ParagraphCapabilities {
        self.api.capabilities()
    }

    fn layout(
        &self,
        description: &ParagraphDescription,
    ) -> Result<ParagraphResult, ParagraphError> {
        description.validate()?;
        let capabilities = self.api.capabilities();
        capabilities
            .require_all(description.required_capabilities())
            .map_err(ParagraphError::UnsupportedCapability)?;

        let request = PackedParagraphRequest::from_description(description)?;
        let cache_key = paragraph_cache_key(&request);
        let BatchedParagraphLayout { output, draw_data } = self
            .api
            .layout(request)
            .map_err(|error| ParagraphError::backend("skia-skparagraph", error.to_string()))?;
        let decoded = output.decode(&description.text)?;

        let result = ParagraphResult::new(
            description,
            cache_key,
            capabilities,
            decoded.geometry,
            decoded.unresolved_glyphs,
        )?;
        let Some(draw_data) = draw_data else {
            return Ok(result);
        };
        let approximate_bytes = draw_data.approximate_bytes();
        let draw_data = self
            .draw_data
            .register(cache_key, Arc::new(draw_data), approximate_bytes)
            .map_err(|error| ParagraphError::backend("skia-skparagraph", error.to_string()))?;
        Ok(result.with_draw_data(draw_data))
    }
}

struct BatchedParagraphLayout {
    output: PackedParagraphOutput,
    draw_data: Option<fission_skia_sys::ParagraphDrawData>,
}

impl BatchedParagraphLayout {
    #[cfg(test)]
    fn geometry_only(output: PackedParagraphOutput) -> Self {
        Self {
            output,
            draw_data: None,
        }
    }
}

/// One batched call boundary suitable for a future safe `fission-skia-sys`
/// wrapper or deterministic tests.
trait BatchedParagraphApi: Send + Sync {
    fn capabilities(&self) -> ParagraphCapabilities;

    fn layout(
        &self,
        request: PackedParagraphRequest,
    ) -> Result<BatchedParagraphLayout, BatchedParagraphError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchedParagraphError {
    operation: String,
    details: String,
}

impl BatchedParagraphError {
    fn new(operation: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            details: details.into(),
        }
    }

    fn native(error: fission_skia_sys::Error) -> Self {
        let details = if error.sequence == 0 {
            format!("{:?}: {}", error.kind, error.message)
        } else {
            format!(
                "{:?} at bridge sequence {}: {}",
                error.kind, error.sequence, error.message
            )
        };
        Self::new(error.operation, details)
    }
}

impl fmt::Display for BatchedParagraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} failed: {}", self.operation, self.details)
    }
}

struct UnavailableParagraphApi {
    error: BatchedParagraphError,
}

impl UnavailableParagraphApi {
    fn new(error: BatchedParagraphError) -> Self {
        Self { error }
    }
}

impl BatchedParagraphApi for UnavailableParagraphApi {
    fn capabilities(&self) -> ParagraphCapabilities {
        ParagraphCapabilities::NONE
    }

    fn layout(
        &self,
        _request: PackedParagraphRequest,
    ) -> Result<BatchedParagraphLayout, BatchedParagraphError> {
        Err(self.error.clone())
    }
}
