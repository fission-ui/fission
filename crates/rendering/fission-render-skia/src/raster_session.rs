//! Backend-owned headless lifecycle for Skia software rendering.

use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};

use fission_render::backend::{
    BackendError, BackendOperation, BackendResult, GraphicsBackendDriver, GraphicsBackendSession,
    PresentReport, Readback, ReadbackRequest, RenderReport, SurfaceMetrics,
};
use fission_render::capabilities::{ColorFormat, GraphicsCapabilities};
use fission_render::diagnostics::{BackendDiagnostics, DiagnosticCategory};
use fission_render::frame::InteractiveFrame;
use fission_render::surface::{
    LossKind, MemoryPressure, Recovery, SessionState, SurfaceDescriptor, SurfaceId, SurfaceKind,
    SurfaceTarget, ThreadAffinity,
};

use crate::error::contract_error;
use crate::SkiaRasterDriver;

static NEXT_RASTER_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

/// An attached, backend-owned Skia software-rendering session.
///
/// This is the production adapter for deterministic headless rendering and
/// raster-buffer presentation. It keeps Skia's headless target private, owns
/// the complete [`GraphicsBackendSession`] lifecycle, and exposes only Fission
/// frame, surface, diagnostic, and readback types.
///
/// Construct it through [`crate::SkiaRasterProfile::create_headless_session`]
/// so paragraph layout and paint share the profile's authoritative draw data.
pub struct SkiaRasterSession {
    session: GraphicsBackendSession<'static>,
    target: HeadlessRasterTarget,
}

impl SkiaRasterSession {
    pub(crate) fn attach(driver: SkiaRasterDriver, metrics: SurfaceMetrics) -> BackendResult<Self> {
        Self::attach_driver(driver, metrics)
    }

    fn attach_driver(
        driver: impl GraphicsBackendDriver + 'static,
        metrics: SurfaceMetrics,
    ) -> BackendResult<Self> {
        let target = HeadlessRasterTarget::new(metrics);
        let mut session = GraphicsBackendSession::new(driver)?;
        session.attach(&target)?;
        Ok(Self { session, target })
    }

    /// Returns the exact capability profile used by this session's frame gate.
    pub fn capabilities(&self) -> &GraphicsCapabilities {
        self.session.capabilities()
    }

    /// Returns the authoritative Fission graphics-session state.
    pub fn state(&self) -> SessionState {
        self.session.state()
    }

    /// Returns the currently attached headless surface metrics.
    pub fn surface_metrics(&self) -> SurfaceMetrics {
        self.target.metrics()
    }

    /// Replaces the raster surface when its physical size or scale changes.
    pub fn resize(&mut self, metrics: SurfaceMetrics) -> BackendResult<()> {
        if self.session.state() == SessionState::Attached && self.target.metrics() == metrics {
            return Ok(());
        }
        self.session.resize(metrics)?;
        self.target.update(metrics);
        Ok(())
    }

    /// Validates and renders one interactive frame without reading it back.
    pub fn render(&mut self, frame: &InteractiveFrame<'_>) -> BackendResult<RenderReport> {
        self.session.render(frame)
    }

    /// Completes the headless presentation step for the last rendered frame.
    pub fn present(&mut self) -> BackendResult<PresentReport> {
        self.session.present()
    }

    /// Reads pixels from the current raster surface.
    pub fn readback(&mut self, request: ReadbackRequest) -> BackendResult<Readback> {
        self.session.readback(request)
    }

    /// Renders, presents, and returns tightly packed premultiplied RGBA pixels.
    ///
    /// A native row stride is removed before returning. Exactly one recovery
    /// and complete-frame retry is attempted for explicit Skia surface-loss or
    /// device-loss errors. All other failures are returned unchanged.
    pub fn render_to_rgba(&mut self, frame: &InteractiveFrame<'_>) -> BackendResult<Vec<u8>> {
        match self.render_to_rgba_once(frame) {
            Ok(pixels) => Ok(pixels),
            Err(error) => {
                let Some(loss) = recoverable_loss(&error) else {
                    return Err(error);
                };
                if self.recover(loss)? == Recovery::Unrecoverable {
                    return Err(error);
                }
                self.render_to_rgba_once(frame)
            }
        }
    }

    /// Releases the raster surface while retaining the backend runtime.
    pub fn suspend(&mut self) -> BackendResult<()> {
        self.session.suspend()
    }

    /// Resumes with a headless surface using the supplied current metrics.
    pub fn resume(&mut self, metrics: SurfaceMetrics) -> BackendResult<()> {
        let next_target = self.target.with_metrics(metrics);
        self.session.resume(&next_target)?;
        self.target = next_target;
        Ok(())
    }

    /// Recreates the lost raster surface or complete Skia raster runtime.
    pub fn recover(&mut self, loss: LossKind) -> BackendResult<Recovery> {
        self.session.recover(loss)
    }

    /// Trims backend-owned caches in response to host memory pressure.
    pub fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()> {
        self.session.trim_memory(pressure)
    }

    /// Returns the current renderer counters, cache state, and recent events.
    pub fn diagnostics(&self) -> BackendDiagnostics {
        self.session.diagnostics()
    }

    /// Tears down the raster surface, context, and engine in dependency order.
    ///
    /// Detaching an already detached adapter is intentionally harmless so host
    /// shutdown and `Drop` can share the same cleanup path.
    pub fn detach(&mut self) -> BackendResult<()> {
        if self.session.state() == SessionState::Detached {
            return Ok(());
        }
        self.session.detach()
    }

    fn render_to_rgba_once(&mut self, frame: &InteractiveFrame<'_>) -> BackendResult<Vec<u8>> {
        let expected_frame_id = frame.metadata().frame_id;
        let render = self.render(frame)?;
        if render.frame_id != Some(expected_frame_id) {
            return Err(contract_error(
                BackendOperation::Render,
                "skia-render-frame-id-mismatch",
                DiagnosticCategory::Lifecycle,
                format!(
                    "Skia raster rendered frame {:?}; the submitted frame was {:?}",
                    render.frame_id, expected_frame_id
                ),
            ));
        }

        let present = self.present()?;
        if present.frame_id != Some(expected_frame_id) {
            return Err(contract_error(
                BackendOperation::Present,
                "skia-present-frame-id-mismatch",
                DiagnosticCategory::Lifecycle,
                format!(
                    "Skia raster presented frame {:?}; the submitted frame was {:?}",
                    present.frame_id, expected_frame_id
                ),
            ));
        }

        let readback = self.readback(ReadbackRequest {
            region: None,
            color_format: ColorFormat::Rgba8Srgb,
        })?;
        tight_rgba(readback, self.target.descriptor.size)
    }
}

impl Drop for SkiaRasterSession {
    fn drop(&mut self) {
        let _ = self.detach();
    }
}

#[derive(Debug, Clone)]
struct HeadlessRasterTarget {
    descriptor: SurfaceDescriptor,
}

impl HeadlessRasterTarget {
    fn new(metrics: SurfaceMetrics) -> Self {
        Self {
            descriptor: SurfaceDescriptor {
                id: SurfaceId(NEXT_RASTER_SURFACE_ID.fetch_add(1, Ordering::Relaxed)),
                kind: SurfaceKind::Headless,
                size: metrics.size,
                scale_factor: metrics.scale_factor,
                color_format: ColorFormat::Rgba8Srgb,
                thread_affinity: ThreadAffinity::CreatingThread,
            },
        }
    }

    fn metrics(&self) -> SurfaceMetrics {
        SurfaceMetrics {
            size: self.descriptor.size,
            scale_factor: self.descriptor.scale_factor,
        }
    }

    fn update(&mut self, metrics: SurfaceMetrics) {
        self.descriptor.size = metrics.size;
        self.descriptor.scale_factor = metrics.scale_factor;
    }

    fn with_metrics(&self, metrics: SurfaceMetrics) -> Self {
        let mut target = self.clone();
        target.update(metrics);
        target
    }
}

impl SurfaceTarget for HeadlessRasterTarget {
    fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn tight_rgba(
    readback: Readback,
    expected: fission_render::surface::PhysicalSize,
) -> BackendResult<Vec<u8>> {
    if readback.size != expected || readback.color_format != ColorFormat::Rgba8Srgb {
        return Err(contract_error(
            BackendOperation::Readback,
            "skia-readback-metadata-mismatch",
            DiagnosticCategory::Surface,
            format!(
                "Skia raster readback returned {:?} {:?}; expected {:?} Rgba8Srgb",
                readback.size, readback.color_format, expected
            ),
        ));
    }

    if expected.is_empty() {
        if readback.row_bytes == 0 && readback.pixels.is_empty() {
            return Ok(Vec::new());
        }
        return Err(contract_error(
            BackendOperation::Readback,
            "skia-readback-layout-invalid",
            DiagnosticCategory::Surface,
            "Skia raster returned storage for an empty readback region",
        ));
    }

    let tight_row_bytes = usize::try_from(expected.width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or_else(|| readback_size_overflow("width", expected.width))?;
    let height = usize::try_from(expected.height)
        .map_err(|_| readback_size_overflow("height", expected.height))?;
    let source_len = readback
        .row_bytes
        .checked_mul(height)
        .ok_or_else(|| readback_size_overflow("row stride", readback.row_bytes))?;
    if readback.row_bytes < tight_row_bytes || readback.pixels.len() != source_len {
        return Err(contract_error(
            BackendOperation::Readback,
            "skia-readback-layout-invalid",
            DiagnosticCategory::Surface,
            format!(
                "Skia raster readback returned row stride {} and {} bytes for {}x{} RGBA pixels",
                readback.row_bytes,
                readback.pixels.len(),
                expected.width,
                expected.height
            ),
        ));
    }
    if readback.row_bytes == tight_row_bytes {
        return Ok(readback.pixels);
    }

    let destination_len = tight_row_bytes
        .checked_mul(height)
        .ok_or_else(|| readback_size_overflow("tight buffer", tight_row_bytes))?;
    let mut pixels = Vec::with_capacity(destination_len);
    for source_row in readback.pixels.chunks_exact(readback.row_bytes) {
        pixels.extend_from_slice(&source_row[..tight_row_bytes]);
    }
    Ok(pixels)
}

fn readback_size_overflow(field: &'static str, value: impl std::fmt::Display) -> BackendError {
    contract_error(
        BackendOperation::Readback,
        "skia-readback-size-overflow",
        DiagnosticCategory::Surface,
        format!("Skia raster readback {field} value {value} overflows the RGBA buffer size"),
    )
}

fn recoverable_loss(error: &BackendError) -> Option<LossKind> {
    match error.code.as_str() {
        "skia-surface-lost" => Some(LossKind::Surface),
        "skia-device-lost" => Some(LossKind::Device),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
