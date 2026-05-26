//! Clipboard host capabilities.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardText {
    pub text: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardWriteTextRequest {
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub content_type: String,
    pub bytes: Vec<u8>,
    pub suggested_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardContent {
    pub items: Vec<ClipboardItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardError {
    pub code: String,
    pub message: String,
}

impl ClipboardError {
    /// Creates a portable clipboard error payload.
    ///
    /// `code` should be a stable, machine-readable reason such as
    /// `unsupported`, `permission_denied`, or `timeout`. `message` should be a
    /// concise human-readable explanation suitable for logs or developer-facing
    /// diagnostics.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Creates the standard unsupported-operation error for this capability.
    ///
    /// `operation` should name the attempted clipboard operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "clipboard operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct ReadClipboardTextCapability;
impl OperationCapability for ReadClipboardTextCapability {
    type Request = ();
    type Ok = ClipboardText;
    type Err = ClipboardError;
}

pub struct WriteClipboardTextCapability;
impl OperationCapability for WriteClipboardTextCapability {
    type Request = ClipboardWriteTextRequest;
    type Ok = ();
    type Err = ClipboardError;
}

pub struct ReadClipboardContentCapability;
impl OperationCapability for ReadClipboardContentCapability {
    type Request = ();
    type Ok = ClipboardContent;
    type Err = ClipboardError;
}

pub struct WriteClipboardContentCapability;
impl OperationCapability for WriteClipboardContentCapability {
    type Request = ClipboardContent;
    type Ok = ();
    type Err = ClipboardError;
}

pub struct ClearClipboardCapability;
impl OperationCapability for ClearClipboardCapability {
    type Request = ();
    type Ok = ();
    type Err = ClipboardError;
}

pub const READ_CLIPBOARD_TEXT: CapabilityType<ReadClipboardTextCapability> =
    CapabilityType::new("fission.clipboard.read_text");
pub const WRITE_CLIPBOARD_TEXT: CapabilityType<WriteClipboardTextCapability> =
    CapabilityType::new("fission.clipboard.write_text");
pub const READ_CLIPBOARD_CONTENT: CapabilityType<ReadClipboardContentCapability> =
    CapabilityType::new("fission.clipboard.read_content");
pub const WRITE_CLIPBOARD_CONTENT: CapabilityType<WriteClipboardContentCapability> =
    CapabilityType::new("fission.clipboard.write_content");
pub const CLEAR_CLIPBOARD: CapabilityType<ClearClipboardCapability> =
    CapabilityType::new("fission.clipboard.clear");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_round_trips() {
        let request = ClipboardWriteTextRequest {
            text: "copy me".into(),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: ClipboardWriteTextRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn clipboard_content_round_trips() {
        let content = ClipboardContent {
            items: vec![ClipboardItem {
                content_type: "text/plain".into(),
                bytes: b"copy me".to_vec(),
                suggested_name: None,
            }],
        };
        let bytes = serde_json::to_vec(&content).unwrap();
        let decoded: ClipboardContent = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, content);
    }
}
