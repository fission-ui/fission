//! SkParagraph's backend-neutral, owned paragraph adapter.
//!
//! The native seam is deliberately expressed as one owned request and one
//! owned result. That keeps Skia pointers, thread affinity, and object lifetime
//! out of [`fission_layout::ParagraphResult`].

mod cache_key;
mod output;
mod request;

use std::fmt;
use std::sync::Arc;

use fission_layout::{
    ParagraphCapabilities, ParagraphCapability, ParagraphDescription, ParagraphEngine,
    ParagraphError, ParagraphResult,
};

use self::cache_key::paragraph_cache_key;
use self::output::PackedParagraphOutput;
use self::request::PackedParagraphRequest;

#[cfg(test)]
mod tests;

pub(super) const COMPLETE_PARAGRAPH_CAPABILITIES: ParagraphCapabilities =
    ParagraphCapabilities::NONE
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
/// The concrete batched API is injected internally. The default implementation
/// makes unsupported state explicit until the native SkParagraph ABI is wired.
pub struct SkiaParagraphEngine {
    api: Arc<dyn BatchedParagraphApi>,
}

impl Default for SkiaParagraphEngine {
    fn default() -> Self {
        Self {
            api: Arc::new(UnsupportedParagraphApi),
        }
    }
}

impl SkiaParagraphEngine {
    pub(super) fn with_api(api: impl BatchedParagraphApi + 'static) -> Self {
        Self { api: Arc::new(api) }
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
        let output = self
            .api
            .layout(request)
            .map_err(|error| ParagraphError::backend("skia-skparagraph", error.to_string()))?;
        let decoded = output.decode(&description.text)?;

        ParagraphResult::new(
            description,
            cache_key,
            capabilities,
            decoded.geometry,
            decoded.unresolved_glyphs,
        )
    }
}

/// One batched call boundary suitable for a future safe `fission-skia-sys`
/// wrapper or deterministic tests.
pub(super) trait BatchedParagraphApi: Send + Sync {
    fn capabilities(&self) -> ParagraphCapabilities;

    fn layout(
        &self,
        request: PackedParagraphRequest,
    ) -> Result<PackedParagraphOutput, BatchedParagraphError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BatchedParagraphError {
    operation: &'static str,
    details: String,
}

impl BatchedParagraphError {
    pub(super) fn new(operation: &'static str, details: impl Into<String>) -> Self {
        Self {
            operation,
            details: details.into(),
        }
    }
}

impl fmt::Display for BatchedParagraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} failed: {}", self.operation, self.details)
    }
}

struct UnsupportedParagraphApi;

impl BatchedParagraphApi for UnsupportedParagraphApi {
    fn capabilities(&self) -> ParagraphCapabilities {
        ParagraphCapabilities::NONE
    }

    fn layout(
        &self,
        _request: PackedParagraphRequest,
    ) -> Result<PackedParagraphOutput, BatchedParagraphError> {
        Err(BatchedParagraphError::new(
            "layout",
            "the native SkParagraph ABI has not been installed",
        ))
    }
}
