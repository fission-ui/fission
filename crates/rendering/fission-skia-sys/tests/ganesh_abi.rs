#![cfg(feature = "test-shim")]

use std::ffi::c_void;
use std::num::{NonZeroU32, NonZeroU64};
use std::ptr::NonNull;
use std::sync::Mutex;

use fission_skia_sys::ffi;
use fission_skia_sys::{
    Color, Engine, ErrorKind, Frame, FrameOp, GaneshContext, GaneshSurface, MemoryPressure,
    NativeWindow, NativeWindowKind, ABI_VERSION,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn ganesh_surface_enforces_zero_size_and_present_ordering() {
    let _guard = TEST_LOCK.lock().unwrap();
    assert_eq!(live_counts(), ffi::TestCounts::default());

    let engine = Engine::new().expect("test engine");
    let required = ffi::FEATURE_GANESH | ffi::FEATURE_VULKAN | ffi::FEATURE_NATIVE_PRESENTATION;
    assert_eq!(engine.build_info().feature_bits & required, required);

    // The context descriptor is a synchronous presentation-support probe. The
    // native owners may disappear after context creation.
    let context = {
        let mut display = Box::new(1_u8);
        let mut surface = Box::new(2_u8);
        let window = unsafe { NativeWindow::wayland(pointer(&mut display), pointer(&mut surface)) };
        assert_eq!(window.kind(), NativeWindowKind::Wayland);
        GaneshContext::new_vulkan(&engine, window).expect("Ganesh context")
    };
    context
        .trim_memory(MemoryPressure::Moderate)
        .expect("memory trim");

    let mut display_a = Box::new(3_u8);
    let mut window_a = Box::new(4_u8);
    let attachment_a =
        unsafe { NativeWindow::wayland(pointer(&mut display_a), pointer(&mut window_a)) };
    let mut surface =
        GaneshSurface::new(&context, attachment_a, 0, 720).expect("zero-sized surface attachment");
    assert!(surface.is_zero_sized());
    assert_eq!(surface.width(), 0);
    assert_eq!(surface.height(), 720);
    assert_eq!(
        surface
            .execute_frame(&Frame::default())
            .expect_err("zero-sized render")
            .kind,
        ErrorKind::InvalidState
    );
    assert_eq!(
        surface.present().expect_err("zero-sized present").kind,
        ErrorKind::InvalidState
    );

    let mut display_b = Box::new(5_u8);
    let mut window_b = Box::new(6_u8);
    let attachment_b =
        unsafe { NativeWindow::wayland(pointer(&mut display_b), pointer(&mut window_b)) };
    surface
        .resize(attachment_b, 4, 3)
        .expect("fresh same-WSI attachment");
    drop((display_a, window_a));
    assert!(!surface.is_zero_sized());
    assert_eq!(
        surface.present().expect_err("present before render").kind,
        ErrorKind::InvalidState
    );

    let frame = Frame::new([FrameOp::Clear(Color::rgba(0.1, 0.2, 0.3, 1.0))]);
    surface.execute_frame(&frame).expect("ready frame");
    assert_eq!(
        surface
            .execute_frame(&frame)
            .expect_err("second render before present")
            .kind,
        ErrorKind::InvalidState
    );
    assert_eq!(
        surface
            .resize(attachment_b, 8, 6)
            .expect_err("resize before present")
            .kind,
        ErrorKind::InvalidState
    );
    surface.present().expect("present ready frame");
    surface
        .resize(attachment_b, 8, 6)
        .expect("resize after present");

    // Safe children retain both ancestors until the surface is destroyed.
    drop(engine);
    drop(context);
    assert_eq!(live_counts(), counts(1, 1, 1));
    drop(surface);
    drop((display_b, window_b));
    assert_eq!(live_counts(), ffi::TestCounts::default());
}

#[test]
fn native_window_kinds_allow_optional_visuals_and_reject_mismatches() {
    let _guard = TEST_LOCK.lock().unwrap();
    assert_eq!(std::mem::size_of::<ffi::NativeWindow>(), 32);
    let engine = Engine::new().expect("test engine");
    let mut display = Box::new(7_u8);
    let xlib =
        unsafe { NativeWindow::xlib(pointer(&mut display), NonZeroU64::new(41).unwrap(), 0) };
    assert_eq!(xlib.kind(), NativeWindowKind::Xlib);
    let context = GaneshContext::new_vulkan(&engine, xlib).expect("Xlib context");
    let mut connection = Box::new(8_u8);
    let xcb =
        unsafe { NativeWindow::xcb(pointer(&mut connection), NonZeroU32::new(42).unwrap(), 0) };
    assert_eq!(xcb.kind(), NativeWindowKind::Xcb);
    assert_eq!(
        GaneshSurface::new(&context, xcb, 1, 1)
            .err()
            .expect("WSI kind mismatch")
            .kind,
        ErrorKind::InvalidArgument
    );
    drop(context);
    drop(engine);
    assert_eq!(live_counts(), ffi::TestCounts::default());
}

#[test]
fn raw_abi_rejects_malformed_descriptors_and_invalid_presentation_handles() {
    let _guard = TEST_LOCK.lock().unwrap();
    let config = ffi::EngineConfig {
        struct_size: std::mem::size_of::<ffi::EngineConfig>() as u32,
        expected_abi_version: ABI_VERSION,
        required_feature_bits: ffi::FEATURE_GANESH
            | ffi::FEATURE_VULKAN
            | ffi::FEATURE_NATIVE_PRESENTATION,
    };
    let mut engine = 0;
    let mut error = ffi::Error::default();
    let status = unsafe { ffi::fission_skia_engine_create(&config, &mut engine, &mut error) };
    assert_eq!(status, ffi::STATUS_OK);

    let invalid = ffi::NativeWindow {
        struct_size: std::mem::size_of::<ffi::NativeWindow>() as u32,
        kind: ffi::NATIVE_WINDOW_XLIB,
        display: 0,
        window: 1,
        visual_id: 0,
    };
    let mut context = 0;
    let status = unsafe {
        ffi::fission_skia_context_create_ganesh_vulkan(engine, &invalid, &mut context, &mut error)
    };
    assert_eq!(status, ffi::STATUS_INVALID_ARGUMENT);
    assert_eq!(context, 0);

    let status = unsafe { ffi::fission_skia_surface_present(u64::MAX, &mut error) };
    assert_eq!(status, ffi::STATUS_INVALID_HANDLE);
    assert_eq!(error.code, ffi::STATUS_INVALID_HANDLE);

    let status = unsafe { ffi::fission_skia_engine_destroy(engine, &mut error) };
    assert_eq!(status, ffi::STATUS_OK);
    assert_eq!(live_counts(), ffi::TestCounts::default());
}

fn pointer(value: &mut Box<u8>) -> NonNull<c_void> {
    NonNull::from(value.as_mut()).cast()
}

fn live_counts() -> ffi::TestCounts {
    let mut counts = ffi::TestCounts::default();
    let mut error = ffi::Error::default();
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
