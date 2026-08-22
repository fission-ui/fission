//! Safe ownership for immutable images decoded by the pinned Skia build.

use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use crate::error::status_result;
use crate::{ffi, Error, ErrorKind, Result};

/// Immutable decoded image data owned by Skia.
///
/// Clones share one opaque native handle. Encoded source bytes are borrowed
/// only for the synchronous decode and are not retained; the returned decoded
/// pixels are independently owned. The final clone may be dropped on any
/// thread; drawing remains bound to the raster surface's owner thread.
#[derive(Clone)]
pub struct DecodedImage {
    inner: Arc<RawImage>,
}

impl DecodedImage {
    /// Decodes one complete encoded image with the codecs in the active Skia
    /// artifact profile. Encoded orientation is applied to the result.
    ///
    /// `max_decoded_bytes` is mandatory and bounds cumulative allocations made
    /// by SkCodec. The oriented N32 premultiplied-sRGB output is preflighted
    /// against the same limit before pixel allocation.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn decode_encoded(encoded: &[u8], max_decoded_bytes: usize) -> Result<Self> {
        if encoded.is_empty() {
            return Err(Error::local(
                ErrorKind::InvalidArgument,
                "DecodedImage::decode_encoded",
                "encoded image bytes must not be empty",
            ));
        }
        if max_decoded_bytes == 0 {
            return Err(Error::local(
                ErrorKind::InvalidArgument,
                "DecodedImage::decode_encoded",
                "decoded image byte limit must be non-zero",
            ));
        }
        let mut handle = 0;
        let mut info = ffi::ImageInfo::default();
        let mut error = ffi::Error::default();
        // SAFETY: encoded remains live and immutable for the synchronous call;
        // both outputs are initialized, aligned, and valid for writes.
        let status = unsafe {
            ffi::fission_skia_image_decode_encoded(
                encoded.as_ptr(),
                encoded.len(),
                max_decoded_bytes,
                &mut handle,
                &mut info,
                &mut error,
            )
        };
        status_result(status, &error)?;
        if handle == 0 {
            return Err(Error::local(
                ErrorKind::Internal,
                "DecodedImage::decode_encoded",
                "bridge reported success with a null decoded image handle",
            ));
        }
        let raw = RawImage {
            handle,
            width: info.width,
            height: info.height,
            approximate_decoded_bytes: info.approximate_decoded_bytes,
        };
        validate_info(&info, max_decoded_bytes, "DecodedImage::decode_encoded")?;
        Ok(Self {
            inner: Arc::new(raw),
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn decode_encoded(_encoded: &[u8], _max_decoded_bytes: usize) -> Result<Self> {
        Err(Error::local(
            ErrorKind::Unsupported,
            "DecodedImage::decode_encoded",
            "native Skia image handles are unavailable in browser builds; use the CanvasKit resource protocol",
        ))
    }

    pub fn width(&self) -> u32 {
        self.inner.width
    }

    pub fn height(&self) -> u32 {
        self.inner.height
    }

    /// Approximate decoded pixel storage used for cache budgeting.
    pub fn approximate_decoded_bytes(&self) -> usize {
        self.inner.approximate_decoded_bytes
    }

    pub(crate) fn raw_handle(&self) -> ffi::ImageHandle {
        self.inner.handle
    }
}

impl std::fmt::Debug for DecodedImage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DecodedImage")
            .field("width", &self.width())
            .field("height", &self.height())
            .field(
                "approximate_decoded_bytes",
                &self.approximate_decoded_bytes(),
            )
            .finish_non_exhaustive()
    }
}

impl PartialEq for DecodedImage {
    fn eq(&self, other: &Self) -> bool {
        self.inner.handle == other.inner.handle
    }
}

impl Eq for DecodedImage {}

#[derive(Debug)]
struct RawImage {
    handle: ffi::ImageHandle,
    width: u32,
    height: u32,
    approximate_decoded_bytes: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for RawImage {
    fn drop(&mut self) {
        let mut error = ffi::Error::default();
        // SAFETY: this Arc payload is the sole owner of the native handle and
        // destroys it exactly once after every safe clone has gone away.
        let status = unsafe { ffi::fission_skia_image_destroy(self.handle, &mut error) };
        debug_assert_eq!(status, ffi::STATUS_OK, "decoded image destruction failed");
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_info(info: &ffi::ImageInfo, max_decoded_bytes: usize, operation: &str) -> Result<()> {
    if info.width == 0 || info.height == 0 {
        return Err(Error::local(
            ErrorKind::Internal,
            operation,
            "bridge reported success with an invalid decoded image",
        ));
    }
    let minimum = usize::try_from(info.width)
        .ok()
        .and_then(|width| width.checked_mul(usize::try_from(info.height).ok()?))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| {
            Error::local(
                ErrorKind::Internal,
                operation,
                "decoded image dimensions overflow this platform",
            )
        })?;
    if info.approximate_decoded_bytes < minimum
        || info.approximate_decoded_bytes > max_decoded_bytes
    {
        return Err(Error::local(
            ErrorKind::Internal,
            operation,
            "bridge reported an invalid decoded image byte estimate",
        ));
    }
    Ok(())
}
