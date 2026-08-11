use std::marker::PhantomData;
use std::rc::Rc;

use fission_render::backend::{
    BackendError, BackendOperation, BackendResult, GraphicsBackendDriver, PresentReport, Readback,
    ReadbackRequest, RenderReport, SurfaceMetrics,
};
use fission_render::capabilities::{ColorFormat, GraphicsCapabilities};
use fission_render::diagnostics::{
    BackendDiagnostic, BackendDiagnostics, DiagnosticCategory, DiagnosticSeverity,
};
use fission_render::frame::{FrameId, ValidatedInteractiveFrame};
use fission_render::surface::{
    LossKind, MemoryPressure, PhysicalSize, Recovery, SessionState, SurfaceKind, SurfaceTarget,
};

use crate::api::{ApiError, ApiErrorKind, PixelRegion, SkiaApi};
use crate::capabilities::skia_raster_capabilities;
use crate::compiler::compile_scene;
use crate::error::{api_error, contract_error, wrong_thread};
use crate::native::NativeSkiaApi;
use crate::thread_owner::ThreadOwner;

const MAX_RECENT_EVENTS: usize = 64;

/// Safe Fission graphics driver for the direct Skia raster ABI.
///
/// The driver intentionally has no public accessors for its native engine,
/// context, or surface. Its `Rc` marker also makes the ownership contract
/// structurally `!Send + !Sync`; every operation independently checks the
/// creating thread before entering the native bridge.
pub struct SkiaRasterDriver {
    inner: RasterDriver<NativeSkiaApi>,
}

impl SkiaRasterDriver {
    pub fn new() -> BackendResult<Self> {
        Ok(Self {
            inner: RasterDriver::try_new(NativeSkiaApi)?,
        })
    }
}

impl GraphicsBackendDriver for SkiaRasterDriver {
    fn capabilities(&self) -> &GraphicsCapabilities {
        self.inner.capabilities()
    }

    fn state(&self) -> SessionState {
        self.inner.state()
    }

    fn attach(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.inner.attach(target)
    }

    fn resize(&mut self, metrics: SurfaceMetrics) -> BackendResult<()> {
        self.inner.resize(metrics)
    }

    fn render_validated(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<RenderReport> {
        self.inner.render_validated(frame)
    }

    fn present(&mut self) -> BackendResult<PresentReport> {
        self.inner.present()
    }

    fn readback(&mut self, request: ReadbackRequest) -> BackendResult<Readback> {
        self.inner.readback(request)
    }

    fn suspend(&mut self) -> BackendResult<()> {
        self.inner.suspend()
    }

    fn resume(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.inner.resume(target)
    }

    fn recover(&mut self, loss: LossKind) -> BackendResult<Recovery> {
        self.inner.recover(loss)
    }

    fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()> {
        self.inner.trim_memory(pressure)
    }

    fn detach(&mut self) -> BackendResult<()> {
        self.inner.detach()
    }

    fn diagnostics(&self) -> BackendDiagnostics {
        self.inner.diagnostics()
    }
}

struct RasterDriver<A: SkiaApi> {
    api: A,
    owner: ThreadOwner,
    capabilities: GraphicsCapabilities,
    state: SessionState,
    engine: Option<A::Engine>,
    context: Option<A::Context>,
    surface: Option<A::Surface>,
    metrics: Option<SurfaceMetrics>,
    last_rendered: Option<FrameId>,
    diagnostics: BackendDiagnostics,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<A: SkiaApi> RasterDriver<A> {
    fn try_new(api: A) -> BackendResult<Self> {
        let owner = ThreadOwner::current();
        let engine = api
            .create_engine()
            .map_err(|error| api_error(BackendOperation::Initialize, error))?;
        let context = api
            .create_raster_context(&engine)
            .map_err(|error| api_error(BackendOperation::Initialize, error))?;
        let capabilities = skia_raster_capabilities();
        Ok(Self {
            api,
            owner,
            capabilities: capabilities.clone(),
            state: SessionState::Detached,
            engine: Some(engine),
            context: Some(context),
            surface: None,
            metrics: None,
            last_rendered: None,
            diagnostics: BackendDiagnostics::new(capabilities.identity, SessionState::Detached),
            _not_send_or_sync: PhantomData,
        })
    }

    fn check_thread(&mut self, operation: BackendOperation) -> BackendResult<()> {
        if let Err(error) = self.owner.check() {
            let error = wrong_thread(operation, error);
            self.record_error(&error);
            Err(error)
        } else {
            Ok(())
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
            "skia-invalid-session-state",
            DiagnosticCategory::Lifecycle,
            format!(
                "cannot perform {operation:?} while Skia raster is {:?}; expected one of {expected:?}",
                self.state
            ),
        );
        self.record_error(&error);
        Err(error)
    }

    fn ensure_runtime(&mut self, operation: BackendOperation) -> BackendResult<()> {
        match (self.engine.is_some(), self.context.is_some()) {
            (true, true) => return Ok(()),
            (false, false) => {}
            _ => {
                let error = contract_error(
                    operation,
                    "skia-inconsistent-runtime",
                    DiagnosticCategory::Lifecycle,
                    "Skia engine and raster context ownership became inconsistent",
                );
                self.record_error(&error);
                return Err(error);
            }
        }

        let engine = match self.api.create_engine() {
            Ok(engine) => engine,
            Err(error) => return Err(self.handle_api_error(operation, error)),
        };
        let context = match self.api.create_raster_context(&engine) {
            Ok(context) => context,
            Err(error) => return Err(self.handle_api_error(operation, error)),
        };
        self.engine = Some(engine);
        self.context = Some(context);
        Ok(())
    }

    fn validate_target(
        &mut self,
        operation: BackendOperation,
        target: &dyn SurfaceTarget,
    ) -> BackendResult<SurfaceMetrics> {
        let descriptor = target.descriptor();
        if descriptor.kind != SurfaceKind::Headless {
            let error = contract_error(
                operation,
                "skia-raster-target-unsupported",
                DiagnosticCategory::Capability,
                format!(
                    "the Skia raster foundation accepts Headless targets, not {:?}",
                    descriptor.kind
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        if descriptor.color_format != ColorFormat::Rgba8Srgb {
            let error = contract_error(
                operation,
                "skia-raster-color-format-unsupported",
                DiagnosticCategory::Capability,
                format!(
                    "the Skia raster foundation requires Rgba8Srgb, not {:?}",
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

    fn create_surface(
        &mut self,
        operation: BackendOperation,
        size: PhysicalSize,
    ) -> BackendResult<Option<A::Surface>> {
        if size.is_empty() {
            return Ok(None);
        }
        let Some(context) = self.context.as_ref() else {
            let error = contract_error(
                operation,
                "skia-raster-context-missing",
                DiagnosticCategory::Lifecycle,
                "cannot create a Skia raster surface without a raster context",
            );
            self.record_error(&error);
            return Err(error);
        };
        match self.api.create_raster_surface(context, size) {
            Ok(surface) => Ok(Some(surface)),
            Err(error) => Err(self.handle_api_error(operation, error)),
        }
    }

    fn replace_surface(&mut self, surface: Option<A::Surface>) {
        let previous = std::mem::replace(&mut self.surface, surface);
        drop(previous);
    }

    fn release_surface(&mut self) {
        drop(self.surface.take());
    }

    fn release_runtime(&mut self) {
        self.release_surface();
        drop(self.context.take());
        drop(self.engine.take());
    }

    fn handle_api_error(
        &mut self,
        operation: BackendOperation,
        native_error: ApiError,
    ) -> BackendError {
        match native_error.kind {
            ApiErrorKind::SurfaceLost => {
                self.release_surface();
                self.state = SessionState::Lost;
            }
            ApiErrorKind::DeviceLost => {
                self.release_runtime();
                self.state = SessionState::Lost;
            }
            _ => {}
        }
        let error = api_error(operation, native_error);
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

    fn push_recovery_event(&mut self, loss: LossKind, recovery: Recovery) {
        self.push_event(BackendDiagnostic {
            severity: DiagnosticSeverity::Info,
            category: match loss {
                LossKind::Surface => DiagnosticCategory::Surface,
                LossKind::Device => DiagnosticCategory::Device,
            },
            code: "skia-recovered".into(),
            message: format!("Skia raster recovered from {loss:?} as {recovery:?}"),
            provenance: None,
        });
    }

    fn validate_frame_metrics(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<SurfaceMetrics> {
        let Some(metrics) = self.metrics else {
            let error = contract_error(
                BackendOperation::Render,
                "skia-raster-surface-missing",
                DiagnosticCategory::Lifecycle,
                "Skia raster has no attached surface metrics",
            );
            self.record_error(&error);
            return Err(error);
        };
        let viewport = &frame.frame().metadata().viewport;
        if viewport.physical_size != metrics.size
            || viewport.scale_factor.get().to_bits() != metrics.scale_factor.get().to_bits()
        {
            let error = contract_error(
                BackendOperation::Render,
                "skia-frame-surface-mismatch",
                DiagnosticCategory::Surface,
                format!(
                    "frame viewport {:?} at {}x does not match attached raster surface {:?} at {}x",
                    viewport.physical_size,
                    viewport.scale_factor.get(),
                    metrics.size,
                    metrics.scale_factor.get()
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        Ok(metrics)
    }

    fn readback_region(
        &mut self,
        request: &ReadbackRequest,
        metrics: SurfaceMetrics,
    ) -> BackendResult<PixelRegion> {
        let Some(region) = request.region else {
            return Ok(PixelRegion::full(metrics.size));
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
                "skia-readback-region-invalid",
                DiagnosticCategory::Surface,
                format!(
                    "logical readback region {region:?} falls outside physical surface {:?}",
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
        Ok(PixelRegion {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }

    fn validate_readback(
        &mut self,
        expected: PixelRegion,
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
                "skia-readback-contract-violation",
                DiagnosticCategory::Device,
                format!(
                    "Skia returned size {actual_size:?}, row_bytes {row_bytes}, and {pixel_len} bytes for {expected:?}"
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        Ok(())
    }
}

impl<A: SkiaApi> GraphicsBackendDriver for RasterDriver<A> {
    fn capabilities(&self) -> &GraphicsCapabilities {
        &self.capabilities
    }

    fn state(&self) -> SessionState {
        self.state
    }

    fn attach(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.check_thread(BackendOperation::Attach)?;
        self.require_state(BackendOperation::Attach, &[SessionState::Detached])?;
        // The outer Fission session deliberately fails closed after every
        // lifecycle-hook error. Mirror that state before entering fallible
        // target validation or allocation so subsequent detach is coherent.
        self.state = SessionState::Lost;
        let metrics = self.validate_target(BackendOperation::Attach, target)?;
        self.ensure_runtime(BackendOperation::Attach)?;
        let surface = self.create_surface(BackendOperation::Attach, metrics.size)?;
        self.replace_surface(surface);
        self.metrics = Some(metrics);
        self.last_rendered = None;
        self.state = SessionState::Attached;
        Ok(())
    }

    fn resize(&mut self, metrics: SurfaceMetrics) -> BackendResult<()> {
        self.check_thread(BackendOperation::Resize)?;
        self.require_state(BackendOperation::Resize, &[SessionState::Attached])?;
        let surface = self.create_surface(BackendOperation::Resize, metrics.size)?;
        self.replace_surface(surface);
        self.metrics = Some(metrics);
        self.last_rendered = None;
        Ok(())
    }

    fn render_validated(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<RenderReport> {
        self.check_thread(BackendOperation::Render)?;
        self.require_state(BackendOperation::Render, &[SessionState::Attached])?;
        let metrics = self.validate_frame_metrics(frame)?;
        let compiled = match compile_scene(frame.frame().scene(), metrics.scale_factor.get()) {
            Ok(compiled) => compiled,
            Err(error) => {
                let error = contract_error(
                    BackendOperation::Render,
                    "skia-frame-lowering-unsupported",
                    DiagnosticCategory::Capability,
                    error.to_string(),
                );
                self.record_error(&error);
                return Err(error);
            }
        };
        if !metrics.size.is_empty() {
            let Some(context) = self.context.as_mut() else {
                let error = contract_error(
                    BackendOperation::Render,
                    "skia-raster-context-missing",
                    DiagnosticCategory::Lifecycle,
                    "cannot render without a Skia raster context",
                );
                self.record_error(&error);
                return Err(error);
            };
            let Some(surface) = self.surface.as_mut() else {
                let error = contract_error(
                    BackendOperation::Render,
                    "skia-raster-surface-missing",
                    DiagnosticCategory::Lifecycle,
                    "cannot render a non-empty frame without a Skia raster surface",
                );
                self.record_error(&error);
                return Err(error);
            };
            if let Err(error) = self.api.execute_frame(context, surface, &compiled.frame) {
                self.diagnostics.counters.dropped_frames =
                    self.diagnostics.counters.dropped_frames.saturating_add(1);
                return Err(self.handle_api_error(BackendOperation::Render, error));
            }
        }
        let frame_id = frame.frame().metadata().frame_id;
        self.last_rendered = Some(frame_id);
        self.diagnostics.counters.frames_rendered =
            self.diagnostics.counters.frames_rendered.saturating_add(1);
        Ok(RenderReport {
            frame_id: Some(frame_id),
            encoded_operations: compiled.source_operations,
            reused_layers: 0,
            uploaded_bytes: 0,
        })
    }

    fn present(&mut self) -> BackendResult<PresentReport> {
        self.check_thread(BackendOperation::Present)?;
        self.require_state(BackendOperation::Present, &[SessionState::Attached])?;
        let Some(frame_id) = self.last_rendered else {
            let error = contract_error(
                BackendOperation::Present,
                "skia-present-before-render",
                DiagnosticCategory::Lifecycle,
                "cannot present before a frame has rendered",
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
        self.check_thread(BackendOperation::Readback)?;
        self.require_state(BackendOperation::Readback, &[SessionState::Attached])?;
        if request.color_format != ColorFormat::Rgba8Srgb {
            let error = contract_error(
                BackendOperation::Readback,
                "skia-readback-color-format-unsupported",
                DiagnosticCategory::Capability,
                format!("Skia raster cannot read back {:?}", request.color_format),
            );
            self.record_error(&error);
            return Err(error);
        }
        if self.last_rendered.is_none() {
            let error = contract_error(
                BackendOperation::Readback,
                "skia-readback-before-render",
                DiagnosticCategory::Lifecycle,
                "cannot read uninitialized raster pixels before the first frame",
            );
            self.record_error(&error);
            return Err(error);
        }
        let Some(metrics) = self.metrics else {
            let error = contract_error(
                BackendOperation::Readback,
                "skia-raster-surface-missing",
                DiagnosticCategory::Lifecycle,
                "cannot read back without attached raster metrics",
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
        let Some(context) = self.context.as_mut() else {
            let error = contract_error(
                BackendOperation::Readback,
                "skia-raster-context-missing",
                DiagnosticCategory::Lifecycle,
                "cannot read back without a Skia raster context",
            );
            self.record_error(&error);
            return Err(error);
        };
        let Some(surface) = self.surface.as_mut() else {
            let error = contract_error(
                BackendOperation::Readback,
                "skia-raster-surface-missing",
                DiagnosticCategory::Lifecycle,
                "cannot read back without a Skia raster surface",
            );
            self.record_error(&error);
            return Err(error);
        };
        let readback = match self.api.read_pixels_rgba8888(context, surface, region) {
            Ok(readback) => readback,
            Err(error) => return Err(self.handle_api_error(BackendOperation::Readback, error)),
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
        self.check_thread(BackendOperation::Suspend)?;
        self.require_state(BackendOperation::Suspend, &[SessionState::Attached])?;
        self.release_surface();
        self.last_rendered = None;
        self.state = SessionState::Suspended;
        Ok(())
    }

    fn resume(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.check_thread(BackendOperation::Resume)?;
        self.require_state(BackendOperation::Resume, &[SessionState::Suspended])?;
        let metrics = self.validate_target(BackendOperation::Resume, target)?;
        self.ensure_runtime(BackendOperation::Resume)?;
        let surface = self.create_surface(BackendOperation::Resume, metrics.size)?;
        self.replace_surface(surface);
        self.metrics = Some(metrics);
        self.last_rendered = None;
        self.state = SessionState::Attached;
        Ok(())
    }

    fn recover(&mut self, loss: LossKind) -> BackendResult<Recovery> {
        self.check_thread(BackendOperation::Recover)?;
        self.require_state(
            BackendOperation::Recover,
            &[
                SessionState::Attached,
                SessionState::Suspended,
                SessionState::Lost,
            ],
        )?;
        let Some(metrics) = self.metrics else {
            let error = contract_error(
                BackendOperation::Recover,
                "skia-recovery-target-missing",
                DiagnosticCategory::Lifecycle,
                "cannot recover before a raster target has been attached",
            );
            self.record_error(&error);
            return Err(error);
        };
        self.state = SessionState::Lost;
        self.last_rendered = None;

        let recovery = match loss {
            LossKind::Surface => {
                self.ensure_runtime(BackendOperation::Recover)?;
                let surface = self.create_surface(BackendOperation::Recover, metrics.size)?;
                self.replace_surface(surface);
                self.diagnostics.counters.surface_recoveries = self
                    .diagnostics
                    .counters
                    .surface_recoveries
                    .saturating_add(1);
                Recovery::Reattached
            }
            LossKind::Device => {
                let engine = match self.api.create_engine() {
                    Ok(engine) => engine,
                    Err(error) => {
                        return Err(self.handle_api_error(BackendOperation::Recover, error))
                    }
                };
                let context = match self.api.create_raster_context(&engine) {
                    Ok(context) => context,
                    Err(error) => {
                        return Err(self.handle_api_error(BackendOperation::Recover, error))
                    }
                };
                let surface = if metrics.size.is_empty() {
                    None
                } else {
                    match self.api.create_raster_surface(&context, metrics.size) {
                        Ok(surface) => Some(surface),
                        Err(error) => {
                            return Err(self.handle_api_error(BackendOperation::Recover, error))
                        }
                    }
                };
                self.release_runtime();
                self.engine = Some(engine);
                self.context = Some(context);
                self.surface = surface;
                self.diagnostics.counters.device_recoveries = self
                    .diagnostics
                    .counters
                    .device_recoveries
                    .saturating_add(1);
                Recovery::DeviceRecreated
            }
        };
        self.state = SessionState::Attached;
        self.push_recovery_event(loss, recovery);
        Ok(recovery)
    }

    fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()> {
        self.check_thread(BackendOperation::TrimMemory)?;
        if let Some(context) = self.context.as_mut() {
            if let Err(error) = self.api.trim_memory(context, pressure) {
                return Err(self.handle_api_error(BackendOperation::TrimMemory, error));
            }
        }
        Ok(())
    }

    fn detach(&mut self) -> BackendResult<()> {
        self.check_thread(BackendOperation::Detach)?;
        self.require_state(
            BackendOperation::Detach,
            &[
                SessionState::Attached,
                SessionState::Suspended,
                SessionState::Lost,
            ],
        )?;
        self.release_runtime();
        self.metrics = None;
        self.last_rendered = None;
        self.state = SessionState::Detached;
        Ok(())
    }

    fn diagnostics(&self) -> BackendDiagnostics {
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.session_state = self.state;
        diagnostics
    }
}

impl<A: SkiaApi> Drop for RasterDriver<A> {
    fn drop(&mut self) {
        self.release_runtime();
        self.state = SessionState::Detached;
    }
}

#[cfg(test)]
mod tests;
