use fission_core::{
    NfcAvailability, NfcEmulationRequest, NfcError, NfcRecord, NfcScanRequest, NfcSessionReceipt,
    NfcTag, NfcTechnology, NfcWriteRequest, CANCEL_NFC_SESSION, EMULATE_NFC_TAG,
    GET_NFC_AVAILABILITY, SCAN_NFC_TAG, WRITE_NFC_TAG,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::Arc;

/// Host-side NFC provider used by shell capability registration.
pub trait NfcHost: Send + Sync + 'static {
    /// Returns NFC support, enabled state, and supported operation modes.
    fn availability(&self) -> Result<NfcAvailability, NfcError>;
    /// Starts a read session and returns the first matching NFC tag.
    ///
    /// `request` carries technology filters, prompt text, timeout, and record
    /// collection behavior.
    fn scan_tag(&self, request: NfcScanRequest) -> Result<NfcTag, NfcError>;
    /// Starts a write session for the supplied NFC records.
    fn write_tag(&self, request: NfcWriteRequest) -> Result<NfcSessionReceipt, NfcError>;
    /// Starts card emulation for hosts and hardware that support it.
    fn emulate_tag(&self, request: NfcEmulationRequest) -> Result<NfcSessionReceipt, NfcError>;
    /// Cancels the active NFC scan, write, or emulation session.
    fn cancel_session(&self) -> Result<(), NfcError>;
}

/// Default provider used when the active shell has no NFC integration.
#[derive(Debug, Default)]
pub struct UnsupportedNfcHost;

impl NfcHost for UnsupportedNfcHost {
    fn availability(&self) -> Result<NfcAvailability, NfcError> {
        Ok(NfcAvailability::default())
    }

    fn scan_tag(&self, _request: NfcScanRequest) -> Result<NfcTag, NfcError> {
        Err(NfcError::unsupported("scan_tag"))
    }

    fn write_tag(&self, _request: NfcWriteRequest) -> Result<NfcSessionReceipt, NfcError> {
        Err(NfcError::unsupported("write_tag"))
    }

    fn emulate_tag(&self, _request: NfcEmulationRequest) -> Result<NfcSessionReceipt, NfcError> {
        Err(NfcError::unsupported("emulate_tag"))
    }

    fn cancel_session(&self) -> Result<(), NfcError> {
        Err(NfcError::unsupported("cancel_session"))
    }
}

/// In-process NFC host for smoke tests and non-OS environments.
#[derive(Debug, Clone)]
pub struct MemoryNfcHost {
    tag: NfcTag,
}

impl MemoryNfcHost {
    pub fn new(tag: NfcTag) -> Self {
        Self { tag }
    }
}

impl Default for MemoryNfcHost {
    fn default() -> Self {
        Self {
            tag: NfcTag {
                id: Some(vec![0xF1, 0x55, 0x10, 0x01]),
                technologies: vec![NfcTechnology::Ndef],
                records: vec![NfcRecord::uri("fission://memory-nfc")],
                raw_payload: None,
            },
        }
    }
}

impl NfcHost for MemoryNfcHost {
    fn availability(&self) -> Result<NfcAvailability, NfcError> {
        Ok(NfcAvailability {
            supported: true,
            enabled: true,
            read: true,
            write: true,
            card_emulation: true,
        })
    }

    fn scan_tag(&self, _request: NfcScanRequest) -> Result<NfcTag, NfcError> {
        Ok(self.tag.clone())
    }

    fn write_tag(&self, request: NfcWriteRequest) -> Result<NfcSessionReceipt, NfcError> {
        Ok(NfcSessionReceipt {
            session_id: Some(format!("memory-nfc-write-{}", request.records.len())),
            completed: true,
        })
    }

    fn emulate_tag(&self, request: NfcEmulationRequest) -> Result<NfcSessionReceipt, NfcError> {
        Ok(NfcSessionReceipt {
            session_id: Some(format!("memory-nfc-emulate-{}", request.records.len())),
            completed: true,
        })
    }

    fn cancel_session(&self) -> Result<(), NfcError> {
        Ok(())
    }
}

pub(crate) fn register_nfc_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn NfcHost>,
) {
    let availability_host = host.clone();
    async_registry.register_operation_capability(GET_NFC_AVAILABILITY, move |(), _| {
        let host = availability_host.clone();
        async move { host.availability() }
    });

    let scan_host = host.clone();
    async_registry.register_operation_capability(SCAN_NFC_TAG, move |request, _| {
        let host = scan_host.clone();
        async move { host.scan_tag(request) }
    });

    let write_host = host.clone();
    async_registry.register_operation_capability(WRITE_NFC_TAG, move |request, _| {
        let host = write_host.clone();
        async move { host.write_tag(request) }
    });

    let emulate_host = host.clone();
    async_registry.register_operation_capability(EMULATE_NFC_TAG, move |request, _| {
        let host = emulate_host.clone();
        async move { host.emulate_tag(request) }
    });

    async_registry.register_operation_capability(CANCEL_NFC_SESSION, move |(), _| {
        let host = host.clone();
        async move { host.cancel_session() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_host_reports_unavailable() {
        let host = UnsupportedNfcHost;
        let availability = host.availability().unwrap();

        assert!(!availability.supported);
        assert!(host.scan_tag(NfcScanRequest::default()).is_err());
    }

    #[test]
    fn memory_host_scans_and_writes() {
        let host = MemoryNfcHost::default();
        let tag = host.scan_tag(NfcScanRequest::default()).unwrap();
        assert_eq!(tag.technologies, vec![NfcTechnology::Ndef]);

        let receipt = host
            .write_tag(NfcWriteRequest {
                records: vec![NfcRecord::text("en", "ok")],
                ..Default::default()
            })
            .unwrap();
        assert!(receipt.completed);
    }
}
