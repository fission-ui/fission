//! Safe ownership for immutable SVG documents parsed by the pinned Skia build.

use std::sync::Arc;

use crate::error::status_result;
use crate::{ffi, Error, ErrorKind, Result};

/// Maximum UTF-8 source length accepted by [`SvgDocument::parse`].
pub const MAX_SVG_DOCUMENT_BYTES: usize = ffi::MAX_SVG_DOCUMENT_BYTES;

/// A retained SVG document owned by SkSVGDOM.
///
/// Clones share one opaque native handle. Source bytes are borrowed only for
/// the synchronous parse and are not retained by this wrapper. The bridge uses
/// SkSVGDOM without an external resource provider, so document rendering cannot
/// load files or network resources. Native access is serialized by the bridge;
/// the final clone may therefore be dropped on any thread while drawing remains
/// bound to the raster surface's owner thread.
#[derive(Clone)]
pub struct SvgDocument {
    inner: Arc<RawSvgDocument>,
}

impl SvgDocument {
    /// Parses one complete UTF-8 SVG document.
    ///
    /// Inputs are bounded by [`MAX_SVG_DOCUMENT_BYTES`]. Embedded NUL, malformed
    /// UTF-8, and DTD/entity declarations are rejected before entering Skia.
    /// The native bridge independently repeats those checks for raw ABI callers.
    pub fn parse(source: &[u8]) -> Result<Self> {
        validate_source(source)?;

        let mut handle = 0;
        let mut error = ffi::Error::default();
        // SAFETY: source remains live and immutable for the synchronous parse,
        // and the output handle and diagnostic record are valid for writes.
        let status = unsafe {
            ffi::fission_skia_svg_document_parse(
                source.as_ptr(),
                source.len(),
                &mut handle,
                &mut error,
            )
        };
        status_result(status, &error)?;
        if handle == 0 {
            return Err(Error::local(
                ErrorKind::Internal,
                "SvgDocument::parse",
                "bridge reported success with a null SVG document handle",
            ));
        }

        Ok(Self {
            inner: Arc::new(RawSvgDocument {
                handle,
                source_bytes_len: source.len(),
            }),
        })
    }

    /// Exact length of the validated UTF-8 source used to create this DOM.
    ///
    /// This is useful as a cache weight input; it is not a measurement or an
    /// upper bound for SkSVGDOM's retained allocations.
    pub fn source_bytes_len(&self) -> usize {
        self.inner.source_bytes_len
    }

    pub(crate) fn raw_handle(&self) -> ffi::SvgDocumentHandle {
        self.inner.handle
    }
}

impl std::fmt::Debug for SvgDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SvgDocument")
            .field("source_bytes_len", &self.source_bytes_len())
            .finish_non_exhaustive()
    }
}

impl PartialEq for SvgDocument {
    fn eq(&self, other: &Self) -> bool {
        self.inner.handle == other.inner.handle
    }
}

impl Eq for SvgDocument {}

#[derive(Debug)]
struct RawSvgDocument {
    handle: ffi::SvgDocumentHandle,
    source_bytes_len: usize,
}

impl Drop for RawSvgDocument {
    fn drop(&mut self) {
        let mut error = ffi::Error::default();
        // SAFETY: this Arc payload is the sole owner of the native handle and
        // destroys it exactly once after every safe clone has gone away.
        let status = unsafe { ffi::fission_skia_svg_document_destroy(self.handle, &mut error) };
        debug_assert_eq!(status, ffi::STATUS_OK, "SVG document destruction failed");
    }
}

fn validate_source(source: &[u8]) -> Result<()> {
    if source.is_empty() {
        return Err(invalid("SVG document bytes must not be empty"));
    }
    if source.len() > MAX_SVG_DOCUMENT_BYTES {
        return Err(invalid(format!(
            "SVG document exceeds the {MAX_SVG_DOCUMENT_BYTES}-byte input limit"
        )));
    }
    if std::str::from_utf8(source).is_err() {
        return Err(invalid("SVG document must be valid UTF-8"));
    }
    if source.contains(&0) {
        return Err(invalid("SVG document must not contain embedded NUL"));
    }
    if contains_ascii_case_insensitive(source, b"<!DOCTYPE")
        || contains_ascii_case_insensitive(source, b"<!ENTITY")
    {
        return Err(invalid(
            "SVG document must not contain DTD or entity declarations",
        ));
    }
    Ok(())
}

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn invalid(message: impl Into<String>) -> Error {
    Error::local(ErrorKind::InvalidArgument, "SvgDocument::parse", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_validation_rejects_unsafe_inputs() {
        assert_eq!(
            validate_source(b"").expect_err("empty input").kind,
            ErrorKind::InvalidArgument
        );
        assert_eq!(
            validate_source(b"<svg>\0</svg>")
                .expect_err("embedded NUL")
                .kind,
            ErrorKind::InvalidArgument
        );
        assert_eq!(
            validate_source(b"<!doctype svg><svg/>")
                .expect_err("DTD")
                .kind,
            ErrorKind::InvalidArgument
        );
        assert_eq!(
            validate_source(&[0xff]).expect_err("invalid UTF-8").kind,
            ErrorKind::InvalidArgument
        );
        validate_source(b"<svg viewBox=\"0 0 1 1\"/>").expect("bounded UTF-8 SVG");
    }
}
