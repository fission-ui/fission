//! Barcode scanner host capabilities.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BarcodeFormat {
    QrCode,
    Aztec,
    DataMatrix,
    Ean13,
    Ean8,
    Code128,
    Code39,
    Code93,
    Codabar,
    Itf,
    Pdf417,
    UpcA,
    UpcE,
    MaxiCode,
    Rss14,
    RssExpanded,
    Other(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarcodePoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarcodeScanRequest {
    pub formats: Vec<BarcodeFormat>,
    pub prompt: Option<String>,
    pub camera_id: Option<String>,
    pub timeout_ms: Option<u64>,
    pub allow_multiple: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarcodeImageDecodeRequest {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub formats: Vec<BarcodeFormat>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarcodeScanResult {
    pub value: String,
    pub format: BarcodeFormat,
    pub raw_bytes: Vec<u8>,
    pub bounds: Vec<BarcodePoint>,
    pub symbology_identifier: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarcodeScanResults {
    pub items: Vec<BarcodeScanResult>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarcodeScannerError {
    pub code: String,
    pub message: String,
}

impl BarcodeScannerError {
    /// Creates a portable barcode scanner error payload.
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
    /// `operation` should name the attempted barcode scanner operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "barcode scanner operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct ScanBarcodeCapability;
impl OperationCapability for ScanBarcodeCapability {
    type Request = BarcodeScanRequest;
    type Ok = BarcodeScanResults;
    type Err = BarcodeScannerError;
}

pub struct DecodeBarcodeImageCapability;
impl OperationCapability for DecodeBarcodeImageCapability {
    type Request = BarcodeImageDecodeRequest;
    type Ok = BarcodeScanResults;
    type Err = BarcodeScannerError;
}

pub struct CancelBarcodeScanCapability;
impl OperationCapability for CancelBarcodeScanCapability {
    type Request = ();
    type Ok = ();
    type Err = BarcodeScannerError;
}

pub const SCAN_BARCODE: CapabilityType<ScanBarcodeCapability> =
    CapabilityType::new("fission.barcode.scan");
pub const DECODE_BARCODE_IMAGE: CapabilityType<DecodeBarcodeImageCapability> =
    CapabilityType::new("fission.barcode.decode_image");
pub const CANCEL_BARCODE_SCAN: CapabilityType<CancelBarcodeScanCapability> =
    CapabilityType::new("fission.barcode.cancel_scan");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barcode_scan_request_round_trips() {
        let request = BarcodeScanRequest {
            formats: vec![BarcodeFormat::QrCode, BarcodeFormat::Code128],
            prompt: Some("Scan the label".into()),
            camera_id: Some("back".into()),
            timeout_ms: Some(10_000),
            allow_multiple: true,
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: BarcodeScanRequest = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, request);
    }

    #[test]
    fn barcode_results_round_trip() {
        let results = BarcodeScanResults {
            items: vec![BarcodeScanResult {
                value: "https://fission.rs".into(),
                format: BarcodeFormat::QrCode,
                raw_bytes: b"https://fission.rs".to_vec(),
                bounds: vec![BarcodePoint { x: 1, y: 2 }, BarcodePoint { x: 3, y: 4 }],
                symbology_identifier: None,
            }],
        };

        let bytes = serde_json::to_vec(&results).unwrap();
        let decoded: BarcodeScanResults = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, results);
    }
}
