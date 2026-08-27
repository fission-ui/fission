#[cfg(not(target_arch = "wasm32"))]
use fission_test_driver::TestCommand;
#[cfg(not(target_arch = "wasm32"))]
use fission_test_driver::TestEvent;
use fission_test_driver::TestResponse;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::io::{Read, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use winit::event_loop::EventLoopProxy;

/// Sender for query responses from the main event loop back to the TCP server.
pub type ResponseSender = fission_test_driver::TestResponseSender;
/// Receiver for query responses.
pub type ResponseReceiver = mpsc::Receiver<TestResponse>;
/// Shared queue used on platforms where winit user events are unreliable.
#[cfg(not(target_arch = "wasm32"))]
pub type PendingEventQueue = Arc<Mutex<VecDeque<TestEvent>>>;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub enum EventInjector {
    Proxy(EventLoopProxy<TestEvent>),
    Queue {
        queue: PendingEventQueue,
        wake_proxy: Option<EventLoopProxy<TestEvent>>,
    },
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_pending_event_queue() -> PendingEventQueue {
    Arc::new(Mutex::new(VecDeque::new()))
}

/// Spawn the TCP test-control server.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_server(
    port: u16,
    bearer_token: Option<String>,
    injector: EventInjector,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .unwrap_or_else(|e| panic!("failed to bind test control port {}: {}", port, e));
        eprintln!("[fission-test-control] listening on port {}", port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_connection(stream, bearer_token.as_deref(), &injector),
                Err(e) => eprintln!("[fission-test-control] accept error: {}", e),
            }
        }
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_connection(mut stream: TcpStream, bearer_token: Option<&str>, injector: &EventInjector) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];

    loop {
        match stream.read(&mut tmp) {
            Ok(0) => return,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => return,
        }
    }

    let request = String::from_utf8_lossy(&buf);
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let method = parts.first().copied().unwrap_or("");
    let path = parts.get(1).copied().unwrap_or("");

    if let Some(expected) = bearer_token {
        if !request_has_bearer_token(&request, expected) {
            send_http_response(
                &mut stream,
                401,
                r#"{"status":"Error","message":"unauthorized"}"#,
            );
            return;
        }
    }

    if path == "/health" {
        send_http_response(&mut stream, 200, r#"{"status":"ok"}"#);
        return;
    }

    if method != "POST" || path != "/cmd" {
        send_http_response(
            &mut stream,
            404,
            r#"{"status":"Error","message":"not found"}"#,
        );
        return;
    }

    let content_length = request
        .lines()
        .find(|line| line.to_lowercase().starts_with("content-length:"))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);

    let header_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
        .unwrap_or(buf.len());

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&tmp[..n]),
            Err(_) => break,
        }
    }

    let body_str = String::from_utf8_lossy(&body);
    let cmd: TestCommand = match serde_json::from_str(&body_str) {
        Ok(cmd) => cmd,
        Err(error) => {
            let resp = TestResponse::Error {
                message: format!("parse error: {}", error),
            };
            send_http_response(&mut stream, 400, &serde_json::to_string(&resp).unwrap());
            return;
        }
    };

    let response = dispatch_command(cmd, injector);
    send_http_response(&mut stream, 200, &serde_json::to_string(&response).unwrap());
}

#[cfg(not(target_arch = "wasm32"))]
fn request_has_bearer_token(request: &str, expected: &str) -> bool {
    request.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("authorization")
                && value.trim() == format!("Bearer {expected}")
        })
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn dispatch_command(cmd: TestCommand, injector: &EventInjector) -> TestResponse {
    match cmd {
        TestCommand::Tap { x, y } => {
            inject_event(injector, TestEvent::MouseMove { x, y });
            inject_event(injector, TestEvent::MouseDown { x, y, button: 0 });
            inject_event(injector, TestEvent::MouseUp { x, y, button: 0 });
            TestResponse::Ok {}
        }
        TestCommand::Drag {
            start_x,
            start_y,
            end_x,
            end_y,
            steps,
        } => {
            let steps = steps.max(1);
            inject_event(
                injector,
                TestEvent::MouseMove {
                    x: start_x,
                    y: start_y,
                },
            );
            inject_event(
                injector,
                TestEvent::MouseDown {
                    x: start_x,
                    y: start_y,
                    button: 0,
                },
            );
            for step in 1..=steps {
                let t = step as f32 / steps as f32;
                let x = start_x + (end_x - start_x) * t;
                let y = start_y + (end_y - start_y) * t;
                inject_event(injector, TestEvent::MouseMove { x, y });
            }
            inject_event(
                injector,
                TestEvent::MouseUp {
                    x: end_x,
                    y: end_y,
                    button: 0,
                },
            );
            TestResponse::Ok {}
        }
        TestCommand::PointerDown {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => {
            inject_event(
                injector,
                TestEvent::PointerDown {
                    pointer_id,
                    kind,
                    x,
                    y,
                    modifiers,
                },
            );
            TestResponse::Ok {}
        }
        TestCommand::PointerMove {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => {
            inject_event(
                injector,
                TestEvent::PointerMove {
                    pointer_id,
                    kind,
                    x,
                    y,
                    modifiers,
                },
            );
            TestResponse::Ok {}
        }
        TestCommand::PointerUp {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => {
            inject_event(
                injector,
                TestEvent::PointerUp {
                    pointer_id,
                    kind,
                    x,
                    y,
                    modifiers,
                },
            );
            TestResponse::Ok {}
        }
        TestCommand::PointerCancel {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        } => {
            inject_event(
                injector,
                TestEvent::PointerCancel {
                    pointer_id,
                    kind,
                    x,
                    y,
                    modifiers,
                },
            );
            TestResponse::Ok {}
        }
        TestCommand::PointerScroll {
            x,
            y,
            dx,
            dy,
            delta_mode,
            phase,
            modifiers,
        } => {
            inject_event(
                injector,
                TestEvent::PointerScroll {
                    x,
                    y,
                    dx,
                    dy,
                    delta_mode,
                    phase,
                    modifiers,
                },
            );
            TestResponse::Ok {}
        }
        TestCommand::Magnify {
            x,
            y,
            scale_factor,
            phase,
            modifiers,
        } => {
            inject_event(
                injector,
                TestEvent::Magnify {
                    x,
                    y,
                    scale_factor,
                    phase,
                    modifiers,
                },
            );
            TestResponse::Ok {}
        }
        TestCommand::TapText { text } => query_event(injector, |response_tx| TestEvent::TapText {
            text,
            response_tx,
        }),
        TestCommand::ResolveSelector { query } => query_event(injector, |response_tx| {
            TestEvent::ResolveSelector { query, response_tx }
        }),
        TestCommand::ScrollIntoView { query } => query_event(injector, |response_tx| {
            TestEvent::ScrollIntoView { query, response_tx }
        }),
        TestCommand::TapSelector { query } => {
            auto_scroll_then_query(injector, query, |query, response_tx| {
                TestEvent::TapSelector { query, response_tx }
            })
        }
        TestCommand::ActivateSelector { query } => {
            auto_scroll_then_query(injector, query, |query, response_tx| {
                TestEvent::ActivateSelector { query, response_tx }
            })
        }
        TestCommand::FocusSelector { query } => {
            auto_scroll_then_query(injector, query, |query, response_tx| {
                TestEvent::FocusSelector { query, response_tx }
            })
        }
        TestCommand::HoverSelector { query } => {
            auto_scroll_then_query(injector, query, |query, response_tx| {
                TestEvent::HoverSelector { query, response_tx }
            })
        }
        TestCommand::RightClickSelector { query } => {
            auto_scroll_then_query(injector, query, |query, response_tx| {
                TestEvent::RightClickSelector { query, response_tx }
            })
        }
        TestCommand::FillText { query, text } => {
            auto_scroll_then_query(injector, query, |query, response_tx| TestEvent::FillText {
                query,
                text,
                response_tx,
            })
        }
        TestCommand::ClearText { query } => {
            auto_scroll_then_query(injector, query, |query, response_tx| TestEvent::ClearText {
                query,
                response_tx,
            })
        }
        TestCommand::Toggle { query } => {
            auto_scroll_then_query(injector, query, |query, response_tx| TestEvent::Toggle {
                query,
                response_tx,
            })
        }
        TestCommand::SelectOption { query } => {
            auto_scroll_then_query(injector, query, |query, response_tx| {
                TestEvent::SelectOption { query, response_tx }
            })
        }
        TestCommand::Scroll { x, y, dx, dy } => {
            inject_event(injector, TestEvent::Scroll { x, y, dx, dy });
            TestResponse::Ok {}
        }
        TestCommand::ExternalFileHover { x, y, paths } => {
            inject_event(injector, TestEvent::ExternalFileHover { x, y, paths });
            TestResponse::Ok {}
        }
        TestCommand::ExternalFileDrop { x, y, paths } => {
            inject_event(injector, TestEvent::ExternalFileDrop { x, y, paths });
            TestResponse::Ok {}
        }
        TestCommand::ExternalFileCancel {} => {
            inject_event(injector, TestEvent::ExternalFileCancel);
            TestResponse::Ok {}
        }
        TestCommand::TypeText { text } => {
            inject_event(injector, TestEvent::TextInput { text });
            TestResponse::Ok {}
        }
        TestCommand::ImePreedit {
            text,
            cursor_start,
            cursor_end,
        } => {
            let cursor = match (cursor_start, cursor_end) {
                (Some(start), Some(end)) => Some((start, end)),
                _ => None,
            };
            inject_event(injector, TestEvent::ImePreedit { text, cursor });
            TestResponse::Ok {}
        }
        TestCommand::ImeCommit { text } => {
            inject_event(injector, TestEvent::ImeCommit { text });
            TestResponse::Ok {}
        }
        TestCommand::ImeCancel {} => {
            inject_event(injector, TestEvent::ImeCancel);
            TestResponse::Ok {}
        }
        TestCommand::PressKey { key, modifiers } => {
            inject_event(
                injector,
                TestEvent::KeyDown {
                    key_code: key.clone(),
                    modifiers,
                },
            );
            inject_event(
                injector,
                TestEvent::KeyUp {
                    key_code: key,
                    modifiers,
                },
            );
            TestResponse::Ok {}
        }
        TestCommand::Screenshot { path } => query_event(injector, |response_tx| {
            TestEvent::Screenshot { path, response_tx }
        }),
        TestCommand::CaptureScreenshot {} => query_event(injector, |response_tx| {
            TestEvent::CaptureScreenshot { response_tx }
        }),
        TestCommand::PauseAnimations {} => query_event(injector, |response_tx| {
            TestEvent::PauseAnimations { response_tx }
        }),
        TestCommand::ResumeAnimations {} => query_event(injector, |response_tx| {
            TestEvent::ResumeAnimations { response_tx }
        }),
        TestCommand::AdvanceClock { ms } => query_event(injector, |response_tx| {
            TestEvent::AdvanceClock { ms, response_tx }
        }),
        TestCommand::CaptureAt { ms } => query_event(injector, |response_tx| {
            TestEvent::CaptureAt { ms, response_tx }
        }),
        TestCommand::WaitForIdle {
            timeout_ms,
            ignore_repeating_motion,
        } => wait_for_motion_idle(injector, timeout_ms, ignore_repeating_motion),
        TestCommand::GetText {} => {
            query_event(injector, |response_tx| TestEvent::GetText { response_tx })
        }
        TestCommand::GetTree {} => {
            query_event(injector, |response_tx| TestEvent::GetTree { response_tx })
        }
        TestCommand::Wait { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            TestResponse::Ok {}
        }
        TestCommand::WaitForSelector { query, timeout_ms } => {
            wait_for_selector_state(injector, query, timeout_ms, SelectorWaitCondition::Present)
        }
        TestCommand::WaitForVisible { query, timeout_ms } => {
            wait_for_selector_state(injector, query, timeout_ms, SelectorWaitCondition::Visible)
        }
        TestCommand::WaitForEnabled { query, timeout_ms } => {
            wait_for_selector_state(injector, query, timeout_ms, SelectorWaitCondition::Enabled)
        }
        TestCommand::WaitForDisabled { query, timeout_ms } => {
            wait_for_selector_state(injector, query, timeout_ms, SelectorWaitCondition::Disabled)
        }
        TestCommand::WaitForValue {
            query,
            value,
            timeout_ms,
        } => wait_for_selector_state(
            injector,
            query,
            timeout_ms,
            SelectorWaitCondition::Value(value),
        ),
        TestCommand::WaitForText { text, timeout_ms } => wait_for_text(injector, text, timeout_ms),
        TestCommand::WaitForGone { query, timeout_ms } => {
            wait_for_selector_state(injector, query, timeout_ms, SelectorWaitCondition::Gone)
        }
        TestCommand::Pump {} => {
            query_event(injector, |response_tx| TestEvent::Pump { response_tx })
        }
        TestCommand::Quit {} => {
            inject_event(injector, TestEvent::Quit);
            TestResponse::Ok {}
        }
        TestCommand::SimulateMouseMove { x, y } => {
            inject_event(injector, TestEvent::MouseMove { x, y });
            TestResponse::Ok {}
        }
        TestCommand::SimulateRightClick { x, y } => {
            inject_event(injector, TestEvent::MouseMove { x, y });
            inject_event(injector, TestEvent::MouseDown { x, y, button: 1 });
            inject_event(injector, TestEvent::MouseUp { x, y, button: 1 });
            TestResponse::Ok {}
        }
        TestCommand::SimulateResize { width, height } => {
            inject_event(injector, TestEvent::Resize { width, height });
            TestResponse::Ok {}
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn wait_for_motion_idle(
    injector: &EventInjector,
    timeout_ms: u64,
    ignore_repeating_motion: bool,
) -> TestResponse {
    let started = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(timeout_ms);
    loop {
        let status = query_event(injector, |response_tx| TestEvent::MotionStatus {
            response_tx,
        });
        match status {
            TestResponse::MotionStatus {
                finite,
                repeating,
                ripples,
            } if finite == 0 && ripples == 0 && (ignore_repeating_motion || repeating == 0) => {
                return TestResponse::Ok {};
            }
            TestResponse::Error { .. } => return status,
            _ if started.elapsed() >= timeout => {
                return TestResponse::Error {
                    message: format!(
                        "timed out after {timeout_ms}ms waiting for motion to become idle"
                    ),
                };
            }
            _ => std::thread::sleep(std::time::Duration::from_millis(8)),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn auto_scroll_then_query<F>(
    injector: &EventInjector,
    query: fission_test_driver::SelectorQuery,
    make_event: F,
) -> TestResponse
where
    F: FnOnce(fission_test_driver::SelectorQuery, ResponseSender) -> TestEvent,
{
    let scroll = query_event(injector, |response_tx| TestEvent::ScrollIntoView {
        query: query.clone().include_hidden(),
        response_tx,
    });
    if matches!(
        scroll,
        TestResponse::Error { .. } | TestResponse::SelectorError { .. }
    ) {
        return scroll;
    }
    let pump = query_event(injector, |response_tx| TestEvent::Pump { response_tx });
    if matches!(
        pump,
        TestResponse::Error { .. } | TestResponse::SelectorError { .. }
    ) {
        return pump;
    }
    query_event(injector, |response_tx| make_event(query, response_tx))
}

#[cfg(not(target_arch = "wasm32"))]
enum SelectorWaitCondition {
    Present,
    Visible,
    Enabled,
    Disabled,
    Value(String),
    Gone,
}

#[cfg(not(target_arch = "wasm32"))]
fn wait_for_selector_state(
    injector: &EventInjector,
    query: fission_test_driver::SelectorQuery,
    timeout_ms: u64,
    condition: SelectorWaitCondition,
) -> TestResponse {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(timeout_ms);
    loop {
        let resp = query_event(injector, |response_tx| TestEvent::ResolveSelector {
            query: query.clone(),
            response_tx,
        });
        let matched = match (&condition, &resp) {
            (SelectorWaitCondition::Gone, TestResponse::SelectorError { failure })
                if failure.kind == fission_test_driver::SelectorFailureKind::NoMatch =>
            {
                true
            }
            (SelectorWaitCondition::Present, TestResponse::SelectorResolved { .. }) => true,
            (SelectorWaitCondition::Visible, TestResponse::SelectorResolved { node }) => {
                node.visibility != fission_test_driver::VisibilityState::Hidden
            }
            (SelectorWaitCondition::Enabled, TestResponse::SelectorResolved { node }) => {
                !node.disabled
            }
            (SelectorWaitCondition::Disabled, TestResponse::SelectorResolved { node }) => {
                node.disabled
            }
            (SelectorWaitCondition::Value(expected), TestResponse::SelectorResolved { node }) => {
                node.value.as_deref() == Some(expected.as_str())
            }
            _ => false,
        };
        if matched {
            return TestResponse::Ok {};
        }
        if start.elapsed() >= timeout {
            let candidates = match resp {
                TestResponse::SelectorResolved { node } => {
                    vec![fission_test_driver::SelectorCandidate {
                        node,
                        rejected_reason: Some("wait condition did not pass".into()),
                    }]
                }
                TestResponse::SelectorError { failure } => failure.candidates,
                _ => Vec::new(),
            };
            return TestResponse::SelectorError {
                failure: fission_test_driver::SelectorFailure {
                    kind: fission_test_driver::SelectorFailureKind::Timeout,
                    selector: query,
                    candidates,
                    message: format!("timed out after {timeout_ms}ms waiting for selector"),
                },
            };
        }
        let _ = query_event(injector, |response_tx| TestEvent::Pump { response_tx });
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn wait_for_text(injector: &EventInjector, text: String, timeout_ms: u64) -> TestResponse {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(timeout_ms);
    loop {
        match query_event(injector, |response_tx| TestEvent::GetText { response_tx }) {
            TestResponse::Text { items } if items.iter().any(|item| item.text.contains(&text)) => {
                return TestResponse::Ok {};
            }
            TestResponse::Error { message } => return TestResponse::Error { message },
            _ => {}
        }
        if start.elapsed() >= timeout {
            return TestResponse::Error {
                message: format!("timed out after {timeout_ms}ms waiting for text `{text}`"),
            };
        }
        let _ = query_event(injector, |response_tx| TestEvent::Pump { response_tx });
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn query_event<F>(injector: &EventInjector, make_event: F) -> TestResponse
where
    F: FnOnce(ResponseSender) -> TestEvent,
{
    let (response_tx, response_rx) = mpsc::channel();
    inject_event(injector, make_event(response_tx));
    wait_for_response(&response_rx)
}

#[cfg(not(target_arch = "wasm32"))]
fn inject_event(injector: &EventInjector, event: TestEvent) {
    match injector {
        EventInjector::Proxy(proxy) => {
            let _ = proxy.send_event(event);
        }
        EventInjector::Queue { queue, wake_proxy } => {
            #[cfg(target_os = "android")]
            let debug_android_events = std::env::var_os("FISSION_DEBUG_ANDROID_EVENTS").is_some();
            #[cfg(target_os = "android")]
            if debug_android_events {
                eprintln!("[android-debug] queue_inject={event:?}");
            }
            if let Ok(mut pending) = queue.lock() {
                pending.push_back(event);
                #[cfg(target_os = "android")]
                if debug_android_events {
                    eprintln!("[android-debug] queue_len={}", pending.len());
                }
            }
            if let Some(proxy) = wake_proxy {
                #[cfg(target_os = "android")]
                if debug_android_events {
                    eprintln!("[android-debug] wake_send");
                }
                let _ = proxy.send_event(TestEvent::Wake);
            }
        }
    }
}

/// Block until the main event loop sends a response, with a 30-second timeout.
#[cfg(not(target_arch = "wasm32"))]
fn wait_for_response(rx: &ResponseReceiver) -> TestResponse {
    match rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(resp) => resp,
        Err(_) => TestResponse::Error {
            message: "timeout waiting for response from event loop".into(),
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn send_http_response(stream: &mut TcpStream, status: u16, body: &str) {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        504 => "Gateway Timeout",
        _ => "Unknown",
    };
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, status_text, body.len(), body
    );
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::request_has_bearer_token;

    #[test]
    fn bearer_capability_is_required_exactly() {
        let request = "GET /health HTTP/1.1\r\nAuthorization: Bearer correct\r\n\r\n";
        assert!(request_has_bearer_token(request, "correct"));
        assert!(!request_has_bearer_token(request, "wrong"));
        assert!(!request_has_bearer_token(
            "GET /health HTTP/1.1\r\n\r\n",
            "correct"
        ));
    }
}
