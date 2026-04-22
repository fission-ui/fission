//! LSP JSON-RPC protocol types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    pub method: Option<String>,
    pub params: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

// --- LSP types (subset) ---

#[derive(Debug, Serialize)]
pub struct InitializeParams {
    #[serde(rename = "processId")]
    pub process_id: Option<u32>,
    #[serde(rename = "rootUri")]
    pub root_uri: Option<String>,
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Serialize, Default)]
pub struct ClientCapabilities {
    #[serde(rename = "textDocument")]
    pub text_document: Option<TextDocumentClientCapabilities>,
}

#[derive(Debug, Serialize, Default)]
pub struct TextDocumentClientCapabilities {
    pub completion: Option<CompletionClientCapabilities>,
    #[serde(rename = "publishDiagnostics")]
    pub publish_diagnostics: Option<PublishDiagnosticsCapabilities>,
}

#[derive(Debug, Serialize, Default)]
pub struct CompletionClientCapabilities {}

#[derive(Debug, Serialize, Default)]
pub struct PublishDiagnosticsCapabilities {}

#[derive(Debug, Serialize)]
pub struct DidOpenTextDocumentParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentItem,
}

#[derive(Debug, Serialize)]
pub struct TextDocumentItem {
    pub uri: String,
    #[serde(rename = "languageId")]
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct DidChangeTextDocumentParams {
    #[serde(rename = "textDocument")]
    pub text_document: VersionedTextDocumentIdentifier,
    #[serde(rename = "contentChanges")]
    pub content_changes: Vec<TextDocumentContentChangeEvent>,
}

#[derive(Debug, Serialize)]
pub struct VersionedTextDocumentIdentifier {
    pub uri: String,
    pub version: i32,
}

#[derive(Debug, Serialize)]
pub struct TextDocumentContentChangeEvent {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct CompletionParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

#[derive(Debug, Serialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
pub struct PublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<LspDiagnostic>,
}

#[derive(Debug, Deserialize)]
pub struct LspDiagnostic {
    pub range: Range,
    pub severity: Option<u32>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<u32>,
    pub detail: Option<String>,
}

pub fn path_to_uri(path: &str) -> String {
    let abs = std::fs::canonicalize(path)
        .unwrap_or_else(|_| std::path::PathBuf::from(path));
    format!("file://{}", abs.to_string_lossy())
}
