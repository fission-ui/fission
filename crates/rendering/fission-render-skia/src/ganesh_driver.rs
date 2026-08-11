use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::Arc;

use fission_render::backend::{
    BackendError, BackendOperation, BackendResult, GraphicsBackendDriver, PresentReport, Readback,
    ReadbackRequest, RenderReport, SurfaceMetrics,
};
use fission_render::capabilities::{ColorFormat, GraphicsCapabilities};
use fission_render::diagnostics::{
    BackendDiagnostic, BackendDiagnostics, CacheDiagnostics, DiagnosticCategory,
    DiagnosticProvenance, DiagnosticSeverity,
};
use fission_render::frame::{FrameId, ValidatedInteractiveFrame};
use fission_render::surface::{
    LossKind, MemoryPressure, NativeWindowTarget, PhysicalSize, Recovery, SessionState,
    SurfaceKind, SurfaceTarget, ThreadAffinity,
};
use fission_skia_sys::{NativeWindow, NativeWindowKind, DEFAULT_GANESH_GPU_CACHE_BYTES};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::api::{ApiError, ApiErrorKind, PixelRegion};
use crate::capabilities::skia_ganesh_capabilities;
use crate::compiler::compile_scene_with_paragraphs;
use crate::error::{api_error, contract_error, contract_error_with_provenance, wrong_thread};
use crate::ganesh_api::{GaneshApi, GaneshPictureRecorder, GaneshResourceCacheUsage};
use crate::ganesh_native::NativeGaneshApi;
use crate::image::SkiaImageCache;
use crate::picture::SkiaPictureCache;
use crate::profile::SkiaParagraphDrawDataRegistry;
use crate::svg::SkiaSvgCache;
use crate::thread_owner::ThreadOwner;

const MAX_RECENT_EVENTS: usize = 64;
const GPU_CACHE_NAME: &str = "skia-ganesh-gpu-resources";
const GPU_CACHE_BYTES_ENV: &str = "FISSION_SKIA_GPU_CACHE_BYTES";

/// Fission graphics driver for direct native Skia Ganesh presentation.
///
/// Construct this driver through [`crate::SkiaGaneshProfile`] so paragraph
/// layout and paint share one authoritative draw-data registry. Native window
/// handles remain host-owned: the host must keep every display connection and
/// window passed to `attach` or `resume` live and unchanged until the session
/// next resumes with a replacement target or detaches.
pub struct SkiaGaneshDriver {
    inner: GaneshDriver<NativeGaneshApi>,
}

impl SkiaGaneshDriver {
    pub(crate) fn with_draw_data_registry(
        paragraph_draw_data: Arc<SkiaParagraphDrawDataRegistry>,
    ) -> BackendResult<Self> {
        Ok(Self {
            inner: GaneshDriver::try_new(NativeGaneshApi, paragraph_draw_data)?,
        })
    }
}

impl GraphicsBackendDriver for SkiaGaneshDriver {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingFrame {
    id: FrameId,
    native_ready: bool,
}

struct GaneshDriver<A: GaneshApi> {
    api: A,
    owner: ThreadOwner,
    capabilities: GraphicsCapabilities,
    state: SessionState,
    engine: Option<A::Engine>,
    context: Option<A::Context>,
    context_window_kind: Option<NativeWindowKind>,
    surface: Option<A::Surface>,
    window: Option<NativeWindow>,
    metrics: Option<SurfaceMetrics>,
    pending_frame: Option<PendingFrame>,
    diagnostics: BackendDiagnostics,
    paragraph_draw_data: Arc<SkiaParagraphDrawDataRegistry>,
    image_cache: SkiaImageCache,
    svg_cache: SkiaSvgCache,
    picture_cache: SkiaPictureCache,
    gpu_cache_budget_bytes: u64,
    gpu_cache_evictions: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<A: GaneshApi> GaneshDriver<A> {
    fn try_new(
        api: A,
        paragraph_draw_data: Arc<SkiaParagraphDrawDataRegistry>,
    ) -> BackendResult<Self> {
        Self::try_new_with_gpu_cache_budget(api, paragraph_draw_data, configured_gpu_cache_bytes())
    }

    fn try_new_with_gpu_cache_budget(
        api: A,
        paragraph_draw_data: Arc<SkiaParagraphDrawDataRegistry>,
        gpu_cache_budget_bytes: u64,
    ) -> BackendResult<Self> {
        let capabilities = skia_ganesh_capabilities();
        let engine = api
            .create_engine()
            .map_err(|error| api_error(BackendOperation::Initialize, error))?;
        Ok(Self {
            api,
            owner: ThreadOwner::current(),
            capabilities: capabilities.clone(),
            state: SessionState::Detached,
            engine: Some(engine),
            context: None,
            context_window_kind: None,
            surface: None,
            window: None,
            metrics: None,
            pending_frame: None,
            diagnostics: BackendDiagnostics::new(capabilities.identity, SessionState::Detached),
            paragraph_draw_data,
            image_cache: SkiaImageCache::new(),
            svg_cache: SkiaSvgCache::new(),
            picture_cache: SkiaPictureCache::new(),
            gpu_cache_budget_bytes,
            gpu_cache_evictions: 0,
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
            "skia-ganesh-invalid-session-state",
            DiagnosticCategory::Lifecycle,
            format!(
                "cannot perform {operation:?} while Skia Ganesh is {:?}; expected one of {expected:?}",
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
    ) -> BackendResult<(SurfaceMetrics, NativeWindow)> {
        let descriptor = target.descriptor();
        if descriptor.kind != SurfaceKind::NativeWindow {
            let error = contract_error(
                operation,
                "skia-ganesh-target-unsupported",
                DiagnosticCategory::Capability,
                format!(
                    "Skia Ganesh requires a NativeWindow target, not {:?}",
                    descriptor.kind
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        if descriptor.color_format != ColorFormat::Bgra8Srgb {
            let error = contract_error(
                operation,
                "skia-ganesh-color-format-unsupported",
                DiagnosticCategory::Capability,
                format!(
                    "the native Ganesh profile requires Bgra8Srgb, not {:?}",
                    descriptor.color_format
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        let Some(target) = target.as_any().downcast_ref::<NativeWindowTarget>() else {
            let error = contract_error(
                operation,
                "skia-ganesh-target-type-invalid",
                DiagnosticCategory::Surface,
                "NativeWindow descriptors must be carried by fission_render::surface::NativeWindowTarget",
            );
            self.record_error(&error);
            return Err(error);
        };
        let window = match lower_native_window(target) {
            Ok(window) => window,
            Err(message) => {
                let error = contract_error(
                    operation,
                    "skia-ganesh-native-handle-invalid",
                    DiagnosticCategory::Surface,
                    message,
                );
                self.record_error(&error);
                return Err(error);
            }
        };
        let required_affinity = native_window_thread_affinity(window.kind());
        if descriptor.thread_affinity != required_affinity {
            let error = contract_error(
                operation,
                "skia-ganesh-thread-affinity-invalid",
                DiagnosticCategory::Surface,
                format!(
                    "native window kind {:?} requires {required_affinity:?} affinity, not {:?}",
                    window.kind(),
                    descriptor.thread_affinity
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        Ok((
            SurfaceMetrics {
                size: descriptor.size,
                scale_factor: descriptor.scale_factor,
            },
            window,
        ))
    }

    fn ensure_engine(&mut self, operation: BackendOperation) -> BackendResult<()> {
        if self.engine.is_some() {
            return Ok(());
        }
        match self.api.create_engine() {
            Ok(engine) => {
                self.engine = Some(engine);
                Ok(())
            }
            Err(error) => Err(self.handle_api_error(operation, error)),
        }
    }

    fn ensure_context(
        &mut self,
        operation: BackendOperation,
        window: NativeWindow,
    ) -> BackendResult<()> {
        if self.context.is_some() && self.context_window_kind == Some(window.kind()) {
            return Ok(());
        }
        self.release_context();
        let Some(engine) = self.engine.as_ref() else {
            let error = contract_error(
                operation,
                "skia-ganesh-engine-missing",
                DiagnosticCategory::Lifecycle,
                "cannot create a Ganesh context without a Skia engine",
            );
            self.record_error(&error);
            return Err(error);
        };
        match self
            .api
            .create_context(engine, window, self.gpu_cache_budget_bytes)
        {
            Ok(context) => {
                self.context = Some(context);
                self.context_window_kind = Some(window.kind());
                Ok(())
            }
            Err(error) => Err(self.handle_api_error(operation, error)),
        }
    }

    fn create_surface(
        &mut self,
        operation: BackendOperation,
        window: NativeWindow,
        metrics: SurfaceMetrics,
    ) -> BackendResult<A::Surface> {
        let Some(context) = self.context.as_ref() else {
            let error = contract_error(
                operation,
                "skia-ganesh-context-missing",
                DiagnosticCategory::Lifecycle,
                "cannot create a Ganesh surface without a Ganesh context",
            );
            self.record_error(&error);
            return Err(error);
        };
        match self.api.create_surface(context, window, metrics.size) {
            Ok(surface) => Ok(surface),
            Err(error) => Err(self.handle_api_error(operation, error)),
        }
    }

    fn release_surface(&mut self) {
        drop(self.surface.take());
        self.pending_frame = None;
    }

    fn release_context(&mut self) {
        self.release_surface();
        drop(self.context.take());
        self.context_window_kind = None;
    }

    fn release_runtime(&mut self) {
        self.release_context();
        self.image_cache.clear();
        self.svg_cache.clear();
        self.picture_cache.clear();
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
            code: "skia-ganesh-recovered".into(),
            message: format!("Skia Ganesh recovered from {loss:?} as {recovery:?}"),
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
                "skia-ganesh-surface-missing",
                DiagnosticCategory::Lifecycle,
                "Skia Ganesh has no attached surface metrics",
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
                    "frame viewport {:?} at {}x does not match attached Ganesh surface {:?} at {}x",
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

    fn require_no_pending_frame(&mut self, operation: BackendOperation) -> BackendResult<()> {
        if self.pending_frame.is_none() {
            return Ok(());
        }
        let error = contract_error(
            operation,
            "skia-ganesh-present-pending",
            DiagnosticCategory::Lifecycle,
            "the previously rendered Ganesh frame must be presented before rendering or resizing",
        );
        self.record_error(&error);
        Err(error)
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
                "skia-ganesh-readback-region-invalid",
                DiagnosticCategory::Surface,
                format!(
                    "logical readback region {region:?} falls outside physical Ganesh surface {:?}",
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
                "skia-ganesh-readback-contract-violation",
                DiagnosticCategory::Device,
                format!(
                    "Skia Ganesh returned size {actual_size:?}, row_bytes {row_bytes}, and {pixel_len} bytes for {expected:?}"
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        Ok(())
    }
}

impl<A: GaneshApi> GraphicsBackendDriver for GaneshDriver<A> {
    fn capabilities(&self) -> &GraphicsCapabilities {
        &self.capabilities
    }

    fn state(&self) -> SessionState {
        self.state
    }

    fn attach(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.check_thread(BackendOperation::Attach)?;
        self.require_state(BackendOperation::Attach, &[SessionState::Detached])?;
        self.state = SessionState::Lost;
        let (metrics, window) = self.validate_target(BackendOperation::Attach, target)?;
        self.window = Some(window);
        self.metrics = Some(metrics);
        self.ensure_engine(BackendOperation::Attach)?;
        self.ensure_context(BackendOperation::Attach, window)?;
        let surface = self.create_surface(BackendOperation::Attach, window, metrics)?;
        self.surface = Some(surface);
        self.pending_frame = None;
        self.state = SessionState::Attached;
        Ok(())
    }

    fn resize(&mut self, metrics: SurfaceMetrics) -> BackendResult<()> {
        self.check_thread(BackendOperation::Resize)?;
        self.require_state(BackendOperation::Resize, &[SessionState::Attached])?;
        self.require_no_pending_frame(BackendOperation::Resize)?;
        let Some(window) = self.window else {
            let error = contract_error(
                BackendOperation::Resize,
                "skia-ganesh-target-missing",
                DiagnosticCategory::Lifecycle,
                "cannot resize before a native Ganesh target has been attached",
            );
            self.record_error(&error);
            return Err(error);
        };
        let result = match self.surface.as_mut() {
            Some(surface) => self.api.resize_surface(surface, window, metrics.size),
            None => {
                let error = contract_error(
                    BackendOperation::Resize,
                    "skia-ganesh-surface-missing",
                    DiagnosticCategory::Lifecycle,
                    "cannot resize without an attached Ganesh surface",
                );
                self.record_error(&error);
                return Err(error);
            }
        };
        if let Err(error) = result {
            return Err(self.handle_api_error(BackendOperation::Resize, error));
        }
        self.metrics = Some(metrics);
        Ok(())
    }

    fn render_validated(
        &mut self,
        frame: &ValidatedInteractiveFrame<'_>,
    ) -> BackendResult<RenderReport> {
        self.check_thread(BackendOperation::Render)?;
        self.require_state(BackendOperation::Render, &[SessionState::Attached])?;
        self.require_no_pending_frame(BackendOperation::Render)?;
        let metrics = self.validate_frame_metrics(frame)?;
        let recorder = GaneshPictureRecorder::new(&self.api);
        let compiled = match compile_scene_with_paragraphs(
            frame.frame().scene(),
            metrics.scale_factor.get(),
            frame.frame().clear_color(),
            frame.frame().resources(),
            &self.image_cache,
            &self.svg_cache,
            frame.frame().paragraph_bindings(),
            self.paragraph_draw_data.as_ref(),
            &self.picture_cache,
            &recorder,
        ) {
            Ok(compiled) => compiled,
            Err(error) => {
                let provenance = DiagnosticProvenance {
                    frame_id: Some(frame.frame().metadata().frame_id),
                    node_id: error.provenance.node_id,
                    operation_index: error.provenance.operation_index(),
                };
                let error = contract_error_with_provenance(
                    BackendOperation::Render,
                    error.diagnostic_code(),
                    error.diagnostic_category(),
                    error.to_string(),
                    provenance,
                );
                self.record_error(&error);
                return Err(error);
            }
        };
        let native_ready = !metrics.size.is_empty();
        if native_ready {
            let result = match self.surface.as_mut() {
                Some(surface) => self.api.execute_frame(surface, &compiled.frame),
                None => {
                    let error = contract_error(
                        BackendOperation::Render,
                        "skia-ganesh-surface-missing",
                        DiagnosticCategory::Lifecycle,
                        "cannot render without an attached Ganesh surface",
                    );
                    self.record_error(&error);
                    return Err(error);
                }
            };
            if let Err(error) = result {
                self.diagnostics.counters.dropped_frames =
                    self.diagnostics.counters.dropped_frames.saturating_add(1);
                return Err(self.handle_api_error(BackendOperation::Render, error));
            }
        }
        let frame_id = frame.frame().metadata().frame_id;
        self.pending_frame = Some(PendingFrame {
            id: frame_id,
            native_ready,
        });
        self.diagnostics.counters.frames_rendered =
            self.diagnostics.counters.frames_rendered.saturating_add(1);
        Ok(RenderReport {
            frame_id: Some(frame_id),
            encoded_operations: compiled.source_operations,
            reused_layers: compiled.reused_layers,
            uploaded_bytes: 0,
        })
    }

    fn present(&mut self) -> BackendResult<PresentReport> {
        self.check_thread(BackendOperation::Present)?;
        self.require_state(BackendOperation::Present, &[SessionState::Attached])?;
        let Some(pending) = self.pending_frame else {
            let error = contract_error(
                BackendOperation::Present,
                "skia-present-before-render",
                DiagnosticCategory::Lifecycle,
                "cannot present before a Ganesh frame has rendered",
            );
            self.record_error(&error);
            return Err(error);
        };
        if pending.native_ready {
            let result = match self.surface.as_mut() {
                Some(surface) => self.api.present(surface),
                None => {
                    let error = contract_error(
                        BackendOperation::Present,
                        "skia-ganesh-surface-missing",
                        DiagnosticCategory::Lifecycle,
                        "cannot present without an attached Ganesh surface",
                    );
                    self.record_error(&error);
                    return Err(error);
                }
            };
            if let Err(error) = result {
                return Err(self.handle_api_error(BackendOperation::Present, error));
            }
        }
        self.pending_frame = None;
        self.diagnostics.counters.frames_presented =
            self.diagnostics.counters.frames_presented.saturating_add(1);
        Ok(PresentReport {
            frame_id: Some(pending.id),
            recovered_surface: false,
        })
    }

    fn readback(&mut self, request: ReadbackRequest) -> BackendResult<Readback> {
        self.check_thread(BackendOperation::Readback)?;
        self.require_state(BackendOperation::Readback, &[SessionState::Attached])?;
        if request.color_format != ColorFormat::Rgba8Srgb {
            let error = contract_error(
                BackendOperation::Readback,
                "skia-ganesh-readback-color-format-unsupported",
                DiagnosticCategory::Capability,
                format!(
                    "Skia Ganesh produces Rgba8Srgb readback, not {:?}",
                    request.color_format
                ),
            );
            self.record_error(&error);
            return Err(error);
        }
        let Some(pending) = self.pending_frame else {
            let error = contract_error(
                BackendOperation::Readback,
                "skia-ganesh-readback-outside-frame",
                DiagnosticCategory::Lifecycle,
                "Ganesh readback is valid only after render and before present",
            );
            self.record_error(&error);
            return Err(error);
        };
        let Some(metrics) = self.metrics else {
            let error = contract_error(
                BackendOperation::Readback,
                "skia-ganesh-surface-missing",
                DiagnosticCategory::Lifecycle,
                "cannot read back without attached Ganesh surface metrics",
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
        if !pending.native_ready {
            let error = contract_error(
                BackendOperation::Readback,
                "skia-ganesh-readback-surface-suspended",
                DiagnosticCategory::Surface,
                "cannot read non-empty pixels from a zero-sized Ganesh frame",
            );
            self.record_error(&error);
            return Err(error);
        }
        let readback = match self.surface.as_mut() {
            Some(surface) => self.api.read_pixels_rgba8888(surface, region),
            None => {
                let error = contract_error(
                    BackendOperation::Readback,
                    "skia-ganesh-surface-missing",
                    DiagnosticCategory::Lifecycle,
                    "cannot read back without an attached Ganesh surface",
                );
                self.record_error(&error);
                return Err(error);
            }
        };
        let readback = match readback {
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
        self.state = SessionState::Suspended;
        Ok(())
    }

    fn resume(&mut self, target: &dyn SurfaceTarget) -> BackendResult<()> {
        self.check_thread(BackendOperation::Resume)?;
        self.require_state(BackendOperation::Resume, &[SessionState::Suspended])?;
        self.state = SessionState::Lost;
        let (metrics, window) = self.validate_target(BackendOperation::Resume, target)?;
        self.window = Some(window);
        self.metrics = Some(metrics);
        self.ensure_engine(BackendOperation::Resume)?;
        self.ensure_context(BackendOperation::Resume, window)?;
        let surface = self.create_surface(BackendOperation::Resume, window, metrics)?;
        self.surface = Some(surface);
        self.pending_frame = None;
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
        let Some(window) = self.window else {
            let error = contract_error(
                BackendOperation::Recover,
                "skia-ganesh-recovery-target-missing",
                DiagnosticCategory::Lifecycle,
                "cannot recover before a native Ganesh target has been attached",
            );
            self.record_error(&error);
            return Err(error);
        };
        let Some(metrics) = self.metrics else {
            let error = contract_error(
                BackendOperation::Recover,
                "skia-ganesh-recovery-metrics-missing",
                DiagnosticCategory::Lifecycle,
                "cannot recover without Ganesh surface metrics",
            );
            self.record_error(&error);
            return Err(error);
        };
        self.state = SessionState::Lost;
        self.pending_frame = None;

        let recovery = match loss {
            LossKind::Surface => {
                self.release_surface();
                self.ensure_engine(BackendOperation::Recover)?;
                self.ensure_context(BackendOperation::Recover, window)?;
                let surface = self.create_surface(BackendOperation::Recover, window, metrics)?;
                self.surface = Some(surface);
                self.diagnostics.counters.surface_recoveries = self
                    .diagnostics
                    .counters
                    .surface_recoveries
                    .saturating_add(1);
                Recovery::Reattached
            }
            LossKind::Device => {
                self.release_runtime();
                self.ensure_engine(BackendOperation::Recover)?;
                self.ensure_context(BackendOperation::Recover, window)?;
                let surface = self.create_surface(BackendOperation::Recover, window, metrics)?;
                self.surface = Some(surface);
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
        if pressure == MemoryPressure::Critical {
            self.image_cache.clear();
            self.svg_cache.clear();
            self.picture_cache.clear();
        }
        if let Some(context) = self.context.as_ref() {
            let before = self.api.resource_cache_usage(context).ok();
            if let Err(error) = self.api.trim_memory(context, pressure) {
                return Err(self.handle_api_error(BackendOperation::TrimMemory, error));
            }
            let after = self.api.resource_cache_usage(context).ok();
            if let (Some(before), Some(after)) = (before, after) {
                self.gpu_cache_evictions = self
                    .gpu_cache_evictions
                    .saturating_add(before.resource_count.saturating_sub(after.resource_count));
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
        self.window = None;
        self.metrics = None;
        self.state = SessionState::Detached;
        Ok(())
    }

    fn diagnostics(&self) -> BackendDiagnostics {
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.session_state = self.state;
        diagnostics.caches.push(self.image_cache.diagnostics());
        diagnostics.caches.push(self.svg_cache.diagnostics());
        diagnostics.caches.push(self.picture_cache.diagnostics());
        let usage = match self
            .context
            .as_ref()
            .map(|context| self.api.resource_cache_usage(context))
        {
            Some(Ok(usage)) => usage,
            Some(Err(error)) => {
                diagnostics.recent_events.push(BackendDiagnostic {
                    severity: DiagnosticSeverity::Warning,
                    category: DiagnosticCategory::Resource,
                    code: "skia-ganesh-gpu-cache-usage-unavailable".into(),
                    message: format!(
                        "Skia {} failed [{}]: {}",
                        error.operation, error.code, error.message
                    ),
                    provenance: None,
                });
                if diagnostics.recent_events.len() > MAX_RECENT_EVENTS {
                    diagnostics.recent_events.remove(0);
                }
                GaneshResourceCacheUsage::default()
            }
            None => GaneshResourceCacheUsage::default(),
        };
        diagnostics.caches.push(CacheDiagnostics {
            name: GPU_CACHE_NAME.into(),
            entries: usage.resource_count,
            used_bytes: usage.resource_bytes,
            budget_bytes: Some(self.gpu_cache_budget_bytes),
            evictions: self.gpu_cache_evictions,
        });
        diagnostics
    }
}

impl<A: GaneshApi> Drop for GaneshDriver<A> {
    fn drop(&mut self) {
        self.release_runtime();
        self.state = SessionState::Detached;
    }
}

fn lower_native_window(target: &NativeWindowTarget) -> Result<NativeWindow, String> {
    match (target.raw_display_handle(), target.raw_window_handle()) {
        (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(window)) => {
            // SAFETY: NativeWindowTarget's constructor requires the host to
            // keep this matching wl_display/wl_surface pair live through every
            // attached renderer use. raw-window-handle makes both non-null.
            Ok(unsafe { NativeWindow::wayland(display.display, window.surface) })
        }
        (RawDisplayHandle::Xlib(display), RawWindowHandle::Xlib(window)) => {
            let display = display
                .display
                .ok_or_else(|| "an Xlib Ganesh target requires a non-null Display pointer".to_owned())?;
            let window_id = u64::try_from(window.window)
                .ok()
                .and_then(NonZeroU64::new)
                .ok_or_else(|| "an Xlib Ganesh target requires a non-zero Window XID".to_owned())?;
            let visual_id = u64::try_from(window.visual_id)
                .map_err(|_| "the Xlib visual ID does not fit the Ganesh ABI".to_owned())?;
            // SAFETY: NativeWindowTarget's contract keeps the Display and XID
            // live. Vulkan WSI permits an unknown visual represented by zero.
            Ok(unsafe { NativeWindow::xlib(display, window_id, visual_id) })
        }
        (RawDisplayHandle::Xcb(display), RawWindowHandle::Xcb(window)) => {
            let connection = display.connection.ok_or_else(|| {
                "an XCB Ganesh target requires a non-null xcb_connection_t pointer".to_owned()
            })?;
            let visual_id = window.visual_id.map_or(0, |visual| visual.get());
            // SAFETY: NativeWindowTarget's contract keeps the connection and
            // non-zero XID live. Vulkan WSI permits an unknown visual as zero.
            Ok(unsafe { NativeWindow::xcb(connection, window.window, visual_id) })
        }
        (RawDisplayHandle::AppKit(_), RawWindowHandle::AppKit(window)) => {
            // SAFETY: NativeWindowTarget's host keeps the NSView live for the
            // attachment lifetime and declares main-thread affinity.
            Ok(unsafe { NativeWindow::appkit(window.ns_view) })
        }
        (RawDisplayHandle::UiKit(_), RawWindowHandle::UiKit(window)) => {
            // SAFETY: NativeWindowTarget's host keeps the UIView live for the
            // attachment lifetime and declares main-thread affinity.
            Ok(unsafe { NativeWindow::uikit(window.ui_view) })
        }
        (RawDisplayHandle::Windows(_), RawWindowHandle::Win32(window)) => {
            // SAFETY: NativeWindowTarget's host keeps the HWND live on its
            // creating thread for the complete attachment lifetime.
            Ok(unsafe { NativeWindow::win32(window.hwnd) })
        }
        (display, window) => Err(format!(
            "native Ganesh requires a matching Linux Wayland/Xlib/XCB, macOS AppKit, iOS UIKit, or Windows Win32 handle pair, got {display:?} and {window:?}"
        )),
    }
}

fn native_window_thread_affinity(kind: NativeWindowKind) -> ThreadAffinity {
    match kind {
        NativeWindowKind::Wayland
        | NativeWindowKind::Xlib
        | NativeWindowKind::Xcb
        | NativeWindowKind::Win32 => ThreadAffinity::CreatingThread,
        NativeWindowKind::AppKit | NativeWindowKind::UIKit => ThreadAffinity::MainThread,
    }
}

fn configured_gpu_cache_bytes() -> u64 {
    configured_gpu_cache_bytes_from(std::env::var(GPU_CACHE_BYTES_ENV).ok().as_deref())
}

fn configured_gpu_cache_bytes_from(value: Option<&str>) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_GANESH_GPU_CACHE_BYTES)
}

#[cfg(test)]
mod tests;
