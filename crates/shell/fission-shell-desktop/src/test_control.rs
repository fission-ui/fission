use fission_test_driver::{TestCommand, TestResponse, TextItem, SemanticNode};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;

pub type CommandSender = mpsc::Sender<(TestCommand, ResponseSender)>;
pub type CommandReceiver = mpsc::Receiver<(TestCommand, ResponseSender)>;
pub type ResponseSender = mpsc::Sender<TestResponse>;

pub fn create_channel() -> (CommandSender, CommandReceiver) {
    mpsc::channel()
}

pub fn spawn_server(port: u16, cmd_tx: CommandSender) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .unwrap_or_else(|e| panic!("failed to bind test control port {}: {}", port, e));
        eprintln!("[fission-test-control] listening on port {}", port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_connection(stream, &cmd_tx),
                Err(e) => eprintln!("[fission-test-control] accept error: {}", e),
            }
        }
    })
}

fn handle_connection(mut stream: TcpStream, cmd_tx: &CommandSender) {
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
        send_http_response(&mut stream, 404, r#"{"status":"Error","message":"not found"}"#);
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

    // Send to event loop and wait for response
    let (resp_tx, resp_rx) = mpsc::channel();
    if cmd_tx.send((cmd, resp_tx)).is_err() {
        send_http_response(
            &mut stream,
            500,
            r#"{"status":"Error","message":"event loop disconnected"}"#,
        );
        return;
    }

    // Wait for response (with timeout)
    match resp_rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(response) => {
            send_http_response(&mut stream, 200, &serde_json::to_string(&response).unwrap());
        }
        Err(_) => {
            send_http_response(
                &mut stream,
                504,
                r#"{"status":"Error","message":"timeout waiting for response"}"#,
            );
        }
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
