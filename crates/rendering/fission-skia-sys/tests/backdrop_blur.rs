#![cfg(feature = "test-shim")]

use fission_skia_sys::ffi;
use fission_skia_sys::{
    Color, Context, Engine, ErrorKind, Frame, FrameOp, Paint, PixelRect, RasterSurface, Rect,
};

#[test]
fn backdrop_blur_is_atomic_and_respects_its_rounded_clip() {
    let engine = Engine::new().expect("test engine");
    assert_ne!(
        engine.build_info().feature_bits & ffi::FEATURE_BACKDROP_BLUR,
        0
    );
    let context = Context::new_raster(&engine).expect("raster context");
    let mut surface = RasterSurface::new(&context, 7, 5).expect("raster surface");

    surface
        .execute_frame(&Frame::new([
            FrameOp::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)),
            FrameOp::FillRect {
                rect: Rect::new(0.0, 0.0, 3.0, 5.0),
                radius: 0.0,
                paint: Paint::solid(Color::rgba(1.0, 1.0, 1.0, 1.0)),
            },
            FrameOp::BackdropBlur {
                bounds: Rect::new(0.0, 0.0, 7.0, 5.0),
                corner_radius: 2.0,
                sigma: 1.0,
            },
        ]))
        .expect("atomic backdrop blur frame");

    let pixels = surface
        .read_pixels_rgba8888(Some(PixelRect::new(0, 0, 7, 5)))
        .expect("readback");
    assert_eq!(pixel(&pixels, 7, 0, 0), [255, 255, 255, 255]);
    assert_eq!(pixel(&pixels, 7, 6, 0), [0, 0, 0, 255]);
    let left_of_edge = pixel(&pixels, 7, 2, 2);
    let right_of_edge = pixel(&pixels, 7, 3, 2);
    assert!(left_of_edge[0] < 255 && left_of_edge[0] > 0);
    assert!(right_of_edge[0] > 0 && right_of_edge[0] < 255);
    assert_eq!(left_of_edge[0], left_of_edge[1]);
    assert_eq!(left_of_edge[1], left_of_edge[2]);
    assert_eq!(right_of_edge[0], right_of_edge[1]);
    assert_eq!(right_of_edge[1], right_of_edge[2]);
}

#[test]
fn zero_sigma_is_an_exact_no_op_and_invalid_values_fail_closed() {
    let engine = Engine::new().expect("test engine");
    let context = Context::new_raster(&engine).expect("raster context");
    let mut surface = RasterSurface::new(&context, 2, 1).expect("raster surface");
    let setup = Frame::new([
        FrameOp::Clear(Color::rgba(1.0, 0.0, 0.0, 1.0)),
        FrameOp::FillRect {
            rect: Rect::new(1.0, 0.0, 1.0, 1.0),
            radius: 0.0,
            paint: Paint::solid(Color::rgba(0.0, 0.0, 1.0, 1.0)),
        },
    ]);
    surface.execute_frame(&setup).expect("setup frame");
    let before = surface
        .read_pixels_rgba8888(Some(PixelRect::new(0, 0, 2, 1)))
        .expect("readback before identity blur");

    surface
        .execute_frame(&Frame::new([FrameOp::BackdropBlur {
            bounds: Rect::new(0.0, 0.0, 2.0, 1.0),
            corner_radius: 0.5,
            sigma: 0.0,
        }]))
        .expect("zero-sigma identity blur");
    let after = surface
        .read_pixels_rgba8888(Some(PixelRect::new(0, 0, 2, 1)))
        .expect("readback after identity blur");
    assert_eq!(after, before);

    for (bounds, corner_radius, sigma) in [
        (Rect::new(0.0, 0.0, -1.0, 1.0), 0.0, 1.0),
        (Rect::new(0.0, 0.0, 1.0, 1.0), -1.0, 1.0),
        (Rect::new(0.0, 0.0, 1.0, 1.0), 0.0, f32::NAN),
        (Rect::new(f32::MAX, 0.0, f32::MAX, 1.0), 0.0, 1.0),
    ] {
        let error = surface
            .execute_frame(&Frame::new([FrameOp::BackdropBlur {
                bounds,
                corner_radius,
                sigma,
            }]))
            .expect_err("invalid backdrop blur must fail closed");
        assert_eq!(error.kind, ErrorKind::InvalidArgument);
    }
}

fn pixel(bytes: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    let offset = (y * width + x) * 4;
    bytes[offset..offset + 4].try_into().expect("RGBA pixel")
}
