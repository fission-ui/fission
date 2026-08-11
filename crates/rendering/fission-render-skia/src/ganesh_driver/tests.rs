use std::collections::VecDeque;
use std::ffi::c_void;
use std::num::NonZeroU32;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use fission_render::backend::{GraphicsBackendSession, ReadbackRequest, SurfaceMetrics};
use fission_render::capabilities::ColorFormat;
use fission_render::external_surface::ExternalSurfaceBindings;
use fission_render::frame::{
    DamageRegion, FrameId, FrameMetadata, FrameViewport, InteractiveFrame, ResourceEpoch,
    SemanticsEpoch,
};
use fission_render::resource::ResourceSnapshot;
use fission_render::surface::{
    LossKind, MemoryPressure, NativeWindowTarget, PhysicalSize, Recovery, ScaleFactor,
    SurfaceDescriptor, SurfaceId, SurfaceKind, SurfaceTarget, ThreadAffinity,
};
use fission_render::{LayoutRect, LayoutSize, RenderScene};
use fission_skia_sys::{NativeWindow, NativeWindowKind};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, AppKitDisplayHandle, AppKitWindowHandle,
    RawDisplayHandle, RawWindowHandle, UiKitDisplayHandle, UiKitWindowHandle, WaylandDisplayHandle,
    WaylandWindowHandle, Win32WindowHandle, WindowsDisplayHandle, XcbDisplayHandle,
    XcbWindowHandle, XlibDisplayHandle, XlibWindowHandle,
};

use super::*;
use crate::api::{ApiReadback, PixelRegion, RasterFrame, RasterRect};
use crate::ganesh_api::GaneshApi;
use crate::profile::new_paragraph_draw_data_registry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MockCall {
    CreateEngine,
    CreateContext(NativeWindowKind, u64),
    CreateSurface(NativeWindowKind, PhysicalSize),
    Resize(NativeWindowKind, PhysicalSize),
    Execute,
    Readback(PixelRegion),
    Present,
    Trim(MemoryPressure),
    ResourceCacheUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MockEvent {
    Call(MockCall),
    Drop(&'static str, u64),
}

#[derive(Clone, Default)]
struct MockApi {
    state: Arc<Mutex<MockState>>,
}

#[derive(Default)]
struct MockState {
    next_id: u64,
    events: Vec<MockEvent>,
    failures: VecDeque<(MockCall, ApiError)>,
    cache_usage: GaneshResourceCacheUsage,
}

impl MockApi {
    fn fail_next(&self, call: MockCall, error: ApiError) {
        self.state.lock().unwrap().failures.push_back((call, error));
    }

    fn events(&self) -> Vec<MockEvent> {
        self.state.lock().unwrap().events.clone()
    }

    fn call(&self, call: MockCall) -> Result<(), ApiError> {
        let mut state = self.state.lock().unwrap();
        state.events.push(MockEvent::Call(call));
        if state
            .failures
            .front()
            .is_some_and(|(expected, _)| *expected == call)
        {
            return Err(state.failures.pop_front().unwrap().1);
        }
        Ok(())
    }

    fn set_cache_usage(&self, resource_count: u64, resource_bytes: u64) {
        self.state.lock().unwrap().cache_usage = GaneshResourceCacheUsage {
            resource_count,
            resource_bytes,
        };
    }

    fn handle(&self, kind: &'static str) -> MockHandle {
        let id = {
            let mut state = self.state.lock().unwrap();
            state.next_id += 1;
            state.next_id
        };
        MockHandle {
            id,
            kind,
            state: Arc::clone(&self.state),
        }
    }
}

struct MockHandle {
    id: u64,
    kind: &'static str,
    state: Arc<Mutex<MockState>>,
}

impl Drop for MockHandle {
    fn drop(&mut self) {
        self.state
            .lock()
            .unwrap()
            .events
            .push(MockEvent::Drop(self.kind, self.id));
    }
}

impl GaneshApi for MockApi {
    type Engine = MockHandle;
    type Context = MockHandle;
    type Surface = MockHandle;

    fn create_engine(&self) -> Result<Self::Engine, ApiError> {
        self.call(MockCall::CreateEngine)?;
        Ok(self.handle("engine"))
    }

    fn create_context(
        &self,
        _engine: &Self::Engine,
        compatible_window: NativeWindow,
        resource_cache_limit_bytes: u64,
    ) -> Result<Self::Context, ApiError> {
        self.call(MockCall::CreateContext(
            compatible_window.kind(),
            resource_cache_limit_bytes,
        ))?;
        Ok(self.handle("context"))
    }

    fn create_surface(
        &self,
        _context: &Self::Context,
        window: NativeWindow,
        size: PhysicalSize,
    ) -> Result<Self::Surface, ApiError> {
        self.call(MockCall::CreateSurface(window.kind(), size))?;
        Ok(self.handle("surface"))
    }

    fn resize_surface(
        &self,
        _surface: &mut Self::Surface,
        window: NativeWindow,
        size: PhysicalSize,
    ) -> Result<(), ApiError> {
        self.call(MockCall::Resize(window.kind(), size))
    }

    fn execute_frame(
        &self,
        _surface: &mut Self::Surface,
        _frame: &RasterFrame,
    ) -> Result<(), ApiError> {
        self.call(MockCall::Execute)
    }

    fn read_pixels_rgba8888(
        &self,
        _surface: &mut Self::Surface,
        region: PixelRegion,
    ) -> Result<ApiReadback, ApiError> {
        self.call(MockCall::Readback(region))?;
        let row_bytes = usize::try_from(region.width)
            .unwrap()
            .checked_mul(4)
            .unwrap();
        let pixel_len = row_bytes
            .checked_mul(usize::try_from(region.height).unwrap())
            .unwrap();
        Ok(ApiReadback {
            size: region.size(),
            row_bytes,
            pixels: vec![0x5a; pixel_len],
        })
    }

    fn present(&self, _surface: &mut Self::Surface) -> Result<(), ApiError> {
        self.call(MockCall::Present)
    }

    fn trim_memory(
        &self,
        _context: &Self::Context,
        pressure: MemoryPressure,
    ) -> Result<(), ApiError> {
        self.call(MockCall::Trim(pressure))?;
        self.state.lock().unwrap().cache_usage = GaneshResourceCacheUsage::default();
        Ok(())
    }

    fn resource_cache_usage(
        &self,
        _context: &Self::Context,
    ) -> Result<GaneshResourceCacheUsage, ApiError> {
        self.call(MockCall::ResourceCacheUsage)?;
        Ok(self.state.lock().unwrap().cache_usage)
    }
}

struct FrameFixture {
    scene: RenderScene,
    metadata: FrameMetadata,
    resources: ResourceSnapshot,
    bindings: ExternalSurfaceBindings,
}

impl FrameFixture {
    fn empty(size: PhysicalSize, scale_factor: f64, frame_id: u64) -> Self {
        let scale_factor = ScaleFactor::new(scale_factor).unwrap();
        let resource_epoch = ResourceEpoch(17);
        Self {
            scene: RenderScene::new(LayoutRect::new(
                0.0,
                0.0,
                size.width as f32 / scale_factor.get() as f32,
                size.height as f32 / scale_factor.get() as f32,
            )),
            metadata: FrameMetadata {
                frame_id: FrameId(frame_id),
                viewport: FrameViewport {
                    logical_size: LayoutSize::new(
                        size.width as f32 / scale_factor.get() as f32,
                        size.height as f32 / scale_factor.get() as f32,
                    ),
                    physical_size: size,
                    scale_factor,
                },
                damage: DamageRegion::Full,
                resource_epoch,
                semantics_epoch: SemanticsEpoch(9),
            },
            resources: ResourceSnapshot::empty(resource_epoch),
            bindings: ExternalSurfaceBindings::new(),
        }
    }

    fn frame(&self) -> InteractiveFrame<'_> {
        InteractiveFrame::new(&self.scene, &self.metadata, &self.resources, &self.bindings)
    }
}

fn session(api: MockApi) -> GraphicsBackendSession<'static> {
    GraphicsBackendSession::new(
        GaneshDriver::try_new_with_gpu_cache_budget(
            api,
            new_paragraph_draw_data_registry(),
            DEFAULT_GANESH_GPU_CACHE_BYTES,
        )
        .unwrap(),
    )
    .unwrap()
}

fn pointer() -> NonNull<c_void> {
    NonNull::from(Box::leak(Box::new(0_u8))).cast()
}

fn descriptor(size: PhysicalSize) -> SurfaceDescriptor {
    descriptor_with_color(size, ColorFormat::Bgra8Srgb)
}

fn descriptor_with_color(size: PhysicalSize, color_format: ColorFormat) -> SurfaceDescriptor {
    descriptor_with_affinity(size, color_format, ThreadAffinity::CreatingThread)
}

fn descriptor_with_affinity(
    size: PhysicalSize,
    color_format: ColorFormat,
    thread_affinity: ThreadAffinity,
) -> SurfaceDescriptor {
    SurfaceDescriptor {
        id: SurfaceId(44),
        kind: SurfaceKind::NativeWindow,
        size,
        scale_factor: ScaleFactor::ONE,
        color_format,
        thread_affinity,
    }
}

fn target(
    size: PhysicalSize,
    display: RawDisplayHandle,
    window: RawWindowHandle,
) -> NativeWindowTarget {
    target_with_affinity(size, display, window, ThreadAffinity::CreatingThread)
}

fn target_with_affinity(
    size: PhysicalSize,
    display: RawDisplayHandle,
    window: RawWindowHandle,
    thread_affinity: ThreadAffinity,
) -> NativeWindowTarget {
    // SAFETY: these tests never pass their inert handles to the native bridge;
    // the leaked pointer tokens remain live for the complete target lifetime.
    unsafe {
        NativeWindowTarget::from_raw_handles(
            descriptor_with_affinity(size, ColorFormat::Bgra8Srgb, thread_affinity),
            display,
            window,
        )
        .unwrap()
    }
}

fn xlib_target(size: PhysicalSize) -> NativeWindowTarget {
    let display = RawDisplayHandle::Xlib(XlibDisplayHandle::new(Some(pointer()), 0));
    let window = RawWindowHandle::Xlib(XlibWindowHandle::new(71));
    target(size, display, window)
}

fn wayland_target(size: PhysicalSize) -> NativeWindowTarget {
    let display = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(pointer()));
    let window = RawWindowHandle::Wayland(WaylandWindowHandle::new(pointer()));
    target(size, display, window)
}

fn appkit_target(size: PhysicalSize) -> NativeWindowTarget {
    target_with_affinity(
        size,
        RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
        RawWindowHandle::AppKit(AppKitWindowHandle::new(pointer())),
        ThreadAffinity::MainThread,
    )
}

fn uikit_target(size: PhysicalSize) -> NativeWindowTarget {
    target_with_affinity(
        size,
        RawDisplayHandle::UiKit(UiKitDisplayHandle::new()),
        RawWindowHandle::UiKit(UiKitWindowHandle::new(pointer())),
        ThreadAffinity::MainThread,
    )
}

fn win32_target(size: PhysicalSize) -> NativeWindowTarget {
    target(
        size,
        RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
        RawWindowHandle::Win32(Win32WindowHandle::new(
            std::num::NonZeroIsize::new(71).unwrap(),
        )),
    )
}

fn android_target(size: PhysicalSize) -> NativeWindowTarget {
    target(
        size,
        RawDisplayHandle::Android(AndroidDisplayHandle::new()),
        RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(pointer())),
    )
}

#[test]
fn lowers_all_supported_native_window_routes_and_allows_unknown_visuals() {
    let size = PhysicalSize::new(640, 480);
    assert_eq!(
        lower_native_window(&wayland_target(size)).unwrap().kind(),
        NativeWindowKind::Wayland
    );
    assert_eq!(
        lower_native_window(&xlib_target(size)).unwrap().kind(),
        NativeWindowKind::Xlib
    );

    let display = RawDisplayHandle::Xcb(XcbDisplayHandle::new(Some(pointer()), 0));
    let window = RawWindowHandle::Xcb(XcbWindowHandle::new(NonZeroU32::new(81).unwrap()));
    let xcb = target(size, display, window);
    assert_eq!(
        lower_native_window(&xcb).unwrap().kind(),
        NativeWindowKind::Xcb
    );
    assert_eq!(
        lower_native_window(&appkit_target(size)).unwrap().kind(),
        NativeWindowKind::AppKit
    );
    assert_eq!(
        lower_native_window(&uikit_target(size)).unwrap().kind(),
        NativeWindowKind::UIKit
    );
    assert_eq!(
        lower_native_window(&win32_target(size)).unwrap().kind(),
        NativeWindowKind::Win32
    );
    assert_eq!(
        lower_native_window(&android_target(size)).unwrap().kind(),
        NativeWindowKind::Android
    );
}

#[test]
fn rejects_missing_or_mismatched_linux_handles_before_the_bridge() {
    let size = PhysicalSize::new(640, 480);
    let missing_display = target(
        size,
        RawDisplayHandle::Xlib(XlibDisplayHandle::new(None, 0)),
        RawWindowHandle::Xlib(XlibWindowHandle::new(1)),
    );
    assert!(lower_native_window(&missing_display)
        .unwrap_err()
        .contains("non-null Display"));

    let mismatch = target(
        size,
        RawDisplayHandle::Xlib(XlibDisplayHandle::new(Some(pointer()), 0)),
        RawWindowHandle::Wayland(WaylandWindowHandle::new(pointer())),
    );
    assert!(lower_native_window(&mismatch)
        .unwrap_err()
        .contains("matching Linux Wayland"));
}

#[test]
fn native_window_descriptors_enforce_platform_thread_affinity() {
    let size = PhysicalSize::new(640, 480);
    let mut linux_session = session(MockApi::default());
    let wrong_linux_affinity = target_with_affinity(
        size,
        RawDisplayHandle::Xlib(XlibDisplayHandle::new(Some(pointer()), 0)),
        RawWindowHandle::Xlib(XlibWindowHandle::new(71)),
        ThreadAffinity::MainThread,
    );
    assert_eq!(
        linux_session
            .attach(&wrong_linux_affinity)
            .unwrap_err()
            .code,
        "skia-ganesh-thread-affinity-invalid"
    );

    let mut apple_session = session(MockApi::default());
    let wrong_apple_affinity = target_with_affinity(
        size,
        RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
        RawWindowHandle::AppKit(AppKitWindowHandle::new(pointer())),
        ThreadAffinity::CreatingThread,
    );
    assert_eq!(
        apple_session
            .attach(&wrong_apple_affinity)
            .unwrap_err()
            .code,
        "skia-ganesh-thread-affinity-invalid"
    );

    let mut windows_session = session(MockApi::default());
    let wrong_windows_affinity = target_with_affinity(
        size,
        RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
        RawWindowHandle::Win32(Win32WindowHandle::new(
            std::num::NonZeroIsize::new(72).unwrap(),
        )),
        ThreadAffinity::MainThread,
    );
    assert_eq!(
        windows_session
            .attach(&wrong_windows_affinity)
            .unwrap_err()
            .code,
        "skia-ganesh-thread-affinity-invalid"
    );

    let mut android_session = session(MockApi::default());
    let wrong_android_affinity = target_with_affinity(
        size,
        RawDisplayHandle::Android(AndroidDisplayHandle::new()),
        RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(pointer())),
        ThreadAffinity::MainThread,
    );
    assert_eq!(
        android_session
            .attach(&wrong_android_affinity)
            .unwrap_err()
            .code,
        "skia-ganesh-thread-affinity-invalid"
    );
}

#[test]
fn constructor_reports_missing_ganesh_support_as_initialization_failure() {
    let api = MockApi::default();
    api.fail_next(
        MockCall::CreateEngine,
        ApiError::new(
            ApiErrorKind::Unsupported,
            "ganesh-features-missing",
            "create_ganesh_engine",
            "required feature bits are absent",
        ),
    );

    let error = match GaneshDriver::try_new(api, new_paragraph_draw_data_registry()) {
        Ok(_) => panic!("Ganesh construction unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(error.operation, BackendOperation::Initialize);
    assert_eq!(error.code, "skia-unsupported");
    assert!(error.message.contains("required feature bits are absent"));
}

#[test]
fn renders_and_presents_directly_then_resizes_the_swapchain() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    let initial = PhysicalSize::new(640, 480);
    session.attach(&xlib_target(initial)).unwrap();

    let frame = FrameFixture::empty(initial, 1.0, 5);
    assert_eq!(
        session.render(&frame.frame()).unwrap().frame_id,
        Some(FrameId(5))
    );
    assert_eq!(session.present().unwrap().frame_id, Some(FrameId(5)));

    let resized = PhysicalSize::new(800, 600);
    session
        .resize(SurfaceMetrics {
            size: resized,
            scale_factor: ScaleFactor::ONE,
        })
        .unwrap();

    assert_eq!(
        api.events()
            .into_iter()
            .filter_map(|event| match event {
                MockEvent::Call(call) => Some(call),
                MockEvent::Drop(_, _) => None,
            })
            .collect::<Vec<_>>(),
        vec![
            MockCall::CreateEngine,
            MockCall::CreateContext(NativeWindowKind::Xlib, DEFAULT_GANESH_GPU_CACHE_BYTES,),
            MockCall::CreateSurface(NativeWindowKind::Xlib, initial),
            MockCall::Execute,
            MockCall::Present,
            MockCall::Resize(NativeWindowKind::Xlib, resized),
        ]
    );
}

#[test]
fn zero_sized_frames_complete_without_executing_or_presenting_native_work() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    let size = PhysicalSize::ZERO;
    session.attach(&xlib_target(size)).unwrap();

    let frame = FrameFixture::empty(size, 1.0, 8);
    session.render(&frame.frame()).unwrap();
    session.present().unwrap();

    assert!(!api.events().iter().any(|event| matches!(
        event,
        MockEvent::Call(MockCall::Execute | MockCall::Present)
    )));
}

#[test]
fn requires_present_before_another_render_or_resize() {
    let api = MockApi::default();
    let mut session = session(api);
    let size = PhysicalSize::new(320, 240);
    session.attach(&xlib_target(size)).unwrap();
    let first = FrameFixture::empty(size, 1.0, 1);
    session.render(&first.frame()).unwrap();

    let second = FrameFixture::empty(size, 1.0, 2);
    assert_eq!(
        session.render(&second.frame()).unwrap_err().code,
        "skia-ganesh-present-pending"
    );
    assert_eq!(
        session
            .resize(SurfaceMetrics {
                size,
                scale_factor: ScaleFactor::ONE,
            })
            .unwrap_err()
            .code,
        "skia-ganesh-present-pending"
    );
}

#[test]
fn suspend_resume_can_recreate_for_a_different_linux_wsi_kind() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    let size = PhysicalSize::new(640, 480);
    session.attach(&xlib_target(size)).unwrap();
    session.suspend().unwrap();
    session.resume(&wayland_target(size)).unwrap();

    let calls = api
        .events()
        .into_iter()
        .filter_map(|event| match event {
            MockEvent::Call(call) => Some(call),
            MockEvent::Drop(_, _) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls,
        vec![
            MockCall::CreateEngine,
            MockCall::CreateContext(NativeWindowKind::Xlib, DEFAULT_GANESH_GPU_CACHE_BYTES,),
            MockCall::CreateSurface(NativeWindowKind::Xlib, size),
            MockCall::CreateContext(NativeWindowKind::Wayland, DEFAULT_GANESH_GPU_CACHE_BYTES,),
            MockCall::CreateSurface(NativeWindowKind::Wayland, size),
        ]
    );
}

#[test]
fn surface_and_device_recovery_rebuild_the_required_native_owners() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    let size = PhysicalSize::new(640, 480);
    session.attach(&xlib_target(size)).unwrap();

    assert_eq!(
        session.recover(LossKind::Surface).unwrap(),
        Recovery::Reattached
    );
    assert_eq!(
        session.recover(LossKind::Device).unwrap(),
        Recovery::DeviceRecreated
    );
    session.trim_memory(MemoryPressure::Critical).unwrap();

    let calls = api
        .events()
        .into_iter()
        .filter_map(|event| match event {
            MockEvent::Call(call) => Some(call),
            MockEvent::Drop(_, _) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls
            .iter()
            .filter(|call| matches!(call, MockCall::CreateEngine))
            .count(),
        2
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| matches!(call, MockCall::CreateContext(..)))
            .count(),
        2
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| matches!(call, MockCall::CreateSurface(_, _)))
            .count(),
        3
    );
    assert_eq!(
        &calls[calls.len() - 3..],
        &[
            MockCall::ResourceCacheUsage,
            MockCall::Trim(MemoryPressure::Critical),
            MockCall::ResourceCacheUsage,
        ]
    );
}

#[test]
fn gpu_cache_policy_is_frozen_reported_and_trimmed_by_the_ganesh_context() {
    assert_eq!(
        configured_gpu_cache_bytes_from(None),
        DEFAULT_GANESH_GPU_CACHE_BYTES
    );
    assert_eq!(
        configured_gpu_cache_bytes_from(Some("not-a-number")),
        DEFAULT_GANESH_GPU_CACHE_BYTES
    );
    assert_eq!(configured_gpu_cache_bytes_from(Some("0")), 0);

    let api = MockApi::default();
    let mut session = session(api.clone());
    let size = PhysicalSize::new(640, 480);
    session.attach(&xlib_target(size)).unwrap();
    api.set_cache_usage(7, 12 * 1024 * 1024);

    let diagnostics = session.diagnostics();
    let cache = diagnostics
        .caches
        .iter()
        .find(|cache| cache.name == GPU_CACHE_NAME)
        .expect("Ganesh GPU cache diagnostics");
    assert_eq!(cache.entries, 7);
    assert_eq!(cache.used_bytes, 12 * 1024 * 1024);
    assert_eq!(cache.budget_bytes, Some(DEFAULT_GANESH_GPU_CACHE_BYTES));
    assert_eq!(cache.evictions, 0);

    session.trim_memory(MemoryPressure::Moderate).unwrap();
    let diagnostics = session.diagnostics();
    let cache = diagnostics
        .caches
        .iter()
        .find(|cache| cache.name == GPU_CACHE_NAME)
        .expect("Ganesh GPU cache diagnostics after trim");
    assert_eq!(cache.entries, 0);
    assert_eq!(cache.used_bytes, 0);
    assert_eq!(cache.evictions, 7);
}

#[test]
fn readback_is_allowed_only_between_render_and_present() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    let size = PhysicalSize::new(64, 32);
    session.attach(&xlib_target(size)).unwrap();

    let request = ReadbackRequest {
        region: None,
        color_format: ColorFormat::Rgba8Srgb,
    };
    assert_eq!(
        session.readback(request).unwrap_err().code,
        "skia-ganesh-readback-outside-frame"
    );

    let frame = FrameFixture::empty(size, 1.0, 91);
    session.render(&frame.frame()).unwrap();
    let readback = session.readback(request).unwrap();
    assert_eq!(readback.size, size);
    assert_eq!(readback.color_format, ColorFormat::Rgba8Srgb);
    assert_eq!(readback.row_bytes, 64 * 4);
    assert_eq!(readback.pixels, vec![0x5a; 64 * 32 * 4]);
    session.present().unwrap();

    assert_eq!(
        session.readback(request).unwrap_err().code,
        "skia-ganesh-readback-outside-frame"
    );
    let calls = api
        .events()
        .into_iter()
        .filter_map(|event| match event {
            MockEvent::Call(call) => Some(call),
            MockEvent::Drop(_, _) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls,
        vec![
            MockCall::CreateEngine,
            MockCall::CreateContext(NativeWindowKind::Xlib, DEFAULT_GANESH_GPU_CACHE_BYTES,),
            MockCall::CreateSurface(NativeWindowKind::Xlib, size),
            MockCall::Execute,
            MockCall::Readback(PixelRegion::full(size)),
            MockCall::Present,
        ]
    );
}

#[test]
fn readback_scales_and_rounds_logical_regions_outward() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    let size = PhysicalSize::new(8, 6);
    let mut target = xlib_target(size);
    let mut descriptor = target.descriptor().clone();
    descriptor.scale_factor = ScaleFactor::new(2.0).unwrap();
    // SAFETY: this replacement uses the same inert, leaked Xlib handle pair
    // as the original test target and never reaches the native bridge.
    target = unsafe {
        NativeWindowTarget::from_raw_handles(
            descriptor,
            target.raw_display_handle(),
            target.raw_window_handle(),
        )
        .unwrap()
    };
    session.attach(&target).unwrap();
    let frame = FrameFixture::empty(size, 2.0, 92);
    session.render(&frame.frame()).unwrap();

    let readback = session
        .readback(ReadbackRequest {
            region: Some(LayoutRect::new(0.25, 0.5, 1.5, 1.0)),
            color_format: ColorFormat::Rgba8Srgb,
        })
        .unwrap();
    assert_eq!(readback.size, PhysicalSize::new(4, 2));
    assert_eq!(readback.row_bytes, 16);
    assert!(api
        .events()
        .contains(&MockEvent::Call(MockCall::Readback(PixelRegion {
            x: 0,
            y: 1,
            width: 4,
            height: 2,
        }))));
}

#[test]
fn readback_rejects_invalid_regions_and_non_rgba_output() {
    let api = MockApi::default();
    let mut session = session(api);
    let size = PhysicalSize::new(64, 32);
    session.attach(&xlib_target(size)).unwrap();
    let frame = FrameFixture::empty(size, 1.0, 93);
    session.render(&frame.frame()).unwrap();

    assert_eq!(
        session
            .readback(ReadbackRequest {
                region: None,
                color_format: ColorFormat::Bgra8Srgb,
            })
            .unwrap_err()
            .code,
        "skia-ganesh-readback-color-format-unsupported"
    );
    assert_eq!(
        session
            .readback(ReadbackRequest {
                region: Some(LayoutRect::new(63.0, 0.0, 2.0, 1.0)),
                color_format: ColorFormat::Rgba8Srgb,
            })
            .unwrap_err()
            .code,
        "skia-ganesh-readback-region-invalid"
    );
}

#[derive(Debug)]
struct ImpostorNativeTarget {
    descriptor: SurfaceDescriptor,
}

impl SurfaceTarget for ImpostorNativeTarget {
    fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn rejects_native_descriptors_without_the_typed_handle_carrier() {
    let api = MockApi::default();
    let mut session = session(api);
    let target = ImpostorNativeTarget {
        descriptor: descriptor(PhysicalSize::new(64, 64)),
    };

    assert_eq!(
        session.attach(&target).unwrap_err().code,
        "skia-ganesh-target-type-invalid"
    );
}

#[test]
fn target_attachment_still_requires_bgra_presentation() {
    let api = MockApi::default();
    let mut session = session(api);
    let size = PhysicalSize::new(64, 64);
    let display = RawDisplayHandle::Xlib(XlibDisplayHandle::new(Some(pointer()), 0));
    let window = RawWindowHandle::Xlib(XlibWindowHandle::new(71));
    // SAFETY: this inert target is rejected on its descriptor before the raw
    // handles can reach the native bridge.
    let target = unsafe {
        NativeWindowTarget::from_raw_handles(
            descriptor_with_color(size, ColorFormat::Rgba8Srgb),
            display,
            window,
        )
        .unwrap()
    };

    assert_eq!(
        session.attach(&target).unwrap_err().code,
        "skia-ganesh-color-format-unsupported"
    );
}

#[test]
fn picture_recording_seam_defaults_to_uncached_lowering_in_driver_tests() {
    let api = MockApi::default();
    assert!(api
        .record_picture(
            RasterRect {
                left: 0.0,
                top: 0.0,
                right: 10.0,
                bottom: 10.0,
            },
            &RasterFrame { commands: vec![] },
        )
        .unwrap()
        .is_none());
}
