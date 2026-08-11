#![cfg(feature = "test-shim")]

use fission_skia_sys::ffi;
use fission_skia_sys::{
    Color, Context, DecodedImage, Engine, ErrorKind, Frame, FrameOp, ImageSampling, PixelRect,
    RasterSurface, Rect,
};

#[test]
fn decoded_images_are_bounded_cloneable_pinned_and_sampled_explicitly() {
    assert_send_sync::<DecodedImage>();
    assert_eq!(live_counts().images, 0);

    let encoded = test_image(
        3,
        1,
        &[[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]],
    );
    let budget_error = DecodedImage::decode_encoded(&encoded, 11)
        .expect_err("preflight must reject output above the caller budget");
    assert_eq!(budget_error.kind, ErrorKind::OutOfMemory);
    assert_eq!(live_counts().images, 0);

    let image = DecodedImage::decode_encoded(&encoded, 12).expect("bounded image decode");
    assert_eq!((image.width(), image.height()), (3, 1));
    assert_eq!(image.approximate_decoded_bytes(), 12);
    assert_eq!(live_counts().images, 1);
    let retained = image.clone();
    drop(image);
    assert_eq!(live_counts().images, 1);

    let engine = Engine::new().expect("test engine");
    assert_ne!(
        engine.build_info().feature_bits & ffi::FEATURE_IMAGE_DECODE,
        0
    );
    let context = Context::new_raster(&engine).expect("raster context");
    let mut surface = RasterSurface::new(&context, 4, 1).expect("raster surface");
    let nearest = Frame::new([
        FrameOp::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)),
        FrameOp::DrawImage {
            image: retained,
            source: Rect::new(1.0, 0.0, 2.0, 1.0),
            destination: Rect::new(0.0, 0.0, 4.0, 1.0),
            sampling: ImageSampling::Nearest,
        },
    ]);
    surface
        .execute_frame(&nearest)
        .expect("nearest cropped image draw");
    assert_eq!(
        surface
            .read_pixels_rgba8888(Some(PixelRect::new(0, 0, 4, 1)))
            .expect("nearest readback"),
        [0, 255, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255,]
    );

    let linear_image = DecodedImage::decode_encoded(&encoded, 12).expect("second decode");
    surface
        .execute_frame(&Frame::new([
            FrameOp::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)),
            FrameOp::DrawImage {
                image: linear_image,
                source: Rect::new(0.0, 0.0, 2.0, 1.0),
                destination: Rect::new(0.0, 0.0, 1.0, 1.0),
                sampling: ImageSampling::Linear,
            },
        ]))
        .expect("linear image draw");
    assert_eq!(
        surface
            .read_pixels_rgba8888(Some(PixelRect::new(0, 0, 1, 1)))
            .expect("linear readback"),
        [128, 128, 0, 255]
    );

    let invalid_image = DecodedImage::decode_encoded(&encoded, 12).expect("validation image");
    let invalid = surface
        .execute_frame(&Frame::new([FrameOp::DrawImage {
            image: invalid_image,
            source: Rect::new(2.0, 0.0, 2.0, 1.0),
            destination: Rect::new(0.0, 0.0, 1.0, 1.0),
            sampling: ImageSampling::Nearest,
        }]))
        .expect_err("source outside the decoded image must fail closed");
    assert_eq!(invalid.kind, ErrorKind::InvalidArgument);

    drop(nearest);
    drop(surface);
    drop(context);
    drop(engine);
    assert_eq!(live_counts(), ffi::TestCounts::default());
}

fn assert_send_sync<T: Send + Sync>() {}

fn test_image(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    assert_eq!(pixels.len(), width as usize * height as usize);
    let mut encoded = Vec::with_capacity(12 + pixels.len() * 4);
    encoded.extend_from_slice(b"FSIM");
    encoded.extend_from_slice(&width.to_le_bytes());
    encoded.extend_from_slice(&height.to_le_bytes());
    for pixel in pixels {
        encoded.extend_from_slice(pixel);
    }
    encoded
}

fn live_counts() -> ffi::TestCounts {
    let mut counts = ffi::TestCounts::default();
    let mut error = ffi::Error::default();
    // SAFETY: both outputs are initialized and valid for the call.
    let status = unsafe { ffi::fission_skia_test_live_counts(&mut counts, &mut error) };
    assert_eq!(status, ffi::STATUS_OK);
    counts
}
