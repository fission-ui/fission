use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::thread::ThreadId;

use fission_layout::ParagraphResultStore;
use fission_render::backend::{GraphicsBackendSession, ReadbackRequest, SurfaceMetrics};
use fission_render::capabilities::{ColorFormat, GraphicsCapabilities};
use fission_render::frame::InteractiveFrame;
use fission_render::surface::{
    LossKind, MemoryPressure, PhysicalSize, Recovery, ScaleFactor, SurfaceDescriptor, SurfaceId,
    SurfaceKind, SurfaceTarget, ThreadAffinity,
};
use fission_render_skia_web::{
    CanvasKitBackendPreference, CanvasKitFont, CanvasKitHost, CanvasKitParagraphHost,
    CanvasKitPixelRegion, CanvasKitProfile, CanvasKitReadback,
};
use js_sys::{ArrayBuffer, Function, Object, Reflect, Uint8Array};
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
thread_local! {
    static CANVASKIT_EXECUTORS: RefCell<ExecutorRegistry> =
        RefCell::new(ExecutorRegistry::default());
}

#[derive(Default)]
struct ExecutorRegistry {
    next_key: u64,
    executors: BTreeMap<u32, BrowserCanvasKitExecutor>,
}

struct BrowserCanvasKitExecutor {
    executor: JsValue,
    submit: Function,
    layout_paragraph: Function,
    destroy_paragraph: Function,
    read_pixels: Function,
    trim_memory: Function,
    destroy: Option<Function>,
}

impl BrowserCanvasKitExecutor {
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
        let layout_paragraph = required_method(&executor, "layoutParagraph")?;
        let destroy_paragraph = required_method(&executor, "destroyParagraph")?;
        let read_pixels = required_method(&executor, "readPixels")?;
        let trim_memory = required_method(&executor, "trimMemory")?;
        Ok(Self {
            executor,
            submit,
            layout_paragraph,
            destroy_paragraph,
            read_pixels,
            trim_memory,
            destroy,
        })
    }
}

impl Drop for BrowserCanvasKitExecutor {
    fn drop(&mut self) {
        if let Some(destroy) = &self.destroy {
            let _ = destroy.call0(&self.executor);
        }
    }
}

/// Send-safe reference to a main-thread JavaScript executor.
///
/// `ParagraphEngine` is a `Send + Sync` contract, but `JsValue` is not. The
/// concrete browser objects therefore remain in their owning thread-local
/// registry and only this numeric key crosses the renderer abstraction.
struct BrowserCanvasKitHost {
    key: u32,
    owner: ThreadId,
}

impl BrowserCanvasKitHost {
    fn from_global(canvas: &HtmlCanvasElement) -> Result<Self, BrowserCanvasKitHostError> {
        let executor = BrowserCanvasKitExecutor::from_global(canvas)?;
        let key = CANVASKIT_EXECUTORS.with(|registry| {
            let mut registry = registry.borrow_mut();
            let next = registry.next_key.checked_add(1).ok_or_else(|| {
                BrowserCanvasKitHostError("CanvasKit executor keys are exhausted".into())
            })?;
            let key = u32::try_from(next).map_err(|_| {
                BrowserCanvasKitHostError("CanvasKit executor keys are exhausted".into())
            })?;
            registry.next_key = next;
            registry.executors.insert(key, executor);
            Ok(key)
        })?;
        Ok(Self {
            key,
            owner: std::thread::current().id(),
        })
    }

    fn with_executor<T>(
        &self,
        operation: &'static str,
        f: impl FnOnce(&mut BrowserCanvasKitExecutor) -> Result<T, BrowserCanvasKitHostError>,
    ) -> Result<T, BrowserCanvasKitHostError> {
        if std::thread::current().id() != self.owner {
            return Err(BrowserCanvasKitHostError(format!(
                "cannot {operation}; the CanvasKit executor belongs to another browser thread"
            )));
        }
        CANVASKIT_EXECUTORS.with(|registry| {
            let mut registry = registry.try_borrow_mut().map_err(|_| {
                BrowserCanvasKitHostError(format!(
                    "cannot {operation}; the CanvasKit executor is already in use"
                ))
            })?;
            let executor = registry.executors.get_mut(&self.key).ok_or_else(|| {
                BrowserCanvasKitHostError(format!(
                    "cannot {operation}; the CanvasKit executor has been retired"
                ))
            })?;
            f(executor)
        })
    }
}

impl Drop for BrowserCanvasKitHost {
    fn drop(&mut self) {
        if std::thread::current().id() == self.owner {
            CANVASKIT_EXECUTORS.with(|registry| {
                if let Ok(mut registry) = registry.try_borrow_mut() {
                    registry.executors.remove(&self.key);
                }
            });
        }
    }
}

impl CanvasKitHost for BrowserCanvasKitHost {
    type Error = BrowserCanvasKitHostError;

    fn exchange(&mut self, request: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        self.with_executor("submit a CanvasKit packet", |host| {
            let request = Uint8Array::from(request.as_slice());
            let response = host
                .submit
                .call1(&host.executor, &request)
                .map_err(|error| BrowserCanvasKitHostError::js("submit CanvasKit packet", error))?;
            copied_packet(response, "CanvasKit submit")
        })
    }

    fn supports_readback(&self) -> bool {
        true
    }

    fn read_pixels_rgba8888(
        &mut self,
        region: CanvasKitPixelRegion,
    ) -> Result<Option<CanvasKitReadback>, Self::Error> {
        self.with_executor("read CanvasKit surface pixels", |host| {
            let response = host
                .read_pixels
                .call4(
                    &host.executor,
                    &JsValue::from_f64(f64::from(region.x)),
                    &JsValue::from_f64(f64::from(region.y)),
                    &JsValue::from_f64(f64::from(region.width)),
                    &JsValue::from_f64(f64::from(region.height)),
                )
                .map_err(|error| {
                    BrowserCanvasKitHostError::js("read CanvasKit surface pixels", error)
                })?;
            if !response.is_instance_of::<Uint8Array>() {
                return Err(BrowserCanvasKitHostError(
                    "CanvasKit readPixels must synchronously return Uint8Array".into(),
                ));
            }
            let pixels = Uint8Array::new(&response).to_vec();
            let row_bytes = usize::try_from(region.width)
                .ok()
                .and_then(|width| width.checked_mul(4))
                .ok_or_else(|| {
                    BrowserCanvasKitHostError("CanvasKit readback row length overflowed".into())
                })?;
            Ok(Some(CanvasKitReadback {
                size: region.size(),
                row_bytes,
                pixels,
            }))
        })
    }

    fn trim_memory(&mut self, pressure: MemoryPressure) -> Result<bool, Self::Error> {
        self.with_executor("trim CanvasKit memory", |host| {
            let pressure = match pressure {
                MemoryPressure::Moderate => 1.0,
                MemoryPressure::Critical => 2.0,
            };
            host.trim_memory
                .call1(&host.executor, &JsValue::from_f64(pressure))
                .map_err(|error| BrowserCanvasKitHostError::js("trim CanvasKit memory", error))?;
            Ok(true)
        })
    }
}

impl CanvasKitParagraphHost for BrowserCanvasKitHost {
    fn layout_paragraph(&mut self, request: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        self.with_executor("layout a CanvasKit paragraph", |host| {
            let request = Uint8Array::from(request.as_slice());
            let response = host
                .layout_paragraph
                .call1(&host.executor, &request)
                .map_err(|error| {
                    BrowserCanvasKitHostError::js("layout CanvasKit paragraph", error)
                })?;
            copied_packet(response, "CanvasKit layoutParagraph")
        })
    }

    fn destroy_paragraph(
        &mut self,
        handle: fission_render_skia_web::ResourceHandle,
    ) -> Result<(), Self::Error> {
        self.with_executor("destroy a CanvasKit paragraph", |host| {
            let value = Object::new();
            Reflect::set(
                &value,
                &JsValue::from_str("slot"),
                &JsValue::from_f64(f64::from(handle.slot)),
            )
            .map_err(|error| BrowserCanvasKitHostError::js("encode paragraph slot", error))?;
            Reflect::set(
                &value,
                &JsValue::from_str("generation"),
                &JsValue::from_f64(f64::from(handle.generation)),
            )
            .map_err(|error| BrowserCanvasKitHostError::js("encode paragraph generation", error))?;
            host.destroy_paragraph
                .call1(&host.executor, value.as_ref())
                .map_err(|error| {
                    BrowserCanvasKitHostError::js("destroy CanvasKit paragraph", error)
                })?;
            Ok(())
        })
    }
}

fn required_method(
    executor: &JsValue,
    name: &'static str,
) -> Result<Function, BrowserCanvasKitHostError> {
    Reflect::get(executor, &JsValue::from_str(name))
        .map_err(|error| BrowserCanvasKitHostError::js("read CanvasKit executor method", error))?
        .dyn_into::<Function>()
        .map_err(|_| {
            BrowserCanvasKitHostError(format!("CanvasKit executor.{name} is not a function"))
        })
}

fn copied_packet(value: JsValue, operation: &str) -> Result<Vec<u8>, BrowserCanvasKitHostError> {
    if value.is_instance_of::<Uint8Array>() || value.is_instance_of::<ArrayBuffer>() {
        return Ok(Uint8Array::new(&value).to_vec());
    }
    Err(BrowserCanvasKitHostError(format!(
        "{operation} must synchronously return Uint8Array or ArrayBuffer"
    )))
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

/// Winit host adapter for CanvasKit's WebGL and software profiles.
pub(super) struct WebCanvasKitPresenter {
    paragraph_store: Arc<ParagraphResultStore>,
    session: GraphicsBackendSession<'static>,
    target: WebCanvasTarget,
    pub(super) report: RendererReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CanvasKitCapture {
    NotRequested,
    Pixels(Vec<u8>),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CanvasKitFrameOutcome {
    Presented(CanvasKitCapture),
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
        packaged_fonts: &[fission_theme::PackagedFont],
    ) -> anyhow::Result<Self> {
        let canvas = window
            .canvas()
            .ok_or_else(|| anyhow::anyhow!("winit web window did not expose a canvas"))?;
        let host = BrowserCanvasKitHost::from_global(&canvas).map_err(anyhow::Error::new)?;
        let profile = CanvasKitProfile::new(
            host,
            crate::app::DEFAULT_FONT_FAMILY,
            packaged_fonts
                .iter()
                .map(|font| CanvasKitFont::new(font.family, font.data))
                .collect(),
        )
        .map_err(|error| {
            anyhow::anyhow!("CanvasKit font profile initialization failed: {error}")
        })?;
        let paragraph_store = Arc::new(ParagraphResultStore::new(Arc::new(
            profile.paragraph_engine(),
        )));
        let (backend_preference, active, backend) = match request {
            RendererRequest::CanvasKitSoftware => (
                CanvasKitBackendPreference::Software,
                "web-canvaskit-software",
                "Skia CanvasKit software raster",
            ),
            RendererRequest::CanvasKitWebGl => (
                CanvasKitBackendPreference::WebGl,
                "web-canvaskit-webgl",
                "Skia CanvasKit Ganesh WebGL",
            ),
            RendererRequest::CanvasKitAuto | RendererRequest::Auto => (
                CanvasKitBackendPreference::Auto,
                "web-canvaskit-auto",
                "Skia CanvasKit WebGL with software fallback",
            ),
            _ => {
                return Err(anyhow::anyhow!(
                    "renderer request `{}` does not select CanvasKit",
                    request.as_str()
                ))
            }
        };
        let mut session = profile
            .create_session(backend_preference)
            .map_err(|error| anyhow::anyhow!("CanvasKit session initialization failed: {error}"))?;
        let target = WebCanvasTarget::new(width, height, scale_factor)?;
        session
            .attach(&target)
            .map_err(|error| anyhow::anyhow!("CanvasKit attach failed: {error}"))?;
        Ok(Self {
            paragraph_store,
            session,
            target,
            report: RendererReport::new(
                active,
                request,
                Some(backend.to_string()),
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

    pub(super) fn paragraph_store(&self) -> Arc<ParagraphResultStore> {
        Arc::clone(&self.paragraph_store)
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
                .map_err(|error| anyhow::anyhow!("CanvasKit resize failed: {error}"))?;
        }
        self.report.width = width.max(1);
        self.report.height = height.max(1);
        self.report.scale_factor = scale_factor;
        Ok(())
    }

    pub(super) fn render_and_present(
        &mut self,
        frame: &InteractiveFrame<'_>,
        capture_requested: bool,
    ) -> anyhow::Result<CanvasKitFrameOutcome> {
        if let Err(error) = self.session.render(frame) {
            return self.recover_surface_or_error("render", error);
        }
        let capture = if capture_requested {
            match self.session.readback(ReadbackRequest {
                region: None,
                color_format: ColorFormat::Rgba8Srgb,
            }) {
                Ok(readback) => {
                    let tight_row_bytes = usize::try_from(readback.size.width)
                        .ok()
                        .and_then(|width| width.checked_mul(4));
                    if tight_row_bytes == Some(readback.row_bytes) {
                        CanvasKitCapture::Pixels(readback.pixels)
                    } else {
                        CanvasKitCapture::Failed(format!(
                            "CanvasKit returned padded screenshot rows ({} bytes for {} pixels)",
                            readback.row_bytes, readback.size.width
                        ))
                    }
                }
                Err(error) => CanvasKitCapture::Failed(error.to_string()),
            }
        } else {
            CanvasKitCapture::NotRequested
        };
        if let Err(error) = self.session.present() {
            return self.recover_surface_or_error("present", error);
        }
        Ok(CanvasKitFrameOutcome::Presented(capture))
    }

    pub(super) fn detach(&mut self) -> anyhow::Result<()> {
        self.session
            .detach()
            .map_err(|error| anyhow::anyhow!("CanvasKit detach failed: {error}"))
    }

    pub(super) fn trim_memory(&mut self, pressure: MemoryPressure) -> anyhow::Result<()> {
        self.session
            .trim_memory(pressure)
            .map_err(|error| anyhow::anyhow!("CanvasKit memory trim failed: {error}"))
    }

    fn recover_surface_or_error(
        &mut self,
        operation: &str,
        error: fission_render::backend::BackendError,
    ) -> anyhow::Result<CanvasKitFrameOutcome> {
        if error.code != "canvaskit-host-surface-lost" {
            return Err(anyhow::anyhow!("CanvasKit {operation} failed: {error}"));
        }
        let recovery = self
            .session
            .recover(LossKind::Surface)
            .map_err(|recovery_error| {
                    anyhow::anyhow!(
                    "CanvasKit {operation} lost its surface ({error}); recovery failed: {recovery_error}"
                )
            })?;
        if recovery == Recovery::Unrecoverable {
            return Err(anyhow::anyhow!(
                "CanvasKit {operation} lost its surface and it was unrecoverable: {error}"
            ));
        }
        Ok(CanvasKitFrameOutcome::SurfaceRecovered(recovery))
    }
}

fn checked_scale_factor(value: f64) -> anyhow::Result<ScaleFactor> {
    ScaleFactor::new(value).map_err(|error| anyhow::anyhow!(error.to_string()))
}
