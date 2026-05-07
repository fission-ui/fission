use fission_test_driver::{TestCommand, TestEvent, TestResponse};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use winit::event_loop::EventLoopProxy;

/// Sender for query responses from the main event loop back to the TCP server.
pub type ResponseSender = mpsc::Sender<TestResponse>;
/// Receiver for query responses.
pub type ResponseReceiver = mpsc::Receiver<TestResponse>;

/// Create a (sender, receiver) pair for query responses.
///
/// The sender is stored by the main event loop; when a query `TestEvent`
/// (GetText, GetTree, Screenshot, etc.) is handled it sends the result
/// through this channel.  The TCP server thread waits on the receiver.
pub fn create_response_channel() -> (ResponseSender, ResponseReceiver) {
    mpsc::channel()
}

/// Spawn the TCP test-control server.
///
/// * `port` — TCP port to bind on 127.0.0.1.
/// * `proxy` — winit `EventLoopProxy` used to inject `TestEvent`s into the
///   main event loop.  Input-simulation events (MouseMove, MouseDown, …) and
///   query events (GetText, Screenshot, …) all go through this proxy so they
///   are handled on the main thread.
/// * `response_rx` — receiver for query results sent back by the main loop.
pub fn spawn_server(
    port: u16,
    proxy: EventLoopProxy<TestEvent>,
    response_rx: ResponseReceiver,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .unwrap_or_else(|e| panic!("failed to bind test control port {}: {}", port, e));
        eprintln!("[fission-test-control] listening on port {}", port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_connection(stream, &proxy, &response_rx),
                Err(e) => eprintln!("[fission-test-control] accept error: {}", e),
            }
        }
    })
}

fn handle_connection(
    mut stream: TcpStream,
    proxy: &EventLoopProxy<TestEvent>,
    response_rx: &ResponseReceiver,
) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];

    // Read HTTP request
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => return,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                // Check for end of HTTP headers
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => return,
        }
    }

    let request = String::from_utf8_lossy(&buf);

    // Parse request line
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let method = parts.first().copied().unwrap_or("");
    let path = parts.get(1).copied().unwrap_or("");

    // Health check
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

    // Extract content-length and body
    let content_length = request
        .lines()
        .find(|l| l.to_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);

    // Find body start
    let header_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .unwrap_or(buf.len());

    let mut body = buf[header_end..].to_vec();

    // Read remaining body if needed
    while body.len() < content_length {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&tmp[..n]),
            Err(_) => break,
        }
    }

    let body_str = String::from_utf8_lossy(&body);

    // Parse command
    let cmd: TestCommand = match serde_json::from_str(&body_str) {
        Ok(c) => c,
        Err(e) => {
            let resp = TestResponse::Error {
                message: format!("parse error: {}", e),
            };
            send_http_response(&mut stream, 400, &serde_json::to_string(&resp).unwrap());
            return;
        }
    };

    // Translate TestCommand → TestEvent(s) injected via the proxy, then
    // optionally wait for a response from the main loop.
    let response = dispatch_command(cmd, proxy, response_rx);
    send_http_response(&mut stream, 200, &serde_json::to_string(&response).unwrap());
}

/// Translate a `TestCommand` into one or more `TestEvent`s injected into the
/// winit event loop via the proxy.
///
/// * **Input-simulation** commands (`Tap`, `Scroll`, `TypeText`, …) inject
///   events that travel through the same handler path as real `WindowEvent`s.
///   They are fire-and-forget from the server's perspective.
///
/// * **Query / control** commands (`GetText`, `Screenshot`, `Pump`, `Quit`)
///   inject an event and then block waiting for the main loop to send a
///   response through `response_rx`.
fn dispatch_command(
    cmd: TestCommand,
    proxy: &EventLoopProxy<TestEvent>,
    response_rx: &ResponseReceiver,
) -> TestResponse {
    match cmd {
        // ── Input simulation: Tap = MouseMove + MouseDown + MouseUp ─────
        TestCommand::Tap { x, y } => {
            let _ = proxy.send_event(TestEvent::MouseMove { x, y });
            let _ = proxy.send_event(TestEvent::MouseDown { x, y, button: 0 });
            let _ = proxy.send_event(TestEvent::MouseUp { x, y, button: 0 });
            // Fire-and-forget — the events will be processed on the next
            // event loop iteration.
            TestResponse::Ok {}
        }

        // ── Drag = MouseMove + MouseDown + interpolated MouseMove + MouseUp ──
        TestCommand::Drag {
            start_x,
            start_y,
            end_x,
            end_y,
            steps,
        } => {
            let steps = steps.max(1);
            let _ = proxy.send_event(TestEvent::MouseMove {
                x: start_x,
                y: start_y,
            });
            let _ = proxy.send_event(TestEvent::MouseDown {
                x: start_x,
                y: start_y,
                button: 0,
            });
            for step in 1..=steps {
                let t = step as f32 / steps as f32;
                let x = start_x + (end_x - start_x) * t;
                let y = start_y + (end_y - start_y) * t;
                let _ = proxy.send_event(TestEvent::MouseMove { x, y });
            }
            let _ = proxy.send_event(TestEvent::MouseUp {
                x: end_x,
                y: end_y,
                button: 0,
            });
            TestResponse::Ok {}
        }

        // ── TapText: needs IR access, so delegate to main loop ──────────
        TestCommand::TapText { text } => {
            let _ = proxy.send_event(TestEvent::TapText { text });
            wait_for_response(response_rx)
        }

        // ── Scroll ──────────────────────────────────────────────────────
        TestCommand::Scroll { x, y, dx, dy } => {
            let _ = proxy.send_event(TestEvent::Scroll { x, y, dx, dy });
            TestResponse::Ok {}
        }

        // ── TypeText: inject individual key events ──────────────────────
        TestCommand::TypeText { text } => {
            let _ = proxy.send_event(TestEvent::TextInput { text });
            TestResponse::Ok {}
        }

        // ── PressKey ────────────────────────────────────────────────────
        TestCommand::PressKey { key, modifiers } => {
            let _ = proxy.send_event(TestEvent::KeyDown {
                key_code: key.clone(),
                modifiers,
            });
            let _ = proxy.send_event(TestEvent::KeyUp {
                key_code: key,
                modifiers,
            });
            TestResponse::Ok {}
        }

        // ── Screenshot: needs GPU access, wait for response ─────────────
        TestCommand::Screenshot { path } => {
            let _ = proxy.send_event(TestEvent::Screenshot { path });
            wait_for_response(response_rx)
        }

        TestCommand::CaptureScreenshot {} => {
            let _ = proxy.send_event(TestEvent::CaptureScreenshot);
            wait_for_response(response_rx)
        }

        // ── GetText: needs IR, wait for response ────────────────────────
        TestCommand::GetText {} => {
            let _ = proxy.send_event(TestEvent::GetText);
            wait_for_response(response_rx)
        }

        // ── GetTree: needs IR, wait for response ────────────────────────
        TestCommand::GetTree {} => {
            let _ = proxy.send_event(TestEvent::GetTree);
            wait_for_response(response_rx)
        }

        // ── Wait: sleep on server thread, then respond ──────────────────
        TestCommand::Wait { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            TestResponse::Ok {}
        }

        // ── Pump: force a frame, wait for completion ────────────────────
        TestCommand::Pump {} => {
            let _ = proxy.send_event(TestEvent::Pump);
            wait_for_response(response_rx)
        }

        // ── Quit ────────────────────────────────────────────────────────
        TestCommand::Quit {} => {
            let _ = proxy.send_event(TestEvent::Quit);
            TestResponse::Ok {}
        }

        // ── NEW: SimulateMouseMove ──────────────────────────────────────
        TestCommand::SimulateMouseMove { x, y } => {
            let _ = proxy.send_event(TestEvent::MouseMove { x, y });
            TestResponse::Ok {}
        }

        // ── NEW: SimulateRightClick ─────────────────────────────────────
        TestCommand::SimulateRightClick { x, y } => {
            let _ = proxy.send_event(TestEvent::MouseMove { x, y });
            let _ = proxy.send_event(TestEvent::MouseDown { x, y, button: 1 });
            let _ = proxy.send_event(TestEvent::MouseUp { x, y, button: 1 });
            TestResponse::Ok {}
        }

        // ── NEW: SimulateResize ─────────────────────────────────────────
        TestCommand::SimulateResize { width, height } => {
            let _ = proxy.send_event(TestEvent::Resize { width, height });
            TestResponse::Ok {}
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
