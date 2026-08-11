#![cfg(feature = "test-shim")]

use fission_skia_sys::ffi;
use fission_skia_sys::{
    Color, Context, DecodedImage, Engine, ErrorKind, Frame, FrameOp, ImageSampling, Paint,
    PixelRect, RasterSurface, RecordedPicture, Rect, SvgDocument,
};

#[test]
fn retained_pictures_are_cloneable_nested_and_independent_of_source_handles() {
    assert_send_sync::<RecordedPicture>();
    assert_eq!(live_counts(), ffi::TestCounts::default());

    let image = DecodedImage::decode_encoded(&test_image(), 4).expect("test image");
    let svg = SvgDocument::parse(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"/>"#)
        .expect("test SVG");
    let source = Frame::new([
        FrameOp::FillRect {
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            radius: 0.0,
            paint: Paint::solid(Color::rgba(0.0, 1.0, 0.0, 1.0)),
        },
        FrameOp::DrawImage {
            image,
            source: Rect::new(0.0, 0.0, 1.0, 1.0),
            destination: Rect::new(1.0, 0.0, 1.0, 1.0),
            sampling: ImageSampling::Nearest,
        },
        FrameOp::DrawSvg {
            document: svg,
            destination: Rect::new(2.0, 0.0, 1.0, 1.0),
        },
    ]);
    let picture =
        RecordedPicture::record(Rect::new(0.0, 0.0, 3.0, 1.0), &source).expect("recorded picture");
    assert_eq!(live_counts().pictures, 1);
    let clone = picture.clone();
    drop(source);
    assert_eq!(live_counts().images, 0);
    assert_eq!(live_counts().svg_documents, 0);
    assert_eq!(live_counts().pictures, 1);

    let nested = RecordedPicture::record(
        Rect::new(0.0, 0.0, 3.0, 1.0),
        &Frame::new([FrameOp::DrawPicture { picture }]),
    )
    .expect("nested picture");
    drop(clone);
    assert_eq!(live_counts().pictures, 1);

    let engine = Engine::new().expect("test engine");
    assert_ne!(
        engine.build_info().feature_bits & ffi::FEATURE_RETAINED_PICTURE,
        0
    );
    let context = Context::new_raster(&engine).expect("raster context");
    let mut surface = RasterSurface::new(&context, 3, 1).expect("raster surface");
    surface
        .execute_frame(&Frame::new([
            FrameOp::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)),
            FrameOp::DrawPicture { picture: nested },
        ]))
        .expect("picture playback");
    assert_eq!(
        surface
            .read_pixels_rgba8888(Some(PixelRect::new(0, 0, 3, 1)))
            .expect("picture readback"),
        [0, 255, 0, 255, 255, 0, 0, 255, 64, 128, 191, 255]
    );

    drop(surface);
    drop(context);
    drop(engine);
    assert_eq!(live_counts(), ffi::TestCounts::default());
}

#[test]
fn recording_rejects_invalid_bounds_unbalanced_state_and_surface_operations() {
    let bounds = Rect::new(0.0, 0.0, 1.0, 1.0);
    for invalid in [
        Rect::new(0.0, 0.0, 0.0, 1.0),
        Rect::new(f32::MAX, 0.0, f32::MAX, 1.0),
        Rect::new(f32::NAN, 0.0, 1.0, 1.0),
    ] {
        assert_eq!(
            RecordedPicture::record(invalid, &Frame::default())
                .expect_err("invalid cull bounds")
                .kind,
            ErrorKind::InvalidArgument
        );
    }
    assert_eq!(
        RecordedPicture::record(bounds, &Frame::new([FrameOp::Save]))
            .expect_err("unbalanced save")
            .kind,
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        RecordedPicture::record(bounds, &Frame::new([FrameOp::Clear(Color::TRANSPARENT)]))
            .expect_err("surface clear")
            .kind,
        ErrorKind::Unsupported
    );
    assert_eq!(
        RecordedPicture::record(
            bounds,
            &Frame::new([FrameOp::BackdropBlur {
                bounds,
                corner_radius: 0.0,
                sigma: 1.0,
            }])
        )
        .expect_err("surface backdrop")
        .kind,
        ErrorKind::Unsupported
    );
}

fn assert_send_sync<T: Send + Sync>() {}

fn test_image() -> Vec<u8> {
    let mut encoded = Vec::from(*b"FSIM");
    encoded.extend_from_slice(&1_u32.to_le_bytes());
    encoded.extend_from_slice(&1_u32.to_le_bytes());
    encoded.extend_from_slice(&[255, 0, 0, 255]);
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
