#![cfg(feature = "test-shim")]

use fission_skia_sys::ffi;
use fission_skia_sys::{
    Color, Context, Engine, ErrorKind, Frame, FrameOp, PixelRect, RasterSurface, Rect, SvgDocument,
    MAX_SVG_DOCUMENT_BYTES,
};

const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2 1">
  <rect width="2" height="1" fill="#123456"/>
</svg>"##;

#[test]
fn svg_documents_are_bounded_cloneable_pinned_and_drawn_with_contain_placement() {
    assert_send_sync::<SvgDocument>();
    assert_eq!(live_counts().svg_documents, 0);

    for invalid in [
        Vec::new(),
        b"<svg>\0</svg>".to_vec(),
        vec![0xff],
        b"<!DOCTYPE svg><svg/>".to_vec(),
        b"<svg>".to_vec(),
    ] {
        let error = SvgDocument::parse(&invalid).expect_err("invalid SVG must fail closed");
        assert_eq!(error.kind, ErrorKind::InvalidArgument);
        assert_eq!(live_counts().svg_documents, 0);
    }

    let oversized = vec![b' '; MAX_SVG_DOCUMENT_BYTES + 1];
    let error = SvgDocument::parse(&oversized).expect_err("oversized SVG must fail closed");
    assert_eq!(error.kind, ErrorKind::InvalidArgument);

    let document = SvgDocument::parse(SVG).expect("bounded SVG parse");
    assert_eq!(document.source_bytes_len(), SVG.len());
    assert_eq!(live_counts().svg_documents, 1);
    let retained = document.clone();
    drop(document);
    assert_eq!(live_counts().svg_documents, 1);

    let engine = Engine::new().expect("test engine");
    assert_ne!(
        engine.build_info().feature_bits & ffi::FEATURE_SVG_DOCUMENT,
        0
    );
    let context = Context::new_raster(&engine).expect("raster context");
    let mut surface = RasterSurface::new(&context, 6, 4).expect("raster surface");
    let frame = Frame::new([
        FrameOp::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)),
        FrameOp::DrawSvg {
            document: retained,
            destination: Rect::new(1.0, 0.0, 4.0, 4.0),
        },
    ]);
    surface.execute_frame(&frame).expect("contained SVG draw");

    let pixels = surface
        .read_pixels_rgba8888(Some(PixelRect::new(0, 0, 6, 4)))
        .expect("SVG readback");
    for y in 0..4 {
        for x in 0..6 {
            let offset = (y * 6 + x) * 4;
            let expected = if (1..=4).contains(&x) && (1..=2).contains(&y) {
                [64, 128, 191, 255]
            } else {
                [0, 0, 0, 255]
            };
            assert_eq!(&pixels[offset..offset + 4], &expected, "pixel ({x}, {y})");
        }
    }

    let invalid_destination = surface
        .execute_frame(&Frame::new([FrameOp::DrawSvg {
            document: SvgDocument::parse(SVG).expect("validation document"),
            destination: Rect::new(0.0, 0.0, 0.0, 1.0),
        }]))
        .expect_err("empty SVG destination must fail closed");
    assert_eq!(invalid_destination.kind, ErrorKind::InvalidArgument);

    drop(frame);
    drop(surface);
    drop(context);
    drop(engine);
    assert_eq!(live_counts(), ffi::TestCounts::default());
}

fn assert_send_sync<T: Send + Sync>() {}

fn live_counts() -> ffi::TestCounts {
    let mut counts = ffi::TestCounts::default();
    let mut error = ffi::Error::default();
    // SAFETY: both outputs are initialized and valid for the call.
    let status = unsafe { ffi::fission_skia_test_live_counts(&mut counts, &mut error) };
    assert_eq!(status, ffi::STATUS_OK);
    counts
}
