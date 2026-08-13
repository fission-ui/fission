use std::sync::Arc;

use fission_render::backend::{
    BackendError, BackendOperation, BackendResult, GraphicsBackendDriver, PresentReport, Readback,
    ReadbackRequest, RenderReport, SurfaceMetrics,
};
use fission_render::capabilities::{
    BackendIdentity, ColorFormat, DisplayOpKind, GraphicsCapabilities, ImageSourceKind, RenderMode,
    SvgProfile, TextFeature, TransformSupport,
};
use fission_render::diagnostics::{
    BackendDiagnostic, BackendDiagnostics, CacheDiagnostics, DiagnosticCategory,
    DiagnosticProvenance, DiagnosticSeverity,
};
use fission_render::frame::{DamageRegion, FrameId, ValidatedInteractiveFrame};
use fission_render::surface::{
    LossKind, MemoryPressure, PhysicalSize, Recovery, SessionState, SurfaceKind, SurfaceTarget,
};
use fission_skia_sys::web::{
    decode, encode, Ack, AlphaMode, BackendPreference as WireBackendPreference, ColorSpace,
    DamageRect, DecodeLimits, Destroy, DestroyReason, ErrorCode, ErrorPacket, Frame, Init, Message,
    Packet, Resize, SessionId, SurfaceSize, HEADER_LEN,
};

use super::host::{CanvasKitHost, CanvasKitPixelRegion};
use super::profile::{CanvasKitFontCatalog, CanvasKitFontCatalogError};
use super::resources::{ResourceMap, ResourceMapError};
use super::{compile_web_scene_with_resources, WebCompileError};
use crate::error::{contract_error, contract_error_with_provenance};
use crate::paragraph_engine::{CanvasKitFontState, CanvasKitParagraphDrawDataRegistry};

const MAX_RECENT_EVENTS: usize = 64;
const MAX_LIFECYCLE_EVENTS_PER_POLL: usize = 32;
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
    paragraph_profile: Option<CanvasKitParagraphProfile>,
    last_accepted_frame: Option<FrameId>,
    // The browser executor renders and flushes atomically during Frame
    // exchange. Rust retains that Ack as one pending logical presentation so
    // callers cannot overwrite or report the same browser frame twice.
    pending_frame: Option<FrameId>,
    surface_restored: bool,
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

struct CanvasKitParagraphProfile {
    catalog: Arc<CanvasKitFontCatalog>,
    font_state: Arc<CanvasKitFontState>,
    draw_data: Arc<CanvasKitParagraphDrawDataRegistry>,
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
        Self::with_optional_paragraph_profile(host, backend_preference, None)
    }

    pub(crate) fn with_paragraph_profile(
        host: H,
        backend_preference: CanvasKitBackendPreference,
        catalog: Arc<CanvasKitFontCatalog>,
        font_state: Arc<CanvasKitFontState>,
        draw_data: Arc<CanvasKitParagraphDrawDataRegistry>,
    ) -> Self {
        Self::with_optional_paragraph_profile(
            host,
            backend_preference,
            Some(CanvasKitParagraphProfile {
                catalog,
                font_state,
                draw_data,
            }),
        )
    }

    fn with_optional_paragraph_profile(
        host: H,
        backend_preference: CanvasKitBackendPreference,
        paragraph_profile: Option<CanvasKitParagraphProfile>,
    ) -> Self {
        let readback =
            backend_preference != CanvasKitBackendPreference::Graphite && host.supports_readback();
        let capabilities =
            canvaskit_capabilities(backend_preference, paragraph_profile.is_some(), readback);
        Self {
            host,
            backend_preference,
            state: SessionState::Detached,
            metrics: None,
            session: None,
            latest_session_id: 0,
            resources: ResourceMap::default(),
            paragraph_profile,
            last_accepted_frame: None,
            pending_frame: None,
            surface_restored: false,
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
                .last_accepted_frame
                .is_some_and(|last| metadata.frame_id.0 <= last.0)
        {
            let error = contract_error(
                BackendOperation::Render,
                "canvaskit-frame-id-not-monotonic",
                DiagnosticCategory::Lifecycle,
                format!(
                    "CanvasKit frame id {} must be greater than the last acknowledged frame id {}",
                    metadata.frame_id.0,
                    self.last_accepted_frame.map_or(0, |frame_id| frame_id.0)
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
        self.last_accepted_frame = None;
        self.pending_frame = None;
        self.surface_restored = false;
        self.clear_paragraph_profile();

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
        result?;
        self.install_profile_fonts(operation)
    }

    fn install_profile_fonts(&mut self, operation: BackendOperation) -> BackendResult<()> {
        let Some(profile) = self.paragraph_profile.as_ref() else {
            return Ok(());
        };
        let catalog = Arc::clone(&profile.catalog);
        let font_state = Arc::clone(&profile.font_state);
        let plan = self
            .resources
            .plan(catalog.resources())
            .map_err(|error| self.resource_error(error))?
            .expect("a new CanvasKit session has an empty resource map");
        self.send_packet(operation, Packet::ResourceBatch(plan.batch.clone()))?;
        self.resources.commit(plan);
        let fonts = catalog
            .wire_fonts(&self.resources)
            .map_err(|error| self.font_catalog_error(operation, error))?;
        font_state.install(fonts);
        Ok(())
    }

    fn font_catalog_error(
        &mut self,
        operation: BackendOperation,
        error: CanvasKitFontCatalogError,
    ) -> BackendError {
        let error = contract_error(
            operation,
            "canvaskit-font-catalog-invalid",
            DiagnosticCategory::Resource,
            error.to_string(),
        );
        self.record_error(&error);
        error
    }

    fn clear_paragraph_profile(&self) {
        if let Some(profile) = self.paragraph_profile.as_ref() {
            profile.draw_data.clear();
            profile.font_state.clear();
        }
    }

    fn readback_region(
        &mut self,
        request: &ReadbackRequest,
        metrics: SurfaceMetrics,
    ) -> BackendResult<CanvasKitPixelRegion> {
        let Some(region) = request.region else {
            return Ok(CanvasKitPixelRegion {
                x: 0,
                y: 0,
                width: metrics.size.width,
                height: metrics.size.height,
            });
        };
        let scale = metrics.scale_factor.get();
        let left = f64::from(region.x()) * scale;
        let top = f64::from(region.y()) * scale;
        let right = f64::from(region.right()) * scale;
        let bottom = f64::from(region.bottom()) * scale;
        let values = [left, top, right, bottom];
        if values.iter().any(|value| !value.is_finite())
            || left < 0.0
            || top < 0.0
            || right < left
            || bottom < top
            || right > f64::from(metrics.size.width)
            || bottom > f64::from(metrics.size.height)
        {
            let error = contract_error(
                BackendOperation::Readback,
                "canvaskit-readback-region-invalid",
                DiagnosticCategory::Surface,
                format!(
                    "logical readback region {region:?} falls outside physical CanvasKit surface {:?}",
                    metrics.size
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        let x = left.floor() as u32;
        let y = top.floor() as u32;
        let right = right.ceil() as u32;
        let bottom = bottom.ceil() as u32;
        Ok(CanvasKitPixelRegion {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }

    fn validate_readback(
        &mut self,
        expected: CanvasKitPixelRegion,
        actual_size: PhysicalSize,
        row_bytes: usize,
        pixel_len: usize,
    ) -> BackendResult<()> {
        let minimum_row_bytes = usize::try_from(expected.width)
            .ok()
            .and_then(|width| width.checked_mul(4));
        let required_bytes = usize::try_from(expected.height)
            .ok()
            .and_then(|height| row_bytes.checked_mul(height));
        if actual_size != expected.size()
            || minimum_row_bytes.map_or(true, |minimum| row_bytes < minimum)
            || required_bytes != Some(pixel_len)
        {
            let error = contract_error(
                BackendOperation::Readback,
                "canvaskit-readback-contract-violation",
                DiagnosticCategory::Device,
                format!(
                    "CanvasKit returned size {actual_size:?}, row_bytes {row_bytes}, and {pixel_len} bytes for {expected:?}"
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        Ok(())
    }

    fn require_no_pending_frame(&mut self, operation: BackendOperation) -> BackendResult<()> {
        if self.pending_frame.is_none() {
            return Ok(());
        }
        let error = contract_error(
            operation,
            "canvaskit-present-pending",
            DiagnosticCategory::Lifecycle,
            "the acknowledged CanvasKit frame must be presented before rendering or resizing",
        );
        self.record_error(&error);
        Err(error)
    }

    fn poll_lifecycle_events(&mut self, operation: BackendOperation) -> BackendResult<()> {
        for index in 0..=MAX_LIFECYCLE_EVENTS_PER_POLL {
            let packet = match self.host.poll_lifecycle_event() {
                Ok(packet) => packet,
                Err(error) => {
                    return Err(self.poisoned_response_error(
                        operation,
                        "canvaskit-lifecycle-event-failed",
                        format!("CanvasKit lifecycle-event polling failed: {error}"),
                    ))
                }
            };
            let Some(packet) = packet else {
                return Ok(());
            };
            if index == MAX_LIFECYCLE_EVENTS_PER_POLL {
                return Err(self.poisoned_response_error(
                    operation,
                    "canvaskit-lifecycle-event-overflow",
                    format!(
                        "CanvasKit emitted more than {MAX_LIFECYCLE_EVENTS_PER_POLL} lifecycle events without yielding"
                    ),
                ));
            }
            self.accept_lifecycle_event(operation, &packet)?;
        }
        unreachable!("the bounded lifecycle-event loop always returns")
    }

    fn accept_lifecycle_event(
        &mut self,
        operation: BackendOperation,
        packet: &[u8],
    ) -> BackendResult<()> {
        let Some(session) = self.session else {
            return Err(self.poisoned_response_error(
                operation,
                "canvaskit-lifecycle-session-missing",
                "CanvasKit emitted a lifecycle event without an active Rust session",
            ));
        };
        if session.poisoned {
            return Err(self.poisoned_response_error(
                operation,
                "canvaskit-session-poisoned",
                "CanvasKit emitted a lifecycle event for an already poisoned session",
            ));
        }
        let response = match decode(packet, &RESPONSE_LIMITS) {
            Ok(response) => response,
            Err(error) => {
                return Err(self.poisoned_response_error(
                    operation,
                    "canvaskit-lifecycle-event-invalid",
                    format!("CanvasKit emitted an invalid lifecycle packet: {error}"),
                ))
            }
        };
        if response.envelope.session != session.id {
            return Err(self.poisoned_response_error(
                operation,
                "canvaskit-lifecycle-session-mismatch",
                format!(
                    "CanvasKit lifecycle session {} does not match active session {}",
                    response.envelope.session.get(),
                    session.id.get()
                ),
            ));
        }
        let Some(expected_response_sequence) = session.last_response_sequence.checked_add(1) else {
            return Err(self.poisoned_response_error(
                operation,
                "canvaskit-response-sequence-exhausted",
                "CanvasKit response sequence is exhausted",
            ));
        };
        if response.envelope.sequence != expected_response_sequence {
            return Err(self.poisoned_response_error(
                operation,
                "canvaskit-lifecycle-sequence-mismatch",
                format!(
                    "CanvasKit lifecycle response sequence {} does not match expected sequence {expected_response_sequence}",
                    response.envelope.sequence
                ),
            ));
        }

        match response.packet {
            Packet::Error(error) if error.failed_sequence == session.last_command_sequence => {
                self.session
                    .as_mut()
                    .expect("the lifecycle event belongs to the installed session")
                    .last_response_sequence = expected_response_sequence;
                self.state = SessionState::Lost;
                self.pending_frame = None;
                self.surface_restored = false;
                let error = self.remote_error(operation, error);
                self.record_error(&error);
                Ok(())
            }
            Packet::Ack(Ack {
                acknowledged_sequence,
            }) if acknowledged_sequence == session.last_command_sequence
                && self.state == SessionState::Lost =>
            {
                self.session
                    .as_mut()
                    .expect("the lifecycle event belongs to the installed session")
                    .last_response_sequence = expected_response_sequence;
                self.surface_restored = true;
                Ok(())
            }
            Packet::Error(error) => Err(self.poisoned_response_error(
                operation,
                "canvaskit-lifecycle-error-sequence-mismatch",
                format!(
                    "CanvasKit lifecycle error targeted command {}, expected {}",
                    error.failed_sequence, session.last_command_sequence
                ),
            )),
            Packet::Ack(Ack {
                acknowledged_sequence,
            }) => Err(self.poisoned_response_error(
                operation,
                "canvaskit-lifecycle-ack-invalid",
                format!(
                    "CanvasKit lifecycle Ack targeted command {acknowledged_sequence} while command {} was active in {:?}",
                    session.last_command_sequence, self.state
                ),
            )),
            packet => Err(self.poisoned_response_error(
                operation,
                "canvaskit-lifecycle-event-kind-invalid",
                format!(
                    "CanvasKit emitted {:?}; lifecycle events must be Ack or Error packets",
                    packet.kind()
                ),
            )),
        }
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
                    self.pending_frame = None;
                    self.surface_restored = false;
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
        self.pending_frame = None;
        self.surface_restored = false;
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
            WebCompileError::MissingResourceHandle { resource_id, kind } => contract_error(
                BackendOperation::Render,
                "canvaskit-resource-handle-missing",
                DiagnosticCategory::Resource,
                format!(
                    "CanvasKit could not resolve {kind} resource {resource_id:?} in the active resource plan"
                ),
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

    fn push_recovery_event(&mut self, recovery: Recovery) {
        self.push_event(BackendDiagnostic {
            severity: DiagnosticSeverity::Info,
            category: DiagnosticCategory::Surface,
            code: "canvaskit-surface-recovered".into(),
            message: format!("CanvasKit recovered its browser surface as {recovery:?}"),
            provenance: None,
        });
    }

    fn unrecoverable(&mut self, loss: LossKind, message: impl Into<String>) -> Recovery {
        self.push_event(BackendDiagnostic {
            severity: DiagnosticSeverity::Warning,
            category: match loss {
                LossKind::Surface => DiagnosticCategory::Surface,
                LossKind::Device => DiagnosticCategory::Device,
            },
            code: match loss {
                LossKind::Surface => "canvaskit-surface-recovery-unavailable",
                LossKind::Device => "canvaskit-device-recovery-unavailable",
            }
            .into(),
            message: message.into(),
            provenance: None,
        });
        Recovery::Unrecoverable
    }

    fn recover_surface(&mut self, metrics: SurfaceMetrics) -> BackendResult<Recovery> {
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.poisoned)
        {
            return Ok(self.unrecoverable(
                LossKind::Surface,
                "CanvasKit cannot recover a surface whose host protocol state is ambiguous",
            ));
        }

        if self.surface_restored && self.session.is_some() {
            self.surface_restored = false;
            self.pending_frame = None;
            self.state = SessionState::Attached;
            self.diagnostics.counters.surface_recoveries = self
                .diagnostics
                .counters
                .surface_recoveries
                .saturating_add(1);
            self.push_recovery_event(Recovery::Reattached);
            return Ok(Recovery::Reattached);
        }

        if self.session.is_some() {
            self.clear_paragraph_profile();
            self.send_packet(
                BackendOperation::Recover,
                Packet::Destroy(Destroy {
                    reason: DestroyReason::ContextLost,
                }),
            )?;
        }
        self.session = None;
        self.resources = ResourceMap::default();
        self.last_accepted_frame = None;
        self.pending_frame = None;
        self.surface_restored = false;

        self.start_session(BackendOperation::Recover, metrics)?;
        self.state = SessionState::Attached;
        self.diagnostics.counters.surface_recoveries = self
            .diagnostics
            .counters
            .surface_recoveries
            .saturating_add(1);
        self.push_recovery_event(Recovery::Reattached);
        Ok(Recovery::Reattached)
    }

    fn render_frame(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<RenderReport> {
        self.poll_lifecycle_events(BackendOperation::Render)?;
        self.require_state(BackendOperation::Render, &[SessionState::Attached])?;
        self.require_no_pending_frame(BackendOperation::Render)?;
        let metrics = self.validate_frame(frame)?;
        let interactive = frame.frame();
        let metadata = interactive.metadata();
        let wire_resource_epoch = if self.paragraph_profile.is_some() {
            metadata.resource_epoch.0.checked_add(1).ok_or_else(|| {
                self.font_catalog_error(
                    BackendOperation::Render,
                    CanvasKitFontCatalogError::EpochExhausted {
                        frame_epoch: metadata.resource_epoch.0,
                    },
                )
            })?
        } else {
            metadata.resource_epoch.0
        };
        let resource_plan = if let Some(profile) = self.paragraph_profile.as_ref() {
            self.resources.plan_with_pinned(
                interactive.resources(),
                profile.catalog.resources(),
                wire_resource_epoch,
            )
        } else {
            self.resources.plan(interactive.resources())
        }
        .map_err(|error| self.resource_error(error))?;

        let (commands, encoded_operations, reused_layers) = if metrics.size.is_empty() {
            let commands = fission_skia_sys::web::encode_commands(&[]).map_err(|error| {
                self.compile_error(WebCompileError::CommandStream(error), metadata.frame_id)
            })?;
            (commands, 0, 0)
        } else {
            let compiled = compile_web_scene_with_resources(
                interactive.scene(),
                metrics.scale_factor.get(),
                interactive.clear_color(),
                interactive.resources(),
                interactive.paragraph_bindings(),
                self.paragraph_profile
                    .as_ref()
                    .map(|profile| profile.draw_data.as_ref()),
                &|resource_id| {
                    self.resources
                        .handle_after(resource_plan.as_ref(), resource_id)
                },
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
                resource_epoch: wire_resource_epoch,
                semantics_epoch: metadata.semantics_epoch.0,
                surface: surface_size(metrics),
                clear_color: color_components(interactive.clear_color()),
                damage,
                commands,
            }),
        )?;

        self.last_accepted_frame = Some(metadata.frame_id);
        self.pending_frame = Some(metadata.frame_id);
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
        self.metrics = Some(metrics);
        self.start_session(BackendOperation::Attach, metrics)?;
        self.state = SessionState::Attached;
        Ok(())
    }

    fn resize(&mut self, metrics: SurfaceMetrics) -> BackendResult<()> {
        self.poll_lifecycle_events(BackendOperation::Resize)?;
        self.require_state(BackendOperation::Resize, &[SessionState::Attached])?;
        self.require_no_pending_frame(BackendOperation::Resize)?;
        self.send_packet(
            BackendOperation::Resize,
            Packet::Resize(Resize {
                surface: surface_size(metrics),
            }),
        )?;
        self.metrics = Some(metrics);
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
        self.poll_lifecycle_events(BackendOperation::Present)?;
        self.require_state(BackendOperation::Present, &[SessionState::Attached])?;
        let Some(frame_id) = self.pending_frame.take() else {
            let error = contract_error(
                BackendOperation::Present,
                "canvaskit-present-without-pending-frame",
                DiagnosticCategory::Lifecycle,
                "CanvasKit has no acknowledged frame pending presentation",
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

    fn readback(&mut self, request: ReadbackRequest) -> BackendResult<Readback> {
        self.poll_lifecycle_events(BackendOperation::Readback)?;
        self.require_state(BackendOperation::Readback, &[SessionState::Attached])?;
        if !self.capabilities.readback {
            let error = contract_error(
                BackendOperation::Readback,
                "canvaskit-readback-unsupported",
                DiagnosticCategory::Capability,
                "the active CanvasKit host does not implement synchronous pixel readback",
            );
            self.record_error(&error);
            return Err(error);
        }
        if request.color_format != ColorFormat::Rgba8Srgb {
            let error = contract_error(
                BackendOperation::Readback,
                "canvaskit-readback-color-format-unsupported",
                DiagnosticCategory::Capability,
                format!(
                    "CanvasKit produces Rgba8Srgb readback, not {:?}",
                    request.color_format
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        if self.last_accepted_frame.is_none() {
            let error = contract_error(
                BackendOperation::Readback,
                "canvaskit-readback-before-render",
                DiagnosticCategory::Lifecycle,
                "cannot read uninitialized CanvasKit pixels before the first frame",
            );
            self.record_error(&error);
            return Err(error);
        }
        let Some(metrics) = self.metrics else {
            let error = contract_error(
                BackendOperation::Readback,
                "canvaskit-readback-surface-missing",
                DiagnosticCategory::Lifecycle,
                "cannot read back without attached CanvasKit surface metrics",
            );
            self.record_error(&error);
            return Err(error);
        };
        let region = self.readback_region(&request, metrics)?;
        if region.width == 0 || region.height == 0 {
            return Ok(Readback {
                size: region.size(),
                color_format: request.color_format,
                row_bytes: 0,
                pixels: Vec::new(),
            });
        }
        let readback = match self.host.read_pixels_rgba8888(region) {
            Ok(Some(readback)) => readback,
            Ok(None) => {
                let error = contract_error(
                    BackendOperation::Readback,
                    "canvaskit-readback-contract-violation",
                    DiagnosticCategory::Device,
                    "the CanvasKit host advertised readback but returned no pixel result",
                );
                self.record_error(&error);
                return Err(error);
            }
            Err(error) => {
                let error = contract_error(
                    BackendOperation::Readback,
                    "canvaskit-readback-failed",
                    DiagnosticCategory::Device,
                    format!("CanvasKit pixel readback failed: {error}"),
                );
                self.record_error(&error);
                return Err(error);
            }
        };
        self.validate_readback(
            region,
            readback.size,
            readback.row_bytes,
            readback.pixels.len(),
        )?;
        Ok(Readback {
            size: readback.size,
            color_format: request.color_format,
            row_bytes: readback.row_bytes,
            pixels: readback.pixels,
        })
    }

    fn suspend(&mut self) -> BackendResult<()> {
        self.poll_lifecycle_events(BackendOperation::Suspend)?;
        self.require_state(BackendOperation::Suspend, &[SessionState::Attached])?;
        self.state = SessionState::Lost;
        self.clear_paragraph_profile();
        self.send_packet(
            BackendOperation::Suspend,
            Packet::Destroy(Destroy {
                reason: DestroyReason::Replaced,
            }),
        )?;
        self.session = None;
        self.resources = ResourceMap::default();
        self.last_accepted_frame = None;
        self.pending_frame = None;
        self.surface_restored = false;
        self.state = SessionState::Suspended;
        Ok(())
    }

    fn resume(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.require_state(BackendOperation::Resume, &[SessionState::Suspended])?;
        self.state = SessionState::Lost;
        let metrics = self.validate_target(BackendOperation::Resume, target)?;
        self.metrics = Some(metrics);
        self.start_session(BackendOperation::Resume, metrics)?;
        self.state = SessionState::Attached;
        Ok(())
    }

    fn recover(&mut self, loss: LossKind) -> BackendResult<Recovery> {
        self.poll_lifecycle_events(BackendOperation::Recover)?;
        self.require_state(
            BackendOperation::Recover,
            &[
                SessionState::Attached,
                SessionState::Suspended,
                SessionState::Lost,
            ],
        )?;
        self.state = SessionState::Lost;
        self.pending_frame = None;

        if loss == LossKind::Device {
            self.surface_restored = false;
            return Ok(self.unrecoverable(
                loss,
                "the synchronous CanvasKit host cannot recreate its owning JavaScript module after device loss",
            ));
        }
        let Some(metrics) = self.metrics else {
            return Ok(self.unrecoverable(
                loss,
                "CanvasKit cannot recover a surface before valid canvas metrics have been retained",
            ));
        };
        self.recover_surface(metrics)
    }

    fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()> {
        match self.host.trim_memory(pressure) {
            Ok(true) => Ok(()),
            Ok(false) => {
                let error = contract_error(
                    BackendOperation::TrimMemory,
                    "canvaskit-memory-pressure-unsupported",
                    DiagnosticCategory::Capability,
                    format!("the active CanvasKit host cannot apply {pressure:?} memory pressure"),
                );
                self.record_error(&error);
                Err(error)
            }
            Err(error) => {
                let error = contract_error(
                    BackendOperation::TrimMemory,
                    "canvaskit-memory-pressure-failed",
                    DiagnosticCategory::Device,
                    format!("CanvasKit failed to apply {pressure:?} memory pressure: {error}"),
                );
                self.record_error(&error);
                Err(error)
            }
        }
    }

    fn detach(&mut self) -> BackendResult<()> {
        self.poll_lifecycle_events(BackendOperation::Detach)?;
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
        self.clear_paragraph_profile();
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
        self.last_accepted_frame = None;
        self.pending_frame = None;
        self.surface_restored = false;
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

impl<H: CanvasKitHost> Drop for CanvasKitDriver<H> {
    fn drop(&mut self) {
        if self
            .session
            .as_ref()
            .is_some_and(|session| !session.poisoned)
        {
            // A healthy protocol session can be retired deterministically even
            // when its owning GraphicsBackendSession was dropped without an
            // explicit detach. Ambiguous sessions deliberately avoid guessing
            // a sequence; the concrete host adapter must destroy its executor
            // when that adapter itself is dropped.
            self.state = SessionState::Lost;
            self.clear_paragraph_profile();
            let _ = self.send_packet(
                BackendOperation::Detach,
                Packet::Destroy(Destroy {
                    reason: DestroyReason::HostShutdown,
                }),
            );
        }
        self.session = None;
        self.resources = ResourceMap::default();
        self.last_accepted_frame = None;
        self.pending_frame = None;
        self.surface_restored = false;
        self.state = SessionState::Detached;
    }
}

fn canvaskit_capabilities(
    backend_preference: CanvasKitBackendPreference,
    paragraph_paint: bool,
    readback: bool,
) -> GraphicsCapabilities {
    let profile = match backend_preference {
        CanvasKitBackendPreference::Auto => "web-canvaskit-auto",
        CanvasKitBackendPreference::Software => "web-canvaskit-software",
        CanvasKitBackendPreference::WebGl => "web-canvaskit-webgl",
        CanvasKitBackendPreference::Graphite => "web-canvaskit-graphite-unavailable",
    };
    let mut capabilities = GraphicsCapabilities::empty(BackendIdentity::new(
        "skia",
        env!("CARGO_PKG_VERSION"),
        profile,
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
        CanvasKitBackendPreference::WebGl => {
            capabilities.render_modes.insert(RenderMode::Gpu);
        }
        // The checked-in executor rejects Graphite until its asynchronous Dawn
        // completion and teardown contract is implemented and qualified.
        CanvasKitBackendPreference::Graphite => {}
    }
    if backend_preference == CanvasKitBackendPreference::Graphite {
        capabilities.color_formats.insert(ColorFormat::Rgba8Srgb);
        return capabilities;
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
        DisplayOpKind::DrawImage,
        DisplayOpKind::DrawPath,
        DisplayOpKind::DrawSvg,
    ]);
    capabilities.image_sources.extend([
        ImageSourceKind::Asset,
        ImageSourceKind::File,
        ImageSourceKind::Network,
        ImageSourceKind::Memory,
    ]);
    capabilities.svg_profile = SvgProfile::GeometryWithFissionPaint;
    if paragraph_paint {
        capabilities
            .display_ops
            .extend([DisplayOpKind::DrawText, DisplayOpKind::DrawRichText]);
        capabilities.text_features.extend([
            TextFeature::CaretPainting,
            TextFeature::RichTextLocale,
            TextFeature::RichTextLineHeight,
            TextFeature::RichTextLetterSpacing,
        ]);
    }
    capabilities.transform_support = TransformSupport::Affine2d;
    capabilities.color_formats.insert(ColorFormat::Rgba8Srgb);
    capabilities.surface_loss_recovery = true;
    capabilities.readback = readback;
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
