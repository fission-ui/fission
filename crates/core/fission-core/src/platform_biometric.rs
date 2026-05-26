//! Biometric authentication host capabilities.
//!
//! These types describe what the app asks the host to do. The shell owns the
//! OS-specific authentication prompt and returns only portable results.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

/// Biometric modality reported by a host.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BiometricKind {
    Face,
    Fingerprint,
    Iris,
    DeviceCredential,
    Unknown,
}

/// Strength requested by an app or reported by a host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BiometricStrength {
    #[default]
    Any,
    Weak,
    Strong,
}

/// Biometric support state for the active device/session.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiometricAvailability {
    pub supported: bool,
    pub enrolled: bool,
    pub strong: bool,
    pub weak: bool,
    pub device_credential: bool,
    pub kinds: Vec<BiometricKind>,
    pub reason: Option<String>,
}

/// Request to authenticate the current user.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiometricAuthenticateRequest {
    pub reason: String,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub fallback_title: Option<String>,
    pub cancel_title: Option<String>,
    pub allow_device_credential: bool,
    pub required_strength: BiometricStrength,
}

impl Default for BiometricAuthenticateRequest {
    fn default() -> Self {
        Self {
            reason: String::new(),
            title: None,
            subtitle: None,
            fallback_title: None,
            cancel_title: None,
            allow_device_credential: true,
            required_strength: BiometricStrength::Any,
        }
    }
}

/// Successful biometric authentication response.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiometricAuthenticateResult {
    pub verified: bool,
    pub kind: Option<BiometricKind>,
    pub used_device_credential: bool,
}

/// Portable biometric error payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiometricError {
    pub code: String,
    pub message: String,
}

impl BiometricError {
    /// Creates a portable biometric error payload.
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
    /// `operation` should name the attempted biometric operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "biometric operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct GetBiometricAvailabilityCapability;
impl OperationCapability for GetBiometricAvailabilityCapability {
    type Request = ();
    type Ok = BiometricAvailability;
    type Err = BiometricError;
}

pub struct AuthenticateBiometricCapability;
impl OperationCapability for AuthenticateBiometricCapability {
    type Request = BiometricAuthenticateRequest;
    type Ok = BiometricAuthenticateResult;
    type Err = BiometricError;
}

pub struct CancelBiometricAuthenticationCapability;
impl OperationCapability for CancelBiometricAuthenticationCapability {
    type Request = ();
    type Ok = ();
    type Err = BiometricError;
}

pub const GET_BIOMETRIC_AVAILABILITY: CapabilityType<GetBiometricAvailabilityCapability> =
    CapabilityType::new("fission.biometric.get_availability");
pub const AUTHENTICATE_BIOMETRIC: CapabilityType<AuthenticateBiometricCapability> =
    CapabilityType::new("fission.biometric.authenticate");
pub const CANCEL_BIOMETRIC_AUTHENTICATION: CapabilityType<CancelBiometricAuthenticationCapability> =
    CapabilityType::new("fission.biometric.cancel_authentication");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biometric_request_round_trips() {
        let request = BiometricAuthenticateRequest {
            reason: "Unlock encrypted notes".into(),
            title: Some("Unlock".into()),
            subtitle: Some("Confirm it is you".into()),
            fallback_title: Some("Use passcode".into()),
            cancel_title: Some("Cancel".into()),
            allow_device_credential: true,
            required_strength: BiometricStrength::Strong,
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: BiometricAuthenticateRequest = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, request);
    }

    #[test]
    fn availability_round_trips() {
        let availability = BiometricAvailability {
            supported: true,
            enrolled: true,
            strong: true,
            weak: true,
            device_credential: true,
            kinds: vec![BiometricKind::Face, BiometricKind::Fingerprint],
            reason: None,
        };

        let bytes = serde_json::to_vec(&availability).unwrap();
        let decoded: BiometricAvailability = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, availability);
    }
}
