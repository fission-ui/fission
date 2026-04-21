//! LSP client that spawns rust-analyzer and communicates via stdin/stdout.

use super::protocol::*;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;

pub struct LspClient {
    child: Child,
    stdin_tx: mpsc::Sender<String>,
    response_rx: mpsc::Receiver<JsonRpcResponse>,
    next_id: u64,
    version: i32,
}

impl LspClient {
    /// Try to create an LSP client by locating rust-analyzer in PATH.
    /// Returns `None` if the binary is not found or cannot be spawned.
    pub fn try_new(root_path: &str) -> Option<Self> {
        // Check that rust-analyzer is available on PATH before spawning.
        let which_result = Command::new("which")
            .arg("rust-analyzer")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match which_result {
            Ok(status) if status.success() => {}
            _ => return None,
        }

        Self::start(root_path)
    }

    /// Spawn rust-analyzer and connect to it.
    /// Returns None if rust-analyzer is not available.
    pub fn start(root_path: &str) -> Option<Self> {
        let child = Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let mut client = Self {
            child,
            stdin_tx: mpsc::channel().0, // placeholder
            response_rx: mpsc::channel().1, // placeholder
            next_id: 1,
            version: 0,
        };

        // Set up IO threads
        let stdin = client.child.stdin.take()?;
        let stdout = client.child.stdout.take()?;

        let (stdin_tx, stdin_rx) = mpsc::channel::<String>();
        let (response_tx, response_rx) = mpsc::channel::<JsonRpcResponse>();

        // Writer thread
        thread::spawn(move || {
            let mut stdin = stdin;
            while let Ok(msg) = stdin_rx.recv() {
                let header = format!("Content-Length: {}\r\n\r\n", msg.len());
                if stdin.write_all(header.as_bytes()).is_err() { break; }
                if stdin.write_all(msg.as_bytes()).is_err() { break; }
                if stdin.flush().is_err() { break; }
            }
        });

        // Reader thread
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                // Read headers
                let mut content_length = 0usize;
                loop {
                    let mut header_line = String::new();
                    if reader.read_line(&mut header_line).unwrap_or(0) == 0 {
                        return;
                    }
                    let line = header_line.trim();
                    if line.is_empty() { break; }
                    if let Some(len_str) = line.strip_prefix("Content-Length: ") {
                        content_length = len_str.parse().unwrap_or(0);
                    }
                }

                if content_length == 0 { continue; }

                // Read body
                let mut body = vec![0u8; content_length];
                if reader.read_exact(&mut body).is_err() { return; }

                if let Ok(response) = serde_json::from_slice::<JsonRpcResponse>(&body) {
                    if response_tx.send(response).is_err() { return; }
                }
            }
        });

        client.stdin_tx = stdin_tx;
        client.response_rx = response_rx;

        // Initialize
        client.initialize(root_path);

        Some(client)
    }

    fn send_request(&mut self, method: &str, params: Option<Value>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: method.into(),
            params,
        };
        let json = serde_json::to_string(&request).unwrap();
        let _ = self.stdin_tx.send(json);
        id
    }

    fn send_notification(&mut self, method: &str, params: Option<Value>) {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
        };
        let json = serde_json::to_string(&notification).unwrap();
        let _ = self.stdin_tx.send(json);
    }

    fn initialize(&mut self, root_path: &str) {
        let uri = path_to_uri(root_path);
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(uri),
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    completion: Some(CompletionClientCapabilities {}),
                    publish_diagnostics: Some(PublishDiagnosticsCapabilities {}),
                }),
            },
        };
        self.send_request("initialize", Some(serde_json::to_value(params).unwrap()));

        // Wait for initialize response (with timeout)
        let _ = self.response_rx.recv_timeout(std::time::Duration::from_secs(10));

        // Send initialized notification
        self.send_notification("initialized", Some(serde_json::json!({})));
    }

    pub fn did_open(&mut self, path: &str, content: &str, language_id: &str) {
        self.version += 1;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: path_to_uri(path),
                language_id: language_id.into(),
                version: self.version,
                text: content.into(),
            },
        };
        self.send_notification("textDocument/didOpen", Some(serde_json::to_value(params).unwrap()));
    }

    pub fn did_change(&mut self, path: &str, content: &str) {
        self.version += 1;
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: path_to_uri(path),
                version: self.version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                text: content.into(),
            }],
        };
        self.send_notification("textDocument/didChange", Some(serde_json::to_value(params).unwrap()));
    }

    pub fn request_completion(&mut self, path: &str, line: u32, character: u32) -> u64 {
        let params = CompletionParams {
            text_document: TextDocumentIdentifier {
                uri: path_to_uri(path),
            },
            position: Position { line, character },
        };
        self.send_request("textDocument/completion", Some(serde_json::to_value(params).unwrap()))
    }

    /// Drain all pending responses/notifications from the server.
    /// Returns diagnostics and completion results.
    pub fn poll(&mut self) -> LspPollResult {
        let mut result = LspPollResult::default();

        while let Ok(response) = self.response_rx.try_recv() {
            // Notification (no id)
            if response.id.is_none() {
                if let Some(method) = &response.method {
                    if method == "textDocument/publishDiagnostics" {
                        if let Some(params) = &response.params {
                            if let Ok(diag_params) = serde_json::from_value::<PublishDiagnosticsParams>(params.clone()) {
                                result.diagnostics.push(diag_params);
                            }
                        }
                    }
                }
                continue;
            }

            // Response to a request
            if let Some(result_value) = &response.result {
                // Try to parse as completion
                if let Ok(items) = serde_json::from_value::<Vec<CompletionItem>>(result_value.clone()) {
                    result.completions = items;
                }
                // CompletionList wrapper
                if let Some(items) = result_value.get("items") {
                    if let Ok(items) = serde_json::from_value::<Vec<CompletionItem>>(items.clone()) {
                        result.completions = items;
                    }
                }
            }
        }

        result
    }

    /// Send shutdown request and exit notification, then kill the child process.
    pub fn shutdown(&mut self) {
        self.send_request("shutdown", None);
        // Give the server a moment to process the shutdown request.
        let _ = self.response_rx.recv_timeout(std::time::Duration::from_secs(2));
        self.send_notification("exit", None);
        // Wait briefly for a clean exit, then force-kill if still running.
        match self.child.try_wait() {
            Ok(Some(_)) => {} // already exited
            _ => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[derive(Debug, Default)]
pub struct LspPollResult {
    pub diagnostics: Vec<PublishDiagnosticsParams>,
    pub completions: Vec<CompletionItem>,
}
