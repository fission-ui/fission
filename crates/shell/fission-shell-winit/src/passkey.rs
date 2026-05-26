use fission_core::{
    PasskeyAuthenticationRequest, PasskeyAuthenticationResult, PasskeyAvailability, PasskeyError,
    PasskeyRegistrationRequest, PasskeyRegistrationResult, AUTHENTICATE_PASSKEY,
    CANCEL_PASSKEY_OPERATION, GET_PASSKEY_AVAILABILITY, REGISTER_PASSKEY,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::Arc;

/// Host-side passkey/WebAuthn provider used by shell capability registration.
pub trait PasskeyHost: Send + Sync + 'static {
    /// Returns passkey support for the current host, origin, and credential backend.
    fn availability(&self) -> Result<PasskeyAvailability, PasskeyError>;
    /// Starts passkey registration and returns WebAuthn registration data.
    ///
    /// `request` contains the relying-party identity, user handle, challenge,
    /// requested algorithms, and authenticator preferences. The returned payload
    /// must be sent to the relying-party server for verification before the app
    /// treats the passkey as enrolled.
    fn register(
        &self,
        request: PasskeyRegistrationRequest,
    ) -> Result<PasskeyRegistrationResult, PasskeyError>;
    /// Starts passkey authentication and returns WebAuthn assertion data.
    ///
    /// `request.challenge` must originate from the server. The returned assertion
    /// proves nothing until the server verifies the challenge, signature,
    /// authenticator data, and credential id against the account.
    fn authenticate(
        &self,
        request: PasskeyAuthenticationRequest,
    ) -> Result<PasskeyAuthenticationResult, PasskeyError>;
    /// Cancels an active passkey prompt where the host allows cancellation.
    fn cancel(&self) -> Result<(), PasskeyError>;
}

/// Default provider used when the active shell has no passkey integration.
#[derive(Debug, Default)]
pub struct UnsupportedPasskeyHost;

impl PasskeyHost for UnsupportedPasskeyHost {
    fn availability(&self) -> Result<PasskeyAvailability, PasskeyError> {
        Ok(PasskeyAvailability {
            reason: Some("passkeys are not supported by this host".into()),
            ..Default::default()
        })
    }

    fn register(
        &self,
        _request: PasskeyRegistrationRequest,
    ) -> Result<PasskeyRegistrationResult, PasskeyError> {
        Err(PasskeyError::unsupported("register"))
    }

    fn authenticate(
        &self,
        _request: PasskeyAuthenticationRequest,
    ) -> Result<PasskeyAuthenticationResult, PasskeyError> {
        Err(PasskeyError::unsupported("authenticate"))
    }

    fn cancel(&self) -> Result<(), PasskeyError> {
        Err(PasskeyError::unsupported("cancel"))
    }
}

/// In-process passkey host for tests and non-OS environments.
#[derive(Debug, Clone)]
pub struct MemoryPasskeyHost {
    availability: PasskeyAvailability,
    registration_result: PasskeyRegistrationResult,
    authentication_result: PasskeyAuthenticationResult,
}

impl MemoryPasskeyHost {
    pub fn new(
        availability: PasskeyAvailability,
        registration_result: PasskeyRegistrationResult,
        authentication_result: PasskeyAuthenticationResult,
    ) -> Self {
        Self {
            availability,
            registration_result,
            authentication_result,
        }
    }
}

impl Default for MemoryPasskeyHost {
    fn default() -> Self {
        Self {
            availability: PasskeyAvailability {
                supported: true,
                secure_context: true,
                platform_authenticator_available: true,
                conditional_ui_available: true,
                cross_platform_authenticator_available: true,
                reason: None,
            },
            registration_result: PasskeyRegistrationResult {
                credential_id: vec![1, 2, 3],
                raw_id: vec![1, 2, 3],
                client_data_json: br#"{"type":"webauthn.create"}"#.to_vec(),
                attestation_object: vec![4, 5, 6],
                authenticator_attachment: None,
                transports: Vec::new(),
            },
            authentication_result: PasskeyAuthenticationResult {
                credential_id: vec![1, 2, 3],
                raw_id: vec![1, 2, 3],
                user_handle: Some(vec![9]),
                client_data_json: br#"{"type":"webauthn.get"}"#.to_vec(),
                authenticator_data: vec![4],
                signature: vec![5, 6],
            },
        }
    }
}

impl PasskeyHost for MemoryPasskeyHost {
    fn availability(&self) -> Result<PasskeyAvailability, PasskeyError> {
        Ok(self.availability.clone())
    }

    fn register(
        &self,
        _request: PasskeyRegistrationRequest,
    ) -> Result<PasskeyRegistrationResult, PasskeyError> {
        Ok(self.registration_result.clone())
    }

    fn authenticate(
        &self,
        _request: PasskeyAuthenticationRequest,
    ) -> Result<PasskeyAuthenticationResult, PasskeyError> {
        Ok(self.authentication_result.clone())
    }

    fn cancel(&self) -> Result<(), PasskeyError> {
        Ok(())
    }
}

pub(crate) fn register_passkey_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn PasskeyHost>,
) {
    let availability_host = host.clone();
    async_registry.register_operation_capability(GET_PASSKEY_AVAILABILITY, move |(), _| {
        let host = availability_host.clone();
        async move { host.availability() }
    });

    let register_host = host.clone();
    async_registry.register_operation_capability(REGISTER_PASSKEY, move |request, _| {
        let host = register_host.clone();
        async move { host.register(request) }
    });

    let authenticate_host = host.clone();
    async_registry.register_operation_capability(AUTHENTICATE_PASSKEY, move |request, _| {
        let host = authenticate_host.clone();
        async move { host.authenticate(request) }
    });

    async_registry.register_operation_capability(CANCEL_PASSKEY_OPERATION, move |(), _| {
        let host = host.clone();
        async move { host.cancel() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::{
        PasskeyAlgorithm, PasskeyAttestationConveyance, PasskeyAuthenticationRequest,
        PasskeyMediation, PasskeyRegistrationRequest, PasskeyRelyingParty, PasskeyUser,
        PasskeyUserVerification,
    };

    #[test]
    fn unsupported_host_reports_unavailable() {
        let host = UnsupportedPasskeyHost;
        let availability = host.availability().unwrap();

        assert!(!availability.supported);
        assert!(host
            .authenticate(PasskeyAuthenticationRequest {
                relying_party_id: "example.com".into(),
                challenge: vec![1],
                allow_credentials: Vec::new(),
                user_verification: PasskeyUserVerification::Preferred,
                mediation: PasskeyMediation::PlatformDefault,
                timeout_ms: None,
            })
            .is_err());
    }

    #[test]
    fn memory_host_registers_and_authenticates() {
        let host = MemoryPasskeyHost::default();
        let availability = host.availability().unwrap();
        assert!(availability.supported);

        let registration = host
            .register(PasskeyRegistrationRequest {
                relying_party: PasskeyRelyingParty::new("example.com", "Example"),
                user: PasskeyUser::new(vec![1], "ada@example.com", "Ada"),
                challenge: vec![2],
                pub_key_algorithms: vec![PasskeyAlgorithm::ES256],
                timeout_ms: None,
                attestation: PasskeyAttestationConveyance::None,
                authenticator_selection: None,
                exclude_credentials: Vec::new(),
            })
            .unwrap();
        assert_eq!(registration.credential_id, vec![1, 2, 3]);

        let authentication = host
            .authenticate(PasskeyAuthenticationRequest {
                relying_party_id: "example.com".into(),
                challenge: vec![3],
                allow_credentials: Vec::new(),
                user_verification: PasskeyUserVerification::Preferred,
                mediation: PasskeyMediation::PlatformDefault,
                timeout_ms: None,
            })
            .unwrap();
        assert_eq!(authentication.credential_id, vec![1, 2, 3]);
    }
}
