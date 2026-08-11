use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

use fission_render::backend::{
    BackendOperation, GraphicsBackendSession, ReadbackRequest, SurfaceMetrics,
};
use fission_render::capabilities::ColorFormat;
use fission_render::external_surface::ExternalSurfaceBindings;
use fission_render::frame::{
    DamageRegion, FrameId, FrameMetadata, FrameViewport, InteractiveFrame, ResourceEpoch,
    SemanticsEpoch,
};
use fission_render::resource::ResourceSnapshot;
use fission_render::surface::{
    LossKind, MemoryPressure, PhysicalSize, Recovery, ScaleFactor, SurfaceDescriptor, SurfaceId,
    SurfaceKind, SurfaceTarget, ThreadAffinity,
};
use fission_render::{LayoutRect, LayoutSize, RenderScene};

use super::*;
use crate::api::{ApiReadback, RasterFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MockCall {
    CreateEngine,
    CreateContext,
    CreateSurface,
    Execute,
    Readback,
    TrimMemory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MockEvent {
    Called(MockCall),
    Dropped(&'static str, u64),
}

#[derive(Clone)]
struct MockApi {
    state: Arc<Mutex<MockState>>,
}

struct MockState {
    next_id: u64,
    events: Vec<MockEvent>,
    failures: VecDeque<(MockCall, ApiError)>,
    last_frame: Option<RasterFrame>,
}

impl Default for MockApi {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState {
                next_id: 1,
                events: Vec::new(),
                failures: VecDeque::new(),
                last_frame: None,
            })),
        }
    }
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
        state.events.push(MockEvent::Called(call));
        if state
            .failures
            .front()
            .is_some_and(|(expected, _)| *expected == call)
        {
            return Err(state.failures.pop_front().unwrap().1);
        }
        Ok(())
    }

    fn handle(&self, kind: &'static str) -> MockHandle {
        let id = {
            let mut state = self.state.lock().unwrap();
            let id = state.next_id;
            state.next_id += 1;
            id
        };
        MockHandle {
            id,
            kind,
            state: self.state.clone(),
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
            .push(MockEvent::Dropped(self.kind, self.id));
    }
}

impl SkiaApi for MockApi {
    type Engine = MockHandle;
    type Context = MockHandle;
    type Surface = MockHandle;

    fn create_engine(&self) -> Result<Self::Engine, ApiError> {
        self.call(MockCall::CreateEngine)?;
        Ok(self.handle("engine"))
    }

    fn create_raster_context(&self, _engine: &Self::Engine) -> Result<Self::Context, ApiError> {
        self.call(MockCall::CreateContext)?;
        Ok(self.handle("context"))
    }

    fn create_raster_surface(
        &self,
        _context: &Self::Context,
        _size: PhysicalSize,
    ) -> Result<Self::Surface, ApiError> {
        self.call(MockCall::CreateSurface)?;
        Ok(self.handle("surface"))
    }

    fn execute_frame(
        &self,
        _context: &mut Self::Context,
        _surface: &mut Self::Surface,
        frame: &RasterFrame,
    ) -> Result<(), ApiError> {
        self.call(MockCall::Execute)?;
        self.state.lock().unwrap().last_frame = Some(frame.clone());
        Ok(())
    }

    fn read_pixels_rgba8888(
        &self,
        _context: &mut Self::Context,
        _surface: &mut Self::Surface,
        region: PixelRegion,
    ) -> Result<ApiReadback, ApiError> {
        self.call(MockCall::Readback)?;
        let row_bytes = usize::try_from(region.width).unwrap() * 4;
        let len = row_bytes * usize::try_from(region.height).unwrap();
        Ok(ApiReadback {
            size: region.size(),
            row_bytes,
            pixels: vec![0; len],
        })
    }

    fn trim_memory(
        &self,
        _context: &mut Self::Context,
        _pressure: MemoryPressure,
    ) -> Result<(), ApiError> {
        self.call(MockCall::TrimMemory)
    }
}

#[derive(Debug)]
struct TestTarget {
    descriptor: SurfaceDescriptor,
}

impl TestTarget {
    fn headless(width: u32, height: u32, scale_factor: f64) -> Self {
        Self {
            descriptor: SurfaceDescriptor {
                id: SurfaceId(1),
                kind: SurfaceKind::Headless,
                size: PhysicalSize::new(width, height),
                scale_factor: ScaleFactor::new(scale_factor).unwrap(),
                color_format: ColorFormat::Rgba8Srgb,
                thread_affinity: ThreadAffinity::CreatingThread,
            },
        }
    }
}

impl SurfaceTarget for TestTarget {
    fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct FrameFixture {
    scene: RenderScene,
    metadata: FrameMetadata,
    resources: ResourceSnapshot,
    bindings: ExternalSurfaceBindings,
}

impl FrameFixture {
    fn empty(width: u32, height: u32, scale_factor: f64, frame_id: u64) -> Self {
        let scale = ScaleFactor::new(scale_factor).unwrap();
        let resource_epoch = ResourceEpoch(3);
        Self {
            scene: RenderScene::new(LayoutRect::new(
                0.0,
                0.0,
                width as f32 / scale_factor as f32,
                height as f32 / scale_factor as f32,
            )),
            metadata: FrameMetadata {
                frame_id: FrameId(frame_id),
                viewport: FrameViewport {
                    logical_size: LayoutSize::new(
                        width as f32 / scale_factor as f32,
                        height as f32 / scale_factor as f32,
                    ),
                    physical_size: PhysicalSize::new(width, height),
                    scale_factor: scale,
                },
                damage: DamageRegion::Full,
                resource_epoch,
                semantics_epoch: SemanticsEpoch(7),
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
    GraphicsBackendSession::new(RasterDriver::try_new(api).unwrap()).unwrap()
}

fn injected_error(kind: ApiErrorKind, operation: &str) -> ApiError {
    ApiError::new(kind, "mock-error", operation, "injected failure")
}

#[test]
fn detach_destroys_surface_before_context_before_engine() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    session.attach(&TestTarget::headless(64, 32, 1.0)).unwrap();

    session.detach().unwrap();

    let drops = api
        .events()
        .into_iter()
        .filter(|event| matches!(event, MockEvent::Dropped(..)))
        .collect::<Vec<_>>();
    assert_eq!(
        drops,
        vec![
            MockEvent::Dropped("surface", 3),
            MockEvent::Dropped("context", 2),
            MockEvent::Dropped("engine", 1),
        ]
    );
}

#[test]
fn suspend_releases_only_surface_and_resume_recreates_it() {
    let api = MockApi::default();
    let target = TestTarget::headless(64, 32, 1.0);
    let mut session = session(api.clone());
    session.attach(&target).unwrap();

    session.suspend().unwrap();
    session.resume(&target).unwrap();

    let events = api.events();
    assert!(events.contains(&MockEvent::Dropped("surface", 3)));
    assert_eq!(
        events
            .iter()
            .filter(|event| *event == &MockEvent::Called(MockCall::CreateEngine))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| *event == &MockEvent::Called(MockCall::CreateSurface))
            .count(),
        2
    );
}

#[test]
fn empty_interactive_frame_executes_and_reads_back_through_the_api_boundary() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    session.attach(&TestTarget::headless(8, 4, 1.0)).unwrap();
    let fixture = FrameFixture::empty(8, 4, 1.0, 42);

    let render = session.render(&fixture.frame()).unwrap();
    let present = session.present().unwrap();
    let readback = session
        .readback(ReadbackRequest {
            region: None,
            color_format: ColorFormat::Rgba8Srgb,
        })
        .unwrap();

    assert_eq!(render.frame_id, Some(FrameId(42)));
    assert_eq!(present.frame_id, Some(FrameId(42)));
    assert_eq!(readback.size, PhysicalSize::new(8, 4));
    assert_eq!(readback.row_bytes, 32);
    assert_eq!(readback.pixels.len(), 128);
    assert_eq!(
        api.state
            .lock()
            .unwrap()
            .last_frame
            .as_ref()
            .unwrap()
            .commands
            .len(),
        1
    );
}

#[test]
fn resize_constructs_replacement_before_destroying_old_surface() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    session.attach(&TestTarget::headless(10, 10, 1.0)).unwrap();

    session
        .resize(SurfaceMetrics {
            size: PhysicalSize::new(20, 20),
            scale_factor: ScaleFactor::ONE,
        })
        .unwrap();

    let events = api.events();
    let create_second = events
        .iter()
        .rposition(|event| event == &MockEvent::Called(MockCall::CreateSurface))
        .unwrap();
    let drop_first = events
        .iter()
        .position(|event| event == &MockEvent::Dropped("surface", 3))
        .unwrap();
    assert!(create_second < drop_first);
}

#[test]
fn surface_loss_is_structured_and_recoverable() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    let target = TestTarget::headless(8, 8, 1.0);
    session.attach(&target).unwrap();
    let fixture = FrameFixture::empty(8, 8, 1.0, 5);
    api.fail_next(
        MockCall::Execute,
        injected_error(ApiErrorKind::SurfaceLost, "execute_frame"),
    );

    let error = session.render(&fixture.frame()).unwrap_err();
    assert_eq!(error.operation, BackendOperation::Render);
    assert_eq!(error.code, "skia-surface-lost");

    assert_eq!(
        session.recover(LossKind::Surface).unwrap(),
        Recovery::Reattached
    );
    assert_eq!(
        session.render(&fixture.frame()).unwrap().frame_id,
        Some(FrameId(5))
    );
    let diagnostics = session.diagnostics();
    assert_eq!(diagnostics.counters.surface_recoveries, 1);
    assert_eq!(diagnostics.counters.dropped_frames, 1);
}

#[test]
fn device_recovery_replaces_the_complete_native_ownership_chain() {
    let api = MockApi::default();
    let mut session = session(api.clone());
    session.attach(&TestTarget::headless(8, 8, 1.0)).unwrap();

    assert_eq!(
        session.recover(LossKind::Device).unwrap(),
        Recovery::DeviceRecreated
    );

    let drops = api
        .events()
        .into_iter()
        .filter(|event| matches!(event, MockEvent::Dropped(..)))
        .collect::<Vec<_>>();
    assert_eq!(
        drops,
        vec![
            MockEvent::Dropped("surface", 3),
            MockEvent::Dropped("context", 2),
            MockEvent::Dropped("engine", 1),
        ]
    );
    assert_eq!(session.diagnostics().counters.device_recoveries, 1);
}

#[test]
fn memory_pressure_reaches_the_owned_context() {
    let api = MockApi::default();
    let mut session = session(api.clone());

    session.trim_memory(MemoryPressure::Critical).unwrap();

    assert!(api
        .events()
        .contains(&MockEvent::Called(MockCall::TrimMemory)));
}

#[test]
fn native_target_is_rejected_without_allocating_a_surface() {
    let api = MockApi::default();
    let mut target = TestTarget::headless(8, 8, 1.0);
    target.descriptor.kind = SurfaceKind::NativeWindow;
    let mut driver = RasterDriver::try_new(api.clone()).unwrap();

    let error = driver.attach(&target).unwrap_err();

    assert_eq!(error.code, "skia-raster-target-unsupported");
    assert!(!api
        .events()
        .contains(&MockEvent::Called(MockCall::CreateSurface)));
}

#[test]
fn initialization_failure_drops_partial_native_ownership() {
    let api = MockApi::default();
    api.fail_next(
        MockCall::CreateContext,
        injected_error(ApiErrorKind::OutOfMemory, "create_context"),
    );

    let error = RasterDriver::try_new(api.clone()).unwrap_err();

    assert_eq!(error.code, "skia-out-of-memory");
    assert!(api.events().contains(&MockEvent::Dropped("engine", 1)));
}

impl<A: SkiaApi> fmt::Debug for RasterDriver<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RasterDriver")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}
