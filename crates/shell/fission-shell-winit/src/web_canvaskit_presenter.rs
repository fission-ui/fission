use std::any::Any;
use std::fmt;

use fission_render::backend::{GraphicsBackendSession, SurfaceMetrics};
use fission_render::capabilities::{ColorFormat, GraphicsCapabilities};
use fission_render::frame::InteractiveFrame;
use fission_render::surface::{
    LossKind, PhysicalSize, Recovery, ScaleFactor, SurfaceDescriptor, SurfaceId, SurfaceKind,
    SurfaceTarget, ThreadAffinity,
};
use fission_render_skia_web::{CanvasKitBackendPreference, CanvasKitDriver, CanvasKitHost};
use js_sys::{ArrayBuffer, Function, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlCanvasElement;
use winit::platform::web::WindowExtWebSys;
use winit::window::Window;

use crate::renderer_diagnostics::{RendererReport, RendererRequest};

const EXECUTOR_FACTORY_GLOBAL: &str = "__FISSION_CANVASKIT_CREATE_EXECUTOR";

/// Browser-side implementation of Fission's owned CanvasKit packet boundary.
///
/// The generated Web bootstrap installs an initialized JavaScript executor
/// factory on `globalThis` before starting the application Wasm module. Every
/// exchange copies both sides of the packet so neither runtime retains a view
/// into the other's linear memory.
struct BrowserCanvasKitHost {
    executor: JsValue,
    submit: Function,
    destroy: Option<Function>,
}

impl BrowserCanvasKitHost {
    fn from_global(canvas: &HtmlCanvasElement) -> Result<Self, BrowserCanvasKitHostError> {
        let global = js_sys::global();
        let factory = Reflect::get(
            &global,
            &JsValue::from_str(EXECUTOR_FACTORY_GLOBAL),
        )
        .map_err(|error| BrowserCanvasKitHostError::js("read CanvasKit executor factory", error))?
        .dyn_into::<Function>()
        .map_err(|_| {
            BrowserCanvasKitHostError(format!(
                "globalThis.{EXECUTOR_FACTORY_GLOBAL} is not a function; the Web bootstrap must initialize Fission's CanvasKit executor factory before starting application Wasm"
            ))
        })?;
        let executor = factory
            .call2(&global, canvas.as_ref(), &JsValue::NULL)
            .map_err(|error| BrowserCanvasKitHostError::js("create CanvasKit executor", error))?;
        if executor.is_null() || executor.is_undefined() {
            return Err(BrowserCanvasKitHostError(
                "CanvasKit executor factory returned no executor".to_string(),
            ));
        }
        let submit = Reflect::get(&executor, &JsValue::from_str("submit"))
            .map_err(|error| BrowserCanvasKitHostError::js("read CanvasKit submit", error))?
            .dyn_into::<Function>()
            .map_err(|_| {
                BrowserCanvasKitHostError("CanvasKit executor.submit is not a function".to_string())
            })?;
        let destroy = Reflect::get(&executor, &JsValue::from_str("destroy"))
            .ok()
            .and_then(|value| value.dyn_into::<Function>().ok());
        Ok(Self {
            executor,
            submit,
            destroy,
        })
    }
}

impl Drop for BrowserCanvasKitHost {
    fn drop(&mut self) {
        if let Some(destroy) = &self.destroy {
            let _ = destroy.call0(&self.executor);
        }
    }
}

impl CanvasKitHost for BrowserCanvasKitHost {
    type Error = BrowserCanvasKitHostError;

    fn exchange(&mut self, request: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        let request = Uint8Array::from(request.as_slice());
        let response = self
            .submit
            .call1(&self.executor, &request)
            .map_err(|error| BrowserCanvasKitHostError::js("submit CanvasKit packet", error))?;
        if response.is_instance_of::<Uint8Array>() {
            return Ok(Uint8Array::new(&response).to_vec());
        }
        if response.is_instance_of::<ArrayBuffer>() {
            return Ok(Uint8Array::new(&response).to_vec());
        }
        Err(BrowserCanvasKitHostError(
            "CanvasKit submit must synchronously return Uint8Array or ArrayBuffer".to_string(),
        ))
    }
}

#[derive(Debug)]
struct BrowserCanvasKitHostError(String);

impl BrowserCanvasKitHostError {
    fn js(operation: &str, error: JsValue) -> Self {
        let details = error
            .as_string()
            .unwrap_or_else(|| format!("JavaScript error: {error:?}"));
        Self(format!("failed to {operation}: {details}"))
    }
}

impl fmt::Display for BrowserCanvasKitHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug)]
struct WebCanvasTarget {
    descriptor: SurfaceDescriptor,
}

impl WebCanvasTarget {
    fn new(width: u32, height: u32, scale_factor: f64) -> anyhow::Result<Self> {
        Ok(Self {
            descriptor: SurfaceDescriptor {
                id: SurfaceId(1),
                kind: SurfaceKind::WebCanvas,
                size: PhysicalSize::new(width.max(1), height.max(1)),
                scale_factor: checked_scale_factor(scale_factor)?,
                color_format: ColorFormat::Rgba8Srgb,
                thread_affinity: ThreadAffinity::MainThread,
            },
        })
    }

    fn metrics(&self) -> SurfaceMetrics {
        SurfaceMetrics {
            size: self.descriptor.size,
            scale_factor: self.descriptor.scale_factor,
        }
    }

    fn update(&mut self, width: u32, height: u32, scale_factor: f64) -> anyhow::Result<bool> {
        let metrics = SurfaceMetrics {
            size: PhysicalSize::new(width.max(1), height.max(1)),
            scale_factor: checked_scale_factor(scale_factor)?,
        };
        if self.metrics() == metrics {
            return Ok(false);
        }
        self.descriptor.size = metrics.size;
        self.descriptor.scale_factor = metrics.scale_factor;
        Ok(true)
    }
}

impl SurfaceTarget for WebCanvasTarget {
    fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Winit host adapter for CanvasKit's software raster profile.
pub(super) struct WebCanvasKitPresenter {
    session: GraphicsBackendSession<'static>,
    target: WebCanvasTarget,
    pub(super) report: RendererReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CanvasKitFrameOutcome {
    Presented,
    SurfaceRecovered(Recovery),
}

impl WebCanvasKitPresenter {
    pub(super) fn new(
        window: &Window,
        request: RendererRequest,
        fallback_reason: Option<String>,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> anyhow::Result<Self> {
        let canvas = window
            .canvas()
            .ok_or_else(|| anyhow::anyhow!("winit web window did not expose a canvas"))?;
        let host = BrowserCanvasKitHost::from_global(&canvas).map_err(anyhow::Error::new)?;
        let driver = CanvasKitDriver::new(host, CanvasKitBackendPreference::Software);
        let mut session = GraphicsBackendSession::new(driver)
            .map_err(|error| anyhow::anyhow!("CanvasKit session initialization failed: {error}"))?;
        let target = WebCanvasTarget::new(width, height, scale_factor)?;
        session
            .attach(&target)
            .map_err(|error| anyhow::anyhow!("CanvasKit software attach failed: {error}"))?;
        Ok(Self {
            session,
            target,
            report: RendererReport::new(
                "web-canvaskit-software",
                request,
                Some("Skia CanvasKit software raster".to_string()),
                None,
                fallback_reason,
                width.max(1),
                height.max(1),
                scale_factor,
            ),
        })
    }

    pub(super) fn capabilities(&self) -> &GraphicsCapabilities {
        self.session.capabilities()
    }

    pub(super) fn sync_surface_metrics(
        &mut self,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> anyhow::Result<()> {
        if self.target.update(width, height, scale_factor)? {
            self.session
                .resize(self.target.metrics())
                .map_err(|error| anyhow::anyhow!("CanvasKit software resize failed: {error}"))?;
        }
        self.report.width = width.max(1);
        self.report.height = height.max(1);
        self.report.scale_factor = scale_factor;
        Ok(())
    }

    pub(super) fn render_and_present(
        &mut self,
        frame: &InteractiveFrame<'_>,
    ) -> anyhow::Result<CanvasKitFrameOutcome> {
        if let Err(error) = self.session.render(frame) {
            return self.recover_surface_or_error("render", error);
        }
        if let Err(error) = self.session.present() {
            return self.recover_surface_or_error("present", error);
        }
        Ok(CanvasKitFrameOutcome::Presented)
    }

    pub(super) fn detach(&mut self) -> anyhow::Result<()> {
        self.session
            .detach()
            .map_err(|error| anyhow::anyhow!("CanvasKit software detach failed: {error}"))
    }

    fn recover_surface_or_error(
        &mut self,
        operation: &str,
        error: fission_render::backend::BackendError,
    ) -> anyhow::Result<CanvasKitFrameOutcome> {
        if error.code != "canvaskit-host-surface-lost" {
            return Err(anyhow::anyhow!(
                "CanvasKit software {operation} failed: {error}"
            ));
        }
        let recovery = self
            .session
            .recover(LossKind::Surface)
            .map_err(|recovery_error| {
                anyhow::anyhow!(
                    "CanvasKit software {operation} lost its surface ({error}); recovery failed: {recovery_error}"
                )
            })?;
        if recovery == Recovery::Unrecoverable {
            return Err(anyhow::anyhow!(
                "CanvasKit software {operation} lost its surface and it was unrecoverable: {error}"
            ));
        }
        Ok(CanvasKitFrameOutcome::SurfaceRecovered(recovery))
    }
}

fn checked_scale_factor(value: f64) -> anyhow::Result<ScaleFactor> {
    ScaleFactor::new(value).map_err(|error| anyhow::anyhow!(error.to_string()))
}
