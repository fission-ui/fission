//! Passkey and WebAuthn host capabilities.
//!
//! Passkeys are credential assertions, not raw biometric access. A browser,
//! operating system, password manager, or hardware authenticator may use a
//! fingerprint, face check, device PIN, or security key touch to unlock the
//! credential, but the app receives WebAuthn-style registration or
//! authentication data for server verification.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

/// Host support state for passkey operations in the active session.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyAvailability {
    /// `true` when the active host can perform at least one passkey operation.
    pub supported: bool,
    /// `true` when the origin or app context satisfies secure-context rules.
    pub secure_context: bool,
    /// `true` when a built-in platform authenticator is available.
    pub platform_authenticator_available: bool,
    /// `true` when the host can present passkeys as conditional sign-in UI.
    pub conditional_ui_available: bool,
    /// `true` when roaming authenticators such as hardware security keys may be used.
    pub cross_platform_authenticator_available: bool,
    /// Human-readable explanation when support is unavailable or degraded.
    pub reason: Option<String>,
}

/// Relying-party identity used for passkey creation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyRelyingParty {
    /// Relying-party id, normally the effective domain that owns the credential.
    pub id: String,
    /// Human-readable product or organization name shown during registration.
    pub name: String,
}

impl PasskeyRelyingParty {
    /// Creates a relying-party identity.
    ///
    /// `id` is normally the effective domain that owns the credential, such as
    /// `example.com`. `name` is the user-facing product or organization name the
    /// authenticator may show during registration.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// User identity supplied during passkey registration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyUser {
    /// Stable opaque user handle supplied by the server.
    pub id: Vec<u8>,
    /// Account identifier, often an email address or username.
    pub name: String,
    /// Human-readable account name shown by authenticators.
    pub display_name: String,
}

impl PasskeyUser {
    /// Creates a user identity for passkey registration.
    ///
    /// `id` should be a stable opaque server-side user handle, not an email
    /// address. `name` is often the account login identifier, while
    /// `display_name` is the human-readable name shown by authenticators.
    pub fn new(
        id: impl Into<Vec<u8>>,
        name: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            display_name: display_name.into(),
        }
    }
}

/// Authenticator transport hints for existing credentials.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasskeyTransport {
    /// USB-connected authenticator.
    Usb,
    /// Near-field communication authenticator.
    Nfc,
    /// Bluetooth Low Energy authenticator.
    Ble,
    /// Platform authenticator built into the device.
    Internal,
    /// Hybrid transport such as phone-assisted sign-in.
    Hybrid,
    /// Transport was not supplied or is not recognized by this Fission version.
    Unknown,
}

/// Public-key algorithm requested during credential creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasskeyAlgorithm {
    /// ECDSA with SHA-256.
    ES256,
    /// RSA PKCS#1 with SHA-256.
    RS256,
    /// Edwards-curve signatures.
    EdDSA,
    /// COSE algorithm id not represented by the named variants.
    Other(i32),
}

/// How strongly the authenticator should verify the user.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasskeyUserVerification {
    /// The authenticator must verify the local user.
    Required,
    /// The authenticator should verify the local user when possible.
    Preferred,
    /// The authenticator should avoid user verification when possible.
    Discouraged,
    /// Let the host choose its platform default.
    #[default]
    PlatformDefault,
}

/// Authenticator attachment preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasskeyAuthenticatorAttachment {
    /// Prefer an authenticator built into the device.
    Platform,
    /// Prefer a roaming authenticator such as a security key or phone.
    CrossPlatform,
}

/// Resident-key requirement during passkey creation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasskeyResidentKeyRequirement {
    /// A discoverable credential is required.
    Required,
    /// A discoverable credential is preferred.
    Preferred,
    /// A discoverable credential should be avoided when possible.
    Discouraged,
    /// Let the host choose its platform default.
    #[default]
    PlatformDefault,
}

/// Attestation conveyance preference for passkey registration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasskeyAttestationConveyance {
    /// Do not request attestation.
    #[default]
    None,
    /// Request indirect attestation where the host supports it.
    Indirect,
    /// Request direct attestation where the host supports it.
    Direct,
    /// Request enterprise attestation where policy permits it.
    Enterprise,
}

/// Browser mediation preference for passkey authentication.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasskeyMediation {
    /// Attempt authentication without visible UI where the host supports it.
    Silent,
    /// Allow optional UI.
    Optional,
    /// Allow conditional passkey UI, commonly used for sign-in forms.
    Conditional,
    /// Require visible mediation.
    Required,
    /// Let the host choose its platform default.
    #[default]
    PlatformDefault,
}

/// Existing credential descriptor used for allow/exclude lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyCredentialDescriptor {
    /// Raw credential id returned by a previous registration.
    pub id: Vec<u8>,
    /// Optional transport hints stored with the credential.
    pub transports: Vec<PasskeyTransport>,
}

impl PasskeyCredentialDescriptor {
    /// Creates a credential descriptor for allow/exclude lists.
    ///
    /// `id` is the raw credential id returned by a prior registration. Transport
    /// hints are optional; pass an empty list when the server has not stored
    /// transport information.
    pub fn new(id: impl Into<Vec<u8>>, transports: Vec<PasskeyTransport>) -> Self {
        Self {
            id: id.into(),
            transports,
        }
    }
}

/// Authenticator selection preferences for registration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyAuthenticatorSelection {
    /// Optional platform-vs-roaming authenticator preference.
    pub attachment: Option<PasskeyAuthenticatorAttachment>,
    /// Resident-key requirement for the new credential.
    pub resident_key: PasskeyResidentKeyRequirement,
    /// User-verification requirement for the registration prompt.
    pub user_verification: PasskeyUserVerification,
}

/// Request to create and register a new passkey.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyRegistrationRequest {
    /// Relying party that owns the credential.
    pub relying_party: PasskeyRelyingParty,
    /// User account identity supplied by the server.
    pub user: PasskeyUser,
    /// Fresh challenge generated by the relying-party server.
    pub challenge: Vec<u8>,
    /// Public-key algorithms the server is willing to accept.
    pub pub_key_algorithms: Vec<PasskeyAlgorithm>,
    /// Optional host prompt timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Attestation preference for the credential creation request.
    pub attestation: PasskeyAttestationConveyance,
    /// Optional authenticator selection preferences.
    pub authenticator_selection: Option<PasskeyAuthenticatorSelection>,
    /// Credentials that should not be registered again for this account.
    pub exclude_credentials: Vec<PasskeyCredentialDescriptor>,
}

/// Successful passkey registration payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyRegistrationResult {
    /// Credential id to store on the server after verification succeeds.
    pub credential_id: Vec<u8>,
    /// Raw credential id as returned by the authenticator.
    pub raw_id: Vec<u8>,
    /// Client data JSON bytes that the server must verify.
    pub client_data_json: Vec<u8>,
    /// Attestation object bytes that the server must verify.
    pub attestation_object: Vec<u8>,
    /// Authenticator attachment actually used, when reported by the host.
    pub authenticator_attachment: Option<PasskeyAuthenticatorAttachment>,
    /// Transports reported for the created credential.
    pub transports: Vec<PasskeyTransport>,
}

/// Request to authenticate with an existing passkey.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyAuthenticationRequest {
    /// Relying-party id whose credential should be used.
    pub relying_party_id: String,
    /// Fresh authentication challenge generated by the server.
    pub challenge: Vec<u8>,
    /// Optional allow-list of credentials that may satisfy the assertion.
    pub allow_credentials: Vec<PasskeyCredentialDescriptor>,
    /// User-verification requirement for the authentication prompt.
    pub user_verification: PasskeyUserVerification,
    /// Mediation preference for the authentication prompt.
    pub mediation: PasskeyMediation,
    /// Optional host prompt timeout in milliseconds.
    pub timeout_ms: Option<u64>,
}

/// Successful passkey authentication payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyAuthenticationResult {
    /// Credential id used for the assertion.
    pub credential_id: Vec<u8>,
    /// Raw credential id as returned by the authenticator.
    pub raw_id: Vec<u8>,
    /// Optional opaque user handle returned by the authenticator.
    pub user_handle: Option<Vec<u8>>,
    /// Client data JSON bytes that the server must verify.
    pub client_data_json: Vec<u8>,
    /// Authenticator data bytes that the server must verify.
    pub authenticator_data: Vec<u8>,
    /// Assertion signature that the server must verify.
    pub signature: Vec<u8>,
}

/// Portable passkey error payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyError {
    /// Stable machine-readable error code.
    pub code: String,
    /// Human-readable error message for logs or developer UI.
    pub message: String,
}

impl PasskeyError {
    /// Creates a portable passkey error payload.
    ///
    /// `code` should be stable and machine-readable, such as `unsupported`,
    /// `not_allowed`, `invalid_state`, or `security_error`. `message` should be
    /// a concise explanation for logs, diagnostics, or developer-facing UI.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Creates the standard unsupported-operation error for this capability.
    ///
    /// Use this from hosts that implement the Fission provider trait but cannot
    /// perform the requested passkey operation on the active platform, origin, or
    /// credential backend.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "passkey operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct GetPasskeyAvailabilityCapability;
impl OperationCapability for GetPasskeyAvailabilityCapability {
    type Request = ();
    type Ok = PasskeyAvailability;
    type Err = PasskeyError;
}

pub struct RegisterPasskeyCapability;
impl OperationCapability for RegisterPasskeyCapability {
    type Request = PasskeyRegistrationRequest;
    type Ok = PasskeyRegistrationResult;
    type Err = PasskeyError;
}

pub struct AuthenticatePasskeyCapability;
impl OperationCapability for AuthenticatePasskeyCapability {
    type Request = PasskeyAuthenticationRequest;
    type Ok = PasskeyAuthenticationResult;
    type Err = PasskeyError;
}

pub struct CancelPasskeyOperationCapability;
impl OperationCapability for CancelPasskeyOperationCapability {
    type Request = ();
    type Ok = ();
    type Err = PasskeyError;
}

pub const GET_PASSKEY_AVAILABILITY: CapabilityType<GetPasskeyAvailabilityCapability> =
    CapabilityType::new("fission.passkey.get_availability");
pub const REGISTER_PASSKEY: CapabilityType<RegisterPasskeyCapability> =
    CapabilityType::new("fission.passkey.register");
pub const AUTHENTICATE_PASSKEY: CapabilityType<AuthenticatePasskeyCapability> =
    CapabilityType::new("fission.passkey.authenticate");
pub const CANCEL_PASSKEY_OPERATION: CapabilityType<CancelPasskeyOperationCapability> =
    CapabilityType::new("fission.passkey.cancel");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passkey_registration_request_round_trips() {
        let request = PasskeyRegistrationRequest {
            relying_party: PasskeyRelyingParty::new("example.com", "Example"),
            user: PasskeyUser::new(vec![1, 2, 3], "ada@example.com", "Ada"),
            challenge: vec![9, 8, 7],
            pub_key_algorithms: vec![PasskeyAlgorithm::ES256, PasskeyAlgorithm::EdDSA],
            timeout_ms: Some(60_000),
            attestation: PasskeyAttestationConveyance::None,
            authenticator_selection: Some(PasskeyAuthenticatorSelection {
                attachment: Some(PasskeyAuthenticatorAttachment::Platform),
                resident_key: PasskeyResidentKeyRequirement::Required,
                user_verification: PasskeyUserVerification::Required,
            }),
            exclude_credentials: vec![PasskeyCredentialDescriptor::new(
                vec![4, 5, 6],
                vec![PasskeyTransport::Internal],
            )],
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: PasskeyRegistrationRequest = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, request);
    }

    #[test]
    fn passkey_authentication_result_round_trips() {
        let result = PasskeyAuthenticationResult {
            credential_id: vec![1, 2, 3],
            raw_id: vec![1, 2, 3],
            user_handle: Some(vec![7, 8]),
            client_data_json: br#"{"type":"webauthn.get"}"#.to_vec(),
            authenticator_data: vec![9],
            signature: vec![10, 11],
        };

        let bytes = serde_json::to_vec(&result).unwrap();
        let decoded: PasskeyAuthenticationResult = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, result);
    }
}
