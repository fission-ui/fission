#![allow(unexpected_cfgs)]

use anyhow::Result;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
#[cfg(target_os = "android")]
use winit::platform::android::{activity::AndroidApp, EventLoopBuilderExtAndroid};
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::{
    dpi::PhysicalPosition,
    event::{Event, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    window::{Window, WindowBuilder},
};

use fission_core::env::VideoStatus;
use fission_core::lowering::LoweringContext;
use fission_core::ui::custom_render::downcast_render_object;
use fission_core::{
    ActionId, AppState, BuildCtx, Env, ImeHandler, InputEvent, KeyCode,
    KeyEvent as FissionKeyEvent, PointerButton, PointerEvent, Runtime, View, Widget,
};
use fission_core::{ActionInput, Effect, EffectPayload, SystemEffect};
use fission_diagnostics::prelude as diag;
use fission_ir::{CoreIR, NodeId, Op, WidgetNodeId};
use fission_layout::{LayoutEngine, LayoutSize};
use fission_render::{LayoutPoint, LayoutRect, Renderer as _};
use fission_render_vello::parley::FontContext;
use fission_render_vello::{RetainedSceneCache, VelloRenderer, VelloTextMeasurer};
use fission_shell::{VideoBackend, VideoEvent, VideoPlayer};
use fission_theme::fonts;
use fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache};

use fission_test_driver::TestEvent;

// Vello / WGPU
use pollster::block_on;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaSupport, Renderer as VelloSceneRenderer, RendererOptions, Scene};

mod compositor;
use compositor::TextureLayerCompositor;
mod pipeline;
pub use pipeline::{InvalidationSet, Pipeline};
mod video_backend;
#[cfg(target_os = "macos")]
use video_backend::MacVideoBackend;
#[cfg(not(target_os = "macos"))]
use video_backend::MockVideoBackend;

mod clipboard;
use clipboard::DesktopClipboard;
mod ime;
use ime::DesktopImeHandler;
pub mod test_control;

use fission_core::action::ActionEnvelope;

/// A single completed background effect result, ready to be dispatched on the main thread.
///
/// Fields: `(req_id, result_payload_or_error, on_ok_continuation, on_err_continuation)`
type EffectResult = (
    u64,
    std::result::Result<EffectPayload, String>,
    Option<ActionEnvelope>,
    Option<ActionEnvelope>,
);

/// Callback signature for application-specific effect handlers.
///
/// The handler receives the opaque `Vec<u8>` payload from `Effect::App(...)`,
/// plus the envelope metadata needed to send a result back on the channel and
/// an event-loop proxy it can use to wake the shell after background work
/// completes.
pub type AppEffectHandler = Box<
    dyn Fn(
            Vec<u8>,
            u64,
            Option<ActionEnvelope>,
            Option<ActionEnvelope>,
            mpsc::Sender<EffectResult>,
            EventLoopProxy<TestEvent>,
        ) + Send
        + Sync,
>;

struct ActivePlayer {
    player: Box<dyn VideoPlayer>,
    last_status: Option<VideoStatus>,
    last_rate: Option<f32>,
    last_volume: Option<f32>,
    last_muted: Option<bool>,
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

#[derive(Debug)]
struct LiveResizeController {
    active_until: Option<Instant>,
    settle_delay: Duration,
    layout_interval: Duration,
    last_layout_at: Option<Instant>,
}

impl LiveResizeController {
    fn new(settle_delay: Duration) -> Self {
        Self {
            active_until: None,
            settle_delay,
            layout_interval: Duration::from_millis(16),
            last_layout_at: None,
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

    fn should_apply_layout(
        &mut self,
        now: Instant,
        has_layout_snapshot: bool,
        force: bool,
    ) -> bool {
        if !has_layout_snapshot || force {
            self.last_layout_at = Some(now);
            return true;
        }

        let settled = !self.is_live(now);
        if settled {
            self.active_until = None;
            self.last_layout_at = Some(now);
            true
        } else {
            let should_refresh = self
                .last_layout_at
                .map(|last| now.saturating_duration_since(last) >= self.layout_interval)
                .unwrap_or(true);
            if should_refresh {
                self.last_layout_at = Some(now);
            }
            should_refresh
        }
    }
}

/// Drain pending effects from the runtime and either execute them synchronously
/// (fire-and-forget effects like `OpenUrl`) or spawn background threads for I/O
/// effects (`FileRead`, `HttpGet`) and send results back through `effect_tx`.
///
/// Returns `true` if any synchronous callback was dispatched (caller should redraw).
fn process_pending_effects(
    runtime: &mut Runtime,
    effect_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    app_effect_handler: Option<&AppEffectHandler>,
) -> bool {
    use std::process::Command;

    let pending = std::mem::take(&mut runtime.pending_effects);
    if pending.is_empty() {
        return false;
    }

    let mut dispatched_callback = false;

    for env in pending {
        match env.effect {
            Effect::System(ref system) => {
                match system {
                    // ── Fire-and-forget: OpenUrl ─────────────────────────────
                    SystemEffect::OpenUrl { url, in_app } => {
                        diag::emit(
                            diag::DiagCategory::Input,
                            diag::DiagLevel::Info,
                            diag::DiagEventKind::InputEvent {
                                kind: format!("system_effect:OpenUrl in_app={}", in_app),
                                target: None,
                                position: None,
                            },
                        );

                        let result = if cfg!(target_os = "macos") {
                            Command::new("open").arg(url).spawn().map(|_| ())
                        } else if cfg!(target_os = "windows") {
                            Command::new("cmd")
                                .args(["/C", "start", url])
                                .spawn()
                                .map(|_| ())
                        } else {
                            Command::new("xdg-open").arg(url).spawn().map(|_| ())
                        };

                        if let Err(e) = result {
                            diag::emit(
                                diag::DiagCategory::Input,
                                diag::DiagLevel::Error,
                                diag::DiagEventKind::InputEvent {
                                    kind: format!("system_effect:OpenUrl failed: {}", e),
                                    target: None,
                                    position: None,
                                },
                            );
                        }

                        // Dispatch immediate callback (fire-and-forget success).
                        if let Some(on_ok) = env.on_ok {
                            let _ = runtime.dispatch_with_input(
                                on_ok,
                                NodeId::derived(0, &[0]),
                                &ActionInput::EffectOk {
                                    req_id: env.req_id,
                                    payload: EffectPayload::Empty,
                                },
                            );
                            dispatched_callback = true;
                        }
                    }

                    // ── Fire-and-forget: Authenticate ────────────────────────
                    SystemEffect::Authenticate { url, .. } => {
                        diag::emit(
                            diag::DiagCategory::Input,
                            diag::DiagLevel::Info,
                            diag::DiagEventKind::InputEvent {
                                kind: "system_effect:Authenticate".into(),
                                target: None,
                                position: None,
                            },
                        );
                        let _ = if cfg!(target_os = "macos") {
                            Command::new("open").arg(url).spawn()
                        } else if cfg!(target_os = "windows") {
                            Command::new("cmd").args(["/C", "start", url]).spawn()
                        } else {
                            Command::new("xdg-open").arg(url).spawn()
                        };

                        if let Some(on_ok) = env.on_ok {
                            let _ = runtime.dispatch_with_input(
                                on_ok,
                                NodeId::derived(0, &[0]),
                                &ActionInput::EffectOk {
                                    req_id: env.req_id,
                                    payload: EffectPayload::Empty,
                                },
                            );
                            dispatched_callback = true;
                        }
                    }

                    // ── Fire-and-forget: Alert (log only) ────────────────────
                    SystemEffect::Alert { title, message } => {
                        diag::emit(
                            diag::DiagCategory::Input,
                            diag::DiagLevel::Info,
                            diag::DiagEventKind::InputEvent {
                                kind: format!("system_effect:Alert title={}", title),
                                target: None,
                                position: None,
                            },
                        );
                        eprintln!("[alert] {}: {}", title, message);

                        if let Some(on_ok) = env.on_ok {
                            let _ = runtime.dispatch_with_input(
                                on_ok,
                                NodeId::derived(0, &[0]),
                                &ActionInput::EffectOk {
                                    req_id: env.req_id,
                                    payload: EffectPayload::Empty,
                                },
                            );
                            dispatched_callback = true;
                        }
                    }

                    // ── Background: FileRead ─────────────────────────────────
                    SystemEffect::FileRead { path } => {
                        let tx = effect_tx.clone();
                        let wake_proxy = event_proxy.clone();
                        let on_ok = env.on_ok.clone();
                        let on_err = env.on_err.clone();
                        let req_id = env.req_id;
                        let path = path.clone();
                        std::thread::spawn(move || {
                            match std::fs::read(&path) {
                                Ok(content) => {
                                    let payload = EffectPayload::InlineBytes(content);
                                    let _ = tx.send((req_id, Ok(payload), on_ok, on_err));
                                }
                                Err(e) => {
                                    let _ = tx.send((req_id, Err(e.to_string()), on_ok, on_err));
                                }
                            }
                            let _ = wake_proxy.send_event(TestEvent::Wake);
                        });
                    }

                    // ── Background: HttpGet ───────────────────────────────────
                    SystemEffect::HttpGet { url, headers } => {
                        let tx = effect_tx.clone();
                        let wake_proxy = event_proxy.clone();
                        let on_ok = env.on_ok.clone();
                        let on_err = env.on_err.clone();
                        let req_id = env.req_id;
                        let url = url.clone();
                        let headers = headers.clone();
                        std::thread::spawn(move || {
                            let curl_result = (|| -> std::result::Result<Vec<u8>, String> {
                                let mut command = std::process::Command::new("curl");
                                command.arg("-fsSL").arg(&url);
                                for (k, v) in &headers {
                                    command.arg("-H").arg(format!("{}: {}", k, v));
                                }
                                let output =
                                    command.output().map_err(|e| format!("curl spawn: {}", e))?;
                                if output.status.success() {
                                    Ok(output.stdout)
                                } else {
                                    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
                                }
                            })();

                            // Minimal blocking HTTP GET using std only (no external crate).
                            // This parses the URL, opens a TCP stream, and reads the response.
                            let result = curl_result.or_else(|_| (|| -> std::result::Result<Vec<u8>, String> {
                                use std::io::{Read, Write};
                                use std::net::TcpStream;

                                // Parse URL (very basic: http://host[:port]/path)
                                let url_trimmed = url.trim();
                                let (scheme, rest) = if let Some(r) = url_trimmed.strip_prefix("https://") {
                                    ("https", r)
                                } else if let Some(r) = url_trimmed.strip_prefix("http://") {
                                    ("http", r)
                                } else {
                                    return Err(format!("unsupported URL scheme: {}", url_trimmed));
                                };

                                let (host_port, path) = match rest.find('/') {
                                    Some(i) => (&rest[..i], &rest[i..]),
                                    None => (rest, "/"),
                                };

                                let default_port: u16 = if scheme == "https" { 443 } else { 80 };
                                let (host, port) = match host_port.rfind(':') {
                                    Some(i) => {
                                        let p = host_port[i + 1..].parse::<u16>().unwrap_or(default_port);
                                        (&host_port[..i], p)
                                    }
                                    None => (host_port, default_port),
                                };

                                if scheme == "https" {
                                    return Err("HTTPS not supported in minimal HttpGet executor; use a custom app effect handler for TLS".into());
                                }

                                let addr = format!("{}:{}", host, port);
                                let mut stream = TcpStream::connect(&addr)
                                    .map_err(|e| format!("connect {}: {}", addr, e))?;
                                stream
                                    .set_read_timeout(Some(std::time::Duration::from_secs(30)))
                                    .ok();

                                let mut request = format!(
                                    "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
                                    path, host
                                );
                                for (k, v) in &headers {
                                    request.push_str(&format!("{}: {}\r\n", k, v));
                                }
                                request.push_str("\r\n");

                                stream
                                    .write_all(request.as_bytes())
                                    .map_err(|e| format!("write: {}", e))?;

                                let mut buf = Vec::new();
                                stream
                                    .read_to_end(&mut buf)
                                    .map_err(|e| format!("read: {}", e))?;

                                // Strip HTTP headers (find \r\n\r\n)
                                let body_start = buf
                                    .windows(4)
                                    .position(|w| w == b"\r\n\r\n")
                                    .map(|i| i + 4)
                                    .unwrap_or(0);

                                Ok(buf[body_start..].to_vec())
                            })());

                            match result {
                                Ok(bytes) => {
                                    let payload = EffectPayload::InlineBytes(bytes);
                                    let _ = tx.send((req_id, Ok(payload), on_ok, on_err));
                                }
                                Err(msg) => {
                                    let _ = tx.send((req_id, Err(msg), on_ok, on_err));
                                }
                            }
                            let _ = wake_proxy.send_event(TestEvent::Wake);
                        });
                    }

                    // ── Cancel / ReleaseResource: no-op at shell level ───────
                    SystemEffect::Cancel { .. } | SystemEffect::ReleaseResource { .. } => {
                        diag::emit(
                            diag::DiagCategory::Input,
                            diag::DiagLevel::Debug,
                            diag::DiagEventKind::InputEvent {
                                kind: format!("system_effect:{:?} (no-op)", system),
                                target: None,
                                position: None,
                            },
                        );
                    }
                }
            }

            // ── App-specific effects ─────────────────────────────────────
            Effect::App(payload) => {
                if let Some(handler) = app_effect_handler {
                    handler(
                        payload,
                        env.req_id,
                        env.on_ok.clone(),
                        env.on_err.clone(),
                        effect_tx.clone(),
                        event_proxy.clone(),
                    );
                } else {
                    diag::emit(
                        diag::DiagCategory::Input,
                        diag::DiagLevel::Warn,
                        diag::DiagEventKind::InputEvent {
                            kind: "app_effect:unhandled (no handler registered)".into(),
                            target: None,
                            position: None,
                        },
                    );
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
fn drain_effect_results(runtime: &mut Runtime, effect_rx: &mpsc::Receiver<EffectResult>) -> bool {
    let mut dispatched = false;

    while let Ok((req_id, result, on_ok, on_err)) = effect_rx.try_recv() {
        match result {
            Ok(payload) => {
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        NodeId::derived(0, &[0]),
                        &ActionInput::EffectOk { req_id, payload },
                    );
                    dispatched = true;
                }
            }
            Err(msg) => {
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        NodeId::derived(0, &[0]),
                        &ActionInput::EffectErr {
                            req_id,
                            message: msg,
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

/// Handle cursor/mouse move — shared by WindowEvent::CursorMoved and TestEvent::MouseMove.
fn handle_cursor_moved(
    x: f32,
    y: f32,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    app_effect_handler: Option<&AppEffectHandler>,
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
        let event = InputEvent::Pointer(PointerEvent::Move { point });
        if let Err(e) = runtime.handle_input(event, ir, layout) {
            eprintln!("Input handling error: {:?}", e);
        }
        invalidations.mark_build();
        if process_pending_effects(runtime, effect_result_tx, event_proxy, app_effect_handler) {
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
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    app_effect_handler: Option<&AppEffectHandler>,
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
            PointerEvent::Down { point, button }
        } else {
            PointerEvent::Up { point, button }
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
        invalidations.mark_build();

        mark_text_trace_handled(pending_text_traces, trace_seq);
        if process_pending_effects(runtime, effect_result_tx, event_proxy, app_effect_handler) {
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
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    app_effect_handler: Option<&AppEffectHandler>,
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
        });
        if let Err(e) = runtime.handle_input(event, ir, layout) {
            eprintln!("Scroll error: {:?}", e);
        }
        // Scroll offsets can affect more than a compositor translation. Virtualized
        // lists, scrollbars, and scroll-aware wrappers depend on the updated offset
        // during build/lowering, so treat scroll as a build invalidation.
        invalidations.mark_build();
        if process_pending_effects(runtime, effect_result_tx, event_proxy, app_effect_handler) {
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

/// Parse a key name string into a `KeyCode`.
fn parse_key_code(key: &str) -> KeyCode {
    match key {
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Escape,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
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
    app_effect_handler: Option<&AppEffectHandler>,
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
                    app_effect_handler,
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
        if process_pending_effects(runtime, effect_result_tx, event_proxy, app_effect_handler) {
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
                }),
                ir,
                snap,
            );
            let _ = runtime.handle_input(
                InputEvent::Pointer(PointerEvent::Up {
                    point,
                    button: PointerButton::Primary,
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

pub struct DesktopApp<S: AppState, W: Widget<S>> {
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
    /// Channel pair for receiving completed background effect results.
    effect_result_tx: mpsc::Sender<EffectResult>,
    effect_result_rx: mpsc::Receiver<EffectResult>,
    /// Optional handler for `Effect::App(...)` payloads.
    app_effect_handler: Option<AppEffectHandler>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: AppState + Default, W: Widget<S> + 'static> DesktopApp<S, W> {
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
            effect_result_tx,
            effect_result_rx,
            app_effect_handler: None,
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

    /// Register a handler for `Effect::App(payload)` effects.
    ///
    /// The handler runs on the calling thread and should spawn its own
    /// background work if needed, sending results through the provided
    /// `mpsc::Sender<EffectResult>`.
    pub fn with_app_effect_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(
                Vec<u8>,
                u64,
                Option<ActionEnvelope>,
                Option<ActionEnvelope>,
                mpsc::Sender<EffectResult>,
                EventLoopProxy<TestEvent>,
            ) + Send
            + Sync
            + 'static,
    {
        self.app_effect_handler = Some(Box::new(handler));
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

        let mut window_builder = WindowBuilder::new().with_title(&self.title);
        if background_test_mode {
            window_builder = window_builder.with_active(false).with_visible(false);
        }
        let window = Arc::new(
            window_builder
                .build(&event_loop)
                .map_err(|e| anyhow::anyhow!("Window build error: {}", e))?,
        );

        let ime_handler: Arc<dyn ImeHandler> = Arc::new(DesktopImeHandler::new(window.clone()));
        self.runtime = self.runtime.with_ime_handler(ime_handler);

        // Vello Context
        let mut render_cx = RenderContext::new();
        let mut surface = block_on(render_cx.create_surface(
            window.clone(),
            window.inner_size().width,
            window.inner_size().height,
            wgpu::PresentMode::AutoVsync,
        ))
        .unwrap();

        // Enable Alpha for video hole punching
        let device_handle = &render_cx.devices[surface.dev_id];
        surface.config.alpha_mode = wgpu::CompositeAlphaMode::PostMultiplied;
        surface
            .surface
            .configure(&device_handle.device, &surface.config);

        // Recreate target texture with COPY_SRC so GPU screenshots work
        let initial_target_texture_size = (surface.config.width, surface.config.height);
        recreate_target_texture(
            &mut surface,
            &render_cx,
            initial_target_texture_size.0,
            initial_target_texture_size.1,
        );
        let mut target_texture_size = initial_target_texture_size;

        let mut scene3d_renderer = fission_3d::render::Scene3DRenderer::new(
            &device_handle.device,
            window.inner_size().width,
            window.inner_size().height,
            wgpu::TextureFormat::Rgba8Unorm, // Same as vello render params
        );

        let mut vello_renderer = VelloSceneRenderer::new(
            &device_handle.device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::all(),
                num_init_threads: None,
                pipeline_cache: None,
            },
        )
        .unwrap();

        let mut scene = Scene::new();
        let mut retained_scene_cache = RetainedSceneCache::default();
        let mut texture_compositor =
            TextureLayerCompositor::new(&device_handle.device, wgpu::TextureFormat::Rgba8Unorm);

        window.request_redraw();

        let mut runtime = self.runtime;
        let mut layout_engine = self.layout_engine;
        let root_widget = self.root_widget;
        let mut env = self.env;
        let mut pipeline = self.pipeline;
        let measurer = self.measurer;
        let effect_result_tx = self.effect_result_tx;
        let effect_result_rx = self.effect_result_rx;
        let app_effect_handler = self.app_effect_handler;

        #[cfg(target_os = "macos")]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MacVideoBackend::new(&window));
        #[cfg(not(target_os = "macos"))]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MockVideoBackend::new());
        let mut players: HashMap<WidgetNodeId, ActivePlayer> = HashMap::new();

        let mut last_cursor_position: Option<PhysicalPosition<f64>> = None;
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
        let test_response_tx: Option<test_control::ResponseSender> =
            std::env::var("FISSION_TEST_CONTROL_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .map(|port| {
                    let (resp_tx, resp_rx) = test_control::create_response_channel();
                    test_control::spawn_server(port, event_proxy.clone(), resp_rx);
                    resp_tx
                });
        // Pending screenshot/pump: path + whether it needs a screenshot (vs pump).
        let mut pending_screenshot_path: Option<String> = None;
        // Simulated viewport size override for test resize events.
        // When set, layout uses these dimensions instead of window.inner_size().
        let mut simulated_viewport: Option<(u32, u32)> = None;
        let mut pending_resize = Some(window.inner_size());
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

                match event {
                    // ═══════════════════════════════════════════════════════
                    // UserEvent — injected by test control server via proxy
                    // ═══════════════════════════════════════════════════════
                    Event::UserEvent(test_event) => {
                        match test_event {
                            TestEvent::MouseMove { x, y } => {
                                // Update cursor position for subsequent button events
                                let scale_factor = window.scale_factor();
                                last_cursor_position = Some(PhysicalPosition::new(
                                    (x as f64) * scale_factor,
                                    (y as f64) * scale_factor,
                                ));
                                handle_cursor_moved(
                                    x, y,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, &event_proxy, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                            }
                            TestEvent::MouseDown { x, y, button } => {
                                let btn = map_test_button(button);
                                handle_mouse_button(
                                    x, y, btn, true,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, &event_proxy, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                    text_trace_enabled, &mut pending_text_traces,
                                    &mut next_text_trace_seq, presented_frames,
                                    &mut last_blink_toggle,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                            }
                            TestEvent::MouseUp { x, y, button } => {
                                let btn = map_test_button(button);
                                handle_mouse_button(
                                    x, y, btn, false,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, &event_proxy, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                    text_trace_enabled, &mut pending_text_traces,
                                    &mut next_text_trace_seq, presented_frames,
                                    &mut last_blink_toggle,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                            }
                            TestEvent::KeyDown { key_code, modifiers } => {
                                let code = parse_key_code(&key_code);
                                handle_key_down::<S>(
                                    code, modifiers,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, &event_proxy, app_effect_handler.as_ref(),
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
                            TestEvent::KeyUp { .. } => {
                                // Key-up is currently a no-op in the framework
                                // (only key-down dispatches actions), but we request
                                // a redraw so any state changes are rendered.
                                request_redraw_logged(
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "test_key_up",
                                );
                            }
                            TestEvent::TextInput { text } => {
                                // Route test typing through IME when the focus target accepts
                                // real text input; otherwise fall back to key events so
                                // app-level rename handlers and similar shortcuts still work.
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
                                            app_effect_handler.as_ref(),
                                        ) {
                                            mark_text_trace_effects(
                                                &mut pending_text_traces,
                                                trace_seq,
                                            );
                                            invalidations.mark_build();
                                        }
                                        request_redraw_logged(
                                            &window,
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
                                                app_effect_handler.as_ref(),
                                                &window,
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
                                handle_scroll(
                                    x, y, dx, dy,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, &event_proxy, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                            }
                            TestEvent::Resize { width, height } => {
                                if width > 0 && height > 0 {
                                    simulated_viewport = Some((width, height));
                                    pending_resize = Some(window.inner_size());
                                    live_resize.note_resize(Instant::now());
                                    invalidations.mark_composite();
                                    request_redraw_logged(
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        resize_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        "test_resize",
                                    );
                                }
                            }
                            TestEvent::TapText { text } => {
                                let resp = handle_tap_text(&text, &mut runtime, &pipeline);
                                if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                                    invalidations.mark_build();
                                    if process_pending_effects(
                                        &mut runtime,
                                        &effect_result_tx,
                                        &event_proxy,
                                        app_effect_handler.as_ref(),
                                    ) {
                                        invalidations.mark_build();
                                    }
                                }
                                if let Some(ref tx) = test_response_tx {
                                    let _ = tx.send(resp);
                                }
                                request_redraw_logged(
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "test_tap_text",
                                );
                            }
                            TestEvent::Screenshot { path } => {
                                pending_screenshot_path = Some(path);
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
                                // Schedule a frame and respond after it renders.
                                pending_screenshot_path = Some("__pump__".into());
                                window.request_redraw();
                            }
                            TestEvent::Wake => {}
                            TestEvent::Wait { ms: _ } => {
                                // Wait is handled on the server thread; this
                                // should not arrive here. If it does, just
                                // respond OK.
                                if let Some(ref tx) = test_response_tx {
                                    let _ = tx.send(fission_test_driver::TestResponse::Ok {});
                                }
                            }
                            TestEvent::Quit => {
                                elwt.exit();
                            }
                        }
                    }

                    // ═══════════════════════════════════════════════════════
                    // AboutToWait — idle / animation / blink / effects
                    // ═══════════════════════════════════════════════════════
                    Event::AboutToWait => {
                        let now = Instant::now();

                        // Video Logic
                        let surfaces = pipeline.take_video_surfaces();
                        let mut active_nodes = std::collections::HashSet::new();

                        for surface in &surfaces {
                            active_nodes.insert(surface.widget_id);

                            // Create player if missing
                            if !players.contains_key(&surface.widget_id) {
                                if let Some(state) = runtime.runtime_state.video.states.get(&surface.widget_id) {
                                    let source = &state.asset_source;
                                    if !source.is_empty() {
                                        let player = video_backend.create_player(source);
                                        if let Some(state) = runtime.runtime_state.video.states.get_mut(&surface.widget_id) {
                                            state.surface_id = Some(player.surface_id());
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
                            }
                        }

                        // Cleanup inactive players
                        players.retain(|id, _| active_nodes.contains(id));

                        // Update backend
                        video_backend.present_surfaces(&surfaces);

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
                        let repeat_animation_interval = if pending_resize.is_some() {
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

                        let focused_text_input = focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
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
                        let effect_results_dispatched = drain_effect_results(&mut runtime, &effect_result_rx);
                        if effect_results_dispatched {
                            invalidations.mark_build();
                            // Background work completed — process any new effects
                            // the continuation reducers may have emitted.
                            if process_pending_effects(&mut runtime, &effect_result_tx, &event_proxy, app_effect_handler.as_ref()) {
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
                                || pending_resize.is_some();
                        let active_keys = active_animation_keys(&runtime);

                        if has_pending_work {
                            let pending_frame = pending_work_redraw_interval(
                                invalidations,
                                pending_resize.is_some(),
                                min_frame,
                                resize_frame,
                            );
                            let redraw_reason = if pending_resize.is_some() {
                                "pending_resize"
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
                                    pending_resize.is_some(),
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
                                    pending_resize.is_some(),
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
                            elwt.set_control_flow(ControlFlow::Wait);
                        }
                    }

                    // ═══════════════════════════════════════════════════════
                    // WindowEvent — real user interaction
                    // ═══════════════════════════════════════════════════════
                    Event::WindowEvent { window_id, event } if window_id == window.id() => {
                        match event {
                            WindowEvent::Resized(size) => {
                                if size.width > 0 && size.height > 0 {
                                    pending_resize = Some(size);
                                    live_resize.note_resize(Instant::now());
                                    invalidations.mark_composite();
                                    request_redraw_logged(
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        resize_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        "window_resized",
                                    );
                                }
                            }
                            WindowEvent::ScaleFactorChanged { .. } => {
                                pending_resize = Some(window.inner_size());
                                live_resize.note_resize(Instant::now());
                                invalidations.mark_composite();
                                request_redraw_logged(
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    resize_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "scale_factor_changed",
                                );
                            }
                            WindowEvent::RedrawRequested => {
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
                                if process_pending_effects(&mut runtime, &effect_result_tx, &event_proxy, app_effect_handler.as_ref()) {
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
                                let swapchain_size =
                                    pending_resize.unwrap_or_else(|| window.inner_size());
                                if swapchain_size.width == 0 || swapchain_size.height == 0 {
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }

                                if swapchain_size.width != surface.config.width
                                    || swapchain_size.height != surface.config.height
                                {
                                    render_cx.resize_surface(
                                        &mut surface,
                                        swapchain_size.width,
                                        swapchain_size.height,
                                    );
                                    let device_handle = &render_cx.devices[surface.dev_id];
                                    surface.config.alpha_mode = wgpu::CompositeAlphaMode::PostMultiplied;
                                    surface.surface.configure(&device_handle.device, &surface.config);
                                }

                                let render_target_size =
                                    simulated_viewport.unwrap_or((swapchain_size.width, swapchain_size.height));
                                if render_target_size != target_texture_size {
                                    recreate_target_texture(
                                        &mut surface,
                                        &render_cx,
                                        render_target_size.0,
                                        render_target_size.1,
                                    );
                                    target_texture_size = render_target_size;
                                }

                                let scale_factor = window.scale_factor();
                                let pending_layout_viewport = if let Some((sw, sh)) = simulated_viewport {
                                    (sw as f32, sh as f32)
                                } else {
                                    (
                                        (swapchain_size.width as f64 / scale_factor) as f32,
                                        (swapchain_size.height as f64 / scale_factor) as f32,
                                    )
                                };

                                let force_resize_layout = invalidations.build
                                    || pipeline.prev_ir.is_none()
                                    || pipeline.last_snapshot.is_none();
                                let apply_resize_layout = if pending_resize.is_some() {
                                    live_resize.should_apply_layout(
                                        now,
                                        pipeline.last_snapshot.is_some(),
                                        force_resize_layout,
                                    )
                                } else {
                                    force_resize_layout
                                };
                                let resize_settled =
                                    pending_resize.is_some() && !live_resize.is_live(now);
                                let target_viewport = LayoutSize {
                                    width: pending_layout_viewport.0,
                                    height: pending_layout_viewport.1,
                                };
                                let viewport_changed = pipeline
                                    .last_viewport
                                    .map(|viewport| viewport.size != target_viewport)
                                    .unwrap_or(true);
                                if pending_resize.is_some()
                                    && apply_resize_layout
                                    && viewport_changed
                                {
                                    invalidations.mark_build();
                                }
                                if resize_settled && apply_resize_layout {
                                    invalidations.mark_build();
                                }

                                let retained_viewport = pipeline
                                    .last_viewport
                                    .map(|viewport| viewport.size)
                                    .unwrap_or(target_viewport);
                                let viewport = if apply_resize_layout {
                                    target_viewport
                                } else {
                                    retained_viewport
                                };
                                env.viewport_size = viewport;

                                if let Some(sync) = &self.sync_env {
                                    let state = runtime.get_app_state::<S>().unwrap();
                                    sync(state, &mut env);
                                }

                                if invalidations.build || pipeline.prev_ir.is_none() {
                                    let (node_tree, registry, anims, videos, web_views, portals) = {
                                        let state = runtime.get_app_state::<S>().unwrap();
                                        let view = View::new(
                                            state,
                                            &runtime.runtime_state,
                                            &env,
                                            pipeline.last_snapshot.as_ref(),
                                        );
                                        let mut ctx = BuildCtx::new();
                                        let node = root_widget.build(&mut ctx, &view);
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
                                        (node, ctx.registry, anims, videos, web_views, portals)
                                    };

                                    runtime.clear_reducers();
                                    runtime.absorb_registry(registry);
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
                                }

                                let layout_updates = match pipeline.ensure_layout(
                                    LayoutRect::new(0.0, 0.0, viewport.width, viewport.height),
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
                                    LayoutSize {
                                        width: pending_layout_viewport.0,
                                        height: pending_layout_viewport.1,
                                    },
                                    viewport,
                                    pending_resize.is_some() && !apply_resize_layout,
                                    &runtime.runtime_state.scroll,
                                    &runtime.runtime_state.animation,
                                    &runtime.runtime_state.video,
                                    &runtime.runtime_state.web,
                                ) {
                                    Ok(_stats) => {
                                        let surface_texture = surface
                                            .surface
                                            .get_current_texture()
                                            .expect("failed to get texture");
                                        let device_handle = &render_cx.devices[surface.dev_id];

                                        let texture_plans = pipeline.texture_compositor_plans();
                                        let clear_color = vello::wgpu::Color {
                                            r: env.theme.tokens.colors.background.r as f64 / 255.0,
                                            g: env.theme.tokens.colors.background.g as f64 / 255.0,
                                            b: env.theme.tokens.colors.background.b as f64 / 255.0,
                                            a: env.theme.tokens.colors.background.a as f64 / 255.0,
                                        };
                                        let texture_plans_fit_limits = texture_plans_fit_device_limits(
                                            texture_plans,
                                            scale_factor,
                                            device_handle.device.limits().max_texture_dimension_2d,
                                        );
                                        let has_active_scroll_offsets = runtime
                                            .runtime_state
                                            .scroll
                                            .offsets
                                            .values()
                                            .any(|offset| offset.abs() > 0.5);
                                        let enable_texture_compositor =
                                            std::env::var("FISSION_ENABLE_TEXTURE_COMPOSITOR")
                                                .ok()
                                                .as_deref()
                                                == Some("1");
                                        if !enable_texture_compositor
                                            || texture_plans.is_empty()
                                            || !texture_plans_fit_limits
                                            || has_active_scroll_offsets
                                        {
                                            let render_params = vello::RenderParams {
                                                base_color: vello::peniko::Color::from_rgba8(
                                                    env.theme.tokens.colors.background.r,
                                                    env.theme.tokens.colors.background.g,
                                                    env.theme.tokens.colors.background.b,
                                                    env.theme.tokens.colors.background.a,
                                                ),
                                                width: render_target_size.0,
                                                height: render_target_size.1,
                                                antialiasing_method: vello::AaConfig::Area,
                                            };

                                            scene.reset();
                                            let retained_scene = pipeline
                                                .retained_scene()
                                                .expect("retained render scene missing before render");
                                            let mut renderer_wrapper = VelloRenderer::new(
                                                &mut scene,
                                                measurer.clone(),
                                                &mut retained_scene_cache,
                                                scale_factor,
                                            );
                                            renderer_wrapper
                                                .render_scene(retained_scene)
                                                .expect("failed to encode retained scene");
                                            vello_renderer
                                                .render_to_texture(
                                                    &device_handle.device,
                                                    &device_handle.queue,
                                                    &scene,
                                                    &surface.target_view,
                                                    &render_params,
                                                )
                                                .expect("failed to render");
                                        } else {
                                            let force_full_compositor_redraw = invalidations.build
                                                || invalidations.layout
                                                || invalidations.paint;
                                            let _compositor_stats = texture_compositor
                                                .render_layers(
                                                    &device_handle.device,
                                                    &device_handle.queue,
                                                    &mut vello_renderer,
                                                    &mut retained_scene_cache,
                                                    measurer.clone(),
                                                    scale_factor,
                                                    render_target_size.0,
                                                    render_target_size.1,
                                                    pipeline.texture_compositor_root_transform(),
                                                    texture_plans,
                                                    force_full_compositor_redraw,
                                                    clear_color,
                                                    &surface.target_view,
                                                )
                                                .expect("failed to composite texture layers");
                                        }

                                        for (_, _rect, payload) in &pipeline.scene_3d_surfaces {
                                            if let Ok(primitives) =
                                                bincode::deserialize::<Vec<fission_3d::Primitive3D>>(payload)
                                            {
                                                let scene3d = fission_3d::Scene3D {
                                                    width: Some(render_target_size.0 as f32),
                                                    height: Some(render_target_size.1 as f32),
                                                    primitives,
                                                };
                                                scene3d_renderer.render(
                                                    &device_handle.device,
                                                    &device_handle.queue,
                                                    &surface.target_view,
                                                    &scene3d,
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

                                        surface.blitter.copy(
                                            &device_handle.device,
                                            &mut encoder,
                                            &surface.target_view,
                                            &surface_view,
                                        );

                                        device_handle.queue.submit(Some(encoder.finish()));

                                        if let Some(path) = pending_screenshot_path.take() {
                                            if let Some(ref tx) = test_response_tx {
                                                if path == "__pump__" {
                                                    let _ =
                                                        tx.send(fission_test_driver::TestResponse::Ok {});
                                                } else {
                                                    let resp = gpu_screenshot(
                                                        &device_handle.device,
                                                        &device_handle.queue,
                                                    &surface.target_texture,
                                                        render_target_size.0,
                                                        render_target_size.1,
                                                        &path,
                                                    );
                                                    let _ = tx.send(resp);
                                                }
                                            }
                                        }

                                        surface_texture.present();
                                        if apply_resize_layout {
                                            pending_resize = None;
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
                                    x, y,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, &event_proxy, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                            }
                            WindowEvent::MouseInput { state, button, .. } => {
                                if let Some(position) = last_cursor_position {
                                    let scale_factor = window.scale_factor();
                                    let x = (position.x / scale_factor) as f32;
                                    let y = (position.y / scale_factor) as f32;
                                    if let Some(btn) = map_mouse_button(button) {
                                        let is_pressed = state.is_pressed();
                                        handle_mouse_button(
                                            x, y, btn, is_pressed,
                                            &mut runtime, &pipeline,
                                            &effect_result_tx, &event_proxy, app_effect_handler.as_ref(),
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
                                        point_x, point_y, dx, dy,
                                        &mut runtime, &pipeline,
                                        &effect_result_tx, &event_proxy, app_effect_handler.as_ref(),
                                        &window, elwt,
                                        &mut last_redraw_at, min_frame, &mut redraw_pending,
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
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
                                        Key::Named(NamedKey::Tab) => Some(KeyCode::Tab),
                                        Key::Named(NamedKey::ArrowLeft) => Some(KeyCode::Left),
                                        Key::Named(NamedKey::ArrowRight) => Some(KeyCode::Right),
                                        Key::Named(NamedKey::ArrowUp) => Some(KeyCode::Up),
                                        Key::Named(NamedKey::ArrowDown) => Some(KeyCode::Down),
                                        Key::Named(NamedKey::Home) => Some(KeyCode::Home),
                                        Key::Named(NamedKey::End) => Some(KeyCode::End),
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
                                            &effect_result_tx, &event_proxy, app_effect_handler.as_ref(),
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
                                        if process_pending_effects(&mut runtime, &effect_result_tx, &event_proxy, app_effect_handler.as_ref()) {
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

fn gpu_screenshot(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    path: &str,
) -> fission_test_driver::TestResponse {
    if width == 0 || height == 0 {
        return fission_test_driver::TestResponse::Error {
            message: "zero-size viewport".into(),
        };
    }

    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
    let buffer_size = (padded_bytes_per_row * height) as u64;

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
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
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
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + (width * bytes_per_pixel) as usize;
        rgba.extend_from_slice(&data[start..end]);
    }

    drop(data);
    staging.unmap();

    match image::save_buffer(path, &rgba, width, height, image::ColorType::Rgba8) {
        Ok(()) => fission_test_driver::TestResponse::Ok {},
        Err(e) => fission_test_driver::TestResponse::Error {
            message: format!("PNG save failed: {}", e),
        },
    }
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
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let new_view = new_texture.create_view(&wgpu::TextureViewDescriptor::default());
    surface.target_texture = new_texture;
    surface.target_view = new_view;
}

#[cfg(test)]
mod tests {
    use super::{
        animation_redraw_interval, repeating_animation_redraw_interval,
        texture_plans_fit_device_limits, LiveResizeController,
    };
    use crate::pipeline::CompositorTexturePlan;
    use fission_core::env::{ActiveAnimation, AnimationStateMap};
    use fission_core::{AnimationPropertyId, WidgetNodeId};
    use fission_layout::LayoutRect;
    use std::collections::HashMap;
    use std::time::Duration;

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
    fn live_resize_defers_layout_until_settled() {
        let settle = Duration::from_millis(90);
        let mut resize = LiveResizeController::new(settle);
        let now = std::time::Instant::now();
        resize.note_resize(now);

        assert!(resize.is_live(now + Duration::from_millis(30)));
        assert!(resize.should_apply_layout(now + Duration::from_millis(30), true, false));
        assert!(!resize.should_apply_layout(now + Duration::from_millis(40), true, false));
        assert!(resize.should_apply_layout(now + Duration::from_millis(95), true, false));
    }

    #[test]
    fn live_resize_force_path_bypasses_settle_delay() {
        let settle = Duration::from_millis(90);
        let mut resize = LiveResizeController::new(settle);
        let now = std::time::Instant::now();
        resize.note_resize(now);

        assert!(resize.should_apply_layout(now + Duration::from_millis(10), true, true));
    }

    #[test]
    fn live_resize_refreshes_layout_periodically_while_dragging() {
        let settle = Duration::from_millis(90);
        let mut resize = LiveResizeController::new(settle);
        let now = std::time::Instant::now();
        resize.note_resize(now);

        assert!(resize.should_apply_layout(now, true, false));
        assert!(!resize.should_apply_layout(now + Duration::from_millis(8), true, false));
        assert!(resize.should_apply_layout(now + Duration::from_millis(20), true, false));
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
}
