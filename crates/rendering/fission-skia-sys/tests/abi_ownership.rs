#![cfg(feature = "test-shim")]

use std::sync::mpsc;

use fission_skia_sys::ffi;
use fission_skia_sys::{
    Affine, Color, Context, Engine, FillRule, Frame, FrameOp, MemoryPressure, Paint, Path,
    PathCommand, PixelRect, RasterSurface, Rect, ABI_VERSION, SKIA_REVISION,
};

#[test]
fn abi_ownership_errors_and_raster_readback_are_coherent() {
    assert_eq!(live_counts(), ffi::TestCounts::default());

    let engine = Engine::new().expect("test engine");
    assert_eq!(engine.build_info().abi_version, ABI_VERSION);
    assert_eq!(engine.build_info().skia_revision, SKIA_REVISION);
    assert_eq!(engine.build_info().profile, "test-shim");
    assert_ne!(engine.build_info().feature_bits & ffi::FEATURE_TEST_SHIM, 0);
    assert_ne!(
        engine.build_info().feature_bits & ffi::FEATURE_PAINT_STATE,
        0
    );
    assert_ne!(
        engine.build_info().feature_bits & ffi::FEATURE_BACKDROP_BLUR,
        0
    );
    assert_ne!(
        engine.build_info().feature_bits & ffi::FEATURE_SVG_DOCUMENT,
        0
    );
    assert_ne!(
        engine.build_info().feature_bits & ffi::FEATURE_RETAINED_PICTURE,
        0
    );

    let context = Context::new_raster(&engine).expect("raster context");
    context
        .trim_memory(MemoryPressure::Moderate)
        .expect("memory pressure notification");
    let mut surface = RasterSurface::new(&context, 4, 3).expect("raster surface");
    assert_eq!(live_counts(), counts(1, 1, 1));

    surface
        .execute_frame(&Frame::new(vec![
            FrameOp::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)),
            FrameOp::FillRect {
                rect: Rect::new(1.0, 1.0, 2.0, 1.0),
                radius: 0.0,
                paint: Paint::solid(Color::rgba(1.0, 0.0, 0.0, 1.0)),
            },
            FrameOp::Save,
            FrameOp::ConcatAffine(Affine::translation(1.0, 0.0)),
            FrameOp::FillPath {
                path: Path::new(
                    FillRule::EvenOdd,
                    vec![
                        PathCommand::MoveTo { x: 0.0, y: 0.0 },
                        PathCommand::LineTo { x: 1.0, y: 0.0 },
                        PathCommand::LineTo { x: 1.0, y: 1.0 },
                        PathCommand::Close,
                    ],
                ),
                paint: Paint::solid(Color::rgba(0.0, 0.0, 1.0, 1.0)),
            },
            FrameOp::Restore,
        ]))
        .expect("basic frame");
    let row = surface
        .read_pixels_rgba8888(Some(PixelRect::new(1, 1, 2, 1)))
        .expect("readback");
    assert_eq!(row, vec![255, 0, 0, 255, 255, 0, 0, 255]);

    // Safe ownership retains parents until their final child is gone.
    drop(engine);
    drop(context);
    assert_eq!(live_counts(), counts(1, 1, 1));
    drop(surface);
    assert_eq!(live_counts(), ffi::TestCounts::default());

    raw_invalid_and_wrong_thread_calls_return_structured_errors();
}

fn raw_invalid_and_wrong_thread_calls_return_structured_errors() {
    let mut error = ffi::Error::default();
    // SAFETY: the raw call receives a deliberately invalid numeric handle and
    // a valid diagnostic output. Invalid handles are part of the ABI contract.
    let status = unsafe { ffi::fission_skia_engine_destroy(u64::MAX, &mut error) };
    assert_eq!(status, ffi::STATUS_INVALID_HANDLE);
    assert_eq!(error.code, ffi::STATUS_INVALID_HANDLE);
    assert_ne!(error.sequence, 0);

    let config = ffi::EngineConfig {
        struct_size: std::mem::size_of::<ffi::EngineConfig>() as u32,
        expected_abi_version: ABI_VERSION,
        required_feature_bits: 0,
    };
    let mut engine = 0;
    let mut error = ffi::Error::default();
    // SAFETY: all pointers reference initialized storage for the call.
    let status = unsafe { ffi::fission_skia_engine_create(&config, &mut engine, &mut error) };
    assert_eq!(status, ffi::STATUS_OK);

    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut context = 0;
        let mut error = ffi::Error::default();
        // SAFETY: the numeric handle is intentionally exercised from the wrong
        // thread; the bridge must reject it without dereferencing caller memory.
        let status =
            unsafe { ffi::fission_skia_context_create_raster(engine, &mut context, &mut error) };
        sender.send((status, error.code, context)).unwrap();
    })
    .join()
    .unwrap();
    assert_eq!(
        receiver.recv().unwrap(),
        (ffi::STATUS_WRONG_THREAD, ffi::STATUS_WRONG_THREAD, 0)
    );

    let mut error = ffi::Error::default();
    // SAFETY: the handle is destroyed once, on its creating thread.
    let status = unsafe { ffi::fission_skia_engine_destroy(engine, &mut error) };
    assert_eq!(status, ffi::STATUS_OK);
    assert_eq!(live_counts(), ffi::TestCounts::default());
}

fn live_counts() -> ffi::TestCounts {
    let mut counts = ffi::TestCounts::default();
    let mut error = ffi::Error::default();
    // SAFETY: both outputs are initialized and valid for the call.
    let status = unsafe { ffi::fission_skia_test_live_counts(&mut counts, &mut error) };
    assert_eq!(status, ffi::STATUS_OK);
    counts
}

fn counts(engines: u64, contexts: u64, surfaces: u64) -> ffi::TestCounts {
    ffi::TestCounts {
        engines,
        contexts,
        surfaces,
        images: 0,
        svg_documents: 0,
        pictures: 0,
    }
}
