//! NFC host capabilities.
//!
//! The core crate only defines portable typed requests, results, and capability
//! identities. Shells decide which platform NFC APIs can satisfy them.

use crate::action::{Action, ActionId};
use crate::capability::{CapabilityType, OperationCapability};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

/// NFC operation support reported by the active shell.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NfcAvailability {
    pub supported: bool,
    pub enabled: bool,
    pub read: bool,
    pub write: bool,
    pub card_emulation: bool,
}

/// NFC technology family requested by an app or discovered on a tag.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NfcTechnology {
    IsoDep,
    NfcA,
    NfcB,
    NfcF,
    NfcV,
    Ndef,
    MifareClassic,
    MifareUltralight,
    Felica,
    Other(String),
}

/// NFC Forum NDEF type-name-format value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NfcRecordTypeNameFormat {
    Empty,
    #[default]
    WellKnown,
    MimeMedia,
    AbsoluteUri,
    External,
    Unknown,
    Unchanged,
    Reserved(u8),
}

/// Portable NDEF-like record payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NfcRecord {
    pub type_name_format: NfcRecordTypeNameFormat,
    pub type_name: Vec<u8>,
    pub id: Vec<u8>,
    pub payload: Vec<u8>,
}

impl NfcRecord {
    /// Creates a portable NFC text record.
    ///
    /// `language` should be a BCP-47-style language code such as `en` or
    /// `en-GB`. `text` is encoded as UTF-8 in the record payload so the host can
    /// write or emulate an NDEF text value without app code building raw bytes.
    pub fn text(language: impl Into<String>, text: impl Into<String>) -> Self {
        let language = language.into();
        let text = text.into();
        let mut payload = Vec::with_capacity(1 + language.len() + text.len());
        payload.push(language.len().min(63) as u8);
        payload.extend_from_slice(language.as_bytes());
        payload.extend_from_slice(text.as_bytes());
        Self {
            type_name_format: NfcRecordTypeNameFormat::WellKnown,
            type_name: b"T".to_vec(),
            id: Vec::new(),
            payload,
        }
    }

    /// Creates a portable NFC URI record.
    ///
    /// `uri` should be the full URI the tag should carry. The helper stores it as
    /// a UTF-8 payload with a URI record type so reducers do not need to know the
    /// raw NDEF byte layout.
    pub fn uri(uri: impl Into<String>) -> Self {
        let uri = uri.into();
        let mut payload = Vec::with_capacity(1 + uri.len());
        payload.push(0);
        payload.extend_from_slice(uri.as_bytes());
        Self {
            type_name_format: NfcRecordTypeNameFormat::WellKnown,
            type_name: b"U".to_vec(),
            id: Vec::new(),
            payload,
        }
    }
}

/// A tag returned by a scan operation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NfcTag {
    pub id: Option<Vec<u8>>,
    pub technologies: Vec<NfcTechnology>,
    pub records: Vec<NfcRecord>,
    pub raw_payload: Option<Vec<u8>>,
}

/// One-shot NFC read request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NfcScanRequest {
    pub technologies: Vec<NfcTechnology>,
    pub message: Option<String>,
    pub timeout_ms: Option<u64>,
    pub read_multiple_records: bool,
}

/// NFC write request. Hosts may require the user to tap a writable tag after
/// this operation starts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NfcWriteRequest {
    pub records: Vec<NfcRecord>,
    pub message: Option<String>,
    pub timeout_ms: Option<u64>,
    pub make_read_only: bool,
}

/// NFC card-emulation request for hosts that support it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NfcEmulationRequest {
    pub records: Vec<NfcRecord>,
    pub message: Option<String>,
    pub timeout_ms: Option<u64>,
}

/// Receipt for write/emulation/session operations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NfcSessionReceipt {
    pub session_id: Option<String>,
    pub completed: bool,
}

/// Portable NFC error payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NfcError {
    pub code: String,
    pub message: String,
}

impl NfcError {
    /// Creates a portable NFC error payload.
    ///
    /// `code` should be stable enough for reducers or tests to match. `message`
    /// should explain the host failure, such as missing hardware, disabled NFC,
    /// timeout, incompatible tag, or unsupported write/emulation mode.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Creates the standard unsupported-operation NFC error.
    ///
    /// `operation` should name the attempted NFC operation, for example `scan`,
    /// `write`, or `emulate`. Hosts should use this when the capability exists
    /// but the current platform or hardware cannot perform that operation.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "NFC operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct GetNfcAvailabilityCapability;
impl OperationCapability for GetNfcAvailabilityCapability {
    type Request = ();
    type Ok = NfcAvailability;
    type Err = NfcError;
}

pub struct ScanNfcTagCapability;
impl OperationCapability for ScanNfcTagCapability {
    type Request = NfcScanRequest;
    type Ok = NfcTag;
    type Err = NfcError;
}

pub struct WriteNfcTagCapability;
impl OperationCapability for WriteNfcTagCapability {
    type Request = NfcWriteRequest;
    type Ok = NfcSessionReceipt;
    type Err = NfcError;
}

pub struct EmulateNfcTagCapability;
impl OperationCapability for EmulateNfcTagCapability {
    type Request = NfcEmulationRequest;
    type Ok = NfcSessionReceipt;
    type Err = NfcError;
}

pub struct CancelNfcSessionCapability;
impl OperationCapability for CancelNfcSessionCapability {
    type Request = ();
    type Ok = ();
    type Err = NfcError;
}

pub const GET_NFC_AVAILABILITY: CapabilityType<GetNfcAvailabilityCapability> =
    CapabilityType::new("fission.nfc.get_availability");
pub const SCAN_NFC_TAG: CapabilityType<ScanNfcTagCapability> =
    CapabilityType::new("fission.nfc.scan_tag");
pub const WRITE_NFC_TAG: CapabilityType<WriteNfcTagCapability> =
    CapabilityType::new("fission.nfc.write_tag");
pub const EMULATE_NFC_TAG: CapabilityType<EmulateNfcTagCapability> =
    CapabilityType::new("fission.nfc.emulate_tag");
pub const CANCEL_NFC_SESSION: CapabilityType<CancelNfcSessionCapability> =
    CapabilityType::new("fission.nfc.cancel_session");

/// Built-in action for host-delivered NFC tag events.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NfcTagDiscovered {
    pub tag: NfcTag,
}

impl Action for NfcTagDiscovered {
    fn static_id() -> ActionId {
        lazy_static! {
            static ref ID: ActionId = ActionId::from_name("fission_core::NfcTagDiscovered");
        }
        *ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_records_round_trip() {
        let records = vec![
            NfcRecord::text("en", "Tap received"),
            NfcRecord::uri("https://fission.rs/docs"),
        ];
        let bytes = serde_json::to_vec(&records).unwrap();
        let decoded: Vec<NfcRecord> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, records);
    }

    #[test]
    fn nfc_scan_request_round_trips() {
        let request = NfcScanRequest {
            technologies: vec![NfcTechnology::Ndef, NfcTechnology::IsoDep],
            message: Some("Hold near the tag".into()),
            timeout_ms: Some(30_000),
            read_multiple_records: true,
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: NfcScanRequest = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, request);
    }

    #[test]
    fn nfc_inbound_action_round_trips() {
        let action = NfcTagDiscovered {
            tag: NfcTag {
                id: Some(vec![1, 2, 3, 4]),
                technologies: vec![NfcTechnology::Ndef],
                records: vec![NfcRecord::uri("fission://open/1")],
                raw_payload: None,
            },
        };

        let envelope: crate::ActionEnvelope = action.clone().into();
        let decoded: NfcTagDiscovered = serde_json::from_slice(&envelope.payload).unwrap();

        assert_eq!(decoded, action);
    }
}
