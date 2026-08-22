use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use fission_render::backend::{
    BackendError, BackendOperation, BackendResult, GraphicsBackendSession, ReadbackRequest,
    SurfaceMetrics,
};
use fission_render::capabilities::{ColorFormat, GraphicsCapabilities};
use fission_render::frame::InteractiveFrame;
use fission_render::surface::{
    LossKind, MemoryPressure, PhysicalSize, Recovery, ScaleFactor, SessionState, SurfaceDescriptor,
    SurfaceId, SurfaceKind,
};
use winit::window::Window;

use crate::native_window_target::{native_thread_affinity, WinitNativeWindowTarget};
use crate::skia_presenter::tight_rgba;

static NEXT_GANESH_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

/// Capture result for a frame that was successfully presented.
///
/// A non-loss readback failure does not strand the acquired swapchain image:
/// the presenter still completes presentation and reports that failure only to
/// the capture requester.
pub(super) enum GaneshCapture {
    NotRequested,
    Pixels(Vec<u8>),
    Failed(BackendError),
}

/// Direct native-window presenter for Skia Ganesh.
///
/// This path owns no wgpu surface or upload texture. The backend session
/// renders into a Skia-wrapped swapchain image and commits that image directly
/// through Vulkan on Linux/Android, Metal on Apple platforms, or D3D12 on
/// Windows. `target` retains the Winit window that owns every borrowed native
/// handle.
pub(super) struct WinitSkiaGaneshPresenter {
    session: GraphicsBackendSession<'static>,
    target: WinitNativeWindowTarget,
    surface_id: SurfaceId,
    metrics: SurfaceMetrics,
}

impl WinitSkiaGaneshPresenter {
    pub(super) fn new(
        profile: &fission_render_skia::SkiaGaneshProfile,
        window: Arc<Window>,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> BackendResult<Self> {
        let surface_id = SurfaceId(NEXT_GANESH_SURFACE_ID.fetch_add(1, Ordering::Relaxed));
        let metrics = build_metrics(BackendOperation::Attach, width, height, scale_factor)?;
        let target = build_target(BackendOperation::Attach, window, surface_id, metrics)?;
        let mut session = profile.create_session()?;
        session.attach(target.target())?;
        Ok(Self {
            session,
            target,
            surface_id,
            metrics,
        })
    }

    pub(super) fn capabilities(&self) -> &GraphicsCapabilities {
        self.session.capabilities()
    }

    pub(super) fn state(&self) -> SessionState {
        self.session.state()
    }

    pub(super) fn sync_surface_metrics(
        &mut self,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> BackendResult<()> {
        let next = build_metrics(BackendOperation::Resize, width, height, scale_factor)?;
        if self.metrics == next {
            return Ok(());
        }
        self.session.resize(next)?;
        self.metrics = next;
        Ok(())
    }

    pub(super) fn suspend(&mut self) -> BackendResult<()> {
        self.session.suspend()
    }

    pub(super) fn resume(
        &mut self,
        window: Arc<Window>,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> BackendResult<()> {
        let metrics = build_metrics(BackendOperation::Resume, width, height, scale_factor)?;
        let target = build_target(BackendOperation::Resume, window, self.surface_id, metrics)?;
        self.session.resume(target.target())?;
        self.target = target;
        self.metrics = metrics;
        Ok(())
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

    /// Render and directly present one frame, optionally reading its RGBA
    /// pixels between those operations. `before_present` performs Winit's
    /// platform presentation notification immediately before the native GPU
    /// backend commits the acquired image.
    pub(super) fn render_and_present(
        &mut self,
        frame: &InteractiveFrame<'_>,
        capture: bool,
        mut before_present: impl FnMut(),
    ) -> BackendResult<GaneshCapture> {
        match self.render_and_present_once(frame, capture, &mut before_present) {
            Ok(capture) => Ok(capture),
            Err(error) => {
                let Some(loss) = skia_loss_kind(&error) else {
                    return Err(error);
                };
                match self.session.recover(loss)? {
                    Recovery::Reattached | Recovery::DeviceRecreated => {
                        self.render_and_present_once(frame, capture, &mut before_present)
                    }
                    Recovery::SwitchedToSoftware | Recovery::Unrecoverable => Err(error),
                }
            }
        }
    }

    fn render_and_present_once(
        &mut self,
        frame: &InteractiveFrame<'_>,
        capture: bool,
        before_present: &mut impl FnMut(),
    ) -> BackendResult<GaneshCapture> {
        let expected_frame_id = frame.metadata().frame_id;
        let render = self.session.render(frame)?;
        if render.frame_id != Some(expected_frame_id) {
            return Err(contract_error(
                BackendOperation::Render,
                "skia-ganesh-render-frame-id-mismatch",
                format!(
                    "Skia Ganesh rendered frame {:?}; Winit submitted {:?}",
                    render.frame_id, expected_frame_id
                ),
            ));
        }

        let capture = if capture {
            match self.session.readback(ReadbackRequest {
                region: None,
                color_format: ColorFormat::Rgba8Srgb,
            }) {
                Ok(readback) => match tight_rgba(readback, self.metrics.size) {
                    Ok(pixels) => GaneshCapture::Pixels(pixels),
                    Err(error) => GaneshCapture::Failed(error),
                },
                Err(error) if skia_loss_kind(&error).is_some() => return Err(error),
                Err(error) => GaneshCapture::Failed(error),
            }
        } else {
            GaneshCapture::NotRequested
        };

        before_present();
        let present = self.session.present()?;
        if present.frame_id != Some(expected_frame_id) {
            return Err(contract_error(
                BackendOperation::Present,
                "skia-ganesh-present-frame-id-mismatch",
                format!(
                    "Skia Ganesh presented frame {:?}; Winit submitted {:?}",
                    present.frame_id, expected_frame_id
                ),
            ));
        }
        Ok(capture)
    }
}

impl Drop for WinitSkiaGaneshPresenter {
    fn drop(&mut self) {
        let _ = self.detach();
    }
}

fn build_metrics(
    operation: BackendOperation,
    width: u32,
    height: u32,
    scale_factor: f64,
) -> BackendResult<SurfaceMetrics> {
    let scale_factor = ScaleFactor::new(scale_factor).map_err(|error| {
        contract_error(
            operation,
            "skia-ganesh-invalid-scale-factor",
            error.to_string(),
        )
    })?;
    Ok(SurfaceMetrics {
        size: PhysicalSize::new(width, height),
        scale_factor,
    })
}

fn build_target(
    operation: BackendOperation,
    window: Arc<Window>,
    id: SurfaceId,
    metrics: SurfaceMetrics,
) -> BackendResult<WinitNativeWindowTarget> {
    WinitNativeWindowTarget::new(
        window,
        SurfaceDescriptor {
            id,
            kind: SurfaceKind::NativeWindow,
            size: metrics.size,
            scale_factor: metrics.scale_factor,
            color_format: ColorFormat::Bgra8Srgb,
            thread_affinity: native_thread_affinity(),
        },
    )
    .map_err(|error| contract_error(operation, "skia-ganesh-native-window", error.to_string()))
}

fn contract_error(
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
    fn metrics_reject_invalid_scale_before_native_attachment() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let error = build_metrics(BackendOperation::Attach, 10, 20, scale).unwrap_err();
            assert_eq!(error.code, "skia-ganesh-invalid-scale-factor");
        }

        let metrics = build_metrics(BackendOperation::Resize, 320, 240, 1.5).unwrap();
        assert_eq!(metrics.size, PhysicalSize::new(320, 240));
        assert_eq!(metrics.scale_factor.get(), 1.5);
    }

    #[test]
    fn recovery_classifier_accepts_only_stable_skia_loss_codes() {
        let surface = contract_error(
            BackendOperation::Render,
            "skia-surface-lost",
            "surface unavailable",
        );
        let device = contract_error(
            BackendOperation::Present,
            "skia-device-lost",
            "device unavailable",
        );
        let unrelated = contract_error(
            BackendOperation::Present,
            "surface-outdated",
            "host surface changed",
        );

        assert_eq!(skia_loss_kind(&surface), Some(LossKind::Surface));
        assert_eq!(skia_loss_kind(&device), Some(LossKind::Device));
        assert_eq!(skia_loss_kind(&unrelated), None);
    }
}
