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
pub fn spawn_server(port: u16, injector: EventInjector) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .unwrap_or_else(|e| panic!("failed to bind test control port {}: {}", port, e));
        eprintln!("[fission-test-control] listening on port {}", port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_connection(stream, &injector),
                Err(e) => eprintln!("[fission-test-control] accept error: {}", e),
            }
        }
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_connection(mut stream: TcpStream, injector: &EventInjector) {
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
        TestCommand::TapText { text } => query_event(injector, |response_tx| TestEvent::TapText {
            text,
            response_tx,
        }),
        TestCommand::Scroll { x, y, dx, dy } => {
            inject_event(injector, TestEvent::Scroll { x, y, dx, dy });
            TestResponse::Ok {}
        }
        TestCommand::TypeText { text } => {
            inject_event(injector, TestEvent::TextInput { text });
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
