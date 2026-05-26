use fission_core::{
    BarcodeFormat, BarcodeImageDecodeRequest, BarcodePoint, BarcodeScanRequest, BarcodeScanResult,
    BarcodeScanResults, BarcodeScannerError, CANCEL_BARCODE_SCAN, DECODE_BARCODE_IMAGE,
    SCAN_BARCODE,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::Arc;

/// Host-side barcode scanner provider.
pub trait BarcodeScannerHost: Send + Sync + 'static {
    /// Runs a live barcode scanning session and returns decoded results.
    fn scan(&self, request: BarcodeScanRequest) -> Result<BarcodeScanResults, BarcodeScannerError>;
    /// Decodes barcode results from image bytes supplied by the app.
    fn decode_image(
        &self,
        request: BarcodeImageDecodeRequest,
    ) -> Result<BarcodeScanResults, BarcodeScannerError>;
    /// Cancels the active live barcode scanning session.
    fn cancel_scan(&self) -> Result<(), BarcodeScannerError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedBarcodeScannerHost;

impl BarcodeScannerHost for UnsupportedBarcodeScannerHost {
    fn scan(
        &self,
        _request: BarcodeScanRequest,
    ) -> Result<BarcodeScanResults, BarcodeScannerError> {
        Err(BarcodeScannerError::unsupported("scan"))
    }

    fn decode_image(
        &self,
        _request: BarcodeImageDecodeRequest,
    ) -> Result<BarcodeScanResults, BarcodeScannerError> {
        Err(BarcodeScannerError::unsupported("decode_image"))
    }

    fn cancel_scan(&self) -> Result<(), BarcodeScannerError> {
        Err(BarcodeScannerError::unsupported("cancel_scan"))
    }
}

#[derive(Debug, Clone)]
pub struct MemoryBarcodeScannerHost {
    results: BarcodeScanResults,
}

impl MemoryBarcodeScannerHost {
    pub fn new(results: BarcodeScanResults) -> Self {
        Self { results }
    }
}

impl Default for MemoryBarcodeScannerHost {
    fn default() -> Self {
        Self {
            results: BarcodeScanResults {
                items: vec![BarcodeScanResult {
                    value: "fission://barcode/memory".into(),
                    format: BarcodeFormat::QrCode,
                    raw_bytes: b"fission://barcode/memory".to_vec(),
                    bounds: vec![
                        BarcodePoint { x: 0, y: 0 },
                        BarcodePoint { x: 64, y: 0 },
                        BarcodePoint { x: 64, y: 64 },
                        BarcodePoint { x: 0, y: 64 },
                    ],
                    symbology_identifier: None,
                }],
            },
        }
    }
}

impl BarcodeScannerHost for MemoryBarcodeScannerHost {
    fn scan(
        &self,
        _request: BarcodeScanRequest,
    ) -> Result<BarcodeScanResults, BarcodeScannerError> {
        Ok(self.results.clone())
    }

    fn decode_image(
        &self,
        _request: BarcodeImageDecodeRequest,
    ) -> Result<BarcodeScanResults, BarcodeScannerError> {
        Ok(self.results.clone())
    }

    fn cancel_scan(&self) -> Result<(), BarcodeScannerError> {
        Ok(())
    }
}

pub(crate) fn register_barcode_scanner_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn BarcodeScannerHost>,
) {
    let scan_host = host.clone();
    async_registry.register_operation_capability(SCAN_BARCODE, move |request, _| {
        let host = scan_host.clone();
        async move { host.scan(request) }
    });

    let decode_host = host.clone();
    async_registry.register_operation_capability(DECODE_BARCODE_IMAGE, move |request, _| {
        let host = decode_host.clone();
        async move { host.decode_image(request) }
    });

    async_registry.register_operation_capability(CANCEL_BARCODE_SCAN, move |(), _| {
        let host = host.clone();
        async move { host.cancel_scan() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_host_reports_errors() {
        let host = UnsupportedBarcodeScannerHost;
        assert!(host.scan(BarcodeScanRequest::default()).is_err());
        assert!(host
            .decode_image(BarcodeImageDecodeRequest::default())
            .is_err());
    }

    #[test]
    fn memory_host_scans_and_decodes() {
        let host = MemoryBarcodeScannerHost::default();
        let scan = host.scan(BarcodeScanRequest::default()).unwrap();
        let decode = host
            .decode_image(BarcodeImageDecodeRequest::default())
            .unwrap();
        assert_eq!(scan, decode);
        assert_eq!(scan.items[0].format, BarcodeFormat::QrCode);
    }
}
