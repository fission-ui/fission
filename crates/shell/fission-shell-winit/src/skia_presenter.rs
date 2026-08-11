use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};

use fission_render::backend::{
    BackendError, BackendOperation, BackendResult, GraphicsBackendSession, Readback,
    ReadbackRequest, SurfaceMetrics,
};
use fission_render::capabilities::{ColorFormat, GraphicsCapabilities};
use fission_render::frame::InteractiveFrame;
use fission_render::surface::{
    LossKind, MemoryPressure, PhysicalSize, Recovery, ScaleFactor, SessionState, SurfaceDescriptor,
    SurfaceId, SurfaceKind, SurfaceTarget, ThreadAffinity,
};

static NEXT_SKIA_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

/// Phase-2 bridge from the backend-neutral graphics session to Winit's current
/// wgpu upload presenter.
///
/// Skia owns rasterization and readback. The existing Winit presenter owns only
/// the final upload and swapchain presentation; this type deliberately makes no
/// Skia-only or no-wgpu qualification claim.
pub(super) struct WinitSkiaRasterPresenter {
    session: GraphicsBackendSession<'static>,
    target: WinitSkiaRasterTarget,
}

impl WinitSkiaRasterPresenter {
    pub(super) fn new(
        profile: &fission_render_skia::SkiaRasterProfile,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> BackendResult<Self> {
        let target = WinitSkiaRasterTarget::new(width, height, scale_factor)?;
        let mut session = profile.create_session()?;
        session.attach(&target)?;
        Ok(Self { session, target })
    }

    pub(super) fn capabilities(&self) -> &GraphicsCapabilities {
        self.session.capabilities()
    }

    #[cfg(test)]
    pub(super) fn state(&self) -> SessionState {
        self.session.state()
    }

    pub(super) fn sync_surface_metrics(
        &mut self,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> BackendResult<()> {
        let next = WinitSkiaRasterTarget::build_metrics(
            BackendOperation::Resize,
            width,
            height,
            scale_factor,
        )?;
        if self.target.current_metrics() == next {
            return Ok(());
        }
        self.session.resize(next)?;
        self.target.update(next);
        Ok(())
    }

    pub(super) fn suspend(&mut self) -> BackendResult<()> {
        self.session.suspend()
    }

    pub(super) fn resume(
        &mut self,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> BackendResult<()> {
        let next = WinitSkiaRasterTarget::build_metrics(
            BackendOperation::Resume,
            width,
            height,
            scale_factor,
        )?;
        self.target.update(next);
        self.session.resume(&self.target)
    }

    pub(super) fn recover(&mut self, loss: LossKind) -> BackendResult<Recovery> {
        self.session.recover(loss)
    }

    pub(super) fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()> {
        self.session.trim_memory(pressure)
    }

    pub(super) fn detach(&mut self) -> BackendResult<()> {
        if self.session.state() == SessionState::Detached {
            return Ok(());
        }
        self.session.detach()
    }

    pub(super) fn render_to_rgba(
        &mut self,
        frame: &InteractiveFrame<'_>,
    ) -> BackendResult<Vec<u8>> {
        match self.render_to_rgba_once(frame) {
            Ok(pixels) => Ok(pixels),
            Err(error) => {
                let Some(loss) = skia_loss_kind(&error) else {
                    return Err(error);
                };
                if self.recover(loss)? == Recovery::Unrecoverable {
                    return Err(error);
                }
                self.render_to_rgba_once(frame)
            }
        }
    }

    fn render_to_rgba_once(&mut self, frame: &InteractiveFrame<'_>) -> BackendResult<Vec<u8>> {
        let expected_frame_id = frame.metadata().frame_id;
        let render = self.session.render(frame)?;
        if render.frame_id != Some(expected_frame_id) {
            return Err(backend_contract_error(
                BackendOperation::Render,
                "skia-render-frame-id-mismatch",
                format!(
                    "Skia rendered frame {:?}; Winit submitted {:?}",
                    render.frame_id, expected_frame_id
                ),
            ));
        }

        let present = self.session.present()?;
        if present.frame_id != Some(expected_frame_id) {
            return Err(backend_contract_error(
                BackendOperation::Present,
                "skia-present-frame-id-mismatch",
                format!(
                    "Skia presented frame {:?}; Winit submitted {:?}",
                    present.frame_id, expected_frame_id
                ),
            ));
        }

        let readback = self.session.readback(ReadbackRequest {
            region: None,
            color_format: ColorFormat::Rgba8Srgb,
        })?;
        tight_rgba(readback, self.target.descriptor.size)
    }
}

impl Drop for WinitSkiaRasterPresenter {
    fn drop(&mut self) {
        let _ = self.detach();
    }
}

#[derive(Debug)]
struct WinitSkiaRasterTarget {
    descriptor: SurfaceDescriptor,
}

impl WinitSkiaRasterTarget {
    fn new(width: u32, height: u32, scale_factor: f64) -> BackendResult<Self> {
        let metrics = Self::build_metrics(BackendOperation::Attach, width, height, scale_factor)?;
        Ok(Self {
            descriptor: SurfaceDescriptor {
                id: SurfaceId(NEXT_SKIA_SURFACE_ID.fetch_add(1, Ordering::Relaxed)),
                kind: SurfaceKind::Headless,
                size: metrics.size,
                scale_factor: metrics.scale_factor,
                color_format: ColorFormat::Rgba8Srgb,
                thread_affinity: ThreadAffinity::CreatingThread,
            },
        })
    }

    fn build_metrics(
        operation: BackendOperation,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> BackendResult<SurfaceMetrics> {
        let scale_factor = ScaleFactor::new(scale_factor).map_err(|error| {
            backend_contract_error(operation, "skia-invalid-scale-factor", error.to_string())
        })?;
        Ok(SurfaceMetrics {
            size: PhysicalSize::new(width, height),
            scale_factor,
        })
    }

    fn current_metrics(&self) -> SurfaceMetrics {
        SurfaceMetrics {
            size: self.descriptor.size,
            scale_factor: self.descriptor.scale_factor,
        }
    }

    fn update(&mut self, metrics: SurfaceMetrics) {
        self.descriptor.size = metrics.size;
        self.descriptor.scale_factor = metrics.scale_factor;
    }
}

impl SurfaceTarget for WinitSkiaRasterTarget {
    fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn tight_rgba(readback: Readback, expected: PhysicalSize) -> BackendResult<Vec<u8>> {
    if readback.size != expected || readback.color_format != ColorFormat::Rgba8Srgb {
        return Err(backend_contract_error(
            BackendOperation::Readback,
            "skia-readback-metadata-mismatch",
            format!(
                "Skia readback returned {:?} {:?}; expected {:?} Rgba8Srgb",
                readback.size, readback.color_format, expected
            ),
        ));
    }

    let tight_row_bytes = usize::try_from(expected.width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or_else(|| {
            backend_contract_error(
                BackendOperation::Readback,
                "skia-readback-size-overflow",
                format!("Skia readback width {} overflows RGBA size", expected.width),
            )
        })?;
    let height = usize::try_from(expected.height).map_err(|_| {
        backend_contract_error(
            BackendOperation::Readback,
            "skia-readback-size-overflow",
            format!(
                "Skia readback height {} cannot be represented",
                expected.height
            ),
        )
    })?;
    let source_len = readback.row_bytes.checked_mul(height).ok_or_else(|| {
        backend_contract_error(
            BackendOperation::Readback,
            "skia-readback-size-overflow",
            "Skia readback row stride overflows the buffer size",
        )
    })?;
    if readback.row_bytes < tight_row_bytes || readback.pixels.len() != source_len {
        return Err(backend_contract_error(
            BackendOperation::Readback,
            "skia-readback-layout-invalid",
            format!(
                "Skia readback returned row stride {} and {} bytes for {}x{} RGBA pixels",
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

    let destination_len = tight_row_bytes.checked_mul(height).ok_or_else(|| {
        backend_contract_error(
            BackendOperation::Readback,
            "skia-readback-size-overflow",
            "Skia tight RGBA buffer size overflowed",
        )
    })?;
    let mut pixels = Vec::with_capacity(destination_len);
    for source_row in readback.pixels.chunks_exact(readback.row_bytes) {
        pixels.extend_from_slice(&source_row[..tight_row_bytes]);
    }
    Ok(pixels)
}

fn backend_contract_error(
    operation: BackendOperation,
    code: &'static str,
    message: impl Into<String>,
) -> BackendError {
    BackendError::new(operation, code, message)
}

fn skia_loss_kind(error: &BackendError) -> Option<LossKind> {
    match error.code.as_str() {
        "skia-surface-lost" => Some(LossKind::Surface),
        "skia-device-lost" => Some(LossKind::Device),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readback_rows_are_repacked_without_padding() {
        let readback = Readback {
            size: PhysicalSize::new(2, 2),
            color_format: ColorFormat::Rgba8Srgb,
            row_bytes: 12,
            pixels: vec![
                1, 2, 3, 4, 5, 6, 7, 8, 90, 91, 92, 93, 9, 10, 11, 12, 13, 14, 15, 16, 94, 95, 96,
                97,
            ],
        };

        assert_eq!(
            tight_rgba(readback, PhysicalSize::new(2, 2)).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
    }

    #[test]
    fn malformed_readback_layout_is_rejected() {
        let readback = Readback {
            size: PhysicalSize::new(2, 1),
            color_format: ColorFormat::Rgba8Srgb,
            row_bytes: 7,
            pixels: vec![0; 7],
        };

        let error = tight_rgba(readback, PhysicalSize::new(2, 1)).unwrap_err();
        assert_eq!(error.code, "skia-readback-layout-invalid");
    }

    #[test]
    fn only_explicit_skia_loss_diagnostics_request_recovery() {
        let surface = BackendError::new(
            BackendOperation::Render,
            "skia-surface-lost",
            "surface unavailable",
        );
        let device = BackendError::new(
            BackendOperation::Render,
            "skia-device-lost",
            "device unavailable",
        );
        let presenter = BackendError::new(
            BackendOperation::Present,
            "surface-outdated",
            "upload presenter changed",
        );

        assert_eq!(skia_loss_kind(&surface), Some(LossKind::Surface));
        assert_eq!(skia_loss_kind(&device), Some(LossKind::Device));
        assert_eq!(skia_loss_kind(&presenter), None);
    }
}
