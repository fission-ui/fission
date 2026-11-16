//! Host printing capabilities for PDF documents.

use crate::{platform_filesystem::FileWriteSource, CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

/// Inclusive page range requested for a PDF print job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintPageRange {
    pub first_page: u32,
    pub last_page: u32,
}

/// Requests that the active host present or submit a PDF to its print service.
///
/// The application may provide bytes for a small document or a one-shot stream
/// for larger documents. A successful result means the host accepted or
/// presented the job; it does not claim that paper was physically printed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintPdfRequest {
    pub pdf: FileWriteSource,
    pub job_name: Option<String>,
    /// Number of copies. The default is one.
    pub copies: u32,
    /// Inclusive page ranges. An empty list means all pages.
    pub page_ranges: Vec<PrintPageRange>,
}

impl Default for PrintPdfRequest {
    fn default() -> Self {
        Self {
            pdf: FileWriteSource::default(),
            job_name: None,
            copies: 1,
            page_ranges: Vec::new(),
        }
    }
}

impl PrintPdfRequest {
    /// Checks settings that have the same meaning on every host.
    pub fn validate(&self) -> Result<(), PrintError> {
        if self.copies == 0 || self.copies > 999 {
            return Err(PrintError::new(
                "invalid_copies",
                "PDF print copies must be between 1 and 999",
            ));
        }
        if self
            .page_ranges
            .iter()
            .any(|range| range.first_page == 0 || range.last_page < range.first_page)
        {
            return Err(PrintError::new(
                "invalid_page_range",
                "PDF print page ranges must use positive, increasing page numbers",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintOutcome {
    /// A native print service accepted the job. This does not mean printing
    /// has physically completed.
    Submitted,
    /// A native print sheet or browser dialog was opened. Some hosts cannot
    /// report whether the user later completed or cancelled it.
    Presented,
    /// The user dismissed a print surface before submitting the job.
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintPdfResult {
    pub outcome: PrintOutcome,
    /// A host-provided queue identifier when one is available.
    pub job_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintError {
    pub code: String,
    pub message: String,
}

impl PrintError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "printing operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct PrintPdfCapability;
impl OperationCapability for PrintPdfCapability {
    type Request = PrintPdfRequest;
    type Ok = PrintPdfResult;
    type Err = PrintError;
}

pub const PRINT_PDF: CapabilityType<PrintPdfCapability> = CapabilityType::new("fission.print.pdf");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_pdf_request_round_trips_bytes_and_page_ranges() {
        let request = PrintPdfRequest {
            pdf: FileWriteSource::Bytes(b"%PDF-1.7".to_vec()),
            job_name: Some("Quarterly report".into()),
            copies: 2,
            page_ranges: vec![PrintPageRange {
                first_page: 2,
                last_page: 4,
            }],
        };

        let encoded = serde_json::to_vec(&request).unwrap();
        let decoded: PrintPdfRequest = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn old_print_request_payloads_use_default_settings() {
        let request: PrintPdfRequest =
            serde_json::from_str(r#"{"pdf":{"Bytes":[37,80,68,70,45]},"job_name":null}"#).unwrap();
        assert_eq!(request.copies, 1);
        assert!(request.page_ranges.is_empty());
    }

    #[test]
    fn print_request_validation_rejects_invalid_settings() {
        assert_eq!(
            PrintPdfRequest {
                copies: 0,
                ..Default::default()
            }
            .validate()
            .unwrap_err()
            .code,
            "invalid_copies"
        );
        assert_eq!(
            PrintPdfRequest {
                page_ranges: vec![PrintPageRange {
                    first_page: 3,
                    last_page: 2,
                }],
                ..Default::default()
            }
            .validate()
            .unwrap_err()
            .code,
            "invalid_page_range"
        );
    }
}
