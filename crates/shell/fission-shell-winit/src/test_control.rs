use fission_test_driver::{TestCommand, TestEvent, TestResponse};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use winit::event_loop::EventLoopProxy;

/// Sender for query responses from the main event loop back to the TCP server.
pub type ResponseSender = mpsc::Sender<TestResponse>;
/// Receiver for query responses.
pub type ResponseReceiver = mpsc::Receiver<TestResponse>;
/// Shared queue used on platforms where winit user events are unreliable.
pub type PendingEventQueue = Arc<Mutex<VecDeque<TestEvent>>>;

#[derive(Clone)]
pub enum EventInjector {
    Proxy(EventLoopProxy<TestEvent>),
    Queue {
        queue: PendingEventQueue,
        wake_proxy: Option<EventLoopProxy<TestEvent>>,
    },
}

/// Create a (sender, receiver) pair for query responses.
///
/// The sender is stored by the main event loop; when a query `TestEvent`
/// (GetText, GetTree, Screenshot, etc.) is handled it sends the result
/// through this channel. The TCP server thread waits on the receiver.
pub fn create_response_channel() -> (ResponseSender, ResponseReceiver) {
    mpsc::channel()
}

pub fn create_pending_event_queue() -> PendingEventQueue {
    Arc::new(Mutex::new(VecDeque::new()))
}

/// Spawn the TCP test-control server.
pub fn spawn_server(
    port: u16,
    injector: EventInjector,
    response_rx: ResponseReceiver,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .unwrap_or_else(|e| panic!("failed to bind test control port {}: {}", port, e));
        eprintln!("[fission-test-control] listening on port {}", port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_connection(stream, &injector, &response_rx),
                Err(e) => eprintln!("[fission-test-control] accept error: {}", e),
            }
        }
    })
}

fn handle_connection(
    mut stream: TcpStream,
    injector: &EventInjector,
    response_rx: &ResponseReceiver,
) {
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

    let response = dispatch_command(cmd, injector, response_rx);
    send_http_response(&mut stream, 200, &serde_json::to_string(&response).unwrap());
}

fn dispatch_command(
    cmd: TestCommand,
    injector: &EventInjector,
    response_rx: &ResponseReceiver,
) -> TestResponse {
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
        TestCommand::TapText { text } => {
            inject_event(injector, TestEvent::TapText { text });
            wait_for_response(response_rx)
        }
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
        TestCommand::Screenshot { path } => {
            inject_event(injector, TestEvent::Screenshot { path });
            wait_for_response(response_rx)
        }
        TestCommand::CaptureScreenshot {} => {
            inject_event(injector, TestEvent::CaptureScreenshot);
            wait_for_response(response_rx)
        }
        TestCommand::GetText {} => {
            inject_event(injector, TestEvent::GetText);
            wait_for_response(response_rx)
        }
        TestCommand::GetTree {} => {
            inject_event(injector, TestEvent::GetTree);
            wait_for_response(response_rx)
        }
        TestCommand::Wait { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            TestResponse::Ok {}
        }
        TestCommand::Pump {} => {
            inject_event(injector, TestEvent::Pump);
            wait_for_response(response_rx)
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
fn wait_for_response(rx: &ResponseReceiver) -> TestResponse {
    match rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(resp) => resp,
        Err(_) => TestResponse::Error {
            message: "timeout waiting for response from event loop".into(),
        },
    }
}

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
