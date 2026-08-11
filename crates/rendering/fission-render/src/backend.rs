use std::fmt;

use serde::{Deserialize, Serialize};

use crate::capabilities::{ColorFormat, GraphicsCapabilities};
use crate::conformance::FrameOperationSource;
use crate::diagnostics::{
    BackendDiagnostic, BackendDiagnostics, DiagnosticCategory, DiagnosticProvenance,
    DiagnosticSeverity,
};
use crate::frame::{
    FrameGateError, FrameId, FrameValidationError, InteractiveFrame, ValidatedInteractiveFrame,
};
use crate::surface::{
    LifecycleError, LossKind, MemoryPressure, PhysicalSize, Recovery, ScaleFactor,
    SessionLifecycle, SessionState, SurfaceTarget,
};
use crate::LayoutRect;

pub type BackendResult<T> = Result<T, BackendError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendOperation {
    Initialize,
    Attach,
    Resize,
    Render,
    Present,
    Readback,
    Suspend,
    Resume,
    Recover,
    TrimMemory,
    Detach,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackendError {
    pub operation: BackendOperation,
    pub code: String,
    pub message: String,
    pub diagnostic: Option<BackendDiagnostic>,
    pub frame_gate_error: Option<FrameGateError>,
}

impl BackendError {
    pub fn new(
        operation: BackendOperation,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            operation,
            code: code.into(),
            message: message.into(),
            diagnostic: None,
            frame_gate_error: None,
        }
    }

    pub fn with_diagnostic(mut self, diagnostic: BackendDiagnostic) -> Self {
        self.diagnostic = Some(diagnostic);
        self
    }

    pub fn from_frame_gate(frame_id: FrameId, error: FrameGateError) -> Self {
        let diagnostic = frame_gate_diagnostic(frame_id, &error);
        Self {
            operation: BackendOperation::Render,
            code: "frame-conformance".into(),
            message: error.to_string(),
            diagnostic: Some(diagnostic),
            frame_gate_error: Some(error),
        }
    }

    fn from_lifecycle(operation: BackendOperation, error: LifecycleError) -> Self {
        let message = error.to_string();
        Self::new(operation, "invalid-session-state", message.clone()).with_diagnostic(
            BackendDiagnostic {
                severity: DiagnosticSeverity::Error,
                category: DiagnosticCategory::Lifecycle,
                code: "invalid-session-state".into(),
                message,
                provenance: None,
            },
        )
    }

    fn capability(
        operation: BackendOperation,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        Self::new(operation, code, message.clone()).with_diagnostic(BackendDiagnostic {
            severity: DiagnosticSeverity::Error,
            category: DiagnosticCategory::Capability,
            code: code.into(),
            message,
            provenance: None,
        })
    }

    fn driver_state_mismatch(
        operation: BackendOperation,
        expected: SessionState,
        actual: SessionState,
    ) -> Self {
        let message = format!(
            "backend driver reported {actual:?} after {operation:?}; expected {expected:?}"
        );
        Self::new(operation, "driver-session-state-mismatch", message.clone()).with_diagnostic(
            BackendDiagnostic {
                severity: DiagnosticSeverity::Error,
                category: DiagnosticCategory::Lifecycle,
                code: "driver-session-state-mismatch".into(),
                message,
                provenance: None,
            },
        )
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} failed ({}): {}",
            self.operation, self.code, self.message
        )
    }
}

impl std::error::Error for BackendError {}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SurfaceMetrics {
    pub size: PhysicalSize,
    pub scale_factor: ScaleFactor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RenderReport {
    pub frame_id: Option<FrameId>,
    pub encoded_operations: u64,
    pub reused_layers: u64,
    pub uploaded_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PresentReport {
    pub frame_id: Option<FrameId>,
    pub recovered_surface: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReadbackRequest {
    pub region: Option<LayoutRect>,
    pub color_format: ColorFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readback {
    pub size: PhysicalSize,
    pub color_format: ColorFormat,
    pub row_bytes: usize,
    pub pixels: Vec<u8>,
}

/// Implementation hooks owned by one interactive graphics backend.
///
/// Backend crates implement this driver, but hosts must submit work through
/// [`GraphicsBackendSession`]. The session wrapper owns the non-overridable
/// frame-conformance gate and is the only code that can construct the
/// [`ValidatedInteractiveFrame`] passed to [`Self::render_validated`].
///
/// A newly constructed driver must report [`SessionState::Detached`]. Lifecycle
/// hooks must report their documented destination state after returning `Ok`.
/// Lifecycle-hook errors are not assumed to be atomic: the owning session
/// treats every failed or inconsistent transition as ambiguous and fails closed
/// instead of claiming the target is still usable.
pub trait GraphicsBackendDriver {
    fn capabilities(&self) -> &GraphicsCapabilities;
    fn state(&self) -> SessionState;

    fn attach(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()>;
    fn resize(&mut self, metrics: SurfaceMetrics) -> BackendResult<()>;
    /// Backend implementation hook reached only after the owning session has
    /// checked frame integrity and capability conformance.
    fn render_validated(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<RenderReport>;
    fn present(&mut self) -> BackendResult<PresentReport>;
    fn readback(&mut self, request: ReadbackRequest) -> BackendResult<Readback>;
    fn suspend(&mut self) -> BackendResult<()>;
    fn resume(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()>;
    fn recover(&mut self, loss: LossKind) -> BackendResult<Recovery>;
    fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()>;
    fn detach(&mut self) -> BackendResult<()>;
    fn diagnostics(&self) -> BackendDiagnostics;
}

/// Fission-owned, non-overridable lifecycle and submission boundary.
///
/// The wrapped driver is intentionally not exposed. This prevents a host from
/// accidentally bypassing frame validation or submitting a token validated
/// for another capability profile. A malicious backend can always violate its
/// own implementation contract; this wrapper makes the safe integration path
/// structurally correct.
pub struct GraphicsBackendSession<'driver> {
    driver: Box<dyn GraphicsBackendDriver + 'driver>,
    lifecycle: SessionLifecycle,
}

impl<'driver> GraphicsBackendSession<'driver> {
    pub fn new(driver: impl GraphicsBackendDriver + 'driver) -> BackendResult<Self> {
        let actual = driver.state();
        if actual != SessionState::Detached {
            return Err(BackendError::from_lifecycle(
                BackendOperation::Initialize,
                LifecycleError {
                    operation: "initialize backend session",
                    actual,
                    expected: vec![SessionState::Detached],
                },
            ));
        }
        Ok(Self {
            driver: Box::new(driver),
            lifecycle: SessionLifecycle::default(),
        })
    }

    pub fn capabilities(&self) -> &GraphicsCapabilities {
        self.driver.capabilities()
    }

    pub fn state(&self) -> SessionState {
        self.lifecycle.state()
    }

    pub fn attach(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        let mut next = self.lifecycle;
        next.attach()
            .map_err(|error| BackendError::from_lifecycle(BackendOperation::Attach, error))?;
        if let Err(error) = self.driver.attach(target) {
            self.fail_closed_after_lifecycle_error();
            return Err(error);
        }
        if let Err(error) =
            self.require_driver_state(BackendOperation::Attach, SessionState::Attached)
        {
            self.fail_closed_after_lifecycle_error();
            return Err(error);
        }
        self.lifecycle = next;
        Ok(())
    }

    pub fn resize(&mut self, metrics: SurfaceMetrics) -> BackendResult<()> {
        self.require_attached(BackendOperation::Resize, "resize")?;
        self.driver.resize(metrics)
    }

    /// Validate the complete frame before backend implementation code can
    /// observe it.
    pub fn render(&mut self, frame: &InteractiveFrame<'_>) -> BackendResult<RenderReport> {
        self.require_attached(BackendOperation::Render, "render")?;
        // Take an immutable capability snapshot. Besides ending the immutable
        // driver borrow before the mutable hook call, this guarantees the
        // validated identity and operation set are the exact set used by the
        // gate even if a driver reports runtime-dependent capabilities.
        let capabilities = self.driver.capabilities().clone();
        let validated = (*frame)
            .validate_for(&capabilities)
            .map_err(|error| BackendError::from_frame_gate(frame.metadata().frame_id, error))?;
        self.driver.render_validated(&validated)
    }

    pub fn present(&mut self) -> BackendResult<PresentReport> {
        self.require_attached(BackendOperation::Present, "present")?;
        self.driver.present()
    }

    pub fn readback(&mut self, request: ReadbackRequest) -> BackendResult<Readback> {
        self.require_attached(BackendOperation::Readback, "read back")?;
        let capabilities = self.driver.capabilities();
        if !capabilities.readback {
            return Err(BackendError::capability(
                BackendOperation::Readback,
                "readback-unsupported",
                format!(
                    "backend {} ({}) does not support readback",
                    capabilities.identity.name, capabilities.identity.profile
                ),
            ));
        }
        if !capabilities.color_formats.contains(&request.color_format) {
            return Err(BackendError::capability(
                BackendOperation::Readback,
                "readback-color-format-unsupported",
                format!(
                    "backend {} ({}) cannot read back {:?}",
                    capabilities.identity.name, capabilities.identity.profile, request.color_format
                ),
            ));
        }
        self.driver.readback(request)
    }

    pub fn suspend(&mut self) -> BackendResult<()> {
        let mut next = self.lifecycle;
        next.suspend()
            .map_err(|error| BackendError::from_lifecycle(BackendOperation::Suspend, error))?;
        if let Err(error) = self.driver.suspend() {
            self.fail_closed_after_lifecycle_error();
            return Err(error);
        }
        if let Err(error) =
            self.require_driver_state(BackendOperation::Suspend, SessionState::Suspended)
        {
            self.fail_closed_after_lifecycle_error();
            return Err(error);
        }
        self.lifecycle = next;
        Ok(())
    }

    pub fn resume(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        let mut next = self.lifecycle;
        next.resume()
            .map_err(|error| BackendError::from_lifecycle(BackendOperation::Resume, error))?;
        if let Err(error) = self.driver.resume(target) {
            self.fail_closed_after_lifecycle_error();
            return Err(error);
        }
        if let Err(error) =
            self.require_driver_state(BackendOperation::Resume, SessionState::Attached)
        {
            self.fail_closed_after_lifecycle_error();
            return Err(error);
        }
        self.lifecycle = next;
        Ok(())
    }

    pub fn recover(&mut self, loss: LossKind) -> BackendResult<Recovery> {
        let mut lost = self.lifecycle;
        match lost.state() {
            SessionState::Attached | SessionState::Suspended => lost
                .mark_lost()
                .expect("attached or suspended sessions can be marked lost"),
            SessionState::Lost => {}
            SessionState::Detached => {
                return Err(BackendError::from_lifecycle(
                    BackendOperation::Recover,
                    LifecycleError {
                        operation: "recover",
                        actual: SessionState::Detached,
                        expected: vec![
                            SessionState::Attached,
                            SessionState::Suspended,
                            SessionState::Lost,
                        ],
                    },
                ));
            }
        }

        // Calling `recover` is itself the host's loss notification. Commit the
        // lost state before entering fallible backend code so a failed recovery
        // cannot leave the wrapper claiming that the old surface is usable.
        self.lifecycle = lost;
        let recovery = self.driver.recover(loss)?;
        if recovery != Recovery::Unrecoverable {
            self.require_driver_state(BackendOperation::Recover, SessionState::Attached)?;
            let mut recovered = lost;
            recovered
                .recover()
                .expect("a successful recovery starts from the lost state");
            self.lifecycle = recovered;
        } else {
            self.require_driver_state(BackendOperation::Recover, SessionState::Lost)?;
        }
        Ok(recovery)
    }

    pub fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()> {
        self.driver.trim_memory(pressure)
    }

    pub fn detach(&mut self) -> BackendResult<()> {
        let mut next = self.lifecycle;
        next.detach()
            .map_err(|error| BackendError::from_lifecycle(BackendOperation::Detach, error))?;
        if let Err(error) = self.driver.detach() {
            self.fail_closed_after_lifecycle_error();
            return Err(error);
        }
        if let Err(error) =
            self.require_driver_state(BackendOperation::Detach, SessionState::Detached)
        {
            self.fail_closed_after_lifecycle_error();
            return Err(error);
        }
        self.lifecycle = next;
        Ok(())
    }

    pub fn diagnostics(&self) -> BackendDiagnostics {
        let mut diagnostics = self.driver.diagnostics();
        diagnostics.session_state = self.lifecycle.state();
        diagnostics
    }

    fn require_attached(
        &mut self,
        operation: BackendOperation,
        operation_name: &'static str,
    ) -> BackendResult<()> {
        self.lifecycle
            .require_attached(operation_name)
            .map_err(|error| BackendError::from_lifecycle(operation, error))?;
        if let Err(error) = self.require_driver_state(operation, SessionState::Attached) {
            self.fail_closed_after_lifecycle_error();
            return Err(error);
        }
        Ok(())
    }

    fn require_driver_state(
        &self,
        operation: BackendOperation,
        expected: SessionState,
    ) -> BackendResult<()> {
        let actual = self.driver.state();
        if actual == expected {
            Ok(())
        } else {
            Err(BackendError::driver_state_mismatch(
                operation, expected, actual,
            ))
        }
    }

    fn fail_closed_after_lifecycle_error(&mut self) {
        self.lifecycle.fail_closed();
    }
}

fn frame_gate_diagnostic(frame_id: FrameId, error: &FrameGateError) -> BackendDiagnostic {
    match error {
        FrameGateError::InvalidFrame(error) => {
            let (category, code) = match error {
                FrameValidationError::InvalidResourceSnapshot(_)
                | FrameValidationError::ResourceEpochMismatch { .. } => {
                    (DiagnosticCategory::Resource, "invalid-resource-snapshot")
                }
                FrameValidationError::InvalidSurfaceBinding(_)
                | FrameValidationError::DuplicateSurfacePlacement(_)
                | FrameValidationError::MissingSurfaceBinding(_)
                | FrameValidationError::BindingWithoutPlacement(_) => (
                    DiagnosticCategory::ExternalSurface,
                    "invalid-interactive-frame",
                ),
            };
            BackendDiagnostic {
                severity: DiagnosticSeverity::Error,
                category,
                code: code.into(),
                message: error.to_string(),
                provenance: Some(DiagnosticProvenance {
                    frame_id: Some(frame_id),
                    node_id: None,
                    operation_index: None,
                }),
            }
        }
        FrameGateError::UnsupportedOperations(error) => {
            let first = error.unsupported_operations.first();
            let operation_index =
                first.and_then(|unsupported| match &unsupported.provenance.source {
                    FrameOperationSource::DisplayList { operation_path, .. } => {
                        operation_path.last().copied()
                    }
                    FrameOperationSource::LayerStyle { .. } => None,
                });
            BackendDiagnostic {
                severity: DiagnosticSeverity::Error,
                category: DiagnosticCategory::Capability,
                code: "unsupported-display-operation".into(),
                message: error.to_string(),
                provenance: Some(DiagnosticProvenance {
                    frame_id: Some(frame_id),
                    node_id: first.and_then(|unsupported| unsupported.provenance.node_id),
                    operation_index,
                }),
            }
        }
        FrameGateError::UnsupportedExternalSurfaces(error) => {
            let first = error.unsupported_bindings.first();
            let operation_index =
                first.and_then(|unsupported| match &unsupported.provenance.source {
                    FrameOperationSource::DisplayList { operation_path, .. } => {
                        operation_path.last().copied()
                    }
                    FrameOperationSource::LayerStyle { .. } => None,
                });
            BackendDiagnostic {
                severity: DiagnosticSeverity::Error,
                category: DiagnosticCategory::Capability,
                code: "unsupported-external-surface-transport".into(),
                message: error.to_string(),
                provenance: Some(DiagnosticProvenance {
                    frame_id: Some(frame_id),
                    node_id: first.and_then(|unsupported| unsupported.provenance.node_id),
                    operation_index,
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{BackendIdentity, DisplayOpKind};
    use crate::external_surface::ExternalSurfaceBindings;
    use crate::frame::{DamageRegion, FrameMetadata, FrameViewport, ResourceEpoch, SemanticsEpoch};
    use crate::resource::ResourceSnapshot;
    use crate::surface::{
        PhysicalSize, ScaleFactor, SurfaceDescriptor, SurfaceId, SurfaceKind, ThreadAffinity,
    };
    use crate::{DisplayList, DisplayOp, LayoutRect, LayoutSize, RenderScene};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn accepts_driver_trait_object(_driver: &mut dyn GraphicsBackendDriver) {}

    #[test]
    fn driver_hook_remains_object_safe() {
        let contract: fn(&mut dyn GraphicsBackendDriver) = accepts_driver_trait_object;
        let _ = contract;
    }

    struct RecordingDriver {
        capabilities: GraphicsCapabilities,
        validated_calls: Arc<AtomicUsize>,
        state: SessionState,
    }

    impl GraphicsBackendDriver for RecordingDriver {
        fn capabilities(&self) -> &GraphicsCapabilities {
            &self.capabilities
        }

        fn state(&self) -> SessionState {
            self.state
        }

        fn attach(&mut self, _target: &dyn SurfaceTarget) -> BackendResult<()> {
            self.state = SessionState::Attached;
            Ok(())
        }

        fn resize(&mut self, _metrics: SurfaceMetrics) -> BackendResult<()> {
            Ok(())
        }

        fn render_validated(
            &mut self,
            frame: &ValidatedInteractiveFrame<'_>,
        ) -> BackendResult<RenderReport> {
            self.validated_calls.fetch_add(1, Ordering::SeqCst);
            Ok(RenderReport {
                frame_id: Some(frame.frame().metadata().frame_id),
                ..RenderReport::default()
            })
        }

        fn present(&mut self) -> BackendResult<PresentReport> {
            Ok(PresentReport::default())
        }

        fn readback(&mut self, request: ReadbackRequest) -> BackendResult<Readback> {
            Ok(Readback {
                size: PhysicalSize::ZERO,
                color_format: request.color_format,
                row_bytes: 0,
                pixels: Vec::new(),
            })
        }

        fn suspend(&mut self) -> BackendResult<()> {
            self.state = SessionState::Suspended;
            Ok(())
        }

        fn resume(&mut self, _target: &dyn SurfaceTarget) -> BackendResult<()> {
            self.state = SessionState::Attached;
            Ok(())
        }

        fn recover(&mut self, loss: LossKind) -> BackendResult<Recovery> {
            self.state = SessionState::Attached;
            Ok(match loss {
                LossKind::Surface => Recovery::Reattached,
                LossKind::Device => Recovery::DeviceRecreated,
            })
        }

        fn trim_memory(&mut self, _pressure: MemoryPressure) -> BackendResult<()> {
            Ok(())
        }

        fn detach(&mut self) -> BackendResult<()> {
            self.state = SessionState::Detached;
            Ok(())
        }

        fn diagnostics(&self) -> BackendDiagnostics {
            BackendDiagnostics::new(self.capabilities.identity.clone(), self.state())
        }
    }

    struct LifecycleDriver {
        capabilities: GraphicsCapabilities,
        calls: Arc<Mutex<Vec<BackendOperation>>>,
        recovery: Recovery,
        state: SessionState,
        fail_attach: bool,
        fail_suspend: bool,
    }

    impl LifecycleDriver {
        fn record(&self, operation: BackendOperation) {
            self.calls.lock().unwrap().push(operation);
        }
    }

    impl GraphicsBackendDriver for LifecycleDriver {
        fn capabilities(&self) -> &GraphicsCapabilities {
            &self.capabilities
        }

        fn state(&self) -> SessionState {
            self.state
        }

        fn attach(&mut self, _target: &dyn SurfaceTarget) -> BackendResult<()> {
            self.record(BackendOperation::Attach);
            if self.fail_attach {
                // Simulate the hardest case: target mutation occurred before
                // the driver discovered and reported its error.
                self.state = SessionState::Attached;
                return Err(BackendError::new(
                    BackendOperation::Attach,
                    "injected-attach-failure",
                    "injected attach failure",
                ));
            }
            self.state = SessionState::Attached;
            Ok(())
        }

        fn resize(&mut self, _metrics: SurfaceMetrics) -> BackendResult<()> {
            self.record(BackendOperation::Resize);
            Ok(())
        }

        fn render_validated(
            &mut self,
            frame: &ValidatedInteractiveFrame<'_>,
        ) -> BackendResult<RenderReport> {
            self.record(BackendOperation::Render);
            Ok(RenderReport {
                frame_id: Some(frame.frame().metadata().frame_id),
                ..RenderReport::default()
            })
        }

        fn present(&mut self) -> BackendResult<PresentReport> {
            self.record(BackendOperation::Present);
            Ok(PresentReport::default())
        }

        fn readback(&mut self, request: ReadbackRequest) -> BackendResult<Readback> {
            self.record(BackendOperation::Readback);
            Ok(Readback {
                size: PhysicalSize::ZERO,
                color_format: request.color_format,
                row_bytes: 0,
                pixels: Vec::new(),
            })
        }

        fn suspend(&mut self) -> BackendResult<()> {
            self.record(BackendOperation::Suspend);
            if self.fail_suspend {
                return Err(BackendError::new(
                    BackendOperation::Suspend,
                    "injected-suspend-failure",
                    "injected suspend failure",
                ));
            }
            self.state = SessionState::Suspended;
            Ok(())
        }

        fn resume(&mut self, _target: &dyn SurfaceTarget) -> BackendResult<()> {
            self.record(BackendOperation::Resume);
            self.state = SessionState::Attached;
            Ok(())
        }

        fn recover(&mut self, _loss: LossKind) -> BackendResult<Recovery> {
            self.record(BackendOperation::Recover);
            self.state = if self.recovery == Recovery::Unrecoverable {
                SessionState::Lost
            } else {
                SessionState::Attached
            };
            Ok(self.recovery)
        }

        fn trim_memory(&mut self, _pressure: MemoryPressure) -> BackendResult<()> {
            self.record(BackendOperation::TrimMemory);
            Ok(())
        }

        fn detach(&mut self) -> BackendResult<()> {
            self.record(BackendOperation::Detach);
            self.state = SessionState::Detached;
            Ok(())
        }

        fn diagnostics(&self) -> BackendDiagnostics {
            BackendDiagnostics::new(self.capabilities.identity.clone(), self.state())
        }
    }

    #[derive(Debug)]
    struct TestTarget {
        descriptor: SurfaceDescriptor,
    }

    impl SurfaceTarget for TestTarget {
        fn descriptor(&self) -> &SurfaceDescriptor {
            &self.descriptor
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn test_target() -> TestTarget {
        TestTarget {
            descriptor: SurfaceDescriptor {
                id: SurfaceId(1),
                kind: SurfaceKind::Headless,
                size: PhysicalSize::new(10, 10),
                scale_factor: ScaleFactor::ONE,
                color_format: ColorFormat::Rgba8Srgb,
                thread_affinity: ThreadAffinity::Any,
            },
        }
    }

    fn frame_fixture<'a>(
        scene: &'a RenderScene,
        metadata: &'a FrameMetadata,
        resources: &'a ResourceSnapshot,
        bindings: &'a ExternalSurfaceBindings,
    ) -> InteractiveFrame<'a> {
        InteractiveFrame::new(scene, metadata, resources, bindings)
    }

    #[test]
    fn render_gate_stops_unsupported_operations_before_backend_hook() {
        let bounds = LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        let mut list = DisplayList::new(bounds);
        list.push(DisplayOp::BackdropFilter {
            rect: bounds,
            filter: fission_ir::op::BackdropFilter::Blur(1.0),
            corner_radius: 0.0,
            bounds,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(list);
        let metadata = FrameMetadata {
            frame_id: FrameId(7),
            viewport: FrameViewport {
                logical_size: LayoutSize {
                    width: 10.0,
                    height: 10.0,
                },
                physical_size: PhysicalSize::new(10, 10),
                scale_factor: ScaleFactor::ONE,
            },
            damage: DamageRegion::Full,
            resource_epoch: ResourceEpoch(1),
            semantics_epoch: SemanticsEpoch(1),
        };
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let bindings = ExternalSurfaceBindings::new();
        let frame = frame_fixture(&scene, &metadata, &resources, &bindings);
        let mut capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("recording", "1", "test"));
        let validated_calls = Arc::new(AtomicUsize::new(0));
        let mut session = GraphicsBackendSession::new(RecordingDriver {
            capabilities: capabilities.clone(),
            validated_calls: Arc::clone(&validated_calls),
            state: SessionState::Detached,
        })
        .unwrap();
        session.attach(&test_target()).unwrap();

        let error = session.render(&frame).unwrap_err();

        assert_eq!(validated_calls.load(Ordering::SeqCst), 0);
        let Some(FrameGateError::UnsupportedOperations(conformance)) = error.frame_gate_error
        else {
            panic!("expected structured conformance error");
        };
        assert_eq!(conformance.unsupported_operations.len(), 1);
        assert_eq!(
            conformance.unsupported_operations[0].operation,
            DisplayOpKind::BackdropFilter
        );

        capabilities
            .display_ops
            .insert(DisplayOpKind::BackdropFilter);
        let mut session = GraphicsBackendSession::new(RecordingDriver {
            capabilities,
            validated_calls: Arc::clone(&validated_calls),
            state: SessionState::Detached,
        })
        .unwrap();
        session.attach(&test_target()).unwrap();
        let report = session.render(&frame).unwrap();
        assert_eq!(report.frame_id, Some(FrameId(7)));
        assert_eq!(validated_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn render_gate_stops_resource_epoch_mismatch_before_backend_hook() {
        let bounds = LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        let scene = RenderScene::new(bounds);
        let metadata = FrameMetadata {
            frame_id: FrameId(8),
            viewport: FrameViewport {
                logical_size: bounds.size,
                physical_size: PhysicalSize::new(10, 10),
                scale_factor: ScaleFactor::ONE,
            },
            damage: DamageRegion::Full,
            resource_epoch: ResourceEpoch(4),
            semantics_epoch: SemanticsEpoch(1),
        };
        let resources = ResourceSnapshot::empty(ResourceEpoch(3));
        let bindings = ExternalSurfaceBindings::new();
        let frame = frame_fixture(&scene, &metadata, &resources, &bindings);
        let validated_calls = Arc::new(AtomicUsize::new(0));
        let mut session = GraphicsBackendSession::new(RecordingDriver {
            capabilities: GraphicsCapabilities::empty(BackendIdentity::new(
                "recording",
                "1",
                "test",
            )),
            validated_calls: Arc::clone(&validated_calls),
            state: SessionState::Detached,
        })
        .unwrap();
        session.attach(&test_target()).unwrap();

        let error = session.render(&frame).unwrap_err();

        assert_eq!(validated_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            error.frame_gate_error,
            Some(FrameGateError::InvalidFrame(
                FrameValidationError::ResourceEpochMismatch {
                    metadata: ResourceEpoch(4),
                    snapshot: ResourceEpoch(3),
                }
            ))
        );
        assert_eq!(
            error.diagnostic.as_ref().map(|event| event.category),
            Some(DiagnosticCategory::Resource)
        );
    }

    #[test]
    fn session_rejects_invalid_lifecycle_order_before_driver_hooks() {
        let bounds = LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        let scene = RenderScene::new(bounds);
        let metadata = FrameMetadata {
            frame_id: FrameId(9),
            viewport: FrameViewport {
                logical_size: bounds.size,
                physical_size: PhysicalSize::new(10, 10),
                scale_factor: ScaleFactor::ONE,
            },
            damage: DamageRegion::Full,
            resource_epoch: ResourceEpoch(1),
            semantics_epoch: SemanticsEpoch(1),
        };
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let bindings = ExternalSurfaceBindings::new();
        let frame = frame_fixture(&scene, &metadata, &resources, &bindings);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut session = GraphicsBackendSession::new(LifecycleDriver {
            capabilities: GraphicsCapabilities::empty(BackendIdentity::new(
                "lifecycle",
                "1",
                "test",
            )),
            calls: Arc::clone(&calls),
            recovery: Recovery::Reattached,
            state: SessionState::Detached,
            fail_attach: false,
            fail_suspend: false,
        })
        .unwrap();

        let error = session.render(&frame).unwrap_err();
        assert_eq!(error.code, "invalid-session-state");
        assert_eq!(
            error.diagnostic.as_ref().map(|event| event.category),
            Some(DiagnosticCategory::Lifecycle)
        );
        assert!(calls.lock().unwrap().is_empty());

        session.attach(&test_target()).unwrap();
        assert_eq!(session.state(), SessionState::Attached);
        assert!(session.attach(&test_target()).is_err());
        assert_eq!(*calls.lock().unwrap(), vec![BackendOperation::Attach]);

        session.suspend().unwrap();
        assert_eq!(session.state(), SessionState::Suspended);
        assert!(session.render(&frame).is_err());
        assert_eq!(
            *calls.lock().unwrap(),
            vec![BackendOperation::Attach, BackendOperation::Suspend]
        );

        session.resume(&test_target()).unwrap();
        assert_eq!(session.state(), SessionState::Attached);
        assert_eq!(
            session.recover(LossKind::Surface).unwrap(),
            Recovery::Reattached
        );
        assert_eq!(session.state(), SessionState::Attached);
        session.detach().unwrap();
        assert_eq!(session.state(), SessionState::Detached);
        assert!(session
            .resize(SurfaceMetrics {
                size: PhysicalSize::new(20, 20),
                scale_factor: ScaleFactor::ONE,
            })
            .is_err());
        assert_eq!(session.diagnostics().session_state, SessionState::Detached);
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                BackendOperation::Attach,
                BackendOperation::Suspend,
                BackendOperation::Resume,
                BackendOperation::Recover,
                BackendOperation::Detach,
            ]
        );
    }

    #[test]
    fn unrecoverable_loss_keeps_session_lost_and_blocks_rendering() {
        let bounds = LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        let scene = RenderScene::new(bounds);
        let metadata = FrameMetadata {
            frame_id: FrameId(10),
            viewport: FrameViewport {
                logical_size: bounds.size,
                physical_size: PhysicalSize::new(10, 10),
                scale_factor: ScaleFactor::ONE,
            },
            damage: DamageRegion::Full,
            resource_epoch: ResourceEpoch(1),
            semantics_epoch: SemanticsEpoch(1),
        };
        let resources = ResourceSnapshot::empty(metadata.resource_epoch);
        let bindings = ExternalSurfaceBindings::new();
        let frame = frame_fixture(&scene, &metadata, &resources, &bindings);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut session = GraphicsBackendSession::new(LifecycleDriver {
            capabilities: GraphicsCapabilities::empty(BackendIdentity::new(
                "lifecycle",
                "1",
                "test",
            )),
            calls: Arc::clone(&calls),
            recovery: Recovery::Unrecoverable,
            state: SessionState::Detached,
            fail_attach: false,
            fail_suspend: false,
        })
        .unwrap();
        session.attach(&test_target()).unwrap();

        assert_eq!(
            session.recover(LossKind::Device).unwrap(),
            Recovery::Unrecoverable
        );
        assert_eq!(session.state(), SessionState::Lost);
        assert!(session.render(&frame).is_err());
        assert_eq!(
            *calls.lock().unwrap(),
            vec![BackendOperation::Attach, BackendOperation::Recover]
        );
    }

    #[test]
    fn session_construction_rejects_a_driver_that_is_already_live() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let error = GraphicsBackendSession::new(LifecycleDriver {
            capabilities: GraphicsCapabilities::empty(BackendIdentity::new(
                "lifecycle",
                "1",
                "test",
            )),
            calls: Arc::clone(&calls),
            recovery: Recovery::Reattached,
            state: SessionState::Attached,
            fail_attach: false,
            fail_suspend: false,
        })
        .err()
        .expect("a live driver must be rejected");

        assert_eq!(error.operation, BackendOperation::Initialize);
        assert_eq!(error.code, "invalid-session-state");
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn ambiguous_lifecycle_hook_failure_marks_a_live_session_lost() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut session = GraphicsBackendSession::new(LifecycleDriver {
            capabilities: GraphicsCapabilities::empty(BackendIdentity::new(
                "lifecycle",
                "1",
                "test",
            )),
            calls: Arc::clone(&calls),
            recovery: Recovery::Reattached,
            state: SessionState::Detached,
            fail_attach: false,
            fail_suspend: true,
        })
        .unwrap();
        session.attach(&test_target()).unwrap();

        let error = session.suspend().unwrap_err();

        assert_eq!(error.code, "injected-suspend-failure");
        assert_eq!(session.state(), SessionState::Lost);
        assert_eq!(session.diagnostics().session_state, SessionState::Lost);
        assert_eq!(
            *calls.lock().unwrap(),
            vec![BackendOperation::Attach, BackendOperation::Suspend]
        );
    }

    #[test]
    fn partially_mutated_attach_failure_poisons_the_session() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut session = GraphicsBackendSession::new(LifecycleDriver {
            capabilities: GraphicsCapabilities::empty(BackendIdentity::new(
                "lifecycle",
                "1",
                "test",
            )),
            calls: Arc::clone(&calls),
            recovery: Recovery::Reattached,
            state: SessionState::Detached,
            fail_attach: true,
            fail_suspend: false,
        })
        .unwrap();

        let error = session.attach(&test_target()).unwrap_err();

        assert_eq!(error.code, "injected-attach-failure");
        assert_eq!(session.state(), SessionState::Lost);
        assert!(session.attach(&test_target()).is_err());
        assert_eq!(*calls.lock().unwrap(), vec![BackendOperation::Attach]);
    }

    #[test]
    fn readback_capabilities_are_gated_before_the_driver_hook() {
        let request = ReadbackRequest {
            region: None,
            color_format: ColorFormat::Rgba8Srgb,
        };

        let unsupported_calls = Arc::new(Mutex::new(Vec::new()));
        let mut unsupported = GraphicsBackendSession::new(LifecycleDriver {
            capabilities: GraphicsCapabilities::empty(BackendIdentity::new(
                "lifecycle",
                "1",
                "test",
            )),
            calls: Arc::clone(&unsupported_calls),
            recovery: Recovery::Reattached,
            state: SessionState::Detached,
            fail_attach: false,
            fail_suspend: false,
        })
        .unwrap();
        unsupported.attach(&test_target()).unwrap();
        let error = unsupported.readback(request).unwrap_err();
        assert_eq!(error.code, "readback-unsupported");
        assert_eq!(
            *unsupported_calls.lock().unwrap(),
            vec![BackendOperation::Attach]
        );

        let wrong_format_calls = Arc::new(Mutex::new(Vec::new()));
        let mut wrong_format_capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("lifecycle", "1", "test"));
        wrong_format_capabilities.readback = true;
        wrong_format_capabilities
            .color_formats
            .insert(ColorFormat::Bgra8Srgb);
        let mut wrong_format = GraphicsBackendSession::new(LifecycleDriver {
            capabilities: wrong_format_capabilities,
            calls: Arc::clone(&wrong_format_calls),
            recovery: Recovery::Reattached,
            state: SessionState::Detached,
            fail_attach: false,
            fail_suspend: false,
        })
        .unwrap();
        wrong_format.attach(&test_target()).unwrap();
        let error = wrong_format.readback(request).unwrap_err();
        assert_eq!(error.code, "readback-color-format-unsupported");
        assert_eq!(
            *wrong_format_calls.lock().unwrap(),
            vec![BackendOperation::Attach]
        );

        let supported_calls = Arc::new(Mutex::new(Vec::new()));
        let mut supported_capabilities =
            GraphicsCapabilities::empty(BackendIdentity::new("lifecycle", "1", "test"));
        supported_capabilities.readback = true;
        supported_capabilities
            .color_formats
            .insert(ColorFormat::Rgba8Srgb);
        let mut supported = GraphicsBackendSession::new(LifecycleDriver {
            capabilities: supported_capabilities,
            calls: Arc::clone(&supported_calls),
            recovery: Recovery::Reattached,
            state: SessionState::Detached,
            fail_attach: false,
            fail_suspend: false,
        })
        .unwrap();
        supported.attach(&test_target()).unwrap();
        supported.readback(request).unwrap();
        assert_eq!(
            *supported_calls.lock().unwrap(),
            vec![BackendOperation::Attach, BackendOperation::Readback]
        );
    }
}
