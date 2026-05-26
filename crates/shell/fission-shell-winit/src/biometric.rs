use fission_core::{
    BiometricAuthenticateRequest, BiometricAuthenticateResult, BiometricAvailability,
    BiometricError, BiometricKind, AUTHENTICATE_BIOMETRIC, CANCEL_BIOMETRIC_AUTHENTICATION,
    GET_BIOMETRIC_AVAILABILITY,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::Arc;

/// Host-side biometric provider used by shell capability registration.
pub trait BiometricHost: Send + Sync + 'static {
    /// Returns local biometric support, enrollment, strength, and modality state.
    fn availability(&self) -> Result<BiometricAvailability, BiometricError>;
    /// Prompts the host to verify the current local user.
    ///
    /// `request` provides the prompt reason, optional titles, fallback behavior,
    /// and required strength. The result reports whether verification succeeded.
    fn authenticate(
        &self,
        request: BiometricAuthenticateRequest,
    ) -> Result<BiometricAuthenticateResult, BiometricError>;
    /// Cancels an active biometric prompt when the platform permits it.
    fn cancel_authentication(&self) -> Result<(), BiometricError>;
}

/// Default provider used when the active shell has no biometric integration.
#[derive(Debug, Default)]
pub struct UnsupportedBiometricHost;

impl BiometricHost for UnsupportedBiometricHost {
    fn availability(&self) -> Result<BiometricAvailability, BiometricError> {
        Ok(BiometricAvailability {
            reason: Some("biometric authentication is not supported by this host".into()),
            ..Default::default()
        })
    }

    fn authenticate(
        &self,
        _request: BiometricAuthenticateRequest,
    ) -> Result<BiometricAuthenticateResult, BiometricError> {
        Err(BiometricError::unsupported("authenticate"))
    }

    fn cancel_authentication(&self) -> Result<(), BiometricError> {
        Err(BiometricError::unsupported("cancel_authentication"))
    }
}

/// In-process biometric host for tests and non-OS environments.
#[derive(Debug, Clone)]
pub struct MemoryBiometricHost {
    availability: BiometricAvailability,
    result: BiometricAuthenticateResult,
}

impl MemoryBiometricHost {
    pub fn new(availability: BiometricAvailability, result: BiometricAuthenticateResult) -> Self {
        Self {
            availability,
            result,
        }
    }
}

impl Default for MemoryBiometricHost {
    fn default() -> Self {
        Self {
            availability: BiometricAvailability {
                supported: true,
                enrolled: true,
                strong: true,
                weak: true,
                device_credential: true,
                kinds: vec![BiometricKind::Face, BiometricKind::Fingerprint],
                reason: None,
            },
            result: BiometricAuthenticateResult {
                verified: true,
                kind: Some(BiometricKind::Face),
                used_device_credential: false,
            },
        }
    }
}

impl BiometricHost for MemoryBiometricHost {
    fn availability(&self) -> Result<BiometricAvailability, BiometricError> {
        Ok(self.availability.clone())
    }

    fn authenticate(
        &self,
        _request: BiometricAuthenticateRequest,
    ) -> Result<BiometricAuthenticateResult, BiometricError> {
        Ok(self.result.clone())
    }

    fn cancel_authentication(&self) -> Result<(), BiometricError> {
        Ok(())
    }
}

pub(crate) fn register_biometric_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn BiometricHost>,
) {
    let availability_host = host.clone();
    async_registry.register_operation_capability(GET_BIOMETRIC_AVAILABILITY, move |(), _| {
        let host = availability_host.clone();
        async move { host.availability() }
    });

    let authenticate_host = host.clone();
    async_registry.register_operation_capability(AUTHENTICATE_BIOMETRIC, move |request, _| {
        let host = authenticate_host.clone();
        async move { host.authenticate(request) }
    });

    async_registry.register_operation_capability(CANCEL_BIOMETRIC_AUTHENTICATION, move |(), _| {
        let host = host.clone();
        async move { host.cancel_authentication() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_host_reports_unavailable() {
        let host = UnsupportedBiometricHost;
        let availability = host.availability().unwrap();

        assert!(!availability.supported);
        assert!(host
            .authenticate(BiometricAuthenticateRequest::default())
            .is_err());
    }

    #[test]
    fn memory_host_authenticates() {
        let host = MemoryBiometricHost::default();
        let availability = host.availability().unwrap();
        assert!(availability.supported);

        let result = host
            .authenticate(BiometricAuthenticateRequest {
                reason: "Unlock".into(),
                ..Default::default()
            })
            .unwrap();
        assert!(result.verified);
    }
}
