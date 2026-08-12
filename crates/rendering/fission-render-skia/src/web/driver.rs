use fission_render::backend::{
    BackendError, BackendOperation, BackendResult, GraphicsBackendDriver, PresentReport, Readback,
    ReadbackRequest, RenderReport, SurfaceMetrics,
};
use fission_render::capabilities::{
    BackendIdentity, ColorFormat, DisplayOpKind, GraphicsCapabilities, RenderMode, TransformSupport,
};
use fission_render::diagnostics::{
    BackendDiagnostic, BackendDiagnostics, CacheDiagnostics, DiagnosticCategory,
    DiagnosticProvenance,
};
use fission_render::frame::{DamageRegion, FrameId, ValidatedInteractiveFrame};
use fission_render::surface::{
    LossKind, MemoryPressure, Recovery, SessionState, SurfaceKind, SurfaceTarget,
};
use fission_skia_sys::web::{
    decode, encode, Ack, AlphaMode, BackendPreference as WireBackendPreference, ColorSpace,
    DamageRect, DecodeLimits, Destroy, DestroyReason, ErrorCode, ErrorPacket, Frame, Init, Message,
    Packet, Resize, SessionId, SurfaceSize, HEADER_LEN,
};

use super::host::CanvasKitHost;
use super::resources::{ResourceMap, ResourceMapError};
use super::{compile_web_scene, WebCompileError};
use crate::error::{contract_error, contract_error_with_provenance};

const MAX_RECENT_EVENTS: usize = 64;
const MAX_HOST_ERROR_BYTES: usize = 4 * 1024;
const RESPONSE_LIMITS: DecodeLimits = DecodeLimits {
    max_packet_bytes: HEADER_LEN + 16 + MAX_HOST_ERROR_BYTES,
    max_resource_updates: 0,
    max_resource_bytes: 0,
    max_resource_slots: 0,
    max_frame_commands: 0,
    max_damage_rects: 0,
    max_error_message_bytes: MAX_HOST_ERROR_BYTES,
    max_surface_dimension: 0,
    max_scale_factor: 16.0,
};

/// Fission graphics driver backed by an injected synchronous CanvasKit host.
///
/// CanvasKit objects stay in the browser. The driver sends only owned protocol
/// packets and commits local lifecycle/resource state after a matching Ack.
pub struct CanvasKitDriver<H: CanvasKitHost> {
    host: H,
    backend_preference: CanvasKitBackendPreference,
    capabilities: GraphicsCapabilities,
    state: SessionState,
    metrics: Option<SurfaceMetrics>,
    session: Option<HostSession>,
    latest_session_id: u64,
    resources: ResourceMap,
    last_rendered: Option<FrameId>,
    diagnostics: BackendDiagnostics,
}

/// CanvasKit execution path requested by a Fission Web graphics profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasKitBackendPreference {
    /// Prefer WebGL and fall back to CanvasKit's software surface.
    Auto,
    /// Require CanvasKit's optimized software raster surface.
    Software,
    /// Require CanvasKit Ganesh over WebGL.
    WebGl,
    /// Require the separately qualified CanvasKit Graphite path.
    Graphite,
}

impl CanvasKitBackendPreference {
    const fn wire(self) -> WireBackendPreference {
        match self {
            Self::Auto => WireBackendPreference::Auto,
            Self::Software => WireBackendPreference::Software,
            Self::WebGl => WireBackendPreference::WebGl,
            Self::Graphite => WireBackendPreference::Graphite,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct HostSession {
    id: SessionId,
    last_command_sequence: u64,
    last_response_sequence: u64,
    poisoned: bool,
}

impl HostSession {
    fn new(id: SessionId) -> Self {
        Self {
            id,
            last_command_sequence: 0,
            last_response_sequence: 0,
            poisoned: false,
        }
    }
}

impl<H: CanvasKitHost> CanvasKitDriver<H> {
    pub fn new(host: H, backend_preference: CanvasKitBackendPreference) -> Self {
        let capabilities = canvaskit_capabilities(backend_preference);
        Self {
            host,
            backend_preference,
            state: SessionState::Detached,
            metrics: None,
            session: None,
            latest_session_id: 0,
            resources: ResourceMap::default(),
            last_rendered: None,
            diagnostics: BackendDiagnostics::new(
                capabilities.identity.clone(),
                SessionState::Detached,
            ),
            capabilities,
        }
    }

    fn require_state(
        &mut self,
        operation: BackendOperation,
        expected: &[SessionState],
    ) -> BackendResult<()> {
        if expected.contains(&self.state) {
            return Ok(());
        }
        let error = contract_error(
            operation,
            "canvaskit-invalid-session-state",
            DiagnosticCategory::Lifecycle,
            format!(
                "cannot perform {operation:?} while CanvasKit is {:?}; expected one of {expected:?}",
                self.state
            ),
        );
        self.record_error(&error);
        Err(error)
    }

    fn validate_target(
        &mut self,
        operation: BackendOperation,
        target: &dyn SurfaceTarget,
    ) -> BackendResult<SurfaceMetrics> {
        let descriptor = target.descriptor();
        if descriptor.kind != SurfaceKind::WebCanvas {
            let error = contract_error(
                operation,
                "canvaskit-target-unsupported",
                DiagnosticCategory::Capability,
                format!(
                    "CanvasKit accepts WebCanvas targets, not {:?}",
                    descriptor.kind
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        if descriptor.color_format != ColorFormat::Rgba8Srgb {
            let error = contract_error(
                operation,
                "canvaskit-color-format-unsupported",
                DiagnosticCategory::Capability,
                format!(
                    "CanvasKit requires Rgba8Srgb surfaces, not {:?}",
                    descriptor.color_format
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        Ok(SurfaceMetrics {
            size: descriptor.size,
            scale_factor: descriptor.scale_factor,
        })
    }

    fn validate_frame(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<SurfaceMetrics> {
        let Some(metrics) = self.metrics else {
            let error = contract_error(
                BackendOperation::Render,
                "canvaskit-surface-missing",
                DiagnosticCategory::Lifecycle,
                "CanvasKit has no attached surface metrics",
            );
            self.record_error(&error);
            return Err(error);
        };
        let metadata = frame.frame().metadata();
        let viewport = &metadata.viewport;
        if viewport.physical_size != metrics.size
            || viewport.scale_factor.get().to_bits() != metrics.scale_factor.get().to_bits()
        {
            let error = contract_error(
                BackendOperation::Render,
                "canvaskit-frame-surface-mismatch",
                DiagnosticCategory::Surface,
                format!(
                    "frame viewport {:?} at {}x does not match CanvasKit surface {:?} at {}x",
                    viewport.physical_size,
                    viewport.scale_factor.get(),
                    metrics.size,
                    metrics.scale_factor.get()
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        if metadata.frame_id.0 == 0
            || self
                .last_rendered
                .is_some_and(|last| metadata.frame_id.0 <= last.0)
        {
            let error = contract_error(
                BackendOperation::Render,
                "canvaskit-frame-id-not-monotonic",
                DiagnosticCategory::Lifecycle,
                format!(
                    "CanvasKit frame id {} must be greater than the last acknowledged frame id {}",
                    metadata.frame_id.0,
                    self.last_rendered.map_or(0, |frame_id| frame_id.0)
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        Ok(metrics)
    }

    fn start_session(
        &mut self,
        operation: BackendOperation,
        metrics: SurfaceMetrics,
    ) -> BackendResult<()> {
        let Some(next_session) = self.latest_session_id.checked_add(1) else {
            let error = contract_error(
                operation,
                "canvaskit-session-id-exhausted",
                DiagnosticCategory::Lifecycle,
                "CanvasKit session identifiers are exhausted",
            );
            self.record_error(&error);
            return Err(error);
        };
        self.latest_session_id = next_session;
        let id = SessionId::new(next_session).expect("an incremented session id is non-zero");
        self.session = Some(HostSession::new(id));
        self.resources = ResourceMap::default();
        self.last_rendered = None;

        let result = self.send_packet(
            operation,
            Packet::Init(Init {
                surface: surface_size(metrics),
                backend: self.backend_preference.wire(),
                color_space: ColorSpace::Srgb,
                alpha_mode: AlphaMode::Premultiplied,
            }),
        );
        if result.is_err()
            && self
                .session
                .as_ref()
                .is_some_and(|session| !session.poisoned)
        {
            // A canonical Error means Init was rejected atomically and no
            // browser session exists. Ambiguous transport/protocol failures
            // retain their poisoned identity for diagnostics and teardown.
            self.session = None;
        }
        result
    }

    fn send_packet(&mut self, operation: BackendOperation, packet: Packet) -> BackendResult<()> {
        let Some(session) = self.session.as_ref() else {
            let error = contract_error(
                operation,
                "canvaskit-session-missing",
                DiagnosticCategory::Lifecycle,
                "CanvasKit has no active host session",
            );
            self.record_error(&error);
            return Err(error);
        };
        if session.poisoned {
            let error = contract_error(
                operation,
                "canvaskit-session-poisoned",
                DiagnosticCategory::Lifecycle,
                "CanvasKit host state is ambiguous after an invalid or missing response",
            );
            self.record_error(&error);
            return Err(error);
        }

        let session_id = session.id;
        let Some(command_sequence) = session.last_command_sequence.checked_add(1) else {
            return Err(self.poisoned_response_error(
                operation,
                "canvaskit-command-sequence-exhausted",
                "CanvasKit command sequence is exhausted",
            ));
        };
        let Some(expected_response_sequence) = session.last_response_sequence.checked_add(1) else {
            return Err(self.poisoned_response_error(
                operation,
                "canvaskit-response-sequence-exhausted",
                "CanvasKit response sequence is exhausted",
            ));
        };
        let message = Message::new(session_id, command_sequence, packet);
        let request = match encode(&message) {
            Ok(request) => request,
            Err(error) => {
                let error = contract_error(
                    operation,
                    "canvaskit-request-invalid",
                    request_error_category(&message.packet),
                    format!("CanvasKit request validation failed: {error}"),
                );
                self.record_error(&error);
                return Err(error);
            }
        };

        let response = match self.host.exchange(request) {
            Ok(response) => response,
            Err(error) => {
                return Err(self.poisoned_response_error(
                    operation,
                    "canvaskit-host-exchange-failed",
                    format!("CanvasKit host exchange failed: {error}"),
                ))
            }
        };
        let response = match decode(&response, &RESPONSE_LIMITS) {
            Ok(response) => response,
            Err(error) => {
                return Err(self.poisoned_response_error(
                    operation,
                    "canvaskit-response-invalid",
                    format!("CanvasKit returned an invalid response packet: {error}"),
                ))
            }
        };

        if response.envelope.session != session_id {
            return Err(self.poisoned_response_error(
                operation,
                "canvaskit-response-session-mismatch",
                format!(
                    "CanvasKit response session {} does not match command session {}",
                    response.envelope.session.get(),
                    session_id.get()
                ),
            ));
        }
        if response.envelope.sequence != expected_response_sequence {
            return Err(self.poisoned_response_error(
                operation,
                "canvaskit-response-sequence-mismatch",
                format!(
                    "CanvasKit response sequence {} does not match expected sequence {expected_response_sequence}",
                    response.envelope.sequence
                ),
            ));
        }

        match response.packet {
            Packet::Ack(Ack {
                acknowledged_sequence,
            }) if acknowledged_sequence == command_sequence => {
                let session = self
                    .session
                    .as_mut()
                    .expect("the host session remains installed during exchange");
                session.last_command_sequence = command_sequence;
                session.last_response_sequence = expected_response_sequence;
                Ok(())
            }
            Packet::Ack(Ack {
                acknowledged_sequence,
            }) => Err(self.poisoned_response_error(
                operation,
                "canvaskit-ack-sequence-mismatch",
                format!(
                    "CanvasKit acknowledged command {acknowledged_sequence}, expected {command_sequence}"
                ),
            )),
            Packet::Error(error) if error.failed_sequence == command_sequence => {
                self.session
                    .as_mut()
                    .expect("the host session remains installed during exchange")
                    .last_response_sequence = expected_response_sequence;
                if error.code == ErrorCode::SurfaceLost {
                    self.state = SessionState::Lost;
                    self.last_rendered = None;
                }
                let error = self.remote_error(operation, error);
                self.record_error(&error);
                Err(error)
            }
            Packet::Error(error) => Err(self.poisoned_response_error(
                operation,
                "canvaskit-error-sequence-mismatch",
                format!(
                    "CanvasKit error targeted command {}, expected {command_sequence}",
                    error.failed_sequence
                ),
            )),
            packet => Err(self.poisoned_response_error(
                operation,
                "canvaskit-response-kind-invalid",
                format!(
                    "CanvasKit returned {:?}; only Ack or Error is valid",
                    packet.kind()
                ),
            )),
        }
    }

    fn poisoned_response_error(
        &mut self,
        operation: BackendOperation,
        code: &'static str,
        message: impl Into<String>,
    ) -> BackendError {
        if let Some(session) = self.session.as_mut() {
            session.poisoned = true;
        }
        self.state = SessionState::Lost;
        self.last_rendered = None;
        let error = contract_error(operation, code, DiagnosticCategory::Device, message);
        self.record_error(&error);
        error
    }

    fn remote_error(&self, operation: BackendOperation, error: ErrorPacket) -> BackendError {
        let (code, category) = match error.code {
            ErrorCode::InvalidPacket => {
                ("canvaskit-host-invalid-packet", DiagnosticCategory::Device)
            }
            ErrorCode::UnsupportedVersion => (
                "canvaskit-host-version-unsupported",
                DiagnosticCategory::Capability,
            ),
            ErrorCode::InvalidState => (
                "canvaskit-host-invalid-state",
                DiagnosticCategory::Lifecycle,
            ),
            ErrorCode::ResourceFailure => (
                "canvaskit-host-resource-failure",
                DiagnosticCategory::Resource,
            ),
            ErrorCode::SurfaceLost => ("canvaskit-host-surface-lost", DiagnosticCategory::Surface),
            ErrorCode::Internal => ("canvaskit-host-internal-error", DiagnosticCategory::Device),
        };
        contract_error(
            operation,
            code,
            category,
            format!(
                "CanvasKit host rejected command {} ({:?}): {}",
                error.failed_sequence, error.code, error.message
            ),
        )
    }

    fn resource_error(&mut self, error: ResourceMapError) -> BackendError {
        let error = contract_error(
            BackendOperation::Render,
            "canvaskit-resource-map-invalid",
            DiagnosticCategory::Resource,
            error.to_string(),
        );
        self.record_error(&error);
        error
    }

    fn compile_error(&mut self, error: WebCompileError, frame_id: FrameId) -> BackendError {
        let error = match error {
            WebCompileError::Scene(error) => {
                let provenance = DiagnosticProvenance {
                    frame_id: Some(frame_id),
                    node_id: error.provenance.node_id,
                    operation_index: error.provenance.operation_index(),
                };
                contract_error_with_provenance(
                    BackendOperation::Render,
                    error.diagnostic_code(),
                    error.diagnostic_category(),
                    error.to_string(),
                    provenance,
                )
            }
            WebCompileError::NativeResource(kind) => contract_error(
                BackendOperation::Render,
                "canvaskit-native-resource-unsupported",
                DiagnosticCategory::Resource,
                format!("CanvasKit cannot execute native-only {kind} resources"),
            ),
            WebCompileError::InvalidGeometry(field) => contract_error(
                BackendOperation::Render,
                "canvaskit-geometry-invalid",
                DiagnosticCategory::Surface,
                format!("CanvasKit received invalid {field}"),
            ),
            WebCompileError::CommandStream(error) => contract_error(
                BackendOperation::Render,
                "canvaskit-command-stream-invalid",
                DiagnosticCategory::Device,
                format!("CanvasKit command encoding failed: {error}"),
            ),
        };
        self.record_error(&error);
        error
    }

    fn record_error(&mut self, error: &BackendError) {
        if let Some(diagnostic) = &error.diagnostic {
            self.push_event(diagnostic.clone());
        }
    }

    fn push_event(&mut self, diagnostic: BackendDiagnostic) {
        self.diagnostics.recent_events.push(diagnostic);
        if self.diagnostics.recent_events.len() > MAX_RECENT_EVENTS {
            self.diagnostics.recent_events.remove(0);
        }
    }

    fn render_frame(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<RenderReport> {
        self.require_state(BackendOperation::Render, &[SessionState::Attached])?;
        let metrics = self.validate_frame(frame)?;
        let interactive = frame.frame();
        let metadata = interactive.metadata();
        let resource_plan = self
            .resources
            .plan(interactive.resources())
            .map_err(|error| self.resource_error(error))?;

        let (commands, encoded_operations, reused_layers) = if metrics.size.is_empty() {
            let commands = fission_skia_sys::web::encode_commands(&[]).map_err(|error| {
                self.compile_error(WebCompileError::CommandStream(error), metadata.frame_id)
            })?;
            (commands, 0, 0)
        } else {
            let compiled = compile_web_scene(
                interactive.scene(),
                metrics.scale_factor.get(),
                interactive.clear_color(),
            )
            .map_err(|error| self.compile_error(error, metadata.frame_id))?;
            (
                compiled.encoded_commands,
                compiled.source_operations,
                compiled.reused_layers,
            )
        };
        let damage = damage_rects(&metadata.damage, metrics);
        let uploaded_bytes = resource_plan.as_ref().map_or(0, |plan| plan.uploaded_bytes);

        if let Some(plan) = resource_plan {
            self.send_packet(
                BackendOperation::Render,
                Packet::ResourceBatch(plan.batch.clone()),
            )?;
            self.resources.commit(plan);
        }

        self.send_packet(
            BackendOperation::Render,
            Packet::Frame(Frame {
                frame_id: metadata.frame_id.0,
                resource_epoch: metadata.resource_epoch.0,
                semantics_epoch: metadata.semantics_epoch.0,
                surface: surface_size(metrics),
                clear_color: color_components(interactive.clear_color()),
                damage,
                commands,
            }),
        )?;

        self.last_rendered = Some(metadata.frame_id);
        self.diagnostics.counters.frames_rendered =
            self.diagnostics.counters.frames_rendered.saturating_add(1);
        Ok(RenderReport {
            frame_id: Some(metadata.frame_id),
            encoded_operations,
            reused_layers,
            uploaded_bytes,
        })
    }
}

impl<H: CanvasKitHost> GraphicsBackendDriver for CanvasKitDriver<H> {
    fn capabilities(&self) -> &GraphicsCapabilities {
        &self.capabilities
    }

    fn state(&self) -> SessionState {
        self.state
    }

    fn attach(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.require_state(BackendOperation::Attach, &[SessionState::Detached])?;
        self.state = SessionState::Lost;
        let metrics = self.validate_target(BackendOperation::Attach, target)?;
        self.start_session(BackendOperation::Attach, metrics)?;
        self.metrics = Some(metrics);
        self.state = SessionState::Attached;
        Ok(())
    }

    fn resize(&mut self, metrics: SurfaceMetrics) -> BackendResult<()> {
        self.require_state(BackendOperation::Resize, &[SessionState::Attached])?;
        self.send_packet(
            BackendOperation::Resize,
            Packet::Resize(Resize {
                surface: surface_size(metrics),
            }),
        )?;
        self.metrics = Some(metrics);
        self.last_rendered = None;
        Ok(())
    }

    fn render_validated(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<RenderReport> {
        let result = self.render_frame(frame);
        if result.is_err() {
            self.diagnostics.counters.dropped_frames =
                self.diagnostics.counters.dropped_frames.saturating_add(1);
        }
        result
    }

    fn present(&mut self) -> BackendResult<PresentReport> {
        self.require_state(BackendOperation::Present, &[SessionState::Attached])?;
        let Some(frame_id) = self.last_rendered else {
            let error = contract_error(
                BackendOperation::Present,
                "canvaskit-present-before-frame-ack",
                DiagnosticCategory::Lifecycle,
                "CanvasKit cannot present before a matching Frame Ack",
            );
            self.record_error(&error);
            return Err(error);
        };
        self.diagnostics.counters.frames_presented =
            self.diagnostics.counters.frames_presented.saturating_add(1);
        Ok(PresentReport {
            frame_id: Some(frame_id),
            recovered_surface: false,
        })
    }

    fn readback(&mut self, _request: ReadbackRequest) -> BackendResult<Readback> {
        let error = contract_error(
            BackendOperation::Readback,
            "canvaskit-readback-unsupported",
            DiagnosticCategory::Capability,
            "the CanvasKit bridge does not implement synchronous pixel readback",
        );
        self.record_error(&error);
        Err(error)
    }

    fn suspend(&mut self) -> BackendResult<()> {
        self.require_state(BackendOperation::Suspend, &[SessionState::Attached])?;
        self.state = SessionState::Lost;
        self.send_packet(
            BackendOperation::Suspend,
            Packet::Destroy(Destroy {
                reason: DestroyReason::Replaced,
            }),
        )?;
        self.session = None;
        self.resources = ResourceMap::default();
        self.last_rendered = None;
        self.state = SessionState::Suspended;
        Ok(())
    }

    fn resume(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.require_state(BackendOperation::Resume, &[SessionState::Suspended])?;
        self.state = SessionState::Lost;
        let metrics = self.validate_target(BackendOperation::Resume, target)?;
        self.start_session(BackendOperation::Resume, metrics)?;
        self.metrics = Some(metrics);
        self.state = SessionState::Attached;
        Ok(())
    }

    fn recover(&mut self, _loss: LossKind) -> BackendResult<Recovery> {
        self.require_state(
            BackendOperation::Recover,
            &[
                SessionState::Attached,
                SessionState::Suspended,
                SessionState::Lost,
            ],
        )?;
        self.state = SessionState::Lost;
        self.last_rendered = None;
        Ok(Recovery::Unrecoverable)
    }

    fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()> {
        let error = contract_error(
            BackendOperation::TrimMemory,
            "canvaskit-memory-pressure-unsupported",
            DiagnosticCategory::Capability,
            format!("the CanvasKit bridge cannot synchronously apply {pressure:?} memory pressure"),
        );
        self.record_error(&error);
        Err(error)
    }

    fn detach(&mut self) -> BackendResult<()> {
        self.require_state(
            BackendOperation::Detach,
            &[
                SessionState::Attached,
                SessionState::Suspended,
                SessionState::Lost,
            ],
        )?;
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.poisoned)
        {
            let error = contract_error(
                BackendOperation::Detach,
                "canvaskit-session-poisoned",
                DiagnosticCategory::Lifecycle,
                "cannot prove a safe Destroy sequence for the poisoned CanvasKit session",
            );
            self.record_error(&error);
            return Err(error);
        }
        self.state = SessionState::Lost;
        if self.session.is_some() {
            self.send_packet(
                BackendOperation::Detach,
                Packet::Destroy(Destroy {
                    reason: DestroyReason::Normal,
                }),
            )?;
        }
        self.session = None;
        self.metrics = None;
        self.resources = ResourceMap::default();
        self.last_rendered = None;
        self.state = SessionState::Detached;
        Ok(())
    }

    fn diagnostics(&self) -> BackendDiagnostics {
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.session_state = self.state;
        diagnostics.caches.push(CacheDiagnostics {
            name: "canvaskit-resource-map".into(),
            entries: self.resources.live_count() as u64,
            used_bytes: 0,
            budget_bytes: None,
            evictions: 0,
        });
        diagnostics
    }
}

fn canvaskit_capabilities(backend_preference: CanvasKitBackendPreference) -> GraphicsCapabilities {
    let mut capabilities = GraphicsCapabilities::empty(BackendIdentity::new(
        "skia",
        env!("CARGO_PKG_VERSION"),
        "web-canvaskit-shapes",
    ));
    match backend_preference {
        CanvasKitBackendPreference::Software => {
            capabilities.render_modes.insert(RenderMode::Software);
        }
        CanvasKitBackendPreference::Auto => {
            capabilities
                .render_modes
                .extend([RenderMode::Gpu, RenderMode::Software]);
        }
        CanvasKitBackendPreference::WebGl | CanvasKitBackendPreference::Graphite => {
            capabilities.render_modes.insert(RenderMode::Gpu);
        }
    }
    capabilities.display_ops.extend([
        DisplayOpKind::Save,
        DisplayOpKind::Restore,
        DisplayOpKind::ClipRect,
        DisplayOpKind::ClipRoundedRect,
        DisplayOpKind::OpacityLayer,
        DisplayOpKind::Translate,
        DisplayOpKind::Transform,
        DisplayOpKind::CachedScene,
        DisplayOpKind::BackdropFilter,
        DisplayOpKind::DrawRect,
        DisplayOpKind::DrawPath,
    ]);
    capabilities.transform_support = TransformSupport::Affine2d;
    capabilities.color_formats.insert(ColorFormat::Rgba8Srgb);
    capabilities
}

fn request_error_category(packet: &Packet) -> DiagnosticCategory {
    match packet {
        Packet::ResourceBatch(_) => DiagnosticCategory::Resource,
        Packet::Init(_) | Packet::Resize(_) | Packet::Frame(_) => DiagnosticCategory::Surface,
        Packet::Destroy(_) | Packet::Ack(_) | Packet::Error(_) => DiagnosticCategory::Lifecycle,
    }
}

fn surface_size(metrics: SurfaceMetrics) -> SurfaceSize {
    SurfaceSize {
        width: metrics.size.width,
        height: metrics.size.height,
        scale_factor: metrics.scale_factor.get() as f32,
    }
}

fn color_components(color: fission_render::Color) -> [f32; 4] {
    [
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        f32::from(color.a) / 255.0,
    ]
}

pub(super) fn damage_rects(damage: &DamageRegion, metrics: SurfaceMetrics) -> Vec<DamageRect> {
    match damage {
        DamageRegion::None => Vec::new(),
        DamageRegion::Full if metrics.size.is_empty() => Vec::new(),
        DamageRegion::Full => vec![DamageRect {
            x: 0,
            y: 0,
            width: metrics.size.width,
            height: metrics.size.height,
        }],
        DamageRegion::Rects(rects) => rects
            .iter()
            .filter_map(|rect| physical_damage_rect(*rect, metrics))
            .collect(),
    }
}

fn physical_damage_rect(
    rect: fission_render::LayoutRect,
    metrics: SurfaceMetrics,
) -> Option<DamageRect> {
    if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
        return None;
    }
    let scale = metrics.scale_factor.get();
    let width = f64::from(metrics.size.width);
    let height = f64::from(metrics.size.height);
    let left = (f64::from(rect.origin.x) * scale).floor().clamp(0.0, width);
    let top = (f64::from(rect.origin.y) * scale)
        .floor()
        .clamp(0.0, height);
    let right = (f64::from(rect.right()) * scale).ceil().clamp(0.0, width);
    let bottom = (f64::from(rect.bottom()) * scale).ceil().clamp(0.0, height);
    if right <= left || bottom <= top {
        return None;
    }
    let x = left as u32;
    let y = top as u32;
    Some(DamageRect {
        x,
        y,
        width: right as u32 - x,
        height: bottom as u32 - y,
    })
}
