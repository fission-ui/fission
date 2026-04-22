//! Plugin message types.
//!
//! These mirror what a protobuf schema would generate. The wire format is:
//! [msg_type: u16 LE] [payload_len: u32 LE] [payload: JSON bytes]
//!
//! Using JSON for the payload keeps things simple while the proto schema
//! defines the contract. A real implementation would use prost/protobuf.

use serde::{Deserialize, Serialize};

/// Message type IDs (stable, never reused)
pub const MSG_FILE_OPENED: u16 = 1;
pub const MSG_FILE_SAVED: u16 = 2;
pub const MSG_FILE_CLOSED: u16 = 3;
pub const MSG_DIAGNOSTIC_UPDATE: u16 = 10;
pub const MSG_COMPLETION_REQUEST: u16 = 20;
pub const MSG_COMPLETION_RESPONSE: u16 = 21;
pub const MSG_EDITOR_ACTION: u16 = 30;
pub const MSG_STATUS_UPDATE: u16 = 40;
pub const MSG_TREE_REFRESH: u16 = 50;
pub const MSG_COMMAND_REGISTER: u16 = 60;
pub const MSG_COMMAND_EXECUTE: u16 = 61;

// --- Message payloads ---

#[derive(Debug, Serialize, Deserialize)]
pub struct FileOpened {
    pub path: String,
    pub language: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSaved {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileClosed {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiagnosticUpdate {
    pub path: String,
    pub diagnostics: Vec<DiagnosticEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiagnosticEntry {
    pub line: u32,
    pub col: u32,
    pub severity: u8, // 1=Error, 2=Warning, 3=Info, 4=Hint
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub path: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub items: Vec<CompletionEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionEntry {
    pub label: String,
    pub kind: String,
    pub detail: Option<String>,
    pub insert_text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EditorAction {
    pub action: String, // "format", "rename", "go_to_definition", etc.
    pub path: String,
    pub line: u32,
    pub character: u32,
    pub payload: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRegister {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandExecute {
    pub id: String,
}

// --- Encoding/Decoding ---

pub fn encode_message(msg_type: u16, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(6 + payload.len());
    buf.extend_from_slice(&msg_type.to_le_bytes());
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

pub fn decode_message(data: &[u8]) -> Option<(u16, &[u8])> {
    if data.len() < 6 { return None; }
    let msg_type = u16::from_le_bytes([data[0], data[1]]);
    let payload_len = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as usize;
    if data.len() < 6 + payload_len { return None; }
    Some((msg_type, &data[6..6 + payload_len]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encode_decode() {
        let payload = serde_json::to_vec(&FileOpened {
            path: "test.rs".into(),
            language: "rust".into(),
            content: "fn main() {}".into(),
        }).unwrap();

        let encoded = encode_message(MSG_FILE_OPENED, &payload);
        let (msg_type, decoded_payload) = decode_message(&encoded).unwrap();

        assert_eq!(msg_type, MSG_FILE_OPENED);
        assert_eq!(decoded_payload, payload.as_slice());

        let decoded: FileOpened = serde_json::from_slice(decoded_payload).unwrap();
        assert_eq!(decoded.path, "test.rs");
    }

    #[test]
    fn all_message_types_distinct() {
        let types = vec![
            MSG_FILE_OPENED, MSG_FILE_SAVED, MSG_FILE_CLOSED,
            MSG_DIAGNOSTIC_UPDATE, MSG_COMPLETION_REQUEST, MSG_COMPLETION_RESPONSE,
            MSG_EDITOR_ACTION, MSG_STATUS_UPDATE, MSG_TREE_REFRESH,
            MSG_COMMAND_REGISTER, MSG_COMMAND_EXECUTE,
        ];
        let unique: std::collections::HashSet<_> = types.iter().collect();
        assert_eq!(types.len(), unique.len(), "All message types must be unique");
    }
}
