use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use fission_test_driver::{TestCommand, TestEvent, TestResponse, WebTestFrame, WebTestFramePhase};
use js_sys::{Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use winit::event_loop::EventLoopProxy;

enum PendingResponse {
    Queued(Result<TestCommand, String>),
    Ready(TestResponse),
    Waiting(Receiver<TestResponse>),
}

const MAX_PENDING_RESPONSES: usize = 1024;
const MAX_COMMAND_JSON_BYTES: usize = 4 * 1024 * 1024;
const POLL_STAGE_ENTERED: u32 = 1;
const POLL_STAGE_RESPONSE_READY: u32 = 2;
const POLL_STAGE_JSON_READY: u32 = 3;

static LAST_POLL_STAGE: AtomicU32 = AtomicU32::new(0);
static WEB_WAKE_TRACE_COUNT: AtomicU32 = AtomicU32::new(0);
static WEB_REDRAW_WAIT_TRACE_COUNT: AtomicU32 = AtomicU32::new(0);
static WEB_EVENT_LOOP_TRACE_COUNT: AtomicU32 = AtomicU32::new(0);
static WEB_LAST_PENDING_WORK_TRACE: AtomicU32 = AtomicU32::new(u32::MAX);
static WEB_LAST_REDRAW_PENDING_TRACE: AtomicU32 = AtomicU32::new(u32::MAX);
static WEB_LAST_REDRAW_REQUEST_TRACE: AtomicU32 = AtomicU32::new(u32::MAX);

#[repr(C)]
struct WebTestDiagnostics {
    stage: u32,
    stack_pointer: u32,
    iteration: u32,
}

struct WebTestDiagnosticsCell(UnsafeCell<WebTestDiagnostics>);

unsafe impl Sync for WebTestDiagnosticsCell {}

static WEB_TEST_DIAGNOSTICS: WebTestDiagnosticsCell =
    WebTestDiagnosticsCell(UnsafeCell::new(WebTestDiagnostics {
        stage: 0,
        stack_pointer: 0,
        iteration: 0,
    }));

/// Record a coarse-grained call-site marker for browser LiveTest crash reports.
/// The browser test shell is single-threaded, so the fields can be read directly
/// from Wasm memory even after a trapped callback has left the stack unusable.
#[cfg(fission_web_test_control)]
#[inline(never)]
pub(crate) fn record_diagnostic(stage: u32, iteration: u32) {
    let marker = 0_u32;
    let stack_pointer = std::ptr::addr_of!(marker) as usize as u32;
    unsafe {
        let diagnostics = WEB_TEST_DIAGNOSTICS.0.get();
        std::ptr::write_volatile(
            std::ptr::addr_of_mut!((*diagnostics).stack_pointer),
            stack_pointer,
        );
        std::ptr::write_volatile(std::ptr::addr_of_mut!((*diagnostics).iteration), iteration);
        std::ptr::write_volatile(std::ptr::addr_of_mut!((*diagnostics).stage), stage);
        std::ptr::read_volatile(std::ptr::addr_of!(marker));
    }

    // LiveTest can become unable to evaluate JavaScript while the Wasm main
    // thread is stuck in synchronous work. Keep useful boundaries visible
    // through CDP, but avoid logging every idle-loop step and every Wake: a
    // trace flood can itself delay browser tasks and obscure the original bug.
    let should_log = match stage {
        300..=353 => true,
        // These markers can run for every browser event-loop iteration. Keep a
        // sparse heartbeat for 360 and report state changes for the other
        // markers so LiveTest does not flood its own CDP event stream while
        // waiting for a command response.
        360 => WEB_EVENT_LOOP_TRACE_COUNT.fetch_add(1, Ordering::Relaxed) % 256 == 0,
        364 => WEB_LAST_PENDING_WORK_TRACE.swap(iteration, Ordering::Relaxed) != iteration,
        365 => WEB_LAST_REDRAW_PENDING_TRACE.swap(iteration, Ordering::Relaxed) != iteration,
        366 => WEB_LAST_REDRAW_REQUEST_TRACE.swap(iteration, Ordering::Relaxed) != iteration,
        400 | 402 | 410 | 411 | 414 | 416 | 417 | 418 | 419 | 420 | 424 | 430 | 432 | 435 => true,
        473 if iteration == 1 => {
            WEB_REDRAW_WAIT_TRACE_COUNT.fetch_add(1, Ordering::Relaxed) % 256 == 0
        }
        437 if iteration == 42 => true,
        437 if iteration == 31 => WEB_WAKE_TRACE_COUNT.fetch_add(1, Ordering::Relaxed) % 4096 == 0,
        450..=470 | 474 | 475 | 476 => true,
        90 | 91 if iteration == 42 => true,
        _ => false,
    };
    if should_log {
        web_sys::console::log_1(&JsValue::from_str(&format!(
            "FISSION_WEB_TEST_TRACE {stage} {iteration}"
        )));
    }
}

#[cfg(not(fission_web_test_control))]
#[inline(always)]
pub(crate) fn record_diagnostic(_stage: u32, _iteration: u32) {}

#[cfg(fission_web_test_control)]
fn diagnostics_address() -> u32 {
    WEB_TEST_DIAGNOSTICS.0.get() as usize as u32
}

// Browser commands are copied from JavaScript into this fixed scratch area.
// Keeping the bridge's command ingress out of wasm-bindgen's string allocator
// is useful in long-running Web LiveTest sessions, where the app itself may
// create many unrelated wasm-bindgen closures.
#[cfg(fission_web_test_control)]
static mut COMMAND_JSON: [u8; MAX_COMMAND_JSON_BYTES] = [0; MAX_COMMAND_JSON_BYTES];

struct WebTestBridgeState {
    next_request_id: u32,
    responses: HashMap<u32, PendingResponse>,
    frame: Option<WebTestFrame>,
    poll_json: Vec<u8>,
    event_proxy: Option<EventLoopProxy<TestEvent>>,
}

impl Default for WebTestBridgeState {
    fn default() -> Self {
        Self {
            next_request_id: 1,
            responses: HashMap::new(),
            frame: None,
            poll_json: Vec::new(),
            event_proxy: None,
        }
    }
}

struct WebTestBridgeStateCell {
    state: UnsafeCell<Option<WebTestBridgeState>>,
}

// Web LiveTest control is compiled only for the single-threaded browser Wasm
// shell. Its callbacks run synchronously on the browser event loop, and the
// closures below never call JavaScript while state is borrowed, so access
// cannot race or re-enter. Keep this state in linear memory rather than Wasm
// TLS: TLS-backed bridge state has trapped in long-running browser sessions.
unsafe impl Sync for WebTestBridgeStateCell {}

static WEB_TEST_BRIDGE_STATE: WebTestBridgeStateCell = WebTestBridgeStateCell {
    state: UnsafeCell::new(None),
};

fn with_bridge_state<R>(f: impl FnOnce(&mut WebTestBridgeState) -> R) -> R {
    let state = unsafe {
        (*WEB_TEST_BRIDGE_STATE.state.get()).get_or_insert_with(WebTestBridgeState::default)
    };
    f(state)
}

fn bridge_state_address() -> u32 {
    std::ptr::addr_of!(WEB_TEST_BRIDGE_STATE) as usize as u32
}

/// Called only after a successful Web renderer submission. The bridge owns
/// this state independently of any particular driver/review workflow.
pub(crate) fn publish_frame(width: f32, height: f32, frame: u64) {
    if cfg!(fission_web_test_control) {
        with_bridge_state(|state| {
            state.frame = Some(WebTestFrame {
                width,
                height,
                frame,
                phase: WebTestFramePhase::Submitted,
            });
        });
    }
}

pub(crate) fn install(proxy: EventLoopProxy<TestEvent>) -> bool {
    if !cfg!(fission_web_test_control) {
        return false;
    }

    with_bridge_state(|state| {
        state.event_proxy = Some(proxy);
    });

    // Invoke scalar WebAssembly functions through small JavaScript adapters.
    // The app-level LiveTest bridge shares one Wasm module with every widget
    // closure in the application. Keeping all bridge calls off wasm-bindgen's
    // closure trampoline avoids coupling long sessions to its externref table
    // and string allocator state.
    let exports = wasm_bindgen::exports();
    let submit_json = Reflect::get(&exports, &JsValue::from_str("fission_web_test_submit_json"))
        .unwrap_or(JsValue::UNDEFINED);
    let command_json_ptr = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_command_json_ptr"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    let command_json_capacity = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_command_json_capacity"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    let poll_json_len = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_poll_json_len"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    let poll_json_ptr = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_poll_json_ptr"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    let frame_present = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_frame_present"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    let frame_width = Reflect::get(&exports, &JsValue::from_str("fission_web_test_frame_width"))
        .unwrap_or(JsValue::UNDEFINED);
    let frame_height = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_frame_height"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    let frame_number_low = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_frame_number_low"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    let frame_number_high = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_frame_number_high"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    let stack_pointer_probe = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_stack_pointer_probe"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    let diagnostics_ptr = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_diagnostics_ptr"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    if [
        &submit_json,
        &command_json_ptr,
        &command_json_capacity,
        &poll_json_len,
        &poll_json_ptr,
        &frame_present,
        &frame_width,
        &frame_height,
        &frame_number_low,
        &frame_number_high,
        &stack_pointer_probe,
        &diagnostics_ptr,
    ]
    .iter()
    .any(|export| !export.is_function())
    {
        return false;
    }

    let memory = wasm_bindgen::memory();
    let dispatch_command = Reflect::get(
        &exports,
        &JsValue::from_str("fission_web_test_dispatch_command"),
    )
    .unwrap_or(JsValue::UNDEFINED);
    if !dispatch_command.is_function() {
        return false;
    }
    let submit_adapter = Function::new_with_args(
        "submitJson,dispatchCommand,commandJsonPtr,commandJsonCapacity,memory",
        "return function(commandJson) { if (typeof commandJson !== 'string') throw new TypeError('command JSON must be a string'); const bytes = new TextEncoder().encode(commandJson); const capacity = commandJsonCapacity() >>> 0; if (bytes.length > capacity) return submitJson(capacity + 1) >>> 0; const address = commandJsonPtr() >>> 0; new Uint8Array(memory.buffer, address, bytes.length).set(bytes); console.log('FISSION_WEB_TEST_TRACE 415 0'); const requestId = submitJson(bytes.length) >>> 0; console.log('FISSION_WEB_TEST_TRACE 416 ' + requestId); if (requestId !== 0) globalThis.setTimeout(() => dispatchCommand(requestId), 0); return requestId; }",
    );
    let Ok(submit) = submit_adapter.call5(
        &JsValue::NULL,
        &submit_json,
        &dispatch_command,
        &command_json_ptr,
        &command_json_capacity,
        &memory,
    ) else {
        return false;
    };

    let poll_adapter = Function::new_with_args(
        "pollJsonLen,pollJsonPtr,memory",
        "return function(requestId) { const length = pollJsonLen(requestId) >>> 0; const address = pollJsonPtr() >>> 0; return new TextDecoder().decode(new Uint8Array(memory.buffer, address, length)); }",
    );
    let Ok(poll) = poll_adapter.call3(
        &JsValue::NULL,
        &poll_json_len,
        &poll_json_ptr,
        &wasm_bindgen::memory(),
    ) else {
        return false;
    };

    let poll_diagnostics_address = std::ptr::addr_of!(LAST_POLL_STAGE) as usize;
    let diagnostics_adapter = Function::new_with_args(
        "memory,address",
        "return function() { return new DataView(memory.buffer).getUint32(address, true); }",
    );
    let Ok(poll_diagnostics) = diagnostics_adapter.call2(
        &JsValue::NULL,
        &memory,
        &JsValue::from_f64(poll_diagnostics_address as f64),
    ) else {
        return false;
    };

    let frame_adapter = Function::new_with_args(
        "isPresent,width,height,frameLow,frameHigh",
        "return function() { if (!isPresent()) return 'null'; const frame = (frameHigh() >>> 0) * 4294967296 + (frameLow() >>> 0); return JSON.stringify({width:width(),height:height(),frame,phase:'submitted'}); }",
    );
    let Ok(frame) = frame_adapter.call5(
        &JsValue::NULL,
        &frame_present,
        &frame_width,
        &frame_height,
        &frame_number_low,
        &frame_number_high,
    ) else {
        return false;
    };

    let diagnostics_address = diagnostics_ptr
        .dyn_ref::<Function>()
        .and_then(|function| function.call0(&JsValue::NULL).ok())
        .unwrap_or(JsValue::UNDEFINED);
    let diagnostics_reader_adapter = Function::new_with_args(
        "memory,address",
        "return function() { const view = new DataView(memory.buffer); const pointer = address >>> 0; return { stage:view.getUint32(pointer,true), stackPointer:view.getUint32(pointer+4,true), iteration:view.getUint32(pointer+8,true) }; }",
    );
    let Ok(diagnostics_reader) =
        diagnostics_reader_adapter.call2(&JsValue::NULL, &memory, &diagnostics_address)
    else {
        return false;
    };

    let descriptor = Object::new();
    if Reflect::set(&descriptor, &JsValue::from_str("get"), &frame).is_err()
        || Reflect::set(
            &descriptor,
            &JsValue::from_str("enumerable"),
            &JsValue::TRUE,
        )
        .is_err()
        || Reflect::set(
            &descriptor,
            &JsValue::from_str("configurable"),
            &JsValue::TRUE,
        )
        .is_err()
    {
        return false;
    }
    let bridge = Object::new();
    Object::define_property(&bridge, &JsValue::from_str("frame"), &descriptor);
    let state_address = bridge_state_address();
    let command_json_address = command_json_ptr
        .dyn_ref::<Function>()
        .and_then(|function| function.call0(&JsValue::NULL).ok())
        .unwrap_or(JsValue::UNDEFINED);
    if Reflect::set(&bridge, &JsValue::from_str("submit"), &submit).is_err()
        || Reflect::set(&bridge, &JsValue::from_str("poll"), &poll).is_err()
        || Reflect::set(
            &bridge,
            &JsValue::from_str("stackPointer"),
            &stack_pointer_probe,
        )
        .is_err()
        || Reflect::set(
            &bridge,
            &JsValue::from_str("diagnostics"),
            &diagnostics_reader,
        )
        .is_err()
        || Reflect::set(
            &bridge,
            &JsValue::from_str("diagnosticsAddress"),
            &diagnostics_address,
        )
        .is_err()
        || Reflect::set(
            &bridge,
            &JsValue::from_str("pollDiagnostics"),
            &poll_diagnostics,
        )
        .is_err()
        || Reflect::set(
            &bridge,
            &JsValue::from_str("pollDiagnosticsAddress"),
            &JsValue::from_f64(poll_diagnostics_address as f64),
        )
        .is_err()
        || Reflect::set(
            &bridge,
            &JsValue::from_str("stateAddress"),
            &JsValue::from_f64(state_address as f64),
        )
        .is_err()
        || Reflect::set(
            &bridge,
            &JsValue::from_str("commandJsonAddress"),
            &command_json_address,
        )
        .is_err()
        || Reflect::set(
            &bridge,
            &JsValue::from_str("memory"),
            &wasm_bindgen::memory(),
        )
        .is_err()
        || Reflect::set(
            &js_sys::global(),
            &JsValue::from_str("__FISSION_TEST__"),
            &bridge,
        )
        .is_err()
    {
        return false;
    }

    true
}

// Keep these as plain Wasm exports. The `#[wasm_bindgen]` synchronous wrapper
// routes calls through `maybe_catch_unwind`, which consults thread-local panic
// state and can trap in a long-running Web LiveTest session.
#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_command_json_ptr() -> u32 {
    std::ptr::addr_of_mut!(COMMAND_JSON) as *mut u8 as usize as u32
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_command_json_capacity() -> u32 {
    MAX_COMMAND_JSON_BYTES as u32
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_bridge_state_ptr() -> u32 {
    bridge_state_address()
}

#[cfg(fission_web_test_control)]
#[no_mangle]
#[inline(never)]
pub extern "C" fn fission_web_test_stack_pointer_probe() -> u32 {
    let marker = 0_u32;
    let address = std::ptr::addr_of!(marker) as usize as u32;
    unsafe {
        std::ptr::read_volatile(std::ptr::addr_of!(marker));
    }
    address
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_diagnostics_ptr() -> u32 {
    diagnostics_address()
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_submit_json(length: u32) -> u32 {
    record_diagnostic(400, length);
    let command = if length as usize > MAX_COMMAND_JSON_BYTES {
        Err(format!(
            "command JSON exceeds the {} byte Web LiveTest bridge limit",
            MAX_COMMAND_JSON_BYTES
        ))
    } else {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                std::ptr::addr_of!(COMMAND_JSON) as *const u8,
                length as usize,
            )
        };
        std::str::from_utf8(bytes)
            .map_err(|error| format!("parse error: command JSON is not UTF-8: {error}"))
            .and_then(|command_json| {
                serde_json::from_str::<TestCommand>(command_json)
                    .map_err(|error| format!("parse error: {error}"))
            })
    };
    record_diagnostic(401, length);
    let proxy_available = with_bridge_state(|state| state.event_proxy.is_some());
    record_diagnostic(402, u32::from(proxy_available));
    if !proxy_available {
        return 0;
    }
    record_diagnostic(403, 0);
    queue_command(command)
}

// Runtime.evaluate is used to submit a LiveTest command from CDP. Dispatching
// its EventLoopProxy immediately can synchronously enter winit's Web event
// loop before Chrome has sent the Runtime.evaluate response. Defer that work
// to a later browser task so the CDP command can return its request id first.
#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_dispatch_command(request_id: u32) {
    record_diagnostic(417, request_id);
    dispatch_queued_command(request_id);
    record_diagnostic(419, request_id);
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_poll_json_len(request_id: u32) -> u32 {
    record_diagnostic(430, request_id);
    record_poll_stage(request_id, POLL_STAGE_ENTERED);
    record_diagnostic(431, request_id);
    let response = poll_response(request_id);
    record_diagnostic(432, request_id);
    record_poll_stage(request_id, POLL_STAGE_RESPONSE_READY);
    let json = match response {
        Some(response) => response_json_string(&response),
        None => "null".to_owned(),
    };
    record_diagnostic(433, json.len() as u32);
    with_bridge_state(|state| {
        record_diagnostic(434, request_id);
        state.poll_json.clear();
        state.poll_json.extend_from_slice(json.as_bytes());
        record_poll_stage(request_id, POLL_STAGE_JSON_READY);
        record_diagnostic(435, state.poll_json.len() as u32);
        state.poll_json.len() as u32
    })
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_poll_json_ptr() -> u32 {
    with_bridge_state(|state| state.poll_json.as_ptr() as usize as u32)
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_frame_present() -> u32 {
    with_bridge_state(|state| u32::from(state.frame.is_some()))
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_frame_width() -> f32 {
    with_bridge_state(|state| state.frame.map_or(0.0, |frame| frame.width))
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_frame_height() -> f32 {
    with_bridge_state(|state| state.frame.map_or(0.0, |frame| frame.height))
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_frame_number_low() -> u32 {
    with_bridge_state(|state| state.frame.map_or(0, |frame| frame.frame as u32))
}

#[cfg(fission_web_test_control)]
#[no_mangle]
pub extern "C" fn fission_web_test_frame_number_high() -> u32 {
    with_bridge_state(|state| state.frame.map_or(0, |frame| (frame.frame >> 32) as u32))
}

fn record_poll_stage(request_id: u32, stage: u32) {
    LAST_POLL_STAGE.store(
        request_id.wrapping_shl(2) | (stage & 0b11),
        Ordering::Relaxed,
    );
}

fn queue_command(command: Result<TestCommand, String>) -> u32 {
    let available = with_bridge_state(|state| state.responses.len() < MAX_PENDING_RESPONSES);
    record_diagnostic(410, u32::from(available));
    if !available {
        // Zero is reserved; generated request ids start at one. The next poll
        // turns this sentinel into a normal, actionable test response without
        // allowing an abandoned caller to grow the map without a bound.
        return 0;
    }

    let request_id = with_bridge_state(|state| {
        let request_id = state.next_request_id;
        state.next_request_id = request_id.wrapping_add(1).max(1);
        request_id
    });
    record_diagnostic(411, request_id);
    let response = PendingResponse::Queued(command);
    record_diagnostic(412, request_id);
    with_bridge_state(|state| {
        state.responses.insert(request_id, response);
    });
    record_diagnostic(413, request_id);
    record_diagnostic(414, request_id);
    request_id
}

fn dispatch_queued_command(request_id: u32) {
    let (command, proxy) = with_bridge_state(|state| {
        let command = match state.responses.remove(&request_id) {
            Some(PendingResponse::Queued(command)) => Some(command),
            Some(response) => {
                state.responses.insert(request_id, response);
                None
            }
            None => None,
        };
        (command, state.event_proxy.clone())
    });
    record_diagnostic(418, u32::from(command.is_some()));
    let Some(command) = command else {
        return;
    };

    let response = match (command, proxy) {
        (Ok(command), Some(proxy)) => dispatch(command, &proxy),
        (Ok(_), None) => PendingResponse::Ready(TestResponse::Error {
            message: "Fission browser event loop is closed".into(),
        }),
        (Err(message), _) => PendingResponse::Ready(TestResponse::Error { message }),
    };
    with_bridge_state(|state| {
        state.responses.insert(request_id, response);
    });
}

fn response_json_string(response: &TestResponse) -> String {
    serde_json::to_string(response).unwrap_or_else(|error| {
        serde_json::json!({
            "status": "Error",
            "message": format!("response serialization failed: {error}")
        })
        .to_string()
    })
}

fn poll_response(request_id: u32) -> Option<TestResponse> {
    record_diagnostic(440, request_id);
    let response = with_bridge_state(|state| {
        let Some(response) = state.responses.remove(&request_id) else {
            return Some(TestResponse::Error {
                message: if request_id == 0 {
                    "too many pending browser test requests".into()
                } else {
                    format!("unknown browser test request {request_id}")
                },
            });
        };

        match response {
            PendingResponse::Queued(command) => {
                state
                    .responses
                    .insert(request_id, PendingResponse::Queued(command));
                None
            }
            PendingResponse::Ready(response) => Some(response),
            PendingResponse::Waiting(receiver) => match receiver.try_recv() {
                Ok(response) => Some(response),
                Err(TryRecvError::Empty) => {
                    state
                        .responses
                        .insert(request_id, PendingResponse::Waiting(receiver));
                    None
                }
                Err(TryRecvError::Disconnected) => Some(TestResponse::Error {
                    message: "browser test response channel closed".into(),
                }),
            },
        }
    });
    record_diagnostic(441, request_id);
    response
}

fn dispatch(command: TestCommand, proxy: &EventLoopProxy<TestEvent>) -> PendingResponse {
    match command {
        TestCommand::Tap { x, y } => ready_after(
            proxy,
            [
                TestEvent::MouseMove { x, y },
                TestEvent::MouseDown { x, y, button: 0 },
                TestEvent::MouseUp { x, y, button: 0 },
            ],
        ),
        TestCommand::Drag {
            start_x,
            start_y,
            end_x,
            end_y,
            steps,
        } => {
            let steps = steps.max(1);
            let mut events = Vec::with_capacity(steps as usize + 3);
            events.push(TestEvent::MouseMove {
                x: start_x,
                y: start_y,
            });
            events.push(TestEvent::MouseDown {
                x: start_x,
                y: start_y,
                button: 0,
            });
            for step in 1..=steps {
                let progress = step as f32 / steps as f32;
                events.push(TestEvent::MouseMove {
                    x: start_x + (end_x - start_x) * progress,
                    y: start_y + (end_y - start_y) * progress,
                });
            }
            events.push(TestEvent::MouseUp {
                x: end_x,
                y: end_y,
                button: 0,
            });
            ready_after(proxy, events)
        }
        TestCommand::PointerDown {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerDown {
                pointer_id,
                kind,
                x,
                y,
                modifiers,
            }],
        ),
        TestCommand::PointerMove {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerMove {
                pointer_id,
                kind,
                x,
                y,
                modifiers,
            }],
        ),
        TestCommand::PointerUp {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerUp {
                pointer_id,
                kind,
                x,
                y,
                modifiers,
            }],
        ),
        TestCommand::PointerCancel {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerCancel {
                pointer_id,
                kind,
                x,
                y,
                modifiers,
            }],
        ),
        TestCommand::PointerScroll {
            x,
            y,
            dx,
            dy,
            delta_mode,
            phase,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::PointerScroll {
                x,
                y,
                dx,
                dy,
                delta_mode,
                phase,
                modifiers,
            }],
        ),
        TestCommand::Magnify {
            x,
            y,
            scale_factor,
            phase,
            modifiers,
        } => ready_after(
            proxy,
            [TestEvent::Magnify {
                x,
                y,
                scale_factor,
                phase,
                modifiers,
            }],
        ),
        TestCommand::TapText { text } => query(proxy, |response_tx| TestEvent::TapText {
            text,
            response_tx,
        }),
        TestCommand::ResolveSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::ResolveSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::TapSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::TapSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::ActivateSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::ActivateSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::FocusSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::FocusSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::HoverSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::HoverSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::RightClickSelector { query: selector } => {
            query(proxy, |response_tx| TestEvent::RightClickSelector {
                query: selector,
                response_tx,
            })
        }
        TestCommand::ScrollIntoView { query: selector } => {
            query(proxy, |response_tx| TestEvent::ScrollIntoView {
                query: selector,
                response_tx,
            })
        }
        TestCommand::FillText {
            query: selector,
            text,
        } => query(proxy, |response_tx| TestEvent::FillText {
            query: selector,
            text,
            response_tx,
        }),
        TestCommand::ClearText { query: selector } => {
            query(proxy, |response_tx| TestEvent::ClearText {
                query: selector,
                response_tx,
            })
        }
        TestCommand::Toggle { query: selector } => query(proxy, |response_tx| TestEvent::Toggle {
            query: selector,
            response_tx,
        }),
        TestCommand::SelectOption { query: selector } => {
            query(proxy, |response_tx| TestEvent::SelectOption {
                query: selector,
                response_tx,
            })
        }
        TestCommand::Scroll { x, y, dx, dy } => {
            ready_after(proxy, [TestEvent::Scroll { x, y, dx, dy }])
        }
        TestCommand::ExternalFileHover { x, y, paths } => {
            ready_after(proxy, [TestEvent::ExternalFileHover { x, y, paths }])
        }
        TestCommand::ExternalFileDrop { x, y, paths } => {
            ready_after(proxy, [TestEvent::ExternalFileDrop { x, y, paths }])
        }
        TestCommand::ExternalFileCancel {} => ready_after(proxy, [TestEvent::ExternalFileCancel]),
        TestCommand::TypeText { text } => ready_after(proxy, [TestEvent::TextInput { text }]),
        TestCommand::ImePreedit {
            text,
            cursor_start,
            cursor_end,
        } => ready_after(
            proxy,
            [TestEvent::ImePreedit {
                text,
                cursor: cursor_start.zip(cursor_end),
            }],
        ),
        TestCommand::ImeCommit { text } => ready_after(proxy, [TestEvent::ImeCommit { text }]),
        TestCommand::ImeCancel {} => ready_after(proxy, [TestEvent::ImeCancel]),
        TestCommand::PressKey { key, modifiers } => ready_after(
            proxy,
            [
                TestEvent::KeyDown {
                    key_code: key.clone(),
                    modifiers,
                },
                TestEvent::KeyUp {
                    key_code: key,
                    modifiers,
                },
            ],
        ),
        TestCommand::PauseAnimations {} => query(proxy, |response_tx| TestEvent::PauseAnimations {
            response_tx,
        }),
        TestCommand::ResumeAnimations {} => query(proxy, |response_tx| {
            TestEvent::ResumeAnimations { response_tx }
        }),
        TestCommand::AdvanceClock { ms } => query(proxy, |response_tx| TestEvent::AdvanceClock {
            ms,
            response_tx,
        }),
        TestCommand::CaptureScreenshot {} => query(proxy, |response_tx| {
            TestEvent::CaptureScreenshot { response_tx }
        }),
        TestCommand::CaptureAt { ms } => query(proxy, |response_tx| TestEvent::CaptureAt {
            ms,
            response_tx,
        }),
        TestCommand::WaitForIdle { .. } => {
            query(proxy, |response_tx| TestEvent::MotionStatus { response_tx })
        }
        TestCommand::GetText {} => query(proxy, |response_tx| TestEvent::GetText { response_tx }),
        TestCommand::GetTree {} => query(proxy, |response_tx| TestEvent::GetTree { response_tx }),
        TestCommand::Pump {} => query(proxy, |response_tx| TestEvent::Pump { response_tx }),
        TestCommand::Quit {} => ready_after(proxy, [TestEvent::Quit]),
        TestCommand::SimulateMouseMove { x, y } => {
            ready_after(proxy, [TestEvent::MouseMove { x, y }])
        }
        TestCommand::SimulateRightClick { x, y } => ready_after(
            proxy,
            [
                TestEvent::MouseMove { x, y },
                TestEvent::MouseDown { x, y, button: 1 },
                TestEvent::MouseUp { x, y, button: 1 },
            ],
        ),
        TestCommand::SimulateResize { .. } => {
            PendingResponse::Ready(fission_test_driver::unsupported_browser_resize_response())
        }
        TestCommand::WaitForSelector { .. }
        | TestCommand::WaitForVisible { .. }
        | TestCommand::WaitForEnabled { .. }
        | TestCommand::WaitForDisabled { .. }
        | TestCommand::WaitForValue { .. }
        | TestCommand::WaitForText { .. }
        | TestCommand::WaitForGone { .. }
        | TestCommand::Wait { .. }
        | TestCommand::Screenshot { .. } => PendingResponse::Ready(TestResponse::Error {
            message: "command is handled by the browser test host".into(),
        }),
    }
}

fn ready_after(
    proxy: &EventLoopProxy<TestEvent>,
    events: impl IntoIterator<Item = TestEvent>,
) -> PendingResponse {
    for event in events {
        if proxy.send_event(event).is_err() {
            return PendingResponse::Ready(TestResponse::Error {
                message: "Fission browser event loop is closed".into(),
            });
        }
    }
    PendingResponse::Ready(TestResponse::Ok {})
}

fn query(
    proxy: &EventLoopProxy<TestEvent>,
    make_event: impl FnOnce(mpsc::Sender<TestResponse>) -> TestEvent,
) -> PendingResponse {
    record_diagnostic(420, 0);
    let (response_tx, response_rx) = mpsc::channel();
    record_diagnostic(421, 0);
    record_diagnostic(422, 0);
    if proxy.send_event(make_event(response_tx)).is_err() {
        record_diagnostic(423, 0);
        PendingResponse::Ready(TestResponse::Error {
            message: "Fission browser event loop is closed".into(),
        })
    } else {
        record_diagnostic(424, 0);
        PendingResponse::Waiting(response_rx)
    }
}
