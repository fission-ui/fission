#![cfg(feature = "test-shim")]

use fission_skia_sys::ffi;
use fission_skia_sys::{
    Color, Context, Engine, Frame, FrameOp, Paint, PixelRect, RasterSurface, Rect,
};

#[test]
fn opacity_layer_composites_the_group_once_and_honors_its_bounds() {
    let engine = Engine::new().expect("test engine");
    assert_ne!(
        engine.build_info().feature_bits & ffi::FEATURE_OPACITY_LAYER,
        0
    );
    let context = Context::new_raster(&engine).expect("raster context");
    let mut surface = RasterSurface::new(&context, 4, 1).expect("raster surface");

    surface
        .execute_frame(&Frame::new([
            FrameOp::Clear(Color::rgba(1.0, 1.0, 1.0, 1.0)),
            FrameOp::OpacityLayer {
                bounds: Rect::new(0.0, 0.0, 3.0, 1.0),
                alpha: 0.5,
            },
            FrameOp::FillRect {
                rect: Rect::new(0.0, 0.0, 2.0, 1.0),
                radius: 0.0,
                paint: Paint::solid(Color::rgba(1.0, 0.0, 0.0, 1.0)),
            },
            FrameOp::FillRect {
                rect: Rect::new(1.0, 0.0, 3.0, 1.0),
                radius: 0.0,
                paint: Paint::solid(Color::rgba(0.0, 0.0, 1.0, 1.0)),
            },
            FrameOp::Restore,
        ]))
        .expect("opacity frame");

    let pixels = surface
        .read_pixels_rgba8888(Some(PixelRect::new(0, 0, 4, 1)))
        .expect("readback");
    let red_only = &pixels[0..4];
    let overlap = &pixels[4..8];
    let blue_only = &pixels[8..12];
    let outside_bounds = &pixels[12..16];

    assert_eq!(red_only, [255, 128, 128, 255]);
    assert_eq!(overlap, blue_only);
    assert_eq!(overlap, [128, 128, 255, 255]);
    assert_eq!(outside_bounds, [255, 255, 255, 255]);
}

#[test]
fn opacity_layer_rejects_non_finite_or_out_of_range_alpha() {
    let engine = Engine::new().expect("test engine");
    let context = Context::new_raster(&engine).expect("raster context");
    let mut surface = RasterSurface::new(&context, 1, 1).expect("raster surface");

    for alpha in [f32::NAN, -0.1, 1.1] {
        let error = surface
            .execute_frame(&Frame::new([
                FrameOp::OpacityLayer {
                    bounds: Rect::new(0.0, 0.0, 1.0, 1.0),
                    alpha,
                },
                FrameOp::Restore,
            ]))
            .expect_err("invalid alpha must fail closed");
        assert_eq!(error.kind, fission_skia_sys::ErrorKind::InvalidArgument);
    }
}
