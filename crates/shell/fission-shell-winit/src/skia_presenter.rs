use fission_render::backend::{BackendOperation, BackendResult, SurfaceMetrics};
use fission_render::capabilities::GraphicsCapabilities;
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
}
