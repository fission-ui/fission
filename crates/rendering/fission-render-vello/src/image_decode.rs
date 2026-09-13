//! Decoding images into pixmaps sized for the renderers.

use std::sync::Arc;
use vello_cpu::peniko::ImageAlphaType;
use vello_cpu::{PixelMetadata, Pixmap};

/// The largest side a decoded image keeps.
///
/// GPU renderers place images in atlas pages of at most this many pixels a side, and no screen
/// needs more pixels than that for one image, so larger images are scaled down once while decoding
/// instead of failing to upload on every frame.
pub(crate) const MAX_DECODED_IMAGE_DIMENSION: u32 = 4096;

pub(crate) fn decode_dynamic_image(
    mut img: image::DynamicImage,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    if let (Some(width), Some(height)) = (cache_width, cache_height) {
        if width > 0 && height > 0 {
            img = img.resize(width, height, image::imageops::FilterType::Triangle);
        }
    }
    if img.width() > MAX_DECODED_IMAGE_DIMENSION || img.height() > MAX_DECODED_IMAGE_DIMENSION {
        img = img.resize(
            MAX_DECODED_IMAGE_DIMENSION,
            MAX_DECODED_IMAGE_DIMENSION,
            image::imageops::FilterType::Triangle,
        );
    }
    let img = img.to_rgba8();
    let (width, height) = img.dimensions();
    // The renderers address pixmaps with 16-bit dimensions.
    let (Ok(width), Ok(height)) = (u16::try_from(width), u16::try_from(height)) else {
        return None;
    };
    Some(Arc::new(Pixmap::from_parts(
        img.into_raw(),
        width,
        height,
        PixelMetadata::new(ImageAlphaType::Alpha, true),
    )))
}
