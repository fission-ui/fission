#![allow(unexpected_cfgs)]

use anyhow::Result;
use base64::Engine;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(target_os = "android")]
use winit::platform::android::{activity::AndroidApp, EventLoopBuilderExtAndroid};
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowBuilderExtWebSys;
use winit::{
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{Event, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    window::{CursorIcon, Window, WindowBuilder, WindowId},
};

use fission_core::env::VideoStatus;
use fission_core::lowering::LoweringContext;
use fission_core::ui::custom_render::downcast_render_object;
use fission_core::{
    Action, ActionId, AppState, BuildCtx, Env, InputEvent, KeyCode, KeyEvent as FissionKeyEvent,
    OpenUrlRequest, PointerButton, PointerEvent, Runtime, RuntimeEffect, ServiceBindings, View,
    Widget, OPEN_URL,
};
use fission_core::{ActionInput, CapabilityInvocationPayload, Effect};
use fission_diagnostics::prelude as diag;
use fission_ir::semantics::MouseCursor;
use fission_ir::{CoreIR, NodeId, Op, WidgetNodeId};
use fission_layout::{LayoutEngine, LayoutSize};
use fission_render::{LayoutPoint, LayoutRect, Renderer as _};
use fission_render_vello::parley::FontContext;
use fission_render_vello::{RetainedSceneCache, VelloRenderer, VelloTextMeasurer};
use fission_shell::async_host::{
    AsyncMessage, AsyncRegistry, RunningServiceHandle, ServiceControlMessage,
};
use fission_shell::{VideoBackend, VideoEvent, VideoPlayer};
use fission_theme::fonts;
use fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache};

use fission_test_driver::TestEvent;

// Vello / WGPU
use pollster::block_on;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaSupport, Renderer as VelloSceneRenderer, RendererOptions, Scene};
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

mod compositor;
use compositor::TextureLayerCompositor;
mod pipeline;
pub use pipeline::{InvalidationSet, Pipeline};
mod software_renderer;
use software_renderer::SoftwareRenderer;
mod video_backend;
#[cfg(target_os = "macos")]
use video_backend::MacVideoBackend;
#[cfg(not(target_os = "macos"))]
use video_backend::MockVideoBackend;
mod web_backend;
#[cfg(target_os = "macos")]
use web_backend::MacWebBackend;
#[cfg(not(target_os = "macos"))]
use web_backend::MockWebBackend;

mod clipboard;
use clipboard::DesktopClipboard;
mod ime;
use ime::{DesktopImeHandler, TextInputConfig};
pub mod test_control;

use fission_core::action::ActionEnvelope;

type EffectResult = AsyncMessage;

type ServiceKey = (String, String);
type ServiceBindingKey = (String, String, u64);

struct ActiveServiceHandle {
    runtime: RunningServiceHandle,
}

fn open_host_url(url: &str) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
            .map(|_| ())
    } else {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
    }
}

fn register_builtin_operation_capabilities(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        OPEN_URL,
        |request: OpenUrlRequest, _| async move {
            let _ = request.in_app;
            open_host_url(&request.url).map_err(|error| error.to_string())?;
            Ok(())
        },
    );
}

struct ActivePlayer {
    player: Box<dyn VideoPlayer>,
    last_status: Option<VideoStatus>,
    last_rate: Option<f32>,
    last_volume: Option<f32>,
    last_muted: Option<bool>,
}

struct RenderState<'w> {
    surface: RenderSurface<'w>,
    target_texture_size: (u32, u32),
    scene3d_renderer: fission_3d::render::Scene3DRenderer,
    main_renderer: MainRenderer,
}

enum MainRenderer {
    Vello {
        renderer: VelloSceneRenderer,
        texture_compositor: TextureLayerCompositor,
    },
    Software,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct WindowViewportState {
    physical_size: PhysicalSize<u32>,
    scale_factor: f64,
}

impl WindowViewportState {
    fn from_window(window: &Window) -> Self {
        Self {
            physical_size: window.inner_size(),
            scale_factor: normalize_scale_factor(window.scale_factor()),
        }
    }

    fn logical_size(self) -> LayoutSize {
        physical_size_to_layout_size(self.physical_size, self.scale_factor)
    }

    fn with_physical_size(self, physical_size: PhysicalSize<u32>) -> Self {
        Self {
            physical_size,
            ..self
        }
    }

    fn with_logical_size(self, logical_size: LayoutSize) -> Self {
        self.with_physical_size(logical_viewport_to_physical_size(
            logical_size,
            self.scale_factor,
        ))
    }

    fn with_scale_factor(self, scale_factor: f64) -> Self {
        let scale_factor = normalize_scale_factor(scale_factor);
        let logical_size = self.logical_size();
        Self {
            physical_size: logical_viewport_to_physical_size(logical_size, scale_factor),
            scale_factor,
        }
    }
}

fn create_render_state<'w>(
    render_cx: &mut RenderContext,
    window: Arc<Window>,
    viewport: WindowViewportState,
) -> anyhow::Result<RenderState<'w>> {
    let mut surface = block_on(render_cx.create_surface(
        window.clone(),
        viewport.physical_size.width,
        viewport.physical_size.height,
        wgpu::PresentMode::AutoVsync,
    ))
    .map_err(|error| anyhow::anyhow!("failed to create render surface: {error}"))?;

    let device_handle = &render_cx.devices[surface.dev_id];
    #[cfg(target_os = "ios")]
    device_handle.device.on_uncaptured_error(Box::new(|error| {
        eprintln!("wgpu uncaptured error: {error}");
    }));
    let downlevel_caps = device_handle.adapter().get_downlevel_capabilities();
    let force_software_renderer = std::env::var("FISSION_FORCE_SOFTWARE_RENDERER")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let supports_indirect_execution = downlevel_caps
        .flags
        .contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION);
    let use_software_renderer = force_software_renderer
        || (cfg!(any(target_os = "ios", target_os = "android")) && !supports_indirect_execution);
    let use_cpu_vello = std::env::var("FISSION_VELLO_USE_CPU")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
        || !supports_indirect_execution;
    if use_software_renderer {
        eprintln!(
            "fission-shell-winit: using software renderer fallback (missing INDIRECT_EXECUTION support)"
        );
    } else if use_cpu_vello {
        eprintln!(
            "fission-shell-winit: using Vello CPU fallback (missing INDIRECT_EXECUTION support)"
        );
    }
    let surface_caps = surface.surface.get_capabilities(device_handle.adapter());
    surface.config.alpha_mode = surface_caps
        .alpha_modes
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::CompositeAlphaMode::PostMultiplied)
        .unwrap_or_else(|| {
            surface_caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
        });
    surface
        .surface
        .configure(&device_handle.device, &surface.config);

    let target_texture_size = (surface.config.width, surface.config.height);
    recreate_target_texture(
        &mut surface,
        render_cx,
        target_texture_size.0,
        target_texture_size.1,
    );

    let scene3d_renderer = fission_3d::render::Scene3DRenderer::new(
        &device_handle.device,
        viewport.physical_size.width,
        viewport.physical_size.height,
        wgpu::TextureFormat::Rgba8Unorm,
    );

    let main_renderer = if use_software_renderer {
        MainRenderer::Software
    } else {
        let renderer = VelloSceneRenderer::new(
            &device_handle.device,
            RendererOptions {
                use_cpu: use_cpu_vello,
                antialiasing_support: AaSupport::all(),
                num_init_threads: None,
                pipeline_cache: None,
            },
        )
        .map_err(|error| anyhow::anyhow!("failed to create vello renderer: {error}"))?;

        let texture_compositor =
            TextureLayerCompositor::new(&device_handle.device, wgpu::TextureFormat::Rgba8Unorm);
        MainRenderer::Vello {
            renderer,
            texture_compositor,
        }
    };

    Ok(RenderState {
        surface,
        target_texture_size,
        scene3d_renderer,
        main_renderer,
    })
}

fn build_window(
    title: &str,
    background_test_mode: bool,
    target: &EventLoopWindowTarget<TestEvent>,
) -> anyhow::Result<Arc<Window>> {
    let mut window_builder = WindowBuilder::new().with_title(title);
    #[cfg(target_arch = "wasm32")]
    {
        window_builder = window_builder.with_append(true).with_prevent_default(true);
    }
    if background_test_mode {
        window_builder = window_builder.with_active(false).with_visible(false);
    }
    Ok(Arc::new(window_builder.build(target).map_err(|e| {
        anyhow::anyhow!("Window build error: {}", e)
    })?))
}

#[cfg(target_os = "android")]
fn current_window(window: &Option<Arc<Window>>) -> Option<&Arc<Window>> {
    window.as_ref()
}

#[cfg(not(target_os = "android"))]
fn current_window(window: &Arc<Window>) -> Option<&Arc<Window>> {
    Some(window)
}

#[cfg(target_os = "android")]
fn current_window_id(window: &Option<Arc<Window>>) -> Option<WindowId> {
    window.as_ref().map(|window| window.id())
}

#[cfg(not(target_os = "android"))]
fn current_window_id(window: &Arc<Window>) -> Option<WindowId> {
    Some(window.id())
}

fn request_redraw_throttled(
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
) {
    let now = Instant::now();
    let next = *last_redraw_at + min_frame;
    if now >= next {
        *last_redraw_at = now;
        *redraw_pending = false;
        window.request_redraw();
    } else {
        *redraw_pending = true;
        elwt.set_control_flow(ControlFlow::WaitUntil(next));
    }
}

fn frame_trace_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("FISSION_FRAME_TRACE")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
    })
}

#[derive(Default)]
struct FrameTraceState {
    enabled: bool,
    redraw_reasons: Vec<String>,
}

impl FrameTraceState {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            redraw_reasons: Vec::new(),
        }
    }

    fn note_redraw_reason(&mut self, reason: impl Into<String>) {
        if !self.enabled {
            return;
        }
        let reason = reason.into();
        if !self
            .redraw_reasons
            .iter()
            .any(|existing| existing == &reason)
        {
            self.redraw_reasons.push(reason);
        }
    }

    fn take_redraw_reasons(&mut self) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        std::mem::take(&mut self.redraw_reasons)
    }

    fn emit(
        &self,
        phase: &str,
        frame: u64,
        active_animation_keys: &[String],
        invalidations: InvalidationSet,
        reasons: &[String],
        detail: &str,
    ) {
        if !self.enabled {
            return;
        }
        let active = if active_animation_keys.is_empty() {
            "none".to_string()
        } else {
            active_animation_keys.join(",")
        };
        let reasons = if reasons.is_empty() {
            "none".to_string()
        } else {
            reasons.join(",")
        };
        eprintln!(
            "[frame-trace] phase={} frame={} invalidation={} active=[{}] reasons=[{}] {}",
            phase,
            frame,
            invalidations.labels().join("+"),
            active,
            reasons,
            detail,
        );
    }
}

fn request_redraw_logged(
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    reason: &str,
) {
    frame_trace.note_redraw_reason(reason);
    request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
}

fn apply_authoritative_resize(
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    next_viewport: WindowViewportState,
    pending_resize: &mut Option<WindowViewportState>,
    resize_needs_settled_frame: &mut bool,
    pending_capture_settle: &mut bool,
    pending_screenshot_path: Option<&str>,
    live_resize: &mut LiveResizeController,
    invalidations: &mut InvalidationSet,
    last_redraw_at: &mut Instant,
    resize_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    reason: &str,
) {
    *pending_resize = Some(next_viewport);
    *resize_needs_settled_frame = true;
    if pending_screenshot_path.is_some() {
        *pending_capture_settle = true;
    }
    live_resize.note_resize(Instant::now());
    invalidations.mark_composite();
    request_redraw_logged(
        window,
        elwt,
        last_redraw_at,
        resize_frame,
        redraw_pending,
        frame_trace,
        reason,
    );
}

fn active_animation_keys(runtime: &Runtime) -> Vec<String> {
    let mut keys = runtime
        .runtime_state
        .animation
        .active
        .iter()
        .map(|((target, property), anim)| {
            let repeat = if anim.repeat { "repeat" } else { "finite" };
            format!("{}:{:?}:{}", target.as_u128(), property, repeat)
        })
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn repeating_animation_redraw_interval(
    animation_map: &fission_core::env::AnimationStateMap,
    default_repeat_frame: Duration,
) -> Option<Duration> {
    animation_map
        .active
        .values()
        .filter(|anim| anim.repeat)
        .map(|anim| {
            anim.frame_interval_ms
                .filter(|ms| *ms > 0)
                .map(Duration::from_millis)
                .unwrap_or(default_repeat_frame)
        })
        .min()
}

fn animation_redraw_interval(
    has_finite_animation: bool,
    repeat_animation_frame: Option<Duration>,
    has_playing_video: bool,
    min_frame: Duration,
) -> Option<Duration> {
    if has_finite_animation || has_playing_video {
        Some(min_frame)
    } else if let Some(repeat_frame) = repeat_animation_frame {
        Some(repeat_frame)
    } else {
        None
    }
}

fn pending_work_redraw_interval(
    invalidations: InvalidationSet,
    pending_resize: bool,
    min_frame: Duration,
    resize_frame: Duration,
) -> Duration {
    if pending_resize && !invalidations.build && !invalidations.paint && !invalidations.composite {
        resize_frame
    } else {
        min_frame
    }
}

fn resize_is_unsettled(pending_resize: bool, needs_settled_frame: bool, live_resize: bool) -> bool {
    pending_resize || needs_settled_frame || live_resize
}

fn resolve_build_viewport(
    last_built_viewport: Option<LayoutSize>,
    target_viewport: LayoutSize,
    has_prev_ir: bool,
    invalidations: &mut InvalidationSet,
) -> LayoutSize {
    let built_viewport = last_built_viewport.unwrap_or(target_viewport);
    if built_viewport != target_viewport {
        // Viewport-sensitive build output must stay aligned with the layout viewport.
        invalidations.mark_build();
    }

    if invalidations.build || !has_prev_ir || last_built_viewport.is_none() {
        target_viewport
    } else {
        built_viewport
    }
}

#[derive(Debug)]
struct LiveResizeController {
    active_until: Option<Instant>,
    settle_delay: Duration,
}

impl LiveResizeController {
    fn new(settle_delay: Duration) -> Self {
        Self {
            active_until: None,
            settle_delay,
        }
    }

    fn note_resize(&mut self, now: Instant) {
        self.active_until = Some(now + self.settle_delay);
    }

    fn is_live(&self, now: Instant) -> bool {
        self.active_until
            .map(|deadline| now < deadline)
            .unwrap_or(false)
    }
}

/// Drain pending effects from the runtime, delegating capability work to the
/// async registry and runtime-control effects to the shell/runtime boundary.
///
/// Returns `true` if any synchronous callback was dispatched (caller should redraw).
fn process_pending_effects(
    runtime: &mut Runtime,
    effect_tx: &mpsc::Sender<AsyncMessage>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
) -> bool {
    let pending = std::mem::take(&mut runtime.pending_effects);
    if pending.is_empty() {
        return false;
    }

    let dispatched_callback = false;
    let wake = {
        let proxy = Arc::new(Mutex::new(event_proxy.clone()));
        Arc::new(move || {
            if let Ok(proxy) = proxy.lock() {
                let _ = proxy.send_event(TestEvent::Wake);
            }
        })
    };

    for env in pending {
        match env.effect {
            Effect::Runtime(ref runtime_effect) => {
                diag::emit(
                    diag::DiagCategory::Input,
                    diag::DiagLevel::Debug,
                    diag::DiagEventKind::InputEvent {
                        kind: format!("runtime_effect:{:?}", runtime_effect),
                        target: None,
                        position: None,
                    },
                );
                match runtime_effect {
                    RuntimeEffect::Cancel { .. } | RuntimeEffect::ReleaseResource { .. } => {}
                }
            }
            Effect::Capability(capability) => match capability {
                CapabilityInvocationPayload::Operation(op) => {
                    if !async_registry.spawn_capability(
                        &op.capability_name,
                        env.req_id,
                        op.request,
                        env.on_ok.clone(),
                        env.on_err.clone(),
                        env.resource.clone(),
                        effect_tx,
                        wake.clone(),
                    ) {
                        let _ = effect_tx.send(AsyncMessage::CapabilityErr {
                            capability_name: op.capability_name,
                            req_id: env.req_id,
                            payload: None,
                            on_err: env.on_err.clone(),
                            message: Some(
                                "no async operation capability handler registered".into(),
                            ),
                            resource: env.resource.clone(),
                        });
                        (wake)();
                    }
                }
            },
            Effect::Job(job) => {
                if !async_registry.spawn_job(
                    &job.job_name,
                    env.req_id,
                    job.payload,
                    env.on_ok.clone(),
                    env.on_err.clone(),
                    env.resource.clone(),
                    effect_tx,
                    wake.clone(),
                ) {
                    let _ = effect_tx.send(AsyncMessage::JobErr {
                        job_name: job.job_name,
                        req_id: env.req_id,
                        payload: None,
                        on_err: env.on_err.clone(),
                        message: Some("no async job handler registered".into()),
                        resource: env.resource.clone(),
                    });
                    (wake)();
                }
            }
            Effect::StartService(start) => {
                let key = (start.service_name.clone(), start.slot_key.clone());
                if let Some(previous) = active_services.remove(&key) {
                    let _ = previous
                        .runtime
                        .control_tx
                        .send(ServiceControlMessage::Stop);
                }

                let instance_id = *next_service_instance_id;
                *next_service_instance_id = next_service_instance_id.saturating_add(1);
                let bindings = env.service_bindings.clone().unwrap_or_default();
                service_bindings.insert(
                    (
                        start.service_name.clone(),
                        start.slot_key.clone(),
                        instance_id,
                    ),
                    bindings,
                );

                match async_registry.spawn_service(
                    &start.service_name,
                    &start.slot_key,
                    instance_id,
                    start.config,
                    env.resource.clone(),
                    effect_tx,
                    wake.clone(),
                ) {
                    Some(handle) => {
                        active_services.insert(key, ActiveServiceHandle { runtime: handle });
                    }
                    None => {
                        let _ = service_bindings.remove(&(
                            start.service_name.clone(),
                            start.slot_key.clone(),
                            instance_id,
                        ));
                        let _ = effect_tx.send(AsyncMessage::ServiceStartFailed {
                            service_name: start.service_name,
                            slot_key: start.slot_key,
                            instance_id,
                            payload: None,
                            message: Some("no async service handler registered".into()),
                            resource: env.resource.clone(),
                        });
                        (wake)();
                    }
                }
            }
            Effect::ServiceCommand(command) => {
                let key = (command.service_name.clone(), command.slot_key.clone());
                if let Some(handle) = active_services.get(&key) {
                    let _ = handle
                        .runtime
                        .control_tx
                        .send(ServiceControlMessage::Command {
                            req_id: env.req_id,
                            payload: command.payload,
                            on_ok: env.on_ok.clone(),
                            on_err: env.on_err.clone(),
                        });
                } else {
                    let _ = effect_tx.send(AsyncMessage::ServiceCommandErr {
                        service_name: command.service_name,
                        slot_key: command.slot_key,
                        instance_id: 0,
                        req_id: env.req_id,
                        payload: None,
                        on_err: env.on_err.clone(),
                        message: Some("service is not running".into()),
                        resource: env.resource.clone(),
                    });
                    (wake)();
                }
            }
            Effect::StopService(stop) => {
                let key = (stop.service_name.clone(), stop.slot_key.clone());
                if let Some(handle) = active_services.remove(&key) {
                    let _ = handle.runtime.control_tx.send(ServiceControlMessage::Stop);
                }
            }
        }
    }

    dispatched_callback
}

/// Drain completed background effect results from the channel and dispatch
/// their continuations on the main thread.
///
/// Returns `true` if any continuation was dispatched (caller should redraw).
fn drain_effect_results(
    runtime: &mut Runtime,
    effect_rx: &mpsc::Receiver<AsyncMessage>,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
) -> bool {
    let mut dispatched = false;

    while let Ok(message) = effect_rx.try_recv() {
        match message {
            AsyncMessage::JobOk {
                job_name,
                req_id,
                payload,
                on_ok,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        NodeId::derived(0, &[0]),
                        &ActionInput::JobOk {
                            job_name,
                            req_id,
                            payload,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::JobErr {
                job_name,
                req_id,
                payload,
                on_err,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        NodeId::derived(0, &[0]),
                        &ActionInput::JobErr {
                            job_name,
                            req_id,
                            payload,
                            message,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::ServiceStarted {
                service_name,
                slot_key,
                instance_id,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let Some(current) = active_services.get(&key) else {
                    continue;
                };
                if current.runtime.instance_id != instance_id {
                    continue;
                }
                if let Some(bindings) =
                    service_bindings.get(&(service_name.clone(), slot_key.clone(), instance_id))
                {
                    if let Some(action) = bindings.on_started.clone() {
                        let _ = runtime.dispatch_with_input(
                            action,
                            NodeId::derived(0, &[0]),
                            &ActionInput::ServiceStarted {
                                service_name,
                                slot_key,
                                instance_id,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceStartFailed {
                service_name,
                slot_key,
                instance_id,
                payload,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        service_bindings.remove(&(service_name, slot_key, instance_id));
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let should_dispatch = active_services
                    .get(&key)
                    .map(|current| current.runtime.instance_id == instance_id)
                    .unwrap_or(true);
                active_services.remove(&key);
                let bindings =
                    service_bindings.remove(&(service_name.clone(), slot_key.clone(), instance_id));
                if should_dispatch {
                    if let Some(action) = bindings.and_then(|bindings| bindings.on_start_failed) {
                        let _ = runtime.dispatch_with_input(
                            action,
                            NodeId::derived(0, &[0]),
                            &ActionInput::ServiceStartFailed {
                                service_name,
                                slot_key,
                                payload,
                                message,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceEvent {
                service_name,
                slot_key,
                instance_id,
                payload,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let Some(current) = active_services.get(&key) else {
                    continue;
                };
                if current.runtime.instance_id != instance_id {
                    continue;
                }
                if let Some(bindings) =
                    service_bindings.get(&(service_name.clone(), slot_key.clone(), instance_id))
                {
                    if let Some(action) = bindings.on_event.clone() {
                        let _ = runtime.dispatch_with_input(
                            action,
                            NodeId::derived(0, &[0]),
                            &ActionInput::ServiceEvent {
                                service_name,
                                slot_key,
                                instance_id,
                                payload,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceStopped {
                service_name,
                slot_key,
                instance_id,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        service_bindings.remove(&(service_name, slot_key, instance_id));
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let should_dispatch = active_services
                    .get(&key)
                    .map(|current| current.runtime.instance_id == instance_id)
                    .unwrap_or(true);
                if should_dispatch {
                    active_services.remove(&key);
                }
                let bindings =
                    service_bindings.remove(&(service_name.clone(), slot_key.clone(), instance_id));
                if should_dispatch {
                    if let Some(action) = bindings.and_then(|bindings| bindings.on_stopped) {
                        let _ = runtime.dispatch_with_input(
                            action,
                            NodeId::derived(0, &[0]),
                            &ActionInput::ServiceStopped {
                                service_name,
                                slot_key,
                                instance_id,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceCommandOk {
                service_name,
                slot_key,
                instance_id,
                req_id,
                payload,
                on_ok,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let Some(current) = active_services.get(&key) else {
                    continue;
                };
                if current.runtime.instance_id != instance_id {
                    continue;
                }
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        NodeId::derived(0, &[0]),
                        &ActionInput::ServiceCommandOk {
                            service_name,
                            slot_key,
                            instance_id,
                            req_id,
                            payload,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::ServiceCommandErr {
                service_name,
                slot_key,
                instance_id,
                req_id,
                payload,
                on_err,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                if instance_id != 0 {
                    let Some(current) = active_services.get(&key) else {
                        continue;
                    };
                    if current.runtime.instance_id != instance_id {
                        continue;
                    }
                }
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        NodeId::derived(0, &[0]),
                        &ActionInput::ServiceCommandErr {
                            service_name,
                            slot_key,
                            instance_id,
                            req_id,
                            payload,
                            message,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::CapabilityOk {
                capability_name,
                req_id,
                payload,
                on_ok,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        NodeId::derived(0, &[0]),
                        &ActionInput::CapabilityOk {
                            capability: capability_name,
                            req_id,
                            payload,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::CapabilityErr {
                capability_name,
                req_id,
                payload,
                on_err,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        NodeId::derived(0, &[0]),
                        &ActionInput::CapabilityErr {
                            capability: capability_name,
                            req_id,
                            payload,
                            message,
                        },
                    );
                    dispatched = true;
                }
            }
        }
    }

    dispatched
}

fn focused_text_input_id(runtime: &Runtime, ir: Option<&CoreIR>) -> Option<NodeId> {
    let focused = runtime.runtime_state.interaction.focused?;
    let ir = ir?;
    let mut current = Some(focused);
    while let Some(id) = current {
        let node = ir.nodes.get(&id)?;
        if let Op::Semantics(sem) = &node.op {
            if sem.role == fission_ir::Role::TextInput {
                return Some(id);
            }
        }
        current = node.parent;
    }
    None
}

fn focused_text_input_config(runtime: &Runtime, ir: Option<&CoreIR>) -> Option<TextInputConfig> {
    let id = focused_text_input_id(runtime, ir)?;
    let ir = ir?;
    let node = ir.nodes.get(&id)?;
    match &node.op {
        Op::Semantics(semantics) => Some(TextInputConfig::from_semantics(semantics)),
        _ => None,
    }
}

fn focused_custom_text_input(runtime: &Runtime, ir: Option<&CoreIR>) -> bool {
    let focused = match runtime.runtime_state.interaction.focused {
        Some(id) => id,
        None => return false,
    };
    let ir = match ir {
        Some(ir) => ir,
        None => return false,
    };
    let mut current = Some(focused);
    while let Some(id) = current {
        if let Some(any_ro) = ir.custom_render_objects.get(&id) {
            if let Some(render_obj) = downcast_render_object(any_ro) {
                if render_obj.accepts_text_input() {
                    return true;
                }
            }
        }
        current = ir.nodes.get(&id).and_then(|node| node.parent);
    }
    false
}

fn reset_text_input_caret(
    runtime: &mut Runtime,
    ir: Option<&CoreIR>,
    last_blink_toggle: &mut Instant,
) {
    if let Some(id) = focused_text_input_id(runtime, ir) {
        runtime.runtime_state.caret_visible.insert(id, true);
        *last_blink_toggle = Instant::now();
    }
}

#[derive(Debug, Clone)]
struct PendingTextTrace {
    seq: u64,
    source: String,
    target: Option<NodeId>,
    started_at: Instant,
    handled_at: Option<Instant>,
    effects_at: Option<Instant>,
    present_after_frame: u64,
}

fn start_text_trace(
    enabled: bool,
    traces: &mut VecDeque<PendingTextTrace>,
    next_seq: &mut u64,
    source: String,
    target: Option<NodeId>,
    presented_frames: u64,
) -> Option<u64> {
    if !enabled {
        return None;
    }
    *next_seq += 1;
    let seq = *next_seq;
    traces.push_back(PendingTextTrace {
        seq,
        source,
        target,
        started_at: Instant::now(),
        handled_at: None,
        effects_at: None,
        present_after_frame: presented_frames + 1,
    });
    Some(seq)
}

fn mark_text_trace_handled(traces: &mut VecDeque<PendingTextTrace>, seq: Option<u64>) {
    if let Some(seq) = seq {
        if let Some(trace) = traces.iter_mut().rev().find(|trace| trace.seq == seq) {
            trace.handled_at = Some(Instant::now());
        }
    }
}

fn mark_text_trace_effects(traces: &mut VecDeque<PendingTextTrace>, seq: Option<u64>) {
    if let Some(seq) = seq {
        if let Some(trace) = traces.iter_mut().rev().find(|trace| trace.seq == seq) {
            trace.effects_at = Some(Instant::now());
        }
    }
}

fn set_text_trace_target(
    traces: &mut VecDeque<PendingTextTrace>,
    seq: Option<u64>,
    target: Option<NodeId>,
) {
    if let Some(seq) = seq {
        if let Some(trace) = traces.iter_mut().rev().find(|trace| trace.seq == seq) {
            trace.target = target;
        }
    }
}

fn cancel_text_trace(traces: &mut VecDeque<PendingTextTrace>, seq: Option<u64>) {
    if let Some(seq) = seq {
        traces.retain(|trace| trace.seq != seq);
    }
}

fn flush_text_traces(
    enabled: bool,
    traces: &mut VecDeque<PendingTextTrace>,
    presented_frames: u64,
) {
    if !enabled {
        traces.clear();
        return;
    }

    loop {
        let should_flush = traces
            .front()
            .map(|trace| trace.present_after_frame <= presented_frames)
            .unwrap_or(false);
        if !should_flush {
            break;
        }

        let Some(trace) = traces.pop_front() else {
            break;
        };
        let now = Instant::now();
        let handled_at = trace.handled_at.unwrap_or(now);
        let effects_at = trace.effects_at.unwrap_or(handled_at);
        let total_ms = now.duration_since(trace.started_at).as_secs_f64() * 1000.0;
        let handle_ms = handled_at.duration_since(trace.started_at).as_secs_f64() * 1000.0;
        let effects_ms = effects_at.duration_since(handled_at).as_secs_f64() * 1000.0;
        let queue_ms = now.duration_since(effects_at).as_secs_f64() * 1000.0;

        let target_u128 = trace.target.map(|id| id.as_u128());
        let msg = format!(
            "text_input_latency seq={} src={} handle_ms={:.2} effects_ms={:.2} queue_ms={:.2} total_ms={:.2} frame={}",
            trace.seq, trace.source, handle_ms, effects_ms, queue_ms, total_ms, presented_frames
        );
        eprintln!("[text-trace] {}", msg);
        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Info,
            diag::DiagEventKind::InputEvent {
                kind: msg,
                target: target_u128,
                position: None,
            },
        );
    }
}

// ─── Extracted handler functions ─────────────────────────────────────────
// These are called by BOTH real WindowEvent handlers AND the TestEvent (UserEvent)
// handler, ensuring test infrastructure exercises the exact same code paths.

/// Map a test button index (0=left, 1=right, 2=middle) to a `PointerButton`.
fn map_test_button(button: u8) -> PointerButton {
    match button {
        0 => PointerButton::Primary,
        1 => PointerButton::Secondary,
        2 => PointerButton::Middle,
        n => PointerButton::Other(n),
    }
}

fn cursor_icon_for(cursor: MouseCursor) -> CursorIcon {
    match cursor {
        MouseCursor::Default => CursorIcon::Default,
        MouseCursor::Pointer => CursorIcon::Pointer,
        MouseCursor::Text => CursorIcon::Text,
        MouseCursor::Crosshair => CursorIcon::Crosshair,
        MouseCursor::Move => CursorIcon::Move,
        MouseCursor::NotAllowed => CursorIcon::NotAllowed,
        MouseCursor::Grab => CursorIcon::Grab,
        MouseCursor::Grabbing => CursorIcon::Grabbing,
        MouseCursor::Wait => CursorIcon::Wait,
        MouseCursor::Help => CursorIcon::Help,
        MouseCursor::VerticalText => CursorIcon::VerticalText,
    }
}

fn sync_window_cursor(window: &Window, runtime: &Runtime) {
    window.set_cursor_icon(cursor_icon_for(runtime.runtime_state.interaction.cursor()));
}

/// Handle cursor/mouse move — shared by WindowEvent::CursorMoved and TestEvent::MouseMove.
fn handle_cursor_moved(
    x: f32,
    y: f32,
    modifiers: u8,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let point = LayoutPoint { x, y };
        let event = InputEvent::Pointer(PointerEvent::Move { point, modifiers });
        if let Err(e) = runtime.handle_input(event, ir, layout) {
            eprintln!("Input handling error: {:?}", e);
        }
        sync_window_cursor(window, runtime);
        invalidations.mark_build();
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            invalidations.mark_build();
            request_redraw_logged(
                window,
                elwt,
                last_redraw_at,
                min_frame,
                redraw_pending,
                frame_trace,
                "pointer_move:effects",
            );
        }
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            "pointer_move",
        );
    }
}

/// Handle mouse button press/release — shared by WindowEvent::MouseInput and
/// TestEvent::MouseDown / TestEvent::MouseUp.
fn handle_mouse_button(
    x: f32,
    y: f32,
    button: PointerButton,
    is_pressed: bool,
    modifiers: u8,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    text_trace_enabled: bool,
    pending_text_traces: &mut VecDeque<PendingTextTrace>,
    next_text_trace_seq: &mut u64,
    presented_frames: u64,
    last_blink_toggle: &mut Instant,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let point = LayoutPoint { x, y };
        let pointer_event = if is_pressed {
            PointerEvent::Down {
                point,
                button,
                modifiers,
            }
        } else {
            PointerEvent::Up {
                point,
                button,
                modifiers,
            }
        };
        let input_event = InputEvent::Pointer(pointer_event);

        let trace_seq = if text_trace_enabled && is_pressed {
            start_text_trace(
                text_trace_enabled,
                pending_text_traces,
                next_text_trace_seq,
                "pointer_down".to_string(),
                None,
                presented_frames,
            )
        } else {
            None
        };

        if let Err(e) = runtime.handle_input(input_event, ir, layout) {
            eprintln!("Input handling error: {:?}", e);
        }
        sync_window_cursor(window, runtime);
        invalidations.mark_build();

        mark_text_trace_handled(pending_text_traces, trace_seq);
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            mark_text_trace_effects(pending_text_traces, trace_seq);
            invalidations.mark_build();
            request_redraw_logged(
                window,
                elwt,
                last_redraw_at,
                min_frame,
                redraw_pending,
                frame_trace,
                if is_pressed {
                    "pointer_down:effects"
                } else {
                    "pointer_up:effects"
                },
            );
        }
        if is_pressed {
            let target = focused_text_input_id(runtime, pipeline.prev_ir.as_ref());
            if target.is_some() {
                set_text_trace_target(pending_text_traces, trace_seq, target);
            } else {
                cancel_text_trace(pending_text_traces, trace_seq);
            }
            reset_text_input_caret(runtime, pipeline.prev_ir.as_ref(), last_blink_toggle);
        }
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            if is_pressed {
                "pointer_down"
            } else {
                "pointer_up"
            },
        );
    }
}

/// Handle scroll — shared by WindowEvent::MouseWheel and TestEvent::Scroll.
fn handle_scroll(
    point_x: f32,
    point_y: f32,
    delta_x: f32,
    delta_y: f32,
    modifiers: u8,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let point = LayoutPoint {
            x: point_x,
            y: point_y,
        };
        let scroll_delta = LayoutPoint {
            x: delta_x,
            y: delta_y,
        };
        let event = InputEvent::Pointer(PointerEvent::Scroll {
            point,
            delta: scroll_delta,
            modifiers,
        });
        if let Err(e) = runtime.handle_input(event, ir, layout) {
            eprintln!("Scroll error: {:?}", e);
        }
        sync_window_cursor(window, runtime);
        // Scroll offsets can affect more than a compositor translation. Virtualized
        // lists, scrollbars, and scroll-aware wrappers depend on the updated offset
        // during build/lowering, so treat scroll as a build invalidation.
        invalidations.mark_build();
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            invalidations.mark_build();
            request_redraw_logged(
                window,
                elwt,
                last_redraw_at,
                min_frame,
                redraw_pending,
                frame_trace,
                "scroll:effects",
            );
        }
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            "scroll",
        );
    }
}

fn handle_cursor_left(
    last_cursor_position: Option<PhysicalPosition<f64>>,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let Some(ir) = &pipeline.prev_ir {
        let point = last_cursor_position.map(|position| {
            let scale_factor = window.scale_factor();
            LayoutPoint::new(
                (position.x / scale_factor) as f32,
                (position.y / scale_factor) as f32,
            )
        });
        match runtime.clear_hover_state(ir, point) {
            Ok(changed) => {
                sync_window_cursor(window, runtime);
                if changed {
                    invalidations.mark_build();
                    if process_pending_effects(
                        runtime,
                        effect_result_tx,
                        event_proxy,
                        async_registry,
                        active_services,
                        service_bindings,
                        next_service_instance_id,
                    ) {
                        invalidations.mark_build();
                        request_redraw_logged(
                            window,
                            elwt,
                            last_redraw_at,
                            min_frame,
                            redraw_pending,
                            frame_trace,
                            "cursor_left:effects",
                        );
                    }
                    request_redraw_logged(
                        window,
                        elwt,
                        last_redraw_at,
                        min_frame,
                        redraw_pending,
                        frame_trace,
                        "cursor_left",
                    );
                }
            }
            Err(error) => eprintln!("Cursor-left handling error: {:?}", error),
        }
    } else {
        sync_window_cursor(window, runtime);
    }
}

/// Parse a key name string into a `KeyCode`.
fn parse_key_code(key: &str) -> KeyCode {
    match key {
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Escape,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Space" => KeyCode::Space,
        s if s.len() == 1 => KeyCode::Char(s.chars().next().unwrap()),
        _ => KeyCode::Space,
    }
}

/// Handle a key-down event — shared by WindowEvent::KeyboardInput and
/// TestEvent::KeyDown / TestEvent::TextInput.
///
/// Returns `true` if the app key handler consumed the event.
fn handle_key_down<S: AppState>(
    code: KeyCode,
    modifiers: u8,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    text_trace_enabled: bool,
    pending_text_traces: &mut VecDeque<PendingTextTrace>,
    next_text_trace_seq: &mut u64,
    presented_frames: u64,
    last_blink_toggle: &mut Instant,
    key_handler: Option<&KeyHandler<S>>,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) -> bool {
    let ir_and_snap = match (&pipeline.prev_ir, &pipeline.last_snapshot) {
        (Some(ir), Some(snap)) => Some((ir, snap)),
        _ => None,
    };

    // App-level key handler intercepts before framework
    if let Some(handler) = key_handler {
        let handler = handler.clone();
        if let Some(state) = runtime.get_app_state_mut::<S>() {
            if handler(state, &code, modifiers) {
                if process_pending_effects(
                    runtime,
                    effect_result_tx,
                    event_proxy,
                    async_registry,
                    active_services,
                    service_bindings,
                    next_service_instance_id,
                ) {
                    invalidations.mark_build();
                    request_redraw_logged(
                        window,
                        elwt,
                        last_redraw_at,
                        min_frame,
                        redraw_pending,
                        frame_trace,
                        "key_handler:effects",
                    );
                }
                invalidations.mark_build();
                request_redraw_logged(
                    window,
                    elwt,
                    last_redraw_at,
                    min_frame,
                    redraw_pending,
                    frame_trace,
                    "key_handler",
                );
                return true;
            }
        }
    }

    if let Some((ir, layout)) = ir_and_snap {
        let target = focused_text_input_id(runtime, pipeline.prev_ir.as_ref());
        let trace_seq = start_text_trace(
            text_trace_enabled && target.is_some(),
            pending_text_traces,
            next_text_trace_seq,
            format!("keyboard:{:?}", code),
            target,
            presented_frames,
        );
        let input_event = InputEvent::Keyboard(FissionKeyEvent::Down {
            key_code: code,
            modifiers,
        });
        if let Err(e) = runtime.handle_input(input_event, ir, layout) {
            eprintln!("Keyboard error: {:?}", e);
        }
        invalidations.mark_build();
        mark_text_trace_handled(pending_text_traces, trace_seq);
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            mark_text_trace_effects(pending_text_traces, trace_seq);
            invalidations.mark_build();
            request_redraw_logged(
                window,
                elwt,
                last_redraw_at,
                min_frame,
                redraw_pending,
                frame_trace,
                "keyboard:effects",
            );
        }
        reset_text_input_caret(runtime, pipeline.prev_ir.as_ref(), last_blink_toggle);
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            "keyboard",
        );
    }

    false
}

/// Build the response for a GetText query.
fn build_get_text_response(pipeline: &Pipeline) -> fission_test_driver::TestResponse {
    use fission_test_driver::{TestResponse, TextItem};
    let mut items = Vec::new();
    if let (Some(ir), Some(snap)) = (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref()) {
        let mut reachable = std::collections::HashSet::new();
        let mut stack = ir.root.into_iter().collect::<Vec<_>>();
        while let Some(node_id) = stack.pop() {
            if !reachable.insert(node_id) {
                continue;
            }
            if let Some(node) = ir.nodes.get(&node_id) {
                stack.extend(node.children.iter().copied());
            }
        }

        for id in reachable {
            let Some(node) = ir.nodes.get(&id) else {
                continue;
            };
            let text_content = match &node.op {
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) => {
                    Some(text.clone())
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) => {
                    Some(runs.iter().map(|r| r.text.clone()).collect::<String>())
                }
                _ => None,
            };
            if let Some(text) = text_content {
                if text.is_empty() {
                    continue;
                }
                let check_id = node.parent.unwrap_or(id);
                let rect = snap
                    .get_node_rect(check_id)
                    .or_else(|| snap.get_node_rect(id));
                let (x, y, w, h) = rect
                    .map(|r| (r.x(), r.y(), r.width(), r.height()))
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                items.push(TextItem {
                    text,
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
    }
    TestResponse::Text { items }
}

fn find_visible_text_center(pipeline: &Pipeline, text: &str) -> Option<(f32, f32)> {
    let fission_test_driver::TestResponse::Text { items } = build_get_text_response(pipeline)
    else {
        return None;
    };
    items
        .into_iter()
        .find(|item| item.text.contains(text) && item.width > 0.0 && item.height > 0.0)
        .map(|item| (item.x + item.width / 2.0, item.y + item.height / 2.0))
}

/// Build the response for a GetTree query.
fn build_get_tree_response(pipeline: &Pipeline) -> fission_test_driver::TestResponse {
    use fission_test_driver::{SemanticNode, TestResponse};
    let mut nodes = Vec::new();
    if let Some(ir) = &pipeline.prev_ir {
        for (id, node) in &ir.nodes {
            if let fission_ir::Op::Semantics(sem) = &node.op {
                let rect = pipeline
                    .last_snapshot
                    .as_ref()
                    .and_then(|s| s.get_node_rect(*id));
                let (x, y, w, h) = rect
                    .map(|r| (r.x(), r.y(), r.width(), r.height()))
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                nodes.push(SemanticNode {
                    role: format!("{:?}", sem.role),
                    label: sem.label.clone(),
                    value: sem.value.clone(),
                    focusable: sem.focusable,
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
    }
    TestResponse::Tree { nodes }
}

/// Handle TapText — find text in the IR, tap at its center.
fn handle_tap_text(
    text: &str,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    use fission_test_driver::TestResponse;
    if let (Some(ir), Some(snap)) = (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref()) {
        if let Some((cx, cy)) = find_visible_text_center(pipeline, text) {
            let point = LayoutPoint::new(cx, cy);
            let _ = runtime.handle_input(
                InputEvent::Pointer(PointerEvent::Down {
                    point,
                    button: PointerButton::Primary,
                    modifiers: 0,
                }),
                ir,
                snap,
            );
            let _ = runtime.handle_input(
                InputEvent::Pointer(PointerEvent::Up {
                    point,
                    button: PointerButton::Primary,
                    modifiers: 0,
                }),
                ir,
                snap,
            );
            TestResponse::Ok {}
        } else {
            TestResponse::Error {
                message: format!("text '{}' not found", text),
            }
        }
    } else {
        TestResponse::Error {
            message: "no frame rendered yet".into(),
        }
    }
}

fn wrap_portal_for_viewport(
    id: Option<WidgetNodeId>,
    node: fission_core::Node,
    env: &Env,
) -> fission_core::Node {
    let builder = fission_core::ui::Container::new(node)
        .width(env.viewport_size.width)
        .height(env.viewport_size.height);
    if let Some(id) = id {
        builder
            .id(fission_core::NodeId::derived(id.as_u128(), &[0x0000_F001]))
            .into_node()
    } else {
        builder.into_node()
    }
}

fn texture_plan_fits_device_limits(
    plan: &crate::pipeline::CompositorTexturePlan,
    scale_factor: f64,
    max_texture_dimension_2d: u32,
) -> bool {
    if plan.scene.is_some() {
        let width = ((plan.bounds.size.width as f64 * scale_factor).ceil() as u32).max(1);
        let height = ((plan.bounds.size.height as f64 * scale_factor).ceil() as u32).max(1);
        if width > max_texture_dimension_2d || height > max_texture_dimension_2d {
            return false;
        }
    }
    plan.children
        .iter()
        .all(|child| texture_plan_fits_device_limits(child, scale_factor, max_texture_dimension_2d))
}

fn texture_plans_fit_device_limits(
    plans: &[crate::pipeline::CompositorTexturePlan],
    scale_factor: f64,
    max_texture_dimension_2d: u32,
) -> bool {
    plans
        .iter()
        .all(|plan| texture_plan_fits_device_limits(plan, scale_factor, max_texture_dimension_2d))
}

pub type KeyHandler<S> = Arc<dyn Fn(&mut S, &fission_core::KeyCode, u8) -> bool + Send + Sync>;
pub type FrameHook<S> = Arc<dyn Fn(&mut S) -> bool + Send + Sync>;

pub struct WinitApp<S: AppState, W: Widget<S>> {
    runtime: Runtime,
    layout_engine: LayoutEngine,
    root_widget: W,
    env: Env,
    pipeline: Pipeline,
    measurer: Arc<VelloTextMeasurer>,
    sync_env: Option<Arc<dyn Fn(&S, &mut Env) + Send + Sync>>,
    key_handler: Option<KeyHandler<S>>,
    frame_hook: Option<FrameHook<S>>,
    title: String,
    test_control_port: Option<u16>,
    /// Channel pair for receiving completed background effect results.
    effect_result_tx: mpsc::Sender<AsyncMessage>,
    effect_result_rx: mpsc::Receiver<AsyncMessage>,
    async_registry: AsyncRegistry,
    startup_action: Option<ActionEnvelope>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: AppState + Default, W: Widget<S> + 'static> WinitApp<S, W> {
    pub fn new(root_widget: W) -> Self {
        let mut runtime = Runtime::default();
        runtime.add_app_state(Box::new(S::default())).unwrap();

        const DEFAULT_FONT_FAMILY: &str = "Fission Default";
        let font_cx = Arc::new(Mutex::new(build_font_context()));
        {
            let mut font_cx = font_cx.lock().unwrap();
            let font_data = fonts::default_font_bytes().to_vec();
            let info_override = FontInfoOverride {
                family_name: Some(DEFAULT_FONT_FAMILY),
                ..Default::default()
            };
            font_cx
                .collection
                .register_fonts(Blob::from(font_data), Some(info_override));
        }
        let measurer = Arc::new(VelloTextMeasurer::new_with_default_family(
            font_cx.clone(),
            DEFAULT_FONT_FAMILY,
        ));
        let env = Env::new(measurer.clone() as Arc<dyn fission_layout::TextMeasurer>);
        let clipboard: Arc<dyn fission_core::env::Clipboard> = Arc::new(DesktopClipboard::new());

        let layout_engine = LayoutEngine::new().with_measurer(measurer.clone());
        let runtime = runtime
            .with_measurer(measurer.clone())
            .with_clipboard(clipboard);

        let (effect_result_tx, effect_result_rx) = mpsc::channel();
        let mut async_registry = AsyncRegistry::new();
        register_builtin_operation_capabilities(&mut async_registry);

        Self {
            runtime,
            layout_engine,
            root_widget,
            env,
            pipeline: Pipeline::new(),
            measurer,
            sync_env: None,
            key_handler: None,
            frame_hook: None,
            title: "Fission".into(),
            test_control_port: None,
            effect_result_tx,
            effect_result_rx,
            async_registry,
            startup_action: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_key_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut S, &fission_core::KeyCode, u8) -> bool + Send + Sync + 'static,
    {
        self.key_handler = Some(Arc::new(handler));
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_test_control_port(mut self, port: u16) -> Self {
        self.test_control_port = Some(port);
        self
    }

    /// Mutate the initial application state before the first frame.
    pub fn with_state_init<F>(mut self, init: F) -> Self
    where
        F: FnOnce(&mut S),
    {
        if let Some(state) = self.runtime.get_app_state_mut::<S>() {
            init(state);
        }
        self
    }

    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    pub fn with_sync_env<F>(mut self, f: F) -> Self
    where
        F: Fn(&S, &mut Env) + Send + Sync + 'static,
    {
        self.sync_env = Some(Arc::new(f));
        self
    }

    /// Register a hook that runs on every `AboutToWait` event with mutable
    /// access to the application state.  Return `true` to request a redraw.
    /// Useful for polling background services (e.g. LSP) between key events.
    pub fn with_frame_hook<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut S) -> bool + Send + Sync + 'static,
    {
        self.frame_hook = Some(Arc::new(f));
        self
    }

    pub fn with_async<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(&mut AsyncRegistry),
    {
        configure(&mut self.async_registry);
        self
    }

    pub fn with_startup_action<A: Action>(mut self, action: A) -> Self {
        self.startup_action = Some(action.into());
        self
    }

    pub fn register_reducer(
        &mut self,
        action_id: ActionId,
        reducer: fn(&mut S, &fission_core::ActionEnvelope, NodeId) -> Result<()>,
    ) -> Result<()> {
        self.runtime.register_reducer::<S>(action_id, reducer)
    }

    pub fn absorb_registry(&mut self, registry: fission_core::ActionRegistry<S>) {
        self.runtime.absorb_persistent_registry(registry);
    }

    pub fn run(self) -> Result<()> {
        self.run_inner(
            #[cfg(target_os = "android")]
            None,
        )
    }

    #[cfg(target_os = "android")]
    pub fn run_with_android_app(self, android_app: AndroidApp) -> Result<()> {
        self.run_inner(Some(android_app))
    }

    fn run_inner(
        mut self,
        #[cfg(target_os = "android")] android_app: Option<AndroidApp>,
    ) -> Result<()> {
        diag::emit(
            diag::DiagCategory::Frame,
            diag::DiagLevel::Info,
            diag::DiagEventKind::FrameStart { root: None },
        );
        diag::init_from_env();

        // Build event loop with TestEvent as the user event type.
        // This allows the test control server to inject events via EventLoopProxy.
        let background_test_mode = std::env::var_os("FISSION_BACKGROUND_TEST").is_some();
        let mut event_loop_builder = EventLoopBuilder::<TestEvent>::with_user_event();
        #[cfg(target_os = "android")]
        if let Some(app) = android_app {
            event_loop_builder.with_android_app(app);
        }
        #[cfg(target_os = "macos")]
        if background_test_mode {
            event_loop_builder.with_activation_policy(ActivationPolicy::Accessory);
            event_loop_builder.with_activate_ignoring_other_apps(false);
            event_loop_builder.with_default_menu(false);
        }
        let event_loop = event_loop_builder
            .build()
            .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))?;
        let event_proxy = event_loop.create_proxy();
        let window_title = self.title.clone();
        let ime_handler = Arc::new(DesktopImeHandler::default());
        self.runtime = self.runtime.with_ime_handler(ime_handler.clone());

        #[cfg(not(target_os = "android"))]
        let window = build_window(&window_title, background_test_mode, &event_loop)?;
        #[cfg(not(target_os = "android"))]
        ime_handler.set_window(Some(window.clone()));
        #[cfg(target_os = "android")]
        let mut window: Option<Arc<Window>> = None;

        // Rendering state is created lazily so Android can wait for a valid
        // native surface after the first resume event.
        #[cfg(target_os = "android")]
        if std::env::var_os("WGPU_BACKEND").is_none() {
            eprintln!("fission-shell-winit: forcing WGPU_BACKEND=gl on Android");
            std::env::set_var("WGPU_BACKEND", "gl");
        }
        let mut render_cx = RenderContext::new();
        let mut render_state: Option<RenderState<'_>> = None;
        let mut scene = Scene::new();
        let mut retained_scene_cache = RetainedSceneCache::default();

        #[cfg(not(target_os = "android"))]
        window.request_redraw();

        let mut runtime = self.runtime;
        let mut layout_engine = self.layout_engine;
        let root_widget = self.root_widget;
        let mut env = self.env;
        let mut pipeline = self.pipeline;
        let measurer = self.measurer;
        let effect_result_tx = self.effect_result_tx;
        let effect_result_rx = self.effect_result_rx;
        let async_registry = self.async_registry;
        let startup_action = self.startup_action;
        let mut startup_dispatched = false;
        let mut next_service_instance_id = 1_u64;
        let mut active_services: HashMap<ServiceKey, ActiveServiceHandle> = HashMap::new();
        let mut service_bindings: HashMap<ServiceBindingKey, ServiceBindings> = HashMap::new();

        #[cfg(target_os = "macos")]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MacVideoBackend::new(&window));
        #[cfg(not(target_os = "macos"))]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MockVideoBackend::new());
        #[cfg(target_os = "macos")]
        let web_backend = MacWebBackend::new(&window);
        #[cfg(not(target_os = "macos"))]
        let web_backend = MockWebBackend::new();
        let mut players: HashMap<WidgetNodeId, ActivePlayer> = HashMap::new();

        let mut last_cursor_position: Option<PhysicalPosition<f64>> = None;
        let mut active_primary_touch: Option<u64> = None;
        let mut touch_positions: HashMap<u64, PhysicalPosition<f64>> = HashMap::new();
        let max_fps = std::env::var("FISSION_MAX_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(60);
        let min_frame = Duration::from_secs_f32(1.0 / max_fps as f32);
        let repeat_animation_fps = std::env::var("FISSION_REPEAT_ANIMATION_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .map(|v| v.min(max_fps))
            .unwrap_or(10);
        let repeat_animation_frame = Duration::from_secs_f32(1.0 / repeat_animation_fps as f32);
        let resize_fps = std::env::var("FISSION_RESIZE_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .map(|v| v.min(max_fps))
            .unwrap_or(60);
        let resize_frame = Duration::from_secs_f32(1.0 / resize_fps as f32);
        let resize_settle_delay = Duration::from_millis(
            std::env::var("FISSION_RESIZE_SETTLE_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(90),
        );
        let mut last_redraw_at = Instant::now()
            .checked_sub(min_frame)
            .unwrap_or_else(Instant::now);
        let mut redraw_pending = false;
        let mut last_frame_time = Instant::now();
        let blink_enabled = std::env::var("FISSION_TEXTINPUT_BLINK")
            .map(|v| !matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "no"))
            .unwrap_or(true);
        let blink_period = Duration::from_millis(
            std::env::var("FISSION_TEXTINPUT_BLINK_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(530),
        );
        let mut last_blink_toggle = Instant::now();
        let mut blink_focus_id: Option<NodeId> = None;
        let text_trace_enabled = std::env::var("FISSION_TEXT_TRACE")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        let mut frame_trace = FrameTraceState::new(frame_trace_enabled());
        let mut presented_frames: u64 = 0;
        let mut next_text_trace_seq: u64 = 0;
        let mut pending_text_traces: VecDeque<PendingTextTrace> = VecDeque::new();
        let mut current_mods: u8 = 0;

        // Test control (enabled via FISSION_TEST_CONTROL_PORT env var).
        // The TCP server injects TestEvents via the EventLoopProxy and receives
        // query responses through a dedicated mpsc channel.
        let test_control_port = self.test_control_port.or_else(|| {
            std::env::var("FISSION_TEST_CONTROL_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
        });
        #[cfg(target_os = "android")]
        let pending_test_events = test_control::create_pending_event_queue();
        let test_response_tx: Option<test_control::ResponseSender> =
            test_control_port.map(|port| {
                let (resp_tx, resp_rx) = test_control::create_response_channel();
                #[cfg(target_os = "android")]
                let injector = test_control::EventInjector::Queue {
                    queue: pending_test_events.clone(),
                    wake_proxy: Some(event_proxy.clone()),
                };
                #[cfg(not(target_os = "android"))]
                let injector = test_control::EventInjector::Proxy(event_proxy.clone());
                test_control::spawn_server(port, injector, resp_rx);
                resp_tx
            });
        // Pending screenshot/pump: path + whether it needs a screenshot (vs pump).
        let mut pending_screenshot_path: Option<String> = None;
        #[cfg(not(target_os = "android"))]
        let mut window_viewport = WindowViewportState::from_window(&window);
        #[cfg(target_os = "android")]
        let mut window_viewport: Option<WindowViewportState> = None;
        #[cfg(not(target_os = "android"))]
        let mut pending_resize = Some(window_viewport);
        #[cfg(target_os = "android")]
        let mut pending_resize = None;
        let mut resize_needs_settled_frame = pending_resize.is_some();
        let mut pending_capture_settle = false;
        let mut last_built_viewport: Option<LayoutSize> = None;
        let mut live_resize = LiveResizeController::new(resize_settle_delay);
        let mut invalidations = InvalidationSet {
            build: true,
            layout: true,
            paint: true,
            composite: true,
        };

        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Wait);
                let debug_android_events = cfg!(target_os = "android")
                    && std::env::var_os("FISSION_DEBUG_ANDROID_EVENTS").is_some();

                let mut handle_test_event = |test_event: TestEvent| {
                    if debug_android_events {
                        eprintln!("[android-events] user_event={test_event:?}");
                    }
                    match test_event {
                        TestEvent::MouseMove { x, y } => {
                            let Some(window) = current_window(&window) else {
                                return;
                            };
                            let scale_factor = window.scale_factor();
                            last_cursor_position = Some(PhysicalPosition::new(
                                (x as f64) * scale_factor,
                                (y as f64) * scale_factor,
                            ));
                            handle_cursor_moved(
                                x, y, 0,
                                &mut runtime, &pipeline,
                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                window, elwt,
                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                &mut frame_trace,
                                &mut invalidations,
                            );
                        }
                        TestEvent::MouseDown { x, y, button } => {
                            let Some(window) = current_window(&window) else {
                                return;
                            };
                            let btn = map_test_button(button);
                            handle_mouse_button(
                                x, y, btn, true, 0,
                                &mut runtime, &pipeline,
                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                window, elwt,
                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                text_trace_enabled, &mut pending_text_traces,
                                &mut next_text_trace_seq, presented_frames,
                                &mut last_blink_toggle,
                                &mut frame_trace,
                                &mut invalidations,
                            );
                        }
                        TestEvent::MouseUp { x, y, button } => {
                            let Some(window) = current_window(&window) else {
                                return;
                            };
                            let btn = map_test_button(button);
                            handle_mouse_button(
                                x, y, btn, false, 0,
                                &mut runtime, &pipeline,
                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                window, elwt,
                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                text_trace_enabled, &mut pending_text_traces,
                                &mut next_text_trace_seq, presented_frames,
                                &mut last_blink_toggle,
                                &mut frame_trace,
                                &mut invalidations,
                            );
                        }
                        TestEvent::KeyDown { key_code, modifiers } => {
                            let Some(window) = current_window(&window) else {
                                return;
                            };
                            let code = parse_key_code(&key_code);
                            handle_key_down::<S>(
                                code, modifiers,
                                &mut runtime, &pipeline,
                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                window, elwt,
                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                text_trace_enabled, &mut pending_text_traces,
                                &mut next_text_trace_seq, presented_frames,
                                &mut last_blink_toggle,
                                self.key_handler.as_ref(),
                                &mut frame_trace,
                                &mut invalidations,
                            );
                        }
                        TestEvent::KeyUp { .. } => {
                            let Some(window) = current_window(&window) else {
                                return;
                            };
                            request_redraw_logged(
                                window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "test_key_up",
                            );
                        }
                        TestEvent::TextInput { text } => {
                            let Some(window) = current_window(&window) else {
                                return;
                            };
                            if let (Some(ir), Some(layout)) =
                                (&pipeline.prev_ir, &pipeline.last_snapshot)
                            {
                                let target =
                                    focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                                if target.is_some()
                                    || focused_custom_text_input(
                                        &runtime,
                                        pipeline.prev_ir.as_ref(),
                                    )
                                {
                                    let trace_seq = start_text_trace(
                                        text_trace_enabled && target.is_some(),
                                        &mut pending_text_traces,
                                        &mut next_text_trace_seq,
                                        format!("test_text_input:{}", text.chars().count()),
                                        target,
                                        presented_frames,
                                    );
                                    runtime
                                        .handle_input(
                                            InputEvent::Ime(
                                                fission_core::event::ImeEvent::Commit {
                                                    text: text.clone(),
                                                },
                                            ),
                                            ir,
                                            layout,
                                        )
                                        .ok();
                                    invalidations.mark_build();
                                    mark_text_trace_handled(
                                        &mut pending_text_traces,
                                        trace_seq,
                                    );
                                    if process_pending_effects(
                                        &mut runtime,
                                        &effect_result_tx,
                                        &event_proxy,

                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                    ) {
                                        mark_text_trace_effects(
                                            &mut pending_text_traces,
                                            trace_seq,
                                        );
                                        invalidations.mark_build();
                                    }
                                    request_redraw_logged(
                                        window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        "test_text_input",
                                    );
                                } else {
                                    for ch in text.chars() {
                                        let key = if ch == ' ' {
                                            KeyCode::Space
                                        } else if ch == '\n' {
                                            KeyCode::Enter
                                        } else {
                                            KeyCode::Char(ch)
                                        };
                                        handle_key_down::<S>(
                                            key,
                                            0,
                                            &mut runtime,
                                            &pipeline,
                                            &effect_result_tx,
                                            &event_proxy,

                                            &async_registry,
                                            &mut active_services,
                                            &mut service_bindings,
                                            &mut next_service_instance_id,
                                            window,
                                            elwt,
                                            &mut last_redraw_at,
                                            min_frame,
                                            &mut redraw_pending,
                                            text_trace_enabled,
                                            &mut pending_text_traces,
                                            &mut next_text_trace_seq,
                                            presented_frames,
                                            &mut last_blink_toggle,
                                            self.key_handler.as_ref(),
                                            &mut frame_trace,
                                            &mut invalidations,
                                        );
                                    }
                                }
                            }
                        }
                        TestEvent::Scroll { x, y, dx, dy } => {
                            let Some(window) = current_window(&window) else {
                                return;
                            };
                            handle_scroll(
                                x, y, dx, dy, 0,
                                &mut runtime, &pipeline,
                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                window, elwt,
                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                &mut frame_trace,
                                &mut invalidations,
                            );
                        }
                        TestEvent::Resize { width, height } => {
                            let Some(window) = current_window(&window) else {
                                return;
                            };
                            if width > 0 && height > 0 {
                                let requested_logical_size =
                                    LayoutSize::new(width as f32, height as f32);
                                let current_viewport = pending_resize
                                    .unwrap_or_else(|| WindowViewportState::from_window(window))
                                    .with_logical_size(requested_logical_size);
                                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                                {
                                    let _ = window.request_inner_size(
                                        native_window_size_for_logical_viewport(
                                            requested_logical_size,
                                        ),
                                    );
                                }
                                #[cfg(not(target_os = "android"))]
                                {
                                    window_viewport = current_viewport;
                                }
                                #[cfg(target_os = "android")]
                                {
                                    window_viewport = Some(current_viewport);
                                }
                                apply_authoritative_resize(
                                    window,
                                    elwt,
                                    current_viewport,
                                    &mut pending_resize,
                                    &mut resize_needs_settled_frame,
                                    &mut pending_capture_settle,
                                    pending_screenshot_path.as_deref(),
                                    &mut live_resize,
                                    &mut invalidations,
                                    &mut last_redraw_at,
                                    resize_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "test_resize",
                                );
                            }
                        }
                        TestEvent::TapText { text } => {
                            let Some(window) = current_window(&window) else {
                                if let Some(ref tx) = test_response_tx {
                                    let _ = tx.send(fission_test_driver::TestResponse::Error {
                                        message: "window not ready".into(),
                                    });
                                }
                                return;
                            };
                            let resp = handle_tap_text(&text, &mut runtime, &pipeline);
                            if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                                invalidations.mark_build();
                                if process_pending_effects(
                                    &mut runtime,
                                    &effect_result_tx,
                                    &event_proxy,

                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                ) {
                                    invalidations.mark_build();
                                }
                            }
                            if let Some(ref tx) = test_response_tx {
                                let _ = tx.send(resp);
                            }
                            request_redraw_logged(
                                window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "test_tap_text",
                            );
                        }
                        TestEvent::Screenshot { path } => {
                            let Some(window) = current_window(&window) else {
                                if let Some(ref tx) = test_response_tx {
                                    let _ = tx.send(fission_test_driver::TestResponse::Error {
                                        message: "window not ready".into(),
                                    });
                                }
                                return;
                            };
                            pending_screenshot_path = Some(path);
                            pending_capture_settle = resize_is_unsettled(
                                pending_resize.is_some(),
                                resize_needs_settled_frame,
                                live_resize.is_live(Instant::now()),
                            );
                            window.request_redraw();
                        }
                        TestEvent::CaptureScreenshot => {
                            let Some(window) = current_window(&window) else {
                                if let Some(ref tx) = test_response_tx {
                                    let _ = tx.send(fission_test_driver::TestResponse::Error {
                                        message: "window not ready".into(),
                                    });
                                }
                                return;
                            };
                            pending_screenshot_path = Some("__capture__".into());
                            pending_capture_settle = resize_is_unsettled(
                                pending_resize.is_some(),
                                resize_needs_settled_frame,
                                live_resize.is_live(Instant::now()),
                            );
                            window.request_redraw();
                        }
                        TestEvent::GetText => {
                            let resp = build_get_text_response(&pipeline);
                            if let Some(ref tx) = test_response_tx {
                                let _ = tx.send(resp);
                            }
                        }
                        TestEvent::GetTree => {
                            let resp = build_get_tree_response(&pipeline);
                            if let Some(ref tx) = test_response_tx {
                                let _ = tx.send(resp);
                            }
                        }
                        TestEvent::Pump => {
                            let Some(window) = current_window(&window) else {
                                if let Some(ref tx) = test_response_tx {
                                    let _ = tx.send(fission_test_driver::TestResponse::Error {
                                        message: "window not ready".into(),
                                    });
                                }
                                return;
                            };
                            pending_screenshot_path = Some("__pump__".into());
                            pending_capture_settle = resize_is_unsettled(
                                pending_resize.is_some(),
                                resize_needs_settled_frame,
                                live_resize.is_live(Instant::now()),
                            );
                            window.request_redraw();
                        }
                        TestEvent::Wake => {}
                        TestEvent::Wait { ms: _ } => {
                            if let Some(ref tx) = test_response_tx {
                                let _ = tx.send(fission_test_driver::TestResponse::Ok {});
                            }
                        }
                        TestEvent::Quit => {
                            elwt.exit();
                        }
                    }
                };

                #[cfg(target_os = "android")]
                let mut drain_pending_test_events = || {
                    loop {
                        let pending = {
                            let mut pending = pending_test_events
                                .lock()
                                .expect("pending test events lock poisoned");
                            pending.pop_front()
                        };
                        let Some(test_event) = pending else {
                            break;
                        };
                        if debug_android_events {
                            eprintln!("[android-debug] draining_test_queue");
                        }
                        handle_test_event(test_event);
                    }
                };

                match event {
                    Event::Resumed => {
                        if debug_android_events {
                            eprintln!("[android-events] resumed");
                        }
                        #[cfg(target_os = "android")]
                        if window.is_none() {
                            match build_window(&window_title, background_test_mode, elwt) {
                                Ok(new_window) => {
                                    ime_handler.set_window(Some(new_window.clone()));
                                    sync_window_cursor(&new_window, &runtime);
                                    window = Some(new_window);
                                }
                                Err(err) => {
                                    eprintln!("window build error: {err}");
                                    elwt.exit();
                                    return;
                                }
                            }
                        }
                        let Some(window) = current_window(&window) else {
                            return;
                        };
                        let current_viewport = WindowViewportState::from_window(window);
                        #[cfg(not(target_os = "android"))]
                        {
                            window_viewport = current_viewport;
                        }
                        #[cfg(target_os = "android")]
                        {
                            window_viewport = Some(current_viewport);
                        }
                        pending_resize = Some(current_viewport);
                        resize_needs_settled_frame = true;
                        if pending_screenshot_path.is_some() {
                            pending_capture_settle = true;
                        }
                        invalidations.mark_composite();
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "app_resumed",
                        );
                    }
                    Event::Suspended => {
                        render_state = None;
                        #[cfg(target_os = "android")]
                        {
                            ime_handler.set_window(None);
                            window = None;
                            window_viewport = None;
                            pending_resize = None;
                            resize_needs_settled_frame = false;
                            pending_capture_settle = false;
                            last_built_viewport = None;
                            last_cursor_position = None;
                            active_primary_touch = None;
                            touch_positions.clear();
                        }
                    }
                    // ═══════════════════════════════════════════════════════
                    // UserEvent — injected by test control server via proxy
                    // ═══════════════════════════════════════════════════════
                    Event::UserEvent(test_event) => {
                        #[cfg(target_os = "android")]
                        if matches!(test_event, TestEvent::Wake) {
                            if debug_android_events {
                                eprintln!("[android-debug] wake_received");
                            }
                            drain_pending_test_events();
                            return;
                        }
                        handle_test_event(test_event)
                    }

                    // ═══════════════════════════════════════════════════════
                    // AboutToWait — idle / animation / blink / effects
                    // ═══════════════════════════════════════════════════════
                    Event::AboutToWait => {
                        let Some(window) = current_window(&window) else {
                            elwt.set_control_flow(ControlFlow::Wait);
                            return;
                        };
                        #[cfg(target_os = "android")]
                        drain_pending_test_events();
                        let now = Instant::now();

                        // Video Logic
                        let mut surfaces = pipeline.video_surfaces.clone();
                        let mut active_nodes = std::collections::HashSet::new();

                        for surface in &mut surfaces {
                            active_nodes.insert(surface.widget_id);

                            // Create player if missing
                            if !players.contains_key(&surface.widget_id) {
                                if let Some(state) = runtime.runtime_state.video.states.get(&surface.widget_id) {
                                    let source = &state.asset_source;
                                    if !source.is_empty() {
                                        let player = video_backend.create_player(source);
                                        surface.surface_id = player.surface_id();
                                        if let Some(state) = runtime.runtime_state.video.states.get_mut(&surface.widget_id) {
                                            state.surface_id = Some(surface.surface_id);
                                        }
                                        players.insert(surface.widget_id, ActivePlayer {
                                            player,
                                            last_status: None,
                                            last_rate: None,
                                            last_volume: None,
                                            last_muted: None,
                                        });
                                    }
                                }
                            } else if let Some(active_player) = players.get(&surface.widget_id) {
                                surface.surface_id = active_player.player.surface_id();
                            }
                        }

                        // Cleanup inactive players
                        players.retain(|id, _| active_nodes.contains(id));

                        // Update backend
                        video_backend.present_surfaces(&surfaces);
                        let web_surfaces = pipeline.web_surfaces.clone();
                        web_backend.present_surfaces(&web_surfaces);

                        // Video Logic - Process Player Events and Sync State
                        for (widget_id, active_player) in players.iter_mut() {
                            if let Some(video_state) = runtime.runtime_state.video.states.get_mut(widget_id) {
                                let player = &mut active_player.player;

                                // Sync player controls from runtime state
                                if active_player.last_status != Some(video_state.status) {
                                    match video_state.status {
                                        VideoStatus::Playing => player.play(),
                                        VideoStatus::Paused => player.pause(),
                                        VideoStatus::Stopped => player.stop(),
                                        _ => {}
                                    }
                                    active_player.last_status = Some(video_state.status);
                                }

                                // Update runtime state from player events
                                for event in player.poll_events() {
                                    match event {
                                        VideoEvent::Ready { duration } => {
                                            video_state.duration_ms = Some(duration);
                                            if video_state.status == VideoStatus::Playing {
                                                player.play();
                                            }
                                        },
                                        VideoEvent::Ended => {
                                            video_state.status = VideoStatus::Ended;
                                            active_player.last_status = Some(VideoStatus::Ended);
                                            request_redraw_logged(
                                                &window,
                                                elwt,
                                                &mut last_redraw_at,
                                                min_frame,
                                                &mut redraw_pending,
                                                &mut frame_trace,
                                                "video_ended",
                                            );
                                        },
                                        VideoEvent::Error(e) => {
                                            eprintln!("Video playback error for {:?}: {:?}", widget_id, e);
                                            video_state.status = VideoStatus::Error;
                                            active_player.last_status = Some(VideoStatus::Error);
                                            request_redraw_logged(
                                                &window,
                                                elwt,
                                                &mut last_redraw_at,
                                                min_frame,
                                                &mut redraw_pending,
                                                &mut frame_trace,
                                                "video_error",
                                            );
                                        },
                                    }
                                }
                                // Sync other properties
                                video_state.position_ms = player.position();

                                if active_player.last_rate != Some(video_state.rate) {
                                    player.set_rate(video_state.rate);
                                    active_player.last_rate = Some(video_state.rate);
                                }
                                if active_player.last_volume != Some(video_state.volume) {
                                    player.set_volume(video_state.volume);
                                    active_player.last_volume = Some(video_state.volume);
                                }
                                if active_player.last_muted != Some(video_state.muted) {
                                    player.set_muted(video_state.muted);
                                    active_player.last_muted = Some(video_state.muted);
                                }

                                if let Some(seek_pos) = video_state.pending_seek.take() {
                                    player.seek_to(seek_pos);
                                }
                            }
                        }

                        let has_finite_animation = runtime
                            .runtime_state
                            .animation
                            .active
                            .values()
                            .any(|anim| !anim.repeat);
                        let resize_unsettled = resize_is_unsettled(
                            pending_resize.is_some(),
                            resize_needs_settled_frame,
                            live_resize.is_live(now),
                        );
                        let repeat_animation_interval = if resize_unsettled || pending_capture_settle {
                            None
                        } else {
                            repeating_animation_redraw_interval(
                                &runtime.runtime_state.animation,
                                repeat_animation_frame,
                            )
                        };
                        let has_playing_video = players.iter().any(|(widget_id, _)| {
                            runtime
                                .runtime_state
                                .video
                                .states
                                .get(widget_id)
                                .map(|state| state.status == VideoStatus::Playing)
                                .unwrap_or(false)
                        });
                        let animation_frame = animation_redraw_interval(
                            has_finite_animation,
                            repeat_animation_interval,
                            has_playing_video,
                            min_frame,
                        );

                        ime_handler.set_text_input_config(focused_text_input_config(
                            &runtime,
                            pipeline.prev_ir.as_ref(),
                        ));
                        let focused_text_input =
                            focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                        if focused_text_input != blink_focus_id {
                            if let Some(prev) = blink_focus_id {
                                runtime.runtime_state.caret_visible.remove(&prev);
                            }
                            blink_focus_id = focused_text_input;
                            if let Some(id) = blink_focus_id {
                                runtime.runtime_state.caret_visible.insert(id, true);
                                last_blink_toggle = now;
                                invalidations.mark_build();
                                request_redraw_logged(
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "caret_focus_changed",
                                );
                            }
                        }

                        // Cursor blink: toggle visibility and request a redraw.
                        if blink_enabled {
                            if let Some(id) = blink_focus_id {
                                if now.duration_since(last_blink_toggle) >= blink_period {
                                    let visible = runtime.runtime_state.caret_visible.get(&id).copied().unwrap_or(true);
                                    runtime.runtime_state.caret_visible.insert(id, !visible);
                                    last_blink_toggle = now;
                                    invalidations.mark_build();
                                    request_redraw_logged(
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        "caret_blink",
                                    );
                                }
                            }
                        }

                        let blink_wake_at = if blink_enabled && blink_focus_id.is_some() {
                            Some(last_blink_toggle + blink_period)
                        } else {
                            None
                        };

                        // Drain completed background effect results and dispatch
                        // their continuations back into the runtime on the main thread.
                        let effect_results_dispatched = drain_effect_results(
                            &mut runtime,
                            &effect_result_rx,
                            &mut active_services,
                            &mut service_bindings,
                        );
                        if effect_results_dispatched {
                            invalidations.mark_build();
                            // Background work completed — process any new effects
                            // the continuation reducers may have emitted.
                            if process_pending_effects(
                                &mut runtime,
                                &effect_result_tx,
                                &event_proxy,

                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                            ) {
                                invalidations.mark_build();
                                request_redraw_logged(
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "effect_continuation",
                                );
                            }
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "effect_result",
                            );
                        }

                        // Application frame hook (e.g. LSP polling).
                        let frame_hook_wants_redraw = if let Some(ref hook) = self.frame_hook {
                            let hook = hook.clone();
                            if let Some(state) = runtime.get_app_state_mut::<S>() {
                                hook(state)
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if frame_hook_wants_redraw {
                            invalidations.mark_build();
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "frame_hook",
                            );
                        }

                        // When a frame_hook is registered, ensure the event loop
                        // wakes at least every 2 seconds so the hook fires even
                        // when no user input or animation is happening (e.g. for
                        // asynchronous LSP diagnostics).
                        let frame_hook_wake_at = if self.frame_hook.is_some() {
                            Some(now + Duration::from_secs(2))
                        } else {
                            None
                        };

                        let has_pending_work =
                            effect_results_dispatched
                                || frame_hook_wants_redraw
                                || invalidations.any()
                                || resize_unsettled
                                || pending_capture_settle;
                        let active_keys = active_animation_keys(&runtime);

                        if has_pending_work {
                            let pending_frame = pending_work_redraw_interval(
                                invalidations,
                                resize_unsettled || pending_capture_settle,
                                min_frame,
                                resize_frame,
                            );
                            let redraw_reason = if resize_unsettled {
                                "pending_resize"
                            } else if pending_capture_settle {
                                "pending_capture_settle"
                            } else if invalidations.build {
                                "pending_work:build"
                            } else if invalidations.layout {
                                "pending_work:layout"
                            } else if invalidations.paint {
                                "pending_work:paint"
                            } else if invalidations.composite {
                                "pending_work:composite"
                            } else if effect_results_dispatched {
                                "pending_work:effects"
                            } else if frame_hook_wants_redraw {
                                "pending_work:frame_hook"
                            } else {
                                "pending_work"
                            };
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                pending_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                redraw_reason,
                            );
                            let reasons = frame_trace.take_redraw_reasons();
                            frame_trace.emit(
                                "about_to_wait",
                                presented_frames + 1,
                                &active_keys,
                                invalidations,
                                &reasons,
                                &format!(
                                    "schedule=pending interval_ms={} pending_resize={} redraw_pending={} highest={}",
                                    pending_frame.as_millis(),
                                    resize_unsettled || pending_capture_settle,
                                    redraw_pending,
                                    invalidations.highest_class(),
                                ),
                            );
                            let mut wake_at = last_redraw_at + pending_frame;
                            if let Some(blink_at) = blink_wake_at {
                                if blink_at < wake_at {
                                    wake_at = blink_at;
                                }
                            }
                            if let Some(hook_at) = frame_hook_wake_at {
                                if hook_at < wake_at {
                                    wake_at = hook_at;
                                }
                            }
                            elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
                        } else if let Some(animation_frame) = animation_frame {
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                animation_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                if has_finite_animation {
                                    "animation:finite"
                                } else if has_playing_video {
                                    "animation:video"
                                } else {
                                    "animation:repeat"
                                },
                            );
                            let reasons = frame_trace.take_redraw_reasons();
                            frame_trace.emit(
                                "about_to_wait",
                                presented_frames + 1,
                                &active_keys,
                                invalidations,
                                &reasons,
                                &format!(
                                    "schedule=animation interval_ms={} pending_resize={} redraw_pending={} highest={}",
                                    animation_frame.as_millis(),
                                    resize_unsettled || pending_capture_settle,
                                    redraw_pending,
                                    invalidations.highest_class(),
                                ),
                            );
                            let mut wake_at = last_redraw_at + animation_frame;
                            if let Some(blink_at) = blink_wake_at {
                                if blink_at < wake_at {
                                    wake_at = blink_at;
                                }
                            }
                            if let Some(hook_at) = frame_hook_wake_at {
                                if hook_at < wake_at {
                                    wake_at = hook_at;
                                }
                            }
                            elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
                        } else if let Some(blink_at) = blink_wake_at {
                            let reasons = frame_trace.take_redraw_reasons();
                            frame_trace.emit(
                                "about_to_wait",
                                presented_frames + 1,
                                &active_keys,
                                invalidations,
                                &reasons,
                                "schedule=blink_wait pending_resize=false redraw_pending=false highest=none",
                            );
                            let mut wake_at = blink_at;
                            if let Some(hook_at) = frame_hook_wake_at {
                                if hook_at < wake_at {
                                    wake_at = hook_at;
                                }
                            }
                            elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
                        } else if let Some(hook_at) = frame_hook_wake_at {
                            let reasons = frame_trace.take_redraw_reasons();
                            frame_trace.emit(
                                "about_to_wait",
                                presented_frames + 1,
                                &active_keys,
                                invalidations,
                                &reasons,
                                "schedule=hook_wait pending_resize=false redraw_pending=false highest=none",
                            );
                            elwt.set_control_flow(ControlFlow::WaitUntil(hook_at));
                        } else {
                            let reasons = frame_trace.take_redraw_reasons();
                            frame_trace.emit(
                                "about_to_wait",
                                presented_frames + 1,
                                &active_keys,
                                invalidations,
                                &reasons,
                                "schedule=idle pending_resize=false redraw_pending=false highest=none",
                            );
                            #[cfg(target_os = "android")]
                            if test_response_tx.is_some() {
                                elwt.set_control_flow(ControlFlow::Poll);
                            } else {
                                elwt.set_control_flow(ControlFlow::WaitUntil(
                                    now + Duration::from_millis(16),
                                ));
                            }
                            #[cfg(not(target_os = "android"))]
                            elwt.set_control_flow(ControlFlow::Wait);
                        }
                    }

                    // ═══════════════════════════════════════════════════════
                    // WindowEvent — real user interaction
                    // ═══════════════════════════════════════════════════════
                    Event::WindowEvent { window_id, event }
                        if current_window_id(&window) == Some(window_id) =>
                    {
                        let Some(window) = current_window(&window) else {
                            return;
                        };
                        match event {
                            WindowEvent::Resized(size) => {
                                if size.width > 0 && size.height > 0 {
                                    let next_viewport = pending_resize
                                        .unwrap_or_else(|| WindowViewportState::from_window(&window))
                                        .with_physical_size(size);
                                    #[cfg(not(target_os = "android"))]
                                    {
                                        window_viewport = next_viewport;
                                    }
                                    #[cfg(target_os = "android")]
                                    {
                                        window_viewport = Some(next_viewport);
                                    }
                                    apply_authoritative_resize(
                                        &window,
                                        elwt,
                                        next_viewport,
                                        &mut pending_resize,
                                        &mut resize_needs_settled_frame,
                                        &mut pending_capture_settle,
                                        pending_screenshot_path.as_deref(),
                                        &mut live_resize,
                                        &mut invalidations,
                                        &mut last_redraw_at,
                                        resize_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        "window_resized",
                                    );
                                }
                            }
                            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                                let next_viewport = pending_resize
                                    .unwrap_or_else(|| WindowViewportState::from_window(&window))
                                    .with_scale_factor(scale_factor);
                                #[cfg(not(target_os = "android"))]
                                {
                                    window_viewport = next_viewport;
                                }
                                #[cfg(target_os = "android")]
                                {
                                    window_viewport = Some(next_viewport);
                                }
                                apply_authoritative_resize(
                                    &window,
                                    elwt,
                                    next_viewport,
                                    &mut pending_resize,
                                    &mut resize_needs_settled_frame,
                                    &mut pending_capture_settle,
                                    pending_screenshot_path.as_deref(),
                                    &mut live_resize,
                                    &mut invalidations,
                                    &mut last_redraw_at,
                                    resize_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "scale_factor_changed",
                                );
                            }
                            WindowEvent::RedrawRequested => {
                                if debug_android_events {
                                    eprintln!("[android-events] redraw_requested");
                                }
                                redraw_pending = false;
                                diag::begin_frame(None);
                                let now = Instant::now();
                                let dt = now.duration_since(last_frame_time);
                                last_frame_time = now;
                                let dt_ms = dt.as_millis() as u64;
                                let pre_tick_active = active_animation_keys(&runtime);
                                match runtime.tick(dt_ms) {
                                    Ok(tick_result) => {
                                        let tick_invalidations =
                                            pipeline.classify_animation_updates(
                                                &tick_result.changed_animations,
                                            );
                                        invalidations.merge(tick_invalidations);
                                        let reasons = if tick_result.changed_animations.is_empty() {
                                            Vec::new()
                                        } else {
                                            tick_result
                                                .changed_animations
                                                .iter()
                                                .map(|(target, property)| {
                                                    format!(
                                                        "tick:{}:{:?}:{}",
                                                        target.as_u128(),
                                                        property,
                                                        tick_invalidations.highest_class()
                                                    )
                                                })
                                                .collect::<Vec<_>>()
                                        };
                                        frame_trace.emit(
                                            "redraw_requested",
                                            presented_frames + 1,
                                            &pre_tick_active,
                                            tick_invalidations,
                                            &reasons,
                                            &format!("dt_ms={}", dt_ms),
                                        );
                                    }
                                    Err(e) => {
                                        eprintln!("Runtime tick error: {:?}", e);
                                    }
                                }
                                if process_pending_effects(
                                    &mut runtime,
                                    &effect_result_tx,
                                    &event_proxy,

                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                ) {
                                    invalidations.mark_build();
                                    request_redraw_logged(
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        "redraw:effects",
                                    );
                                }
                                let viewport_state = pending_resize.unwrap_or_else(|| {
                                    #[cfg(not(target_os = "android"))]
                                    {
                                        window_viewport
                                    }
                                    #[cfg(target_os = "android")]
                                    {
                                        window_viewport
                                            .unwrap_or_else(|| WindowViewportState::from_window(&window))
                                    }
                                });
                                #[cfg(not(target_os = "android"))]
                                {
                                    window_viewport = viewport_state;
                                }
                                #[cfg(target_os = "android")]
                                {
                                    window_viewport = Some(viewport_state);
                                }
                                let swapchain_size = viewport_state.physical_size;
                                if swapchain_size.width == 0 || swapchain_size.height == 0 {
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }

                                if render_state.is_none() {
                                    match create_render_state(
                                        &mut render_cx,
                                        window.clone(),
                                        viewport_state,
                                    ) {
                                        Ok(state) => {
                                            render_state = Some(state);
                                        }
                                        Err(err) => {
                                            eprintln!("render surface not ready yet: {err}");
                                            request_redraw_logged(
                                                &window,
                                                elwt,
                                                &mut last_redraw_at,
                                                min_frame,
                                                &mut redraw_pending,
                                                &mut frame_trace,
                                                "render_surface_pending",
                                            );
                                            diag::end_frame(diag::FrameStats::default());
                                            return;
                                        }
                                    }
                                }
                                let render_state = render_state.as_mut().expect("render state");

                                let mut surface_target_replaced = false;
                                if swapchain_size.width != render_state.surface.config.width
                                    || swapchain_size.height != render_state.surface.config.height
                                {
                                    render_cx.resize_surface(
                                        &mut render_state.surface,
                                        swapchain_size.width,
                                        swapchain_size.height,
                                    );
                                    let device_handle =
                                        &render_cx.devices[render_state.surface.dev_id];
                                    render_state.surface.config.alpha_mode =
                                        wgpu::CompositeAlphaMode::PostMultiplied;
                                    render_state
                                        .surface
                                        .surface
                                        .configure(&device_handle.device, &render_state.surface.config);
                                    sync_tracked_target_texture_size_to_surface(
                                        &mut render_state.target_texture_size,
                                        swapchain_size,
                                    );
                                    surface_target_replaced = true;
                                }
                                let device_handle = &render_cx.devices[render_state.surface.dev_id];

                                let scale_factor = viewport_state.scale_factor;
                                let pending_layout_viewport = viewport_state.logical_size();
                                let render_target_size = (swapchain_size.width, swapchain_size.height);
                                if surface_target_replaced
                                    || render_target_size != render_state.target_texture_size
                                {
                                    recreate_target_texture(
                                        &mut render_state.surface,
                                        &render_cx,
                                        render_target_size.0,
                                        render_target_size.1,
                                    );
                                    // Keep the 3D depth target in lockstep with the shared render target.
                                    render_state.scene3d_renderer.resize(
                                        &device_handle.device,
                                        render_target_size.0,
                                        render_target_size.1,
                                    );
                                    render_state.target_texture_size = render_target_size;
                                }

                                let resize_settled =
                                    resize_needs_settled_frame && !live_resize.is_live(now);
                                let target_viewport = pending_layout_viewport;
                                let build_viewport = resolve_build_viewport(
                                    last_built_viewport,
                                    target_viewport,
                                    pipeline.prev_ir.is_some(),
                                    &mut invalidations,
                                );
                                env.viewport_size = build_viewport;

                                if let Some(sync) = &self.sync_env {
                                    let state = runtime.get_app_state::<S>().unwrap();
                                    sync(state, &mut env);
                                }

                                if invalidations.build || pipeline.prev_ir.is_none() {
                                    let (node_tree, registry, resources, anims, videos, web_views, portals) = {
                                        let state = runtime.get_app_state::<S>().unwrap();
                                        let view = View::new(
                                            state,
                                            &runtime.runtime_state,
                                            &env,
                                            pipeline.last_snapshot.as_ref(),
                                        );
                                        let mut ctx = BuildCtx::new();
                                        let node = root_widget.build(&mut ctx, &view);
                                        let resources = ctx.take_resources();
                                        let anims = ctx.take_animation_requests();
                                        let videos = ctx.take_video_registrations();
                                        let web_views = ctx.take_web_registrations();
                                        let portals_with_ids = ctx.take_portals();

                                        let portals = portals_with_ids
                                            .into_iter()
                                            .map(|(id, node)| wrap_portal_for_viewport(id, node, &env))
                                            .collect::<Vec<_>>();

                                        diag::emit(
                                            diag::DiagCategory::Layout,
                                            diag::DiagLevel::Debug,
                                            diag::DiagEventKind::PortalsComposed {
                                                portal_count: portals.len() as u32,
                                            },
                                        );
                                        (node, ctx.registry, resources, anims, videos, web_views, portals)
                                    };

                                    runtime.clear_reducers();
                                    runtime.absorb_registry(registry);
                                    if let Err(err) = runtime.reconcile_resources(resources) {
                                        eprintln!("Runtime resource reconciliation error: {:?}", err);
                                    }
                                    if !startup_dispatched {
                                        if let Some(action) = startup_action.clone() {
                                            if let Err(err) =
                                                runtime.dispatch(action, NodeId::derived(0, &[0]))
                                            {
                                                eprintln!("Startup action error: {:?}", err);
                                            }
                                        }
                                        startup_dispatched = true;
                                    }
                                    runtime.sync_animation_requests(&anims);
                                    for (target, req) in anims {
                                        runtime.enqueue_animation(target, req);
                                    }
                                    runtime.sync_video_nodes(&videos);
                                    runtime.sync_web_nodes(&web_views);

                                    let final_root = fission_core::Node::Overlay(
                                        fission_core::ui::Overlay {
                                            id: None,
                                            content: Box::new(node_tree),
                                            overlay: Box::new(fission_core::Node::ZStack(
                                                fission_core::ui::ZStack {
                                                    children: portals,
                                                    ..Default::default()
                                                },
                                            )),
                                        },
                                    );

                                    let mut lower_cx = LoweringContext::new(
                                        &env,
                                        &runtime.runtime_state,
                                        runtime.measurer.as_ref(),
                                        pipeline.last_snapshot.as_ref(),
                                    );
                                    let root_id = final_root.lower(&mut lower_cx);
                                    lower_cx.ir.root = Some(root_id);

                                    let pipeline_invalidations =
                                        pipeline.replace_ir(lower_cx.ir, &env);
                                    invalidations.merge(pipeline_invalidations);
                                    last_built_viewport = Some(build_viewport);
                                }

                                let layout_updates = match pipeline.ensure_layout(
                                    LayoutRect::new(
                                        0.0,
                                        0.0,
                                        target_viewport.width,
                                        target_viewport.height,
                                    ),
                                    &mut layout_engine,
                                    &runtime.runtime_state.scroll,
                                ) {
                                    Ok(updates) => updates,
                                    Err(e) => {
                                        eprintln!("Layout error: {:?}", e);
                                        diag::end_frame(diag::FrameStats::default());
                                        return;
                                    }
                                };

                                if layout_updates > 0 {
                                    if let (Some(ir), Some(layout)) =
                                        (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref())
                                    {
                                        runtime.post_layout_hook(ir, layout);
                                    }
                                }

                                match pipeline.prepare_current(
                                    target_viewport,
                                    target_viewport,
                                    false,
                                    &runtime.runtime_state.scroll,
                                    &runtime.runtime_state.animation,
                                    &runtime.runtime_state.video,
                                    &runtime.runtime_state.web,
                                ) {
                                    Ok(_stats) => {
                                        let surface_texture = render_state
                                            .surface
                                            .surface
                                            .get_current_texture()
                                            .expect("failed to get texture");
                                        let device_handle =
                                            &render_cx.devices[render_state.surface.dev_id];

                                        let clear_color = vello::wgpu::Color {
                                            r: env.theme.tokens.colors.background.r as f64 / 255.0,
                                            g: env.theme.tokens.colors.background.g as f64 / 255.0,
                                            b: env.theme.tokens.colors.background.b as f64 / 255.0,
                                            a: env.theme.tokens.colors.background.a as f64 / 255.0,
                                        };
                                        match &mut render_state.main_renderer {
                                            MainRenderer::Vello {
                                                renderer,
                                                texture_compositor,
                                            } => {
                                                let texture_plans =
                                                    pipeline.texture_compositor_plans();
                                                let texture_plans_fit_limits =
                                                    texture_plans_fit_device_limits(
                                                        texture_plans,
                                                        scale_factor,
                                                        device_handle
                                                            .device
                                                            .limits()
                                                            .max_texture_dimension_2d,
                                                    );
                                                let has_active_scroll_offsets = runtime
                                                    .runtime_state
                                                    .scroll
                                                    .offsets
                                                    .values()
                                                    .any(|offset| offset.abs() > 0.5);
                                                let enable_texture_compositor =
                                                    std::env::var(
                                                        "FISSION_ENABLE_TEXTURE_COMPOSITOR",
                                                    )
                                                    .ok()
                                                    .as_deref()
                                                        == Some("1");
                                                if !enable_texture_compositor
                                                    || texture_plans.is_empty()
                                                    || !texture_plans_fit_limits
                                                    || has_active_scroll_offsets
                                                {
                                                    let render_params = vello::RenderParams {
                                                        base_color:
                                                            vello::peniko::Color::from_rgba8(
                                                                env.theme.tokens.colors
                                                                    .background
                                                                    .r,
                                                                env.theme.tokens.colors
                                                                    .background
                                                                    .g,
                                                                env.theme.tokens.colors
                                                                    .background
                                                                    .b,
                                                                env.theme.tokens.colors
                                                                    .background
                                                                    .a,
                                                            ),
                                                        width: render_target_size.0,
                                                        height: render_target_size.1,
                                                        antialiasing_method:
                                                            vello::AaConfig::Area,
                                                    };

                                                    scene.reset();
                                                    let retained_scene = pipeline
                                                        .retained_scene()
                                                        .expect(
                                                            "retained render scene missing before render",
                                                        );
                                                    let mut renderer_wrapper =
                                                        VelloRenderer::new(
                                                            &mut scene,
                                                            measurer.clone(),
                                                            &mut retained_scene_cache,
                                                            scale_factor,
                                                        );
                                                    renderer_wrapper
                                                        .render_scene(retained_scene)
                                                        .expect(
                                                            "failed to encode retained scene",
                                                        );
                                                    renderer
                                                        .render_to_texture(
                                                            &device_handle.device,
                                                            &device_handle.queue,
                                                            &scene,
                                                            &render_state.surface.target_view,
                                                            &render_params,
                                                        )
                                                        .expect("failed to render");
                                                } else {
                                                    let force_full_compositor_redraw =
                                                        invalidations.build
                                                            || invalidations.layout
                                                            || invalidations.paint;
                                                    let _compositor_stats = texture_compositor
                                                        .render_layers(
                                                            &device_handle.device,
                                                            &device_handle.queue,
                                                            renderer,
                                                            &mut retained_scene_cache,
                                                            measurer.clone(),
                                                            scale_factor,
                                                            render_target_size.0,
                                                            render_target_size.1,
                                                            pipeline
                                                                .texture_compositor_root_transform(),
                                                            texture_plans,
                                                            force_full_compositor_redraw,
                                                            clear_color,
                                                            &render_state.surface.target_view,
                                                        )
                                                        .expect(
                                                            "failed to composite texture layers",
                                                        );
                                                }
                                            }
                                            MainRenderer::Software => {
                                                let retained_scene = pipeline
                                                    .retained_scene()
                                                    .expect(
                                                        "retained render scene missing before render",
                                                    );
                                                let rgba = SoftwareRenderer::render(
                                                    retained_scene,
                                                    render_target_size.0,
                                                    render_target_size.1,
                                                    fission_render::Color {
                                                        r: env.theme.tokens.colors.background.r,
                                                        g: env.theme.tokens.colors.background.g,
                                                        b: env.theme.tokens.colors.background.b,
                                                        a: env.theme.tokens.colors.background.a,
                                                    },
                                                    scale_factor as f32,
                                                )
                                                .expect(
                                                    "failed to rasterize software frame",
                                                );
                                                device_handle.queue.write_texture(
                                                    wgpu::TexelCopyTextureInfo {
                                                        texture: &render_state
                                                            .surface
                                                            .target_texture,
                                                        mip_level: 0,
                                                        origin: wgpu::Origin3d::ZERO,
                                                        aspect: wgpu::TextureAspect::All,
                                                    },
                                                    &rgba,
                                                    wgpu::TexelCopyBufferLayout {
                                                        offset: 0,
                                                        bytes_per_row: Some(
                                                            render_target_size.0 * 4,
                                                        ),
                                                        rows_per_image: Some(
                                                            render_target_size.1,
                                                        ),
                                                    },
                                                    wgpu::Extent3d {
                                                        width: render_target_size.0,
                                                        height: render_target_size.1,
                                                        depth_or_array_layers: 1,
                                                    },
                                                );
                                            }
                                        }

                                        for (_, rect, payload) in &pipeline.scene_3d_surfaces {
                                            if let Ok(primitives) =
                                                bincode::deserialize::<Vec<fission_3d::Primitive3D>>(payload)
                                            {
                                                let scene3d = fission_3d::Scene3D {
                                                    width: Some(rect.size.width),
                                                    height: Some(rect.size.height),
                                                    primitives,
                                                };
                                                let scale = scale_factor as f32;
                                                render_state.scene3d_renderer.render_in_rect(
                                                    &device_handle.device,
                                                    &device_handle.queue,
                                                    &render_state.surface.target_view,
                                                    &scene3d,
                                                    fission_3d::render::Scene3DViewport {
                                                        x: rect.origin.x * scale,
                                                        y: rect.origin.y * scale,
                                                        width: rect.size.width * scale,
                                                        height: rect.size.height * scale,
                                                    },
                                                );
                                            }
                                        }

                                        let surface_view = surface_texture
                                            .texture
                                            .create_view(&wgpu::TextureViewDescriptor::default());

                                        let mut encoder = device_handle.device.create_command_encoder(
                                            &wgpu::CommandEncoderDescriptor {
                                                label: Some("Surface Blit"),
                                            },
                                        );

                                        render_state.surface.blitter.copy(
                                            &device_handle.device,
                                            &mut encoder,
                                            &render_state.surface.target_view,
                                            &surface_view,
                                        );

                                        device_handle.queue.submit(Some(encoder.finish()));

                                        let capture_ready =
                                            !pending_capture_settle || resize_settled;
                                        if capture_ready {
                                            pending_capture_settle = false;
                                        }
                                        if capture_ready {
                                            if let Some(path) = pending_screenshot_path.take() {
                                                let screenshot_dimensions =
                                                    layout_size_to_image_dimensions(target_viewport);
                                                if let Some(ref tx) = test_response_tx {
                                                    if path == "__pump__" {
                                                        let _ = tx.send(
                                                            fission_test_driver::TestResponse::Ok {},
                                                        );
                                                    } else if path == "__capture__" {
                                                        let resp = gpu_screenshot(
                                                            &device_handle.device,
                                                            &device_handle.queue,
                                                            &render_state.surface.target_texture,
                                                            render_target_size.0,
                                                            render_target_size.1,
                                                            screenshot_dimensions.0,
                                                            screenshot_dimensions.1,
                                                            None,
                                                        );
                                                        let _ = tx.send(resp);
                                                    } else {
                                                        let resp = gpu_screenshot(
                                                            &device_handle.device,
                                                            &device_handle.queue,
                                                            &render_state.surface.target_texture,
                                                            render_target_size.0,
                                                            render_target_size.1,
                                                            screenshot_dimensions.0,
                                                            screenshot_dimensions.1,
                                                            Some(&path),
                                                        );
                                                        let _ = tx.send(resp);
                                                    }
                                                }
                                            }
                                        }

                                        surface_texture.present();
                                        pending_resize = None;
                                        if resize_settled {
                                            resize_needs_settled_frame = false;
                                        }
                                        invalidations = InvalidationSet::default();

                                        presented_frames = presented_frames.saturating_add(1);
                                        flush_text_traces(
                                            text_trace_enabled,
                                            &mut pending_text_traces,
                                            presented_frames,
                                        );

                                        diag::end_frame(diag::FrameStats::default());
                                    }
                                    Err(e) => {
                                        eprintln!("Pipeline error: {:?}", e);
                                        diag::end_frame(diag::FrameStats::default());
                                    }
                                }
                            }
                            WindowEvent::CloseRequested => {
                                elwt.exit();
                            }
                            // Input Handling — delegates to the same extracted functions
                            // that TestEvent handlers use.
                            WindowEvent::CursorMoved { position, .. } => {
                                last_cursor_position = Some(position);
                                let scale_factor = window.scale_factor();
                                let x = (position.x / scale_factor) as f32;
                                let y = (position.y / scale_factor) as f32;
                                handle_cursor_moved(
                                    x, y, current_mods,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                            }
                            WindowEvent::CursorLeft { .. } => {
                                handle_cursor_left(
                                    last_cursor_position,
                                    &mut runtime,
                                    &pipeline,
                                    &effect_result_tx,
                                    &event_proxy,

                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                                last_cursor_position = None;
                            }
                            WindowEvent::MouseInput { state, button, .. } => {
                                if let Some(position) = last_cursor_position {
                                    let scale_factor = window.scale_factor();
                                    let x = (position.x / scale_factor) as f32;
                                    let y = (position.y / scale_factor) as f32;
                                    if let Some(btn) = map_mouse_button(button) {
                                        let is_pressed = state.is_pressed();
                                        handle_mouse_button(
                                            x, y, btn, is_pressed, current_mods,
                                            &mut runtime, &pipeline,
                                            &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                            &window, elwt,
                                            &mut last_redraw_at, min_frame, &mut redraw_pending,
                                            text_trace_enabled, &mut pending_text_traces,
                                            &mut next_text_trace_seq, presented_frames,
                                            &mut last_blink_toggle,
                                            &mut frame_trace,
                                            &mut invalidations,
                                        );
                                    }
                                }
                            }
                            WindowEvent::MouseWheel { delta, .. } => {
                                if let Some(position) = last_cursor_position {
                                    let scale_factor = window.scale_factor();
                                    let point_x = (position.x / scale_factor) as f32;
                                    let point_y = (position.y / scale_factor) as f32;

                                    let (dx, dy) = match delta {
                                        MouseScrollDelta::LineDelta(x, y) => (-x * 50.0, -y * 50.0),
                                        MouseScrollDelta::PixelDelta(p) => (
                                            -(p.x / scale_factor) as f32,
                                            -(p.y / scale_factor) as f32,
                                        ),
                                    };

                                    if std::env::var("FISSION_SCROLL_TRACE").ok().as_deref() == Some("1") {
                                        eprintln!(
                                            "[scroll-trace] mousewheel raw={:?} point=({:.1},{:.1}) delta=({:.1},{:.1})",
                                            delta, point_x, point_y, dx, dy
                                        );
                                    }
                                    handle_scroll(
                                        point_x, point_y, dx, dy, current_mods,
                                        &mut runtime, &pipeline,
                                        &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                        &window, elwt,
                                        &mut last_redraw_at, min_frame, &mut redraw_pending,
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
                                }
                            }
                            WindowEvent::Touch(touch) => {
                                let position = touch.location;
                                last_cursor_position = Some(position);
                                touch_positions.insert(touch.id, position);

                                let scale_factor = window.scale_factor();
                                let x = (position.x / scale_factor) as f32;
                                let y = (position.y / scale_factor) as f32;

                                match touch.phase {
                                    TouchPhase::Started => {
                                        if active_primary_touch.is_none() {
                                            active_primary_touch = Some(touch.id);
                                        }
                                        if active_primary_touch == Some(touch.id) {
                                            handle_cursor_moved(
                                                x, y, current_mods,
                                                &mut runtime, &pipeline,
                                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                                &window, elwt,
                                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                                &mut frame_trace,
                                                &mut invalidations,
                                            );
                                            handle_mouse_button(
                                                x, y, PointerButton::Primary, true, current_mods,
                                                &mut runtime, &pipeline,
                                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                                &window, elwt,
                                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                                text_trace_enabled, &mut pending_text_traces,
                                                &mut next_text_trace_seq, presented_frames,
                                                &mut last_blink_toggle,
                                                &mut frame_trace,
                                                &mut invalidations,
                                            );
                                        }
                                    }
                                    TouchPhase::Moved => {
                                        if active_primary_touch == Some(touch.id) {
                                            handle_cursor_moved(
                                                x, y, current_mods,
                                                &mut runtime, &pipeline,
                                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                                &window, elwt,
                                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                                &mut frame_trace,
                                                &mut invalidations,
                                            );
                                        }
                                    }
                                    TouchPhase::Ended | TouchPhase::Cancelled => {
                                        if active_primary_touch == Some(touch.id) {
                                            handle_cursor_moved(
                                                x, y, current_mods,
                                                &mut runtime, &pipeline,
                                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                                &window, elwt,
                                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                                &mut frame_trace,
                                                &mut invalidations,
                                            );
                                            handle_mouse_button(
                                                x, y, PointerButton::Primary, false, current_mods,
                                                &mut runtime, &pipeline,
                                                &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                                &window, elwt,
                                                &mut last_redraw_at, min_frame, &mut redraw_pending,
                                                text_trace_enabled, &mut pending_text_traces,
                                                &mut next_text_trace_seq, presented_frames,
                                                &mut last_blink_toggle,
                                                &mut frame_trace,
                                                &mut invalidations,
                                            );
                                            active_primary_touch = None;
                                        }
                                        touch_positions.remove(&touch.id);
                                    }
                                }
                            }
                            WindowEvent::ModifiersChanged(modifiers) => {
                                current_mods = 0;
                                if modifiers.state().shift_key() { current_mods |= 1; }
                                if modifiers.state().alt_key() { current_mods |= 2; }
                                if modifiers.state().control_key() { current_mods |= 4; }
                                if modifiers.state().super_key() { current_mods |= 8; }
                            }
                            WindowEvent::KeyboardInput { event, .. } => {
                                if event.state.is_pressed() {
                                    use winit::keyboard::{Key, NamedKey};
                                    let key_code = match event.logical_key {
                                        Key::Named(NamedKey::Space) => Some(KeyCode::Space),
                                        Key::Named(NamedKey::Enter) => Some(KeyCode::Enter),
                                        Key::Named(NamedKey::Escape) => Some(KeyCode::Escape),
                                        Key::Named(NamedKey::Backspace) => Some(KeyCode::Backspace),
                                        Key::Named(NamedKey::Delete) => Some(KeyCode::Delete),
                                        Key::Named(NamedKey::Tab) => Some(KeyCode::Tab),
                                        Key::Named(NamedKey::ArrowLeft) => Some(KeyCode::Left),
                                        Key::Named(NamedKey::ArrowRight) => Some(KeyCode::Right),
                                        Key::Named(NamedKey::ArrowUp) => Some(KeyCode::Up),
                                        Key::Named(NamedKey::ArrowDown) => Some(KeyCode::Down),
                                        Key::Named(NamedKey::Home) => Some(KeyCode::Home),
                                        Key::Named(NamedKey::End) => Some(KeyCode::End),
                                        Key::Named(NamedKey::PageUp) => Some(KeyCode::PageUp),
                                        Key::Named(NamedKey::PageDown) => Some(KeyCode::PageDown),
                                        _ => {
                                            if let Some(text) = &event.text {
                                                text.chars().next().map(KeyCode::Char)
                                            } else {
                                                None
                                            }
                                        }
                                    };

                                    if let Some(code) = key_code {
                                        handle_key_down::<S>(
                                            code, current_mods,
                                            &mut runtime, &pipeline,
                                            &effect_result_tx, &event_proxy,
                                &async_registry, &mut active_services, &mut service_bindings, &mut next_service_instance_id,
                                            &window, elwt,
                                            &mut last_redraw_at, min_frame, &mut redraw_pending,
                                            text_trace_enabled, &mut pending_text_traces,
                                            &mut next_text_trace_seq, presented_frames,
                                            &mut last_blink_toggle,
                                            self.key_handler.as_ref(),
                                            &mut frame_trace,
                                            &mut invalidations,
                                        );
                                    }
                                }
                            }
                            WindowEvent::Ime(ime) => {
                                if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
                                    let (input_event, source) = match ime {
                                        Ime::Commit(text) => (
                                            Some(InputEvent::Ime(fission_core::event::ImeEvent::Commit { text: text.clone() })),
                                            Some(format!("ime_commit:{}", text.chars().count())),
                                        ),
                                        Ime::Preedit(text, _) => (
                                            Some(InputEvent::Ime(fission_core::event::ImeEvent::Preedit { text: text.clone() })),
                                            Some(format!("ime_preedit:{}", text.chars().count())),
                                        ),
                                        _ => (None, None),
                                    };

                                    if let Some(e) = input_event {
                                        let target = focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                                        let trace_seq = start_text_trace(
                                            text_trace_enabled && target.is_some(),
                                            &mut pending_text_traces,
                                            &mut next_text_trace_seq,
                                            source.unwrap_or_else(|| "ime".to_string()),
                                            target,
                                            presented_frames,
                                        );
                                        runtime.handle_input(e, ir, layout).ok();
                                        invalidations.mark_build();
                                        mark_text_trace_handled(&mut pending_text_traces, trace_seq);
                                        if process_pending_effects(
                                            &mut runtime,
                                            &effect_result_tx,
                                            &event_proxy,

                                            &async_registry,
                                            &mut active_services,
                                            &mut service_bindings,
                                            &mut next_service_instance_id,
                                        ) {
                                            mark_text_trace_effects(&mut pending_text_traces, trace_seq);
                                            invalidations.mark_build();
                                            request_redraw_logged(
                                                &window,
                                                elwt,
                                                &mut last_redraw_at,
                                                min_frame,
                                                &mut redraw_pending,
                                                &mut frame_trace,
                                                "ime:effects",
                                            );
                                        }
                                        reset_text_input_caret(&mut runtime, pipeline.prev_ir.as_ref(), &mut last_blink_toggle);
                                        request_redraw_logged(
                                            &window,
                                            elwt,
                                            &mut last_redraw_at,
                                            min_frame,
                                            &mut redraw_pending,
                                            &mut frame_trace,
                                            "ime",
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            })
            .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
    }
}

fn build_font_context() -> FontContext {
    let use_system_fonts = std::env::var("FISSION_USE_SYSTEM_FONTS")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let options = CollectionOptions {
        shared: false,
        system_fonts: use_system_fonts,
    };
    FontContext {
        collection: Collection::new(options),
        source_cache: SourceCache::default(),
    }
}

// Helpers...
fn map_mouse_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Other(id) => Some(PointerButton::Other(id as u8)),
        _ => None,
    }
}

fn clamp_copy_extent_to_texture(
    requested_width: u32,
    requested_height: u32,
    actual_width: u32,
    actual_height: u32,
) -> (u32, u32) {
    (
        requested_width.min(actual_width).max(1),
        requested_height.min(actual_height).max(1),
    )
}

fn gpu_screenshot(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    texture_width: u32,
    texture_height: u32,
    output_width: u32,
    output_height: u32,
    path: Option<&str>,
) -> fission_test_driver::TestResponse {
    let actual_texture_width = texture.width();
    let actual_texture_height = texture.height();
    let (texture_width, texture_height) = clamp_copy_extent_to_texture(
        texture_width,
        texture_height,
        actual_texture_width,
        actual_texture_height,
    );
    if output_width == 0 || output_height == 0 {
        return fission_test_driver::TestResponse::Error {
            message: "zero-size viewport".into(),
        };
    }

    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = texture_width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
    let buffer_size = (padded_bytes_per_row * texture_height) as u64;

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot staging"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("screenshot copy"),
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(texture_height),
            },
        },
        wgpu::Extent3d {
            width: texture_width,
            height: texture_height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let (tx, rx) = std::sync::mpsc::channel();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    let _ = device.poll(wgpu::PollType::Wait);

    match rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return fission_test_driver::TestResponse::Error {
                message: format!("buffer map failed: {:?}", e),
            };
        }
        Err(e) => {
            return fission_test_driver::TestResponse::Error {
                message: format!("buffer map channel error: {}", e),
            };
        }
    }

    let data = staging.slice(..).get_mapped_range();

    // Remove row padding (texture is Rgba8Unorm, no swizzle needed)
    let mut rgba = Vec::with_capacity((texture_width * texture_height * 4) as usize);
    for row in 0..texture_height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + (texture_width * bytes_per_pixel) as usize;
        rgba.extend_from_slice(&data[start..end]);
    }

    drop(data);
    staging.unmap();

    let (rgba, width, height) = if texture_width == output_width && texture_height == output_height
    {
        (rgba, texture_width, texture_height)
    } else if let Some(resized) = downscale_rgba_box(
        &rgba,
        texture_width,
        texture_height,
        output_width,
        output_height,
    ) {
        (resized, output_width, output_height)
    } else {
        let Some(image) = image::RgbaImage::from_raw(texture_width, texture_height, rgba) else {
            return fission_test_driver::TestResponse::Error {
                message: "failed to decode screenshot RGBA buffer".into(),
            };
        };
        let resized = image::imageops::resize(
            &image,
            output_width,
            output_height,
            image::imageops::FilterType::Triangle,
        );
        (resized.into_raw(), output_width, output_height)
    };

    let mut png = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new(&mut png);
        if let Err(e) = encoder.write_image(&rgba, width, height, image::ExtendedColorType::Rgba8) {
            return fission_test_driver::TestResponse::Error {
                message: format!("PNG encode failed: {}", e),
            };
        }
    }

    if let Some(path) = path {
        match std::fs::write(path, &png) {
            Ok(()) => fission_test_driver::TestResponse::Ok {},
            Err(e) => fission_test_driver::TestResponse::Error {
                message: format!("PNG save failed: {}", e),
            },
        }
    } else {
        fission_test_driver::TestResponse::Screenshot {
            png_base64: base64::engine::general_purpose::STANDARD.encode(png),
            width,
            height,
        }
    }
}

fn downscale_rgba_box(
    rgba: &[u8],
    input_width: u32,
    input_height: u32,
    output_width: u32,
    output_height: u32,
) -> Option<Vec<u8>> {
    if output_width == 0
        || output_height == 0
        || input_width % output_width != 0
        || input_height % output_height != 0
    {
        return None;
    }

    let scale_x = input_width / output_width;
    let scale_y = input_height / output_height;
    if scale_x <= 1 && scale_y <= 1 {
        return None;
    }

    let samples_per_pixel = scale_x.checked_mul(scale_y)?;
    let mut out = vec![0u8; (output_width * output_height * 4) as usize];

    for out_y in 0..output_height {
        let src_y0 = out_y * scale_y;
        for out_x in 0..output_width {
            let src_x0 = out_x * scale_x;
            let mut sum = [0u32; 4];
            for dy in 0..scale_y {
                let src_y = src_y0 + dy;
                let row_offset = ((src_y * input_width) * 4) as usize;
                for dx in 0..scale_x {
                    let src_x = src_x0 + dx;
                    let src_index = row_offset + (src_x * 4) as usize;
                    sum[0] += rgba[src_index] as u32;
                    sum[1] += rgba[src_index + 1] as u32;
                    sum[2] += rgba[src_index + 2] as u32;
                    sum[3] += rgba[src_index + 3] as u32;
                }
            }

            let dst_index = (((out_y * output_width) + out_x) * 4) as usize;
            out[dst_index] = (sum[0] / samples_per_pixel) as u8;
            out[dst_index + 1] = (sum[1] / samples_per_pixel) as u8;
            out[dst_index + 2] = (sum[2] / samples_per_pixel) as u8;
            out[dst_index + 3] = (sum[3] / samples_per_pixel) as u8;
        }
    }

    Some(out)
}

fn layout_size_to_image_dimensions(size: LayoutSize) -> (u32, u32) {
    let width = size.width.max(1.0).round() as u32;
    let height = size.height.max(1.0).round() as u32;
    (width.max(1), height.max(1))
}

fn normalize_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn physical_size_to_layout_size(size: PhysicalSize<u32>, scale_factor: f64) -> LayoutSize {
    let scale_factor = normalize_scale_factor(scale_factor);
    LayoutSize {
        width: (size.width as f64 / scale_factor) as f32,
        height: (size.height as f64 / scale_factor) as f32,
    }
}

fn logical_viewport_to_render_target_size(size: LayoutSize, scale_factor: f64) -> (u32, u32) {
    let scale_factor = normalize_scale_factor(scale_factor);
    let width = (size.width.max(1.0) as f64 * scale_factor).ceil() as u32;
    let height = (size.height.max(1.0) as f64 * scale_factor).ceil() as u32;
    (width.max(1), height.max(1))
}

fn logical_viewport_to_physical_size(size: LayoutSize, scale_factor: f64) -> PhysicalSize<u32> {
    let (width, height) = logical_viewport_to_render_target_size(size, scale_factor);
    PhysicalSize::new(width, height)
}

fn recreate_target_texture(
    surface: &mut RenderSurface,
    render_cx: &RenderContext,
    width: u32,
    height: u32,
) {
    let device = &render_cx.devices[surface.dev_id].device;
    let size = wgpu::Extent3d {
        width: width.max(1),
        height: height.max(1),
        depth_or_array_layers: 1,
    };
    let new_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fission_target_with_copy"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm, // Must match Vello's internal format
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let new_view = new_texture.create_view(&wgpu::TextureViewDescriptor::default());
    surface.target_texture = new_texture;
    surface.target_view = new_view;
}

fn sync_tracked_target_texture_size_to_surface(
    target_texture_size: &mut (u32, u32),
    surface_size: PhysicalSize<u32>,
) {
    *target_texture_size = (surface_size.width.max(1), surface_size.height.max(1));
}

fn native_window_size_for_logical_viewport(size: LayoutSize) -> LogicalSize<f64> {
    LogicalSize::new(size.width as f64, size.height as f64)
}

#[cfg(test)]
mod tests {
    use super::{
        animation_redraw_interval, clamp_copy_extent_to_texture, cursor_icon_for,
        downscale_rgba_box, layout_size_to_image_dimensions, logical_viewport_to_physical_size,
        logical_viewport_to_render_target_size, native_window_size_for_logical_viewport,
        normalize_scale_factor, physical_size_to_layout_size, repeating_animation_redraw_interval,
        resize_is_unsettled, resolve_build_viewport, sync_tracked_target_texture_size_to_surface,
        texture_plans_fit_device_limits, LiveResizeController, WindowViewportState,
    };
    use crate::pipeline::CompositorTexturePlan;
    use crate::InvalidationSet;
    use fission_core::env::{ActiveAnimation, AnimationStateMap};
    use fission_core::{AnimationPropertyId, WidgetNodeId};
    use fission_ir::semantics::MouseCursor;
    use fission_layout::{LayoutRect, LayoutSize};
    use std::collections::HashMap;
    use std::time::Duration;
    use winit::dpi::PhysicalSize;
    use winit::window::CursorIcon;

    #[test]
    fn semantic_cursor_icons_map_to_winit_icons() {
        assert_eq!(cursor_icon_for(MouseCursor::Default), CursorIcon::Default);
        assert_eq!(cursor_icon_for(MouseCursor::Pointer), CursorIcon::Pointer);
        assert_eq!(cursor_icon_for(MouseCursor::Text), CursorIcon::Text);
        assert_eq!(
            cursor_icon_for(MouseCursor::NotAllowed),
            CursorIcon::NotAllowed
        );
        assert_eq!(
            cursor_icon_for(MouseCursor::VerticalText),
            CursorIcon::VerticalText
        );
    }

    #[test]
    fn repeating_animation_uses_reduced_frame_rate() {
        let min_frame = Duration::from_millis(16);
        let repeat_frame = Duration::from_millis(66);
        assert_eq!(
            animation_redraw_interval(false, Some(repeat_frame), false, min_frame),
            Some(repeat_frame)
        );
    }

    #[test]
    fn finite_animation_keeps_full_frame_rate() {
        let min_frame = Duration::from_millis(16);
        assert_eq!(
            animation_redraw_interval(true, None, false, min_frame),
            Some(min_frame)
        );
        assert_eq!(
            animation_redraw_interval(false, None, true, min_frame),
            Some(min_frame)
        );
    }

    #[test]
    fn idle_video_does_not_force_full_frame_rate() {
        let min_frame = Duration::from_millis(16);
        let repeat_frame = Duration::from_millis(66);
        assert_eq!(
            animation_redraw_interval(false, Some(repeat_frame), false, min_frame),
            Some(repeat_frame)
        );
    }

    #[test]
    fn no_repeat_interval_means_no_idle_animation_redraw() {
        let min_frame = Duration::from_millis(16);
        assert_eq!(
            animation_redraw_interval(false, None, false, min_frame),
            None
        );
    }

    #[test]
    fn repeat_animation_interval_uses_low_priority_hint() {
        let mut animation = AnimationStateMap::default();
        animation.active.insert(
            (
                WidgetNodeId::explicit("spinner"),
                AnimationPropertyId::opacity(),
            ),
            ActiveAnimation {
                target: WidgetNodeId::explicit("spinner"),
                property: AnimationPropertyId::opacity(),
                start_value: 0.3,
                end_value: 1.0,
                start_time: 0,
                duration: 600,
                repeat: true,
                frame_interval_ms: Some(166),
                easing: fission_core::EasingFunction::Linear,
            },
        );
        assert_eq!(
            repeating_animation_redraw_interval(&animation, Duration::from_millis(66)),
            Some(Duration::from_millis(166))
        );
    }

    #[test]
    fn repeat_animation_interval_chooses_fastest_active_repeat() {
        let mut animation = AnimationStateMap {
            values: HashMap::new(),
            active: HashMap::new(),
        };
        animation.active.insert(
            (
                WidgetNodeId::explicit("slow"),
                AnimationPropertyId::opacity(),
            ),
            ActiveAnimation {
                target: WidgetNodeId::explicit("slow"),
                property: AnimationPropertyId::opacity(),
                start_value: 0.3,
                end_value: 1.0,
                start_time: 0,
                duration: 600,
                repeat: true,
                frame_interval_ms: Some(200),
                easing: fission_core::EasingFunction::Linear,
            },
        );
        animation.active.insert(
            (
                WidgetNodeId::explicit("fast"),
                AnimationPropertyId::opacity(),
            ),
            ActiveAnimation {
                target: WidgetNodeId::explicit("fast"),
                property: AnimationPropertyId::opacity(),
                start_value: 0.3,
                end_value: 1.0,
                start_time: 0,
                duration: 600,
                repeat: true,
                frame_interval_ms: Some(100),
                easing: fission_core::EasingFunction::Linear,
            },
        );
        assert_eq!(
            repeating_animation_redraw_interval(&animation, Duration::from_millis(66)),
            Some(Duration::from_millis(100))
        );
    }

    #[test]
    fn live_resize_reports_unsettled_until_deadline() {
        let settle = Duration::from_millis(90);
        let mut resize = LiveResizeController::new(settle);
        let now = std::time::Instant::now();
        resize.note_resize(now);

        assert!(resize.is_live(now + Duration::from_millis(30)));
        assert!(resize_is_unsettled(
            false,
            false,
            resize.is_live(now + Duration::from_millis(30))
        ));
        assert!(!resize.is_live(now + Duration::from_millis(95)));
    }

    #[test]
    fn viewport_resize_forces_build_viewport_refresh() {
        let target = LayoutSize::new(1440.0, 900.0);
        let mut invalidations = InvalidationSet::default();

        let build_viewport = resolve_build_viewport(
            Some(LayoutSize::new(1024.0, 768.0)),
            target,
            true,
            &mut invalidations,
        );

        assert!(invalidations.build);
        assert_eq!(build_viewport, target);
    }

    #[test]
    fn stable_viewport_preserves_existing_build_viewport() {
        let target = LayoutSize::new(1024.0, 768.0);
        let mut invalidations = InvalidationSet::default();

        let build_viewport = resolve_build_viewport(Some(target), target, true, &mut invalidations);

        assert!(!invalidations.build);
        assert_eq!(build_viewport, target);
    }

    #[test]
    fn oversized_texture_plan_forces_scene_fallback() {
        let plans = vec![CompositorTexturePlan {
            key: 1,
            bounds: LayoutRect::new(0.0, 0.0, 320.0, 9000.0),
            scene: Some(fission_render::RenderScene::new(LayoutRect::new(
                0.0, 0.0, 320.0, 9000.0,
            ))),
            scene_cache_key: Some(1),
            content_key: 1,
            local_dynamic: false,
            composite_dynamic: false,
            opacity: 1.0,
            transform: None,
            transform_clip: false,
            clip: None,
            children: Vec::new(),
            source_layer_path: None,
        }];
        assert!(!texture_plans_fit_device_limits(&plans, 1.0, 8192));
    }

    #[test]
    fn nested_texture_plans_must_all_fit_device_limits() {
        let child = CompositorTexturePlan {
            key: 2,
            bounds: LayoutRect::new(0.0, 0.0, 400.0, 8400.0),
            scene: Some(fission_render::RenderScene::new(LayoutRect::new(
                0.0, 0.0, 400.0, 8400.0,
            ))),
            scene_cache_key: Some(2),
            content_key: 2,
            local_dynamic: false,
            composite_dynamic: false,
            opacity: 1.0,
            transform: None,
            transform_clip: false,
            clip: None,
            children: Vec::new(),
            source_layer_path: None,
        };
        let plans = vec![CompositorTexturePlan {
            key: 1,
            bounds: LayoutRect::new(0.0, 0.0, 800.0, 600.0),
            scene: None,
            scene_cache_key: None,
            content_key: 3,
            local_dynamic: false,
            composite_dynamic: false,
            opacity: 1.0,
            transform: None,
            transform_clip: false,
            clip: None,
            children: vec![child],
            source_layer_path: None,
        }];
        assert!(!texture_plans_fit_device_limits(&plans, 1.0, 8192));
    }

    #[test]
    fn screenshot_dimensions_follow_logical_viewport() {
        let dims = layout_size_to_image_dimensions(fission_layout::LayoutSize::new(1600.0, 1200.0));
        assert_eq!(dims, (1600, 1200));

        let rounded =
            layout_size_to_image_dimensions(fission_layout::LayoutSize::new(999.6, 700.4));
        assert_eq!(rounded, (1000, 700));
    }

    #[test]
    fn simulated_resize_uses_physical_render_target_size() {
        let dims = logical_viewport_to_render_target_size(
            fission_layout::LayoutSize::new(1600.0, 1200.0),
            2.0,
        );
        assert_eq!(dims, (3200, 2400));

        let fractional = logical_viewport_to_render_target_size(
            fission_layout::LayoutSize::new(430.0, 900.0),
            1.5,
        );
        assert_eq!(fractional, (645, 1350));
    }

    #[test]
    fn physical_viewport_maps_to_logical_size_with_scale_factor() {
        let logical = physical_size_to_layout_size(PhysicalSize::new(1728, 1117), 1.5);
        assert_eq!(logical.width, 1152.0);
        assert!((logical.height - 744.6667).abs() < 0.001);
    }

    #[test]
    fn scale_factor_change_preserves_logical_viewport_until_resize_arrives() {
        let viewport = WindowViewportState {
            physical_size: PhysicalSize::new(1600, 1200),
            scale_factor: 1.0,
        }
        .with_scale_factor(2.0);

        assert_eq!(viewport.physical_size, PhysicalSize::new(3200, 2400));
        assert_eq!(
            viewport.logical_size(),
            fission_layout::LayoutSize::new(1600.0, 1200.0)
        );
    }

    #[test]
    fn resized_event_overrides_scale_factor_prediction_authoritatively() {
        let viewport = WindowViewportState {
            physical_size: PhysicalSize::new(1600, 1200),
            scale_factor: 1.0,
        }
        .with_scale_factor(1.5)
        .with_physical_size(PhysicalSize::new(2412, 1809));

        assert_eq!(viewport.physical_size, PhysicalSize::new(2412, 1809));
        assert_eq!(
            viewport.logical_size(),
            fission_layout::LayoutSize::new(1608.0, 1206.0)
        );
    }

    #[test]
    fn fractional_logical_viewports_round_up_for_render_targets() {
        let physical =
            logical_viewport_to_physical_size(fission_layout::LayoutSize::new(430.2, 900.1), 1.5);
        assert_eq!(physical, PhysicalSize::new(646, 1351));
    }

    #[test]
    fn scale_factor_prediction_never_undershoots_fractional_viewports() {
        let initial = WindowViewportState {
            physical_size: PhysicalSize::new(1728, 1117),
            scale_factor: 1.5,
        };
        let predicted = initial.with_scale_factor(2.0);

        assert_eq!(predicted.physical_size, PhysicalSize::new(2304, 1490));
        assert!(predicted.logical_size().width >= initial.logical_size().width);
        assert!(predicted.logical_size().height >= initial.logical_size().height);
    }

    #[test]
    fn logical_resize_updates_native_viewport_prediction() {
        let initial = WindowViewportState {
            physical_size: PhysicalSize::new(800, 632),
            scale_factor: 2.0,
        };
        let resized = initial.with_logical_size(fission_layout::LayoutSize::new(1600.0, 1200.0));

        assert_eq!(resized.physical_size, PhysicalSize::new(3200, 2400));
        assert_eq!(
            resized.logical_size(),
            fission_layout::LayoutSize::new(1600.0, 1200.0)
        );
    }

    #[test]
    fn logical_resize_requests_logical_window_dimensions() {
        let requested = native_window_size_for_logical_viewport(fission_layout::LayoutSize::new(
            1600.0, 2200.0,
        ));

        assert_eq!(requested.width, 1600.0);
        assert_eq!(requested.height, 2200.0);
    }

    #[test]
    fn invalid_scale_factors_fall_back_to_unit_scale() {
        assert_eq!(normalize_scale_factor(0.0), 1.0);
        assert_eq!(normalize_scale_factor(-2.0), 1.0);
        assert_eq!(normalize_scale_factor(f64::NAN), 1.0);
        assert_eq!(normalize_scale_factor(f64::INFINITY), 1.0);
        assert_eq!(normalize_scale_factor(1.5), 1.5);
    }

    #[test]
    fn invalid_scale_factor_does_not_shrink_viewport_math() {
        let logical = physical_size_to_layout_size(PhysicalSize::new(1600, 1200), 0.0);
        assert_eq!(logical, fission_layout::LayoutSize::new(1600.0, 1200.0));

        let render_target = logical_viewport_to_render_target_size(
            fission_layout::LayoutSize::new(1600.0, 1200.0),
            0.0,
        );
        assert_eq!(render_target, (1600, 1200));
    }

    #[test]
    fn surface_resize_resets_custom_target_texture_tracking() {
        let mut tracked_target_texture_size = (1600, 1200);

        sync_tracked_target_texture_size_to_surface(
            &mut tracked_target_texture_size,
            PhysicalSize::new(1055, 791),
        );

        assert_eq!(tracked_target_texture_size, (1055, 791));
        assert_ne!(
            tracked_target_texture_size,
            logical_viewport_to_render_target_size(
                fission_layout::LayoutSize::new(1600.0, 1200.0),
                1.0,
            )
        );
    }

    #[test]
    fn resize_settle_signal_tracks_real_resize_state() {
        assert!(resize_is_unsettled(true, false, false));
        assert!(resize_is_unsettled(false, true, false));
        assert!(resize_is_unsettled(false, false, true));
        assert!(!resize_is_unsettled(false, false, false));
    }

    #[test]
    fn screenshot_copy_extent_never_exceeds_texture_bounds() {
        assert_eq!(
            clamp_copy_extent_to_texture(1600, 1200, 1055, 791),
            (1055, 791)
        );
        assert_eq!(clamp_copy_extent_to_texture(0, 0, 1055, 791), (1, 1));
        assert_eq!(
            clamp_copy_extent_to_texture(640, 480, 1055, 791),
            (640, 480)
        );
    }

    #[test]
    fn integer_downscale_uses_fast_box_path() {
        let rgba = vec![
            10, 20, 30, 255, 30, 40, 50, 255, 50, 60, 70, 255, 70, 80, 90, 255,
        ];
        let downscaled = downscale_rgba_box(&rgba, 2, 2, 1, 1).expect("downscale");
        assert_eq!(downscaled, vec![40, 50, 60, 255]);
    }
}
