use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use fission_render::backend::{
    BackendResult, GraphicsBackendDriver, PresentReport, ReadbackRequest, RenderReport,
};
use fission_render::capabilities::{ColorFormat, DisplayOpKind, GraphicsCapabilities};
use fission_render::diagnostics::BackendDiagnostics;
use fission_render::external_surface::ExternalSurfaceBindings;
use fission_render::frame::{
    DamageRegion, FrameId, FrameMetadata, FrameViewport, InteractiveFrame, ResourceEpoch,
    SemanticsEpoch, ValidatedInteractiveFrame,
};
use fission_render::resource::ResourceSnapshot;
use fission_render::surface::{
    MemoryPressure, PhysicalSize, ScaleFactor, SurfaceDescriptor, SurfaceKind, SurfaceTarget,
    ThreadAffinity,
};
use fission_render::{LayoutRect, LayoutSize, RenderScene};

use super::*;
use crate::capabilities::skia_raster_profile_capabilities;

#[derive(Debug, Clone, PartialEq)]
enum DriverEvent {
    Attach(SurfaceDescriptor),
    Resize(SurfaceMetrics),
    Render(FrameId),
    Present,
    Readback,
    Suspend,
    Resume(SurfaceDescriptor),
    Recover(LossKind),
    TrimMemory(MemoryPressure),
    Detach,
}

#[derive(Clone)]
struct DriverControls {
    state: Arc<Mutex<ControlState>>,
}

struct ControlState {
    events: Vec<DriverEvent>,
    render_failures: VecDeque<BackendError>,
    resize_failure: Option<BackendError>,
    readback: Readback,
}

impl DriverControls {
    fn new(readback: Readback) -> Self {
        Self {
            state: Arc::new(Mutex::new(ControlState {
                events: Vec::new(),
                render_failures: VecDeque::new(),
                resize_failure: None,
                readback,
            })),
        }
    }

    fn fail_next_render(&self, code: &'static str) {
        self.state
            .lock()
            .unwrap()
            .render_failures
            .push_back(BackendError::new(
                BackendOperation::Render,
                code,
                "injected render failure",
            ));
    }

    fn fail_next_resize(&self) {
        self.state.lock().unwrap().resize_failure = Some(BackendError::new(
            BackendOperation::Resize,
            "injected-resize-failure",
            "injected resize failure",
        ));
    }

    fn events(&self) -> Vec<DriverEvent> {
        self.state.lock().unwrap().events.clone()
    }

    fn push(&self, event: DriverEvent) {
        self.state.lock().unwrap().events.push(event);
    }
}

struct TestDriver {
    controls: DriverControls,
    capabilities: GraphicsCapabilities,
    state: SessionState,
    last_rendered: Option<FrameId>,
}

impl TestDriver {
    fn new(controls: DriverControls) -> Self {
        Self {
            controls,
            capabilities: skia_raster_profile_capabilities(),
            state: SessionState::Detached,
            last_rendered: None,
        }
    }
}

impl GraphicsBackendDriver for TestDriver {
    fn capabilities(&self) -> &GraphicsCapabilities {
        &self.capabilities
    }

    fn state(&self) -> SessionState {
        self.state
    }

    fn attach(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.controls
            .push(DriverEvent::Attach(target.descriptor().clone()));
        self.state = SessionState::Attached;
        Ok(())
    }

    fn resize(&mut self, metrics: SurfaceMetrics) -> BackendResult<()> {
        self.controls.push(DriverEvent::Resize(metrics));
        if let Some(error) = self.controls.state.lock().unwrap().resize_failure.take() {
            return Err(error);
        }
        self.last_rendered = None;
        Ok(())
    }

    fn render_validated(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<RenderReport> {
        let frame_id = frame.frame().metadata().frame_id;
        self.controls.push(DriverEvent::Render(frame_id));
        let failure = self
            .controls
            .state
            .lock()
            .unwrap()
            .render_failures
            .pop_front();
        if let Some(error) = failure {
            if recoverable_loss(&error).is_some() {
                self.state = SessionState::Lost;
            }
            return Err(error);
        }
        self.last_rendered = Some(frame_id);
        Ok(RenderReport {
            frame_id: Some(frame_id),
            encoded_operations: 0,
            reused_layers: 0,
            uploaded_bytes: 0,
        })
    }

    fn present(&mut self) -> BackendResult<PresentReport> {
        self.controls.push(DriverEvent::Present);
        Ok(PresentReport {
            frame_id: self.last_rendered,
            recovered_surface: false,
        })
    }

    fn readback(&mut self, _request: ReadbackRequest) -> BackendResult<Readback> {
        self.controls.push(DriverEvent::Readback);
        Ok(self.controls.state.lock().unwrap().readback.clone())
    }

    fn suspend(&mut self) -> BackendResult<()> {
        self.controls.push(DriverEvent::Suspend);
        self.last_rendered = None;
        self.state = SessionState::Suspended;
        Ok(())
    }

    fn resume(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.controls
            .push(DriverEvent::Resume(target.descriptor().clone()));
        self.last_rendered = None;
        self.state = SessionState::Attached;
        Ok(())
    }

    fn recover(&mut self, loss: LossKind) -> BackendResult<Recovery> {
        self.controls.push(DriverEvent::Recover(loss));
        self.last_rendered = None;
        self.state = SessionState::Attached;
        Ok(match loss {
            LossKind::Surface => Recovery::Reattached,
            LossKind::Device => Recovery::DeviceRecreated,
        })
    }

    fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()> {
        self.controls.push(DriverEvent::TrimMemory(pressure));
        Ok(())
    }

    fn detach(&mut self) -> BackendResult<()> {
        self.controls.push(DriverEvent::Detach);
        self.last_rendered = None;
        self.state = SessionState::Detached;
        Ok(())
    }

    fn diagnostics(&self) -> BackendDiagnostics {
        BackendDiagnostics::new(self.capabilities.identity.clone(), self.state)
    }
}

struct FrameFixture {
    scene: RenderScene,
    metadata: FrameMetadata,
    resources: ResourceSnapshot,
    bindings: ExternalSurfaceBindings,
}

impl FrameFixture {
    fn empty(metrics: SurfaceMetrics, frame_id: u64) -> Self {
        let scale = metrics.scale_factor.get() as f32;
        let logical_size = LayoutSize::new(
            metrics.size.width as f32 / scale,
            metrics.size.height as f32 / scale,
        );
        let resource_epoch = ResourceEpoch(7);
        Self {
            scene: RenderScene::new(LayoutRect::new(
                0.0,
                0.0,
                logical_size.width,
                logical_size.height,
            )),
            metadata: FrameMetadata {
                frame_id: FrameId(frame_id),
                viewport: FrameViewport {
                    logical_size,
                    physical_size: metrics.size,
                    scale_factor: metrics.scale_factor,
                },
                damage: DamageRegion::Full,
                resource_epoch,
                semantics_epoch: SemanticsEpoch(11),
            },
            resources: ResourceSnapshot::empty(resource_epoch),
            bindings: ExternalSurfaceBindings::new(),
        }
    }

    fn frame(&self) -> InteractiveFrame<'_> {
        InteractiveFrame::new(&self.scene, &self.metadata, &self.resources, &self.bindings)
    }
}

fn metrics(width: u32, height: u32, scale_factor: f64) -> SurfaceMetrics {
    SurfaceMetrics {
        size: PhysicalSize::new(width, height),
        scale_factor: ScaleFactor::new(scale_factor).unwrap(),
    }
}

fn transparent_readback(size: PhysicalSize) -> Readback {
    let row_bytes = usize::try_from(size.width).unwrap() * 4;
    Readback {
        size,
        color_format: ColorFormat::Rgba8Srgb,
        row_bytes,
        pixels: vec![0; row_bytes * usize::try_from(size.height).unwrap()],
    }
}

fn session(controls: DriverControls, metrics: SurfaceMetrics) -> SkiaRasterSession {
    SkiaRasterSession::attach_driver(TestDriver::new(controls), metrics).unwrap()
}

#[test]
fn headless_session_owns_and_attaches_the_raster_target() {
    let initial = metrics(64, 32, 2.0);
    let controls = DriverControls::new(transparent_readback(initial.size));
    let session = session(controls.clone(), initial);

    assert_eq!(session.state(), SessionState::Attached);
    assert_eq!(session.surface_metrics(), initial);
    assert!(session
        .capabilities()
        .supports_display_op(DisplayOpKind::DrawText));

    let DriverEvent::Attach(descriptor) = &controls.events()[0] else {
        panic!("the first driver operation must attach the target");
    };
    assert_eq!(descriptor.kind, SurfaceKind::Headless);
    assert_eq!(descriptor.size, initial.size);
    assert_eq!(descriptor.scale_factor, initial.scale_factor);
    assert_eq!(descriptor.color_format, ColorFormat::Rgba8Srgb);
    assert_eq!(descriptor.thread_affinity, ThreadAffinity::CreatingThread);
}

#[test]
fn failed_resize_does_not_publish_unattached_metrics() {
    let initial = metrics(8, 8, 1.0);
    let replacement = metrics(16, 12, 2.0);
    let controls = DriverControls::new(transparent_readback(initial.size));
    controls.fail_next_resize();
    let mut session = session(controls, initial);

    let error = session.resize(replacement).unwrap_err();

    assert_eq!(error.code, "injected-resize-failure");
    assert_eq!(session.surface_metrics(), initial);
}

#[test]
fn unchanged_resize_still_enforces_the_graphics_session_lifecycle() {
    let initial = metrics(8, 8, 1.0);
    let controls = DriverControls::new(transparent_readback(initial.size));
    let mut session = session(controls, initial);
    session.suspend().unwrap();

    let error = session.resize(initial).unwrap_err();

    assert_eq!(error.code, "invalid-session-state");
    assert_eq!(session.state(), SessionState::Suspended);
}

#[test]
fn resume_uses_the_current_host_metrics_without_changing_target_identity() {
    let initial = metrics(8, 8, 1.0);
    let resumed = metrics(24, 12, 1.5);
    let controls = DriverControls::new(transparent_readback(initial.size));
    let mut session = session(controls.clone(), initial);
    let initial_id = match &controls.events()[0] {
        DriverEvent::Attach(descriptor) => descriptor.id,
        event => panic!("expected attach event, got {event:?}"),
    };

    session.suspend().unwrap();
    session.resume(resumed).unwrap();

    assert_eq!(session.state(), SessionState::Attached);
    assert_eq!(session.surface_metrics(), resumed);
    let resume = controls
        .events()
        .into_iter()
        .find_map(|event| match event {
            DriverEvent::Resume(descriptor) => Some(descriptor),
            _ => None,
        })
        .expect("resume target descriptor");
    assert_eq!(resume.id, initial_id);
    assert_eq!(resume.size, resumed.size);
    assert_eq!(resume.scale_factor, resumed.scale_factor);
}

#[test]
fn render_to_rgba_recovers_explicit_surface_loss_and_removes_row_padding() {
    let surface = metrics(2, 2, 1.0);
    let controls = DriverControls::new(Readback {
        size: surface.size,
        color_format: ColorFormat::Rgba8Srgb,
        row_bytes: 12,
        pixels: vec![
            1, 2, 3, 4, 5, 6, 7, 8, 90, 91, 92, 93, 9, 10, 11, 12, 13, 14, 15, 16, 94, 95, 96, 97,
        ],
    });
    controls.fail_next_render("skia-surface-lost");
    let mut session = session(controls.clone(), surface);
    let fixture = FrameFixture::empty(surface, 42);

    let pixels = session.render_to_rgba(&fixture.frame()).unwrap();

    assert_eq!(
        pixels,
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
    let events = controls.events();
    assert!(matches!(events.first(), Some(DriverEvent::Attach(_))));
    assert_eq!(
        &events[1..],
        &[
            DriverEvent::Render(FrameId(42)),
            DriverEvent::Recover(LossKind::Surface),
            DriverEvent::Render(FrameId(42)),
            DriverEvent::Present,
            DriverEvent::Readback,
        ]
    );
}

#[test]
fn render_to_rgba_never_recovers_an_unclassified_failure() {
    let surface = metrics(2, 2, 1.0);
    let controls = DriverControls::new(transparent_readback(surface.size));
    controls.fail_next_render("skia-out-of-memory");
    let mut session = session(controls.clone(), surface);
    let fixture = FrameFixture::empty(surface, 9);

    let error = session.render_to_rgba(&fixture.frame()).unwrap_err();

    assert_eq!(error.code, "skia-out-of-memory");
    assert!(!controls
        .events()
        .iter()
        .any(|event| matches!(event, DriverEvent::Recover(_))));
}

#[test]
fn render_to_rgba_retries_a_recoverable_failure_only_once() {
    let surface = metrics(2, 2, 1.0);
    let controls = DriverControls::new(transparent_readback(surface.size));
    controls.fail_next_render("skia-device-lost");
    controls.fail_next_render("skia-device-lost");
    let mut session = session(controls.clone(), surface);
    let fixture = FrameFixture::empty(surface, 10);

    let error = session.render_to_rgba(&fixture.frame()).unwrap_err();

    assert_eq!(error.code, "skia-device-lost");
    assert_eq!(
        controls
            .events()
            .iter()
            .filter(|event| matches!(event, DriverEvent::Recover(LossKind::Device)))
            .count(),
        1
    );
}

#[test]
fn loss_classification_accepts_only_the_two_stable_skia_codes() {
    let error = |code| BackendError::new(BackendOperation::Render, code, "failure");

    assert_eq!(
        recoverable_loss(&error("skia-surface-lost")),
        Some(LossKind::Surface)
    );
    assert_eq!(
        recoverable_loss(&error("skia-device-lost")),
        Some(LossKind::Device)
    );
    assert_eq!(recoverable_loss(&error("surface-lost")), None);
    assert_eq!(recoverable_loss(&error("skia-out-of-memory")), None);
}

#[test]
fn empty_readback_requires_an_empty_native_layout() {
    let expected = PhysicalSize::new(0, 12);
    let valid = Readback {
        size: expected,
        color_format: ColorFormat::Rgba8Srgb,
        row_bytes: 0,
        pixels: Vec::new(),
    };
    assert!(tight_rgba(valid, expected).unwrap().is_empty());

    let invalid = Readback {
        size: expected,
        color_format: ColorFormat::Rgba8Srgb,
        row_bytes: 4,
        pixels: vec![0; 48],
    };
    assert_eq!(
        tight_rgba(invalid, expected).unwrap_err().code,
        "skia-readback-layout-invalid"
    );
}
