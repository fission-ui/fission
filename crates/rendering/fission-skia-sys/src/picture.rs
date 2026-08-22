//! Safe ownership for immutable retained Skia pictures.

use std::sync::Arc;

use crate::error::status_result;
use crate::{ffi, Error, ErrorKind, Frame, Rect, Result};

/// An immutable frame fragment recorded by Skia.
///
/// Clones share one opaque native handle. Recording is synchronous and copies
/// every paint operation into the picture, including native references needed
/// by nested pictures, decoded images, paragraphs, and expanded SVG content.
/// The final clone may be dropped on any thread; registry access and native
/// destruction are serialized by the bridge.
#[derive(Clone)]
pub struct RecordedPicture {
    inner: Arc<RawPicture>,
}

impl RecordedPicture {
    /// Records `frame` with explicit cull bounds.
    ///
    /// Bounds must be finite and non-empty. Surface-dependent operations such
    /// as `FrameOp::Clear` and `FrameOp::BackdropBlur` are rejected rather
    /// than changing meaning when the picture is replayed elsewhere.
    pub fn record(bounds: Rect, frame: &Frame) -> Result<Self> {
        let bounds = recording_bounds(bounds)?;
        frame.ensure_picture_recordable()?;

        let encoded = frame.encode()?;
        let raw_frame = encoded.as_raw();
        let mut handle = 0;
        let mut error = ffi::Error::default();
        // SAFETY: bounds and raw_frame remain valid for the synchronous call;
        // encoded pins every resource referenced by raw_frame until it returns.
        let status = unsafe {
            ffi::fission_skia_picture_record(&bounds, &raw_frame, &mut handle, &mut error)
        };
        status_result(status, &error)?;
        if handle == 0 {
            return Err(Error::local(
                ErrorKind::Internal,
                "RecordedPicture::record",
                "bridge reported success with a null recorded picture handle",
            ));
        }
        Ok(Self {
            inner: Arc::new(RawPicture { handle }),
        })
    }

    pub(crate) fn raw_handle(&self) -> ffi::PictureHandle {
        self.inner.handle
    }
}

impl std::fmt::Debug for RecordedPicture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecordedPicture")
            .finish_non_exhaustive()
    }
}

impl PartialEq for RecordedPicture {
    fn eq(&self, other: &Self) -> bool {
        self.inner.handle == other.inner.handle
    }
}

impl Eq for RecordedPicture {}

#[derive(Debug)]
struct RawPicture {
    handle: ffi::PictureHandle,
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for RawPicture {
    fn drop(&mut self) {
        let mut error = ffi::Error::default();
        // SAFETY: this Arc payload owns the live native handle and releases it
        // exactly once after all safe clones and encoded frames have dropped.
        let status = unsafe { ffi::fission_skia_picture_destroy(self.handle, &mut error) };
        debug_assert_eq!(
            status,
            ffi::STATUS_OK,
            "recorded picture destruction failed"
        );
    }
}

fn recording_bounds(bounds: Rect) -> Result<ffi::Rect> {
    if ![bounds.x, bounds.y, bounds.width, bounds.height]
        .iter()
        .all(|value| value.is_finite())
        || bounds.width <= 0.0
        || bounds.height <= 0.0
        || !(bounds.x + bounds.width).is_finite()
        || !(bounds.y + bounds.height).is_finite()
    {
        return Err(Error::local(
            ErrorKind::InvalidArgument,
            "RecordedPicture::record",
            "picture cull bounds must be finite and non-empty with finite edges",
        ));
    }
    Ok(ffi::Rect {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
    })
}
