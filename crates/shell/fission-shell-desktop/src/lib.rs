use anyhow::Result;
use std::collections::{HashMap, VecDeque};
use std::num::NonZeroU32;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    keyboard::PhysicalKey,
    window::{Window, WindowBuilder},
};

use fission_core::env::{VideoState, VideoStateMap, VideoStatus};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::{
    Action, ActionId, AppState, BuildCtx, Clock, Env, ImeHandler, InputEvent, KeyCode,
    KeyEvent as FissionKeyEvent, Lower, Node, PointerButton, PointerEvent, Runtime, ScrollStateMap,
    View, Widget,
};
use fission_core::{ActionInput, Effect, EffectPayload, SystemEffect};
use fission_diagnostics::prelude as diag;
use fission_ir::{op::Color as IrColor, CoreIR, FlexDirection, NodeId, Op, PaintOp, WidgetNodeId};
use fission_layout::{LayoutEngine, LayoutSize};
use fission_render::{
    Color as RenderColor, DisplayList, LayoutPoint, LayoutRect, LayoutUnit, Renderer,
};
use fission_render_vello::parley::FontContext;
use fission_render_vello::{VelloRenderer, VelloTextMeasurer};
use fission_shell::{Platform, VideoBackend, VideoEvent, VideoPlayer};
use fission_theme::fonts;
use fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache};

use fission_test_driver::TestEvent;

// Vello / WGPU
use pollster::block_on;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, AaSupport, Renderer as VelloSceneRenderer, RendererOptions, Scene};

mod pipeline;
pub use pipeline::Pipeline;
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
/// plus the envelope metadata needed to send a result back on the channel.
pub type AppEffectHandler = Box<
    dyn Fn(Vec<u8>, u64, Option<ActionEnvelope>, Option<ActionEnvelope>, mpsc::Sender<EffectResult>)
        + Send
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

/// Drain pending effects from the runtime and either execute them synchronously
/// (fire-and-forget effects like `OpenUrl`) or spawn background threads for I/O
/// effects (`FileRead`, `HttpGet`) and send results back through `effect_tx`.
///
/// Returns `true` if any synchronous callback was dispatched (caller should redraw).
fn process_pending_effects(
    runtime: &mut Runtime,
    effect_tx: &mpsc::Sender<EffectResult>,
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
                        let on_ok = env.on_ok.clone();
                        let on_err = env.on_err.clone();
                        let req_id = env.req_id;
                        let path = path.clone();
                        std::thread::spawn(move || {
                            match std::fs::read_to_string(&path) {
                                Ok(content) => {
                                    let payload = EffectPayload::InlineBytes(content.into_bytes());
                                    let _ = tx.send((req_id, Ok(payload), on_ok, on_err));
                                }
                                Err(e) => {
                                    let _ = tx.send((req_id, Err(e.to_string()), on_ok, on_err));
                                }
                            }
                        });
                    }

                    // ── Background: HttpGet ───────────────────────────────────
                    SystemEffect::HttpGet { url, headers } => {
                        let tx = effect_tx.clone();
                        let on_ok = env.on_ok.clone();
                        let on_err = env.on_err.clone();
                        let req_id = env.req_id;
                        let url = url.clone();
                        let headers = headers.clone();
                        std::thread::spawn(move || {
                            // Minimal blocking HTTP GET using std only (no external crate).
                            // This parses the URL, opens a TCP stream, and reads the response.
                            let result = (|| -> std::result::Result<Vec<u8>, String> {
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
                            })();

                            match result {
                                Ok(bytes) => {
                                    let payload = EffectPayload::InlineBytes(bytes);
                                    let _ = tx.send((req_id, Ok(payload), on_ok, on_err));
                                }
                                Err(msg) => {
                                    let _ = tx.send((req_id, Err(msg), on_ok, on_err));
                                }
                            }
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
fn drain_effect_results(
    runtime: &mut Runtime,
    effect_rx: &mpsc::Receiver<EffectResult>,
) -> bool {
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
        let handle_ms = handled_at
            .duration_since(trace.started_at)
            .as_secs_f64()
            * 1000.0;
        let effects_ms = effects_at
            .duration_since(handled_at)
            .as_secs_f64()
            * 1000.0;
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
    app_effect_handler: Option<&AppEffectHandler>,
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let point = LayoutPoint { x, y };
        let event = InputEvent::Pointer(PointerEvent::Move { point });
        if let Err(e) = runtime.handle_input(event, ir, layout) {
            eprintln!("Input handling error: {:?}", e);
        }
        if process_pending_effects(runtime, effect_result_tx, app_effect_handler) {
            request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
        }
        request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
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

        mark_text_trace_handled(pending_text_traces, trace_seq);
        if process_pending_effects(runtime, effect_result_tx, app_effect_handler) {
            mark_text_trace_effects(pending_text_traces, trace_seq);
            request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
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
        request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
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
    app_effect_handler: Option<&AppEffectHandler>,
    window: &Window,
    elwt: &EventLoopWindowTarget<TestEvent>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let point = LayoutPoint { x: point_x, y: point_y };
        let scroll_delta = LayoutPoint { x: delta_x, y: delta_y };
        let event = InputEvent::Pointer(PointerEvent::Scroll { point, delta: scroll_delta });
        if let Err(e) = runtime.handle_input(event, ir, layout) {
            eprintln!("Scroll error: {:?}", e);
        }
        if process_pending_effects(runtime, effect_result_tx, app_effect_handler) {
            request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
        }
        request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
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
                if process_pending_effects(runtime, effect_result_tx, app_effect_handler) {
                    request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
                }
                request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
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
        mark_text_trace_handled(pending_text_traces, trace_seq);
        if process_pending_effects(runtime, effect_result_tx, app_effect_handler) {
            mark_text_trace_effects(pending_text_traces, trace_seq);
            request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
        }
        reset_text_input_caret(runtime, pipeline.prev_ir.as_ref(), last_blink_toggle);
        request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
    }

    false
}

/// Build the response for a GetText query.
fn build_get_text_response(pipeline: &Pipeline) -> fission_test_driver::TestResponse {
    use fission_test_driver::{TextItem, TestResponse};
    let mut items = Vec::new();
    if let (Some(ir), Some(snap)) = (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref()) {
        for (id, node) in &ir.nodes {
            let text_content = match &node.op {
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) => Some(text.clone()),
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) => {
                    Some(runs.iter().map(|r| r.text.clone()).collect::<String>())
                }
                _ => None,
            };
            if let Some(text) = text_content {
                if text.is_empty() { continue; }
                let check_id = node.parent.unwrap_or(*id);
                let rect = snap.get_node_rect(check_id).or_else(|| snap.get_node_rect(*id));
                let (x, y, w, h) = rect.map(|r| (r.x(), r.y(), r.width(), r.height())).unwrap_or((0.0, 0.0, 0.0, 0.0));
                items.push(TextItem { text, x, y, width: w, height: h });
            }
        }
    }
    TestResponse::Text { items }
}

/// Build the response for a GetTree query.
fn build_get_tree_response(pipeline: &Pipeline) -> fission_test_driver::TestResponse {
    use fission_test_driver::{SemanticNode, TestResponse};
    let mut nodes = Vec::new();
    if let Some(ir) = &pipeline.prev_ir {
        for (id, node) in &ir.nodes {
            if let fission_ir::Op::Semantics(sem) = &node.op {
                let rect = pipeline.last_snapshot.as_ref()
                    .and_then(|s| s.get_node_rect(*id));
                let (x, y, w, h) = rect.map(|r| (r.x(), r.y(), r.width(), r.height())).unwrap_or((0.0, 0.0, 0.0, 0.0));
                nodes.push(SemanticNode {
                    role: format!("{:?}", sem.role),
                    label: sem.label.clone(),
                    value: sem.value.clone(),
                    focusable: sem.focusable,
                    x, y, width: w, height: h,
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
        let mut found = None;
        for (id, node) in &ir.nodes {
            let txt = match &node.op {
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text: t, .. }) => Some(t.as_str()),
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) => {
                    let combined: String = runs.iter().map(|r| r.text.clone()).collect();
                    if combined.contains(text) { Some("") } else { None }
                }
                _ => None,
            };
            if let Some(t) = txt {
                if t.contains(text) || t.is_empty() {
                    let check_id = node.parent.unwrap_or(*id);
                    if let Some(rect) = snap.get_node_rect(check_id).or_else(|| snap.get_node_rect(*id)) {
                        found = Some((rect.x() + rect.width() / 2.0, rect.y() + rect.height() / 2.0));
                        break;
                    }
                }
            }
        }
        if let Some((cx, cy)) = found {
            let point = LayoutPoint::new(cx, cy);
            let _ = runtime.handle_input(InputEvent::Pointer(PointerEvent::Down { point, button: PointerButton::Primary }), ir, snap);
            let _ = runtime.handle_input(InputEvent::Pointer(PointerEvent::Up { point, button: PointerButton::Primary }), ir, snap);
            TestResponse::Ok {}
        } else {
            TestResponse::Error { message: format!("text '{}' not found", text) }
        }
    } else {
        TestResponse::Error { message: "no frame rendered yet".into() }
    }
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
        F: Fn(Vec<u8>, u64, Option<ActionEnvelope>, Option<ActionEnvelope>, mpsc::Sender<EffectResult>)
            + Send
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

    pub fn run(mut self) -> Result<()> {
        diag::emit(
            diag::DiagCategory::Frame,
            diag::DiagLevel::Info,
            diag::DiagEventKind::FrameStart { root: None },
        );
        diag::init_from_env();

        // Build event loop with TestEvent as the user event type.
        // This allows the test control server to inject events via EventLoopProxy.
        let event_loop = EventLoopBuilder::<TestEvent>::with_user_event()
            .build()
            .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))?;
        let event_proxy = event_loop.create_proxy();

        let window = Arc::new(
            WindowBuilder::new()
                .with_title(&self.title)
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
        recreate_target_texture(&mut surface, &render_cx);

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
        let mut presented_frames: u64 = 0;
        let mut next_text_trace_seq: u64 = 0;
        let mut pending_text_traces: VecDeque<PendingTextTrace> = VecDeque::new();

        let mut current_mods: u8 = 0;

        // Test control (enabled via FISSION_TEST_CONTROL_PORT env var).
        // The TCP server injects TestEvents via the EventLoopProxy and receives
        // query responses through a dedicated mpsc channel.
        let test_response_tx: Option<test_control::ResponseSender> = std::env::var("FISSION_TEST_CONTROL_PORT")
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
                                    &effect_result_tx, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                );
                            }
                            TestEvent::MouseDown { x, y, button } => {
                                let btn = map_test_button(button);
                                handle_mouse_button(
                                    x, y, btn, true,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                    text_trace_enabled, &mut pending_text_traces,
                                    &mut next_text_trace_seq, presented_frames,
                                    &mut last_blink_toggle,
                                );
                            }
                            TestEvent::MouseUp { x, y, button } => {
                                let btn = map_test_button(button);
                                handle_mouse_button(
                                    x, y, btn, false,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                    text_trace_enabled, &mut pending_text_traces,
                                    &mut next_text_trace_seq, presented_frames,
                                    &mut last_blink_toggle,
                                );
                            }
                            TestEvent::KeyDown { key_code, modifiers } => {
                                let code = parse_key_code(&key_code);
                                handle_key_down::<S>(
                                    code, modifiers,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                    text_trace_enabled, &mut pending_text_traces,
                                    &mut next_text_trace_seq, presented_frames,
                                    &mut last_blink_toggle,
                                    self.key_handler.as_ref(),
                                );
                            }
                            TestEvent::KeyUp { .. } => {
                                // Key-up is currently a no-op in the framework
                                // (only key-down dispatches actions), but we request
                                // a redraw so any state changes are rendered.
                                request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
                            }
                            TestEvent::TextInput { text } => {
                                // Type each character via handle_key_down so custom
                                // render objects receive the events (same path as real
                                // keyboard input).
                                for ch in text.chars() {
                                    let key = if ch == ' ' { KeyCode::Space } else if ch == '\n' { KeyCode::Enter } else { KeyCode::Char(ch) };
                                    handle_key_down::<S>(
                                        key, 0,
                                        &mut runtime, &pipeline,
                                        &effect_result_tx, app_effect_handler.as_ref(),
                                        &window, elwt,
                                        &mut last_redraw_at, min_frame, &mut redraw_pending,
                                        text_trace_enabled, &mut pending_text_traces,
                                        &mut next_text_trace_seq, presented_frames,
                                        &mut last_blink_toggle,
                                        self.key_handler.as_ref(),
                                    );
                                }
                            }
                            TestEvent::Scroll { x, y, dx, dy } => {
                                handle_scroll(
                                    x, y, dx, dy,
                                    &mut runtime, &pipeline,
                                    &effect_result_tx, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
                                );
                            }
                            TestEvent::Resize { width, height } => {
                                if width > 0 && height > 0 {
                                    // Store the simulated size so layout uses
                                    // it instead of window.inner_size().
                                    simulated_viewport = Some((width, height));
                                    pipeline.last_viewport = None;
                                    window.request_redraw();
                                }
                            }
                            TestEvent::TapText { text } => {
                                let resp = handle_tap_text(&text, &mut runtime, &pipeline);
                                if let Some(ref tx) = test_response_tx {
                                    let _ = tx.send(resp);
                                }
                                request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
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
                        let dt = now.duration_since(last_frame_time);
                        last_frame_time = now;

                        // Tick Runtime (Animations)
                        let dt_ms = dt.as_millis() as u64;
                        if let Err(e) = runtime.tick(dt_ms) {
                            eprintln!("Runtime tick error: {:?}", e);
                        }

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
                                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
                                        },
                                        VideoEvent::Error(e) => {
                                            eprintln!("Video playback error for {:?}: {:?}", widget_id, e);
                                            video_state.status = VideoStatus::Error;
                                            active_player.last_status = Some(VideoStatus::Error);
                                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
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

                        // Check if we need a redraw (Animation or Video playing)
                        // Only force continuous redraws for non-repeating animations
                        // or active video players. Repeating animations (spinners, skeleton
                        // shimmer) should not burn CPU when idle.
                        let has_finite_animation = runtime.runtime_state.animation.active.values()
                            .any(|a| !a.repeat);
                        let needs_redraw = has_finite_animation || !players.is_empty();

                        let focused_text_input = focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                        if focused_text_input != blink_focus_id {
                            if let Some(prev) = blink_focus_id {
                                runtime.runtime_state.caret_visible.remove(&prev);
                            }
                            blink_focus_id = focused_text_input;
                            if let Some(id) = blink_focus_id {
                                runtime.runtime_state.caret_visible.insert(id, true);
                                last_blink_toggle = now;
                                request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
                            }
                        }

                        // Cursor blink: toggle visibility and request a redraw.
                        if blink_enabled {
                            if let Some(id) = blink_focus_id {
                                if now.duration_since(last_blink_toggle) >= blink_period {
                                    let visible = runtime.runtime_state.caret_visible.get(&id).copied().unwrap_or(true);
                                    runtime.runtime_state.caret_visible.insert(id, !visible);
                                    last_blink_toggle = now;
                                    request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
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
                            // Background work completed — process any new effects
                            // the continuation reducers may have emitted.
                            if process_pending_effects(&mut runtime, &effect_result_tx, app_effect_handler.as_ref()) {
                                request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
                            }
                            window.request_redraw();
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
                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
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

                        if needs_redraw || redraw_pending || effect_results_dispatched || frame_hook_wants_redraw {
                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
                            let mut wake_at = last_redraw_at + min_frame;
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
                            let mut wake_at = blink_at;
                            if let Some(hook_at) = frame_hook_wake_at {
                                if hook_at < wake_at {
                                    wake_at = hook_at;
                                }
                            }
                            elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
                        } else if let Some(hook_at) = frame_hook_wake_at {
                            elwt.set_control_flow(ControlFlow::WaitUntil(hook_at));
                        } else {
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
                                    // Invalidate viewport to force full layout rebuild
                                    pipeline.last_viewport = None;
                                    request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
                                }
                            }
                            WindowEvent::ScaleFactorChanged { .. } => {
                                // Invalidate viewport to force full layout rebuild
                                pipeline.last_viewport = None;
                                request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
                            }
                            WindowEvent::RedrawRequested => {
                                redraw_pending = false;
                                diag::begin_frame(None);
                                // Drain pending effects before building the next frame.
                                // This prevents the effect queue from growing unbounded.
                                if process_pending_effects(&mut runtime, &effect_result_tx, app_effect_handler.as_ref()) {
                                    request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
                                }
                                let size = window.inner_size();
                                if size.width > 0 && size.height > 0 {
                                    if size.width != surface.config.width || size.height != surface.config.height {
                                        render_cx.resize_surface(&mut surface, size.width, size.height);
                                        // Re-apply alpha mode after resize
                                        let device_handle = &render_cx.devices[surface.dev_id];
                                        surface.config.alpha_mode = wgpu::CompositeAlphaMode::PostMultiplied;
                                        surface.surface.configure(&device_handle.device, &surface.config);
                                        // Recreate target texture with COPY_SRC for screenshots
                                        recreate_target_texture(&mut surface, &render_cx);
                                    }

                                    let scale_factor = window.scale_factor();
                                    // Use simulated viewport dimensions when
                                    // a test resize event has been injected,
                                    // otherwise derive from the real window size.
                                    let (layout_width, layout_height) = if let Some((sw, sh)) = simulated_viewport {
                                        (sw as f32, sh as f32)
                                    } else {
                                        (
                                            (size.width as f64 / scale_factor) as f32,
                                            (size.height as f64 / scale_factor) as f32,
                                        )
                                    };
                                    env.viewport_size = LayoutSize {
                                        width: layout_width,
                                        height: layout_height,
                                    };

                                    if let Some(sync) = &self.sync_env {
                                        let state = runtime.get_app_state::<S>().unwrap();
                                        sync(state, &mut env);
                                    }

                                    let (node_tree, registry, anims, videos, web_views, portals) = {
                                        let state = runtime.get_app_state::<S>().unwrap();
                                        let view = View::new(state, &runtime.runtime_state, &env, pipeline.last_snapshot.as_ref());
                                        let mut ctx = BuildCtx::new();
                                        let node = root_widget.build(&mut ctx, &view);
                                        let anims = ctx.take_animation_requests();
                                        let videos = ctx.take_video_registrations();
                                        let web_views = ctx.take_web_registrations();
                                        let portals_with_ids = ctx.take_portals();

                                        let portals = portals_with_ids.into_iter().map(|(id, node)| {
                                            if let Some(id) = id {
                                                // Use a derived ID for the wrapper to avoid conflict with the widget's own node
                                                let wrapper_id = fission_core::NodeId::derived(id.as_u128(), &[0x0000_F001]);
                                                fission_core::ui::Container::new(node)
                                                    .id(wrapper_id)
                                                    .width(env.viewport_size.width)
                                                    .height(env.viewport_size.height)
                                                    .into_node()
                                            } else {
                                                node
                                            }
                                        }).collect::<Vec<_>>();

                                        // Emit portal summary to diagnostics
                                        {
                                            use fission_diagnostics::prelude as diag;
                                            diag::emit(
                                                diag::DiagCategory::Layout,
                                                diag::DiagLevel::Debug,
                                                diag::DiagEventKind::PortalsComposed { portal_count: portals.len() as u32 },
                                            );
                                        }
                                        (node, ctx.registry, anims, videos, web_views, portals)
                                    };

                                    runtime.clear_reducers();
                                    runtime.absorb_registry(registry);
                                    for (target, req) in anims {
                                        runtime.enqueue_animation(target, req);
                                    }
                                    runtime.sync_video_nodes(&videos);
                                    runtime.sync_web_nodes(&web_views);

                                    // Always compose an overlay layer above content.
                                    // Portals are injected into that layer and never
                                    // participate in normal layout.
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
                                        }
                                    );

                                    let mut lower_cx = LoweringContext::new(&env, &runtime.runtime_state, runtime.measurer.as_ref(), pipeline.last_snapshot.as_ref());
                                    let root_id = final_root.lower(&mut lower_cx);
                                    lower_cx.ir.root = Some(root_id);
                                    let cx_ir = lower_cx.ir;

                                    let viewport = LayoutSize {
                                        width: layout_width,
                                        height: layout_height,
                                    };

                                    // Vello Rendering
                                    scene.reset();

                                    let mut renderer_wrapper = VelloRenderer::new(&mut scene, measurer.clone(), scale_factor);

                                    match pipeline.render(
                                        cx_ir,
                                        viewport,
                                        &mut layout_engine,
                                        &runtime.runtime_state.scroll,
                                        &mut renderer_wrapper,
                                        &runtime.runtime_state.video,
                                        &runtime.runtime_state.web,
                                        &env,
                                    ) {
                                        Ok(_stats) => {
                                            let surface_texture = surface.surface.get_current_texture().expect("failed to get texture");
                                            let device_handle = &render_cx.devices[surface.dev_id];

                                            let render_params = vello::RenderParams {
                                                base_color: vello::peniko::Color::from_rgb8(30, 30, 30),
                                                width: size.width,
                                                height: size.height,
                                                antialiasing_method: vello::AaConfig::Area,
                                            };

                                            vello_renderer.render_to_texture(
                                                &device_handle.device,
                                                &device_handle.queue,
                                                &scene,
                                                &surface.target_view,
                                                &render_params,
                                            ).expect("failed to render");

                                            for (_, _rect, payload) in &pipeline.scene_3d_surfaces {
                                                if let Ok(primitives) = bincode::deserialize::<Vec<fission_3d::Primitive3D>>(payload) {
                                                    let scene3d = fission_3d::Scene3D {
                                                        width: Some(size.width as f32),
                                                        height: Some(size.height as f32),
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

                                            let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

                                            let mut encoder = device_handle.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                                label: Some("Surface Blit"),
                                            });

                                            surface.blitter.copy(
                                                &device_handle.device,
                                                &mut encoder,
                                                &surface.target_view,
                                                &surface_view,
                                            );

                                            device_handle.queue.submit(Some(encoder.finish()));

                                            // GPU screenshot / pump response BEFORE present
                                            if let Some(path) = pending_screenshot_path.take() {
                                                if let Some(ref tx) = test_response_tx {
                                                    if path == "__pump__" {
                                                        let _ = tx.send(fission_test_driver::TestResponse::Ok {});
                                                    } else {
                                                        let resp = gpu_screenshot(
                                                            &device_handle.device,
                                                            &device_handle.queue,
                                                            &surface.target_texture,
                                                            size.width,
                                                            size.height,
                                                            &path,
                                                        );
                                                        let _ = tx.send(resp);
                                                    }
                                                }
                                            }

                                            surface_texture.present();

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
                                        }
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
                                    &effect_result_tx, app_effect_handler.as_ref(),
                                    &window, elwt,
                                    &mut last_redraw_at, min_frame, &mut redraw_pending,
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
                                            &effect_result_tx, app_effect_handler.as_ref(),
                                            &window, elwt,
                                            &mut last_redraw_at, min_frame, &mut redraw_pending,
                                            text_trace_enabled, &mut pending_text_traces,
                                            &mut next_text_trace_seq, presented_frames,
                                            &mut last_blink_toggle,
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
                                        &effect_result_tx, app_effect_handler.as_ref(),
                                        &window, elwt,
                                        &mut last_redraw_at, min_frame, &mut redraw_pending,
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
                                            &effect_result_tx, app_effect_handler.as_ref(),
                                            &window, elwt,
                                            &mut last_redraw_at, min_frame, &mut redraw_pending,
                                            text_trace_enabled, &mut pending_text_traces,
                                            &mut next_text_trace_seq, presented_frames,
                                            &mut last_blink_toggle,
                                            self.key_handler.as_ref(),
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
                                        mark_text_trace_handled(&mut pending_text_traces, trace_seq);
                                        if process_pending_effects(&mut runtime, &effect_result_tx, app_effect_handler.as_ref()) {
                                            mark_text_trace_effects(&mut pending_text_traces, trace_seq);
                                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
                                        }
                                        reset_text_input_caret(&mut runtime, pipeline.prev_ir.as_ref(), &mut last_blink_toggle);
                                        request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame, &mut redraw_pending);
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
    staging.slice(..).map_async(wgpu::MapMode::Read, move |result| {
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
) {
    let device = &render_cx.devices[surface.dev_id].device;
    let size = wgpu::Extent3d {
        width: surface.config.width.max(1),
        height: surface.config.height.max(1),
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
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let new_view = new_texture.create_view(&wgpu::TextureViewDescriptor::default());
    surface.target_texture = new_texture;
    surface.target_view = new_view;
}
