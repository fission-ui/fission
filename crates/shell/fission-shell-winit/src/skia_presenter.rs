use fission_render::backend::{
    BackendError, BackendOperation, BackendResult, Readback, SurfaceMetrics,
};
use fission_render::capabilities::{ColorFormat, GraphicsCapabilities};
use fission_render::frame::InteractiveFrame;
#[cfg(test)]
use fission_render::surface::SessionState;
use fission_render::surface::{MemoryPressure, PhysicalSize, ScaleFactor};
use fission_render_skia::SkiaRasterSession;

/// Winit's upload presenter around the backend-owned Skia raster lifecycle.
///
/// Skia owns the headless surface, rendering, recovery, readback, and cache
/// lifecycle. Winit owns only the final wgpu texture upload and swapchain
/// presentation until a direct native raster presenter is introduced.
pub(super) struct WinitSkiaRasterPresenter {
    session: SkiaRasterSession,
}

impl WinitSkiaRasterPresenter {
    pub(super) fn new(
        profile: &fission_render_skia::SkiaRasterProfile,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> BackendResult<Self> {
        let metrics = surface_metrics(BackendOperation::Attach, width, height, scale_factor)?;
        Ok(Self {
            session: profile.create_headless_session(metrics)?,
        })
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
        let metrics = surface_metrics(BackendOperation::Resize, width, height, scale_factor)?;
        self.session.resize(metrics)
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
        let metrics = surface_metrics(BackendOperation::Resume, width, height, scale_factor)?;
        self.session.resume(metrics)
    }

    pub(super) fn trim_memory(&mut self, pressure: MemoryPressure) -> BackendResult<()> {
        self.session.trim_memory(pressure)
    }

    pub(super) fn detach(&mut self) -> BackendResult<()> {
        self.session.detach()
    }

    pub(super) fn render_to_rgba(
        &mut self,
        frame: &InteractiveFrame<'_>,
    ) -> BackendResult<Vec<u8>> {
        self.session.render_to_rgba(frame)
    }
}

fn surface_metrics(
    operation: BackendOperation,
    width: u32,
    height: u32,
    scale_factor: f64,
) -> BackendResult<SurfaceMetrics> {
    let scale_factor = ScaleFactor::new(scale_factor).map_err(|error| {
        fission_render::backend::BackendError::new(
            operation,
            "skia-invalid-scale-factor",
            error.to_string(),
        )
    })?;
    Ok(SurfaceMetrics {
        size: PhysicalSize::new(width, height),
        scale_factor,
    })
}

/// Validates backend readback metadata and removes native row padding.
///
/// Direct Ganesh capture happens before presentation and may carry a native
/// row stride. Keeping its screenshot conversion at the presenter boundary
/// avoids exposing a shell-specific buffer contract from the renderer crate.
pub(super) fn tight_rgba(readback: Readback, expected: PhysicalSize) -> BackendResult<Vec<u8>> {
    if readback.size != expected || readback.color_format != ColorFormat::Rgba8Srgb {
        return Err(readback_error(
            "skia-readback-metadata-mismatch",
            format!(
                "Skia readback returned {:?} {:?}; expected {:?} Rgba8Srgb",
                readback.size, readback.color_format, expected
            ),
        ));
    }

    if expected.is_empty() {
        if readback.row_bytes == 0 && readback.pixels.is_empty() {
            return Ok(Vec::new());
        }
        return Err(readback_error(
            "skia-readback-layout-invalid",
            "Skia returned storage for an empty readback region",
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
        return Err(readback_error(
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

    let destination_len = tight_row_bytes
        .checked_mul(height)
        .ok_or_else(|| readback_size_overflow("tight row stride", tight_row_bytes))?;
    let mut pixels = Vec::with_capacity(destination_len);
    for source_row in readback.pixels.chunks_exact(readback.row_bytes) {
        pixels.extend_from_slice(&source_row[..tight_row_bytes]);
    }
    Ok(pixels)
}

fn readback_size_overflow(field: &'static str, value: impl std::fmt::Display) -> BackendError {
    readback_error(
        "skia-readback-size-overflow",
        format!("Skia readback {field} value {value} overflows the RGBA buffer size"),
    )
}

fn readback_error(code: &'static str, message: impl Into<String>) -> BackendError {
    BackendError::new(BackendOperation::Readback, code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_metrics_reject_non_finite_or_non_positive_scale() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let error = surface_metrics(BackendOperation::Attach, 10, 20, scale).unwrap_err();
            assert_eq!(error.code, "skia-invalid-scale-factor");
        }
    }

    #[test]
    fn surface_metrics_preserve_physical_size_and_scale() {
        let metrics = surface_metrics(BackendOperation::Resize, 320, 240, 1.5).unwrap();

        assert_eq!(metrics.size, PhysicalSize::new(320, 240));
        assert_eq!(metrics.scale_factor.get(), 1.5);
    }

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
    fn empty_readback_requires_an_empty_layout() {
        let empty = Readback {
            size: PhysicalSize::new(0, 20),
            color_format: ColorFormat::Rgba8Srgb,
            row_bytes: 0,
            pixels: Vec::new(),
        };
        assert!(tight_rgba(empty, PhysicalSize::new(0, 20))
            .unwrap()
            .is_empty());

        let malformed = Readback {
            size: PhysicalSize::new(0, 20),
            color_format: ColorFormat::Rgba8Srgb,
            row_bytes: 4,
            pixels: vec![0; 80],
        };
        let error = tight_rgba(malformed, PhysicalSize::new(0, 20)).unwrap_err();
        assert_eq!(error.code, "skia-readback-layout-invalid");
    }

    #[test]
    fn malformed_readback_metadata_and_layout_are_rejected() {
        let wrong_format = Readback {
            size: PhysicalSize::new(2, 1),
            color_format: ColorFormat::Bgra8Srgb,
            row_bytes: 8,
            pixels: vec![0; 8],
        };
        let error = tight_rgba(wrong_format, PhysicalSize::new(2, 1)).unwrap_err();
        assert_eq!(error.code, "skia-readback-metadata-mismatch");

        let short_row = Readback {
            size: PhysicalSize::new(2, 1),
            color_format: ColorFormat::Rgba8Srgb,
            row_bytes: 7,
            pixels: vec![0; 7],
        };
        let error = tight_rgba(short_row, PhysicalSize::new(2, 1)).unwrap_err();
        assert_eq!(error.code, "skia-readback-layout-invalid");

        let truncated = Readback {
            size: PhysicalSize::new(2, 2),
            color_format: ColorFormat::Rgba8Srgb,
            row_bytes: 8,
            pixels: vec![0; 8],
        };
        let error = tight_rgba(truncated, PhysicalSize::new(2, 2)).unwrap_err();
        assert_eq!(error.code, "skia-readback-layout-invalid");
    }
}
