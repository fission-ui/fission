//! Haptics host capabilities.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HapticImpactStyle {
    Light,
    #[default]
    Medium,
    Heavy,
    Soft,
    Rigid,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HapticNotificationKind {
    #[default]
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HapticImpactRequest {
    pub style: HapticImpactStyle,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HapticNotificationRequest {
    pub kind: HapticNotificationKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HapticPatternStep {
    pub duration_ms: u64,
    pub intensity: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HapticPatternRequest {
    pub steps: Vec<HapticPatternStep>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HapticError {
    pub code: String,
    pub message: String,
}

impl HapticError {
    /// Creates a portable haptic error payload.
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
    /// `operation` should name the attempted haptic operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "haptic operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct HapticImpactCapability;
impl OperationCapability for HapticImpactCapability {
    type Request = HapticImpactRequest;
    type Ok = ();
    type Err = HapticError;
}

pub struct HapticNotificationCapability;
impl OperationCapability for HapticNotificationCapability {
    type Request = HapticNotificationRequest;
    type Ok = ();
    type Err = HapticError;
}

pub struct HapticSelectionCapability;
impl OperationCapability for HapticSelectionCapability {
    type Request = ();
    type Ok = ();
    type Err = HapticError;
}

pub struct HapticPatternCapability;
impl OperationCapability for HapticPatternCapability {
    type Request = HapticPatternRequest;
    type Ok = ();
    type Err = HapticError;
}

pub const HAPTIC_IMPACT: CapabilityType<HapticImpactCapability> =
    CapabilityType::new("fission.haptics.impact");
pub const HAPTIC_NOTIFICATION: CapabilityType<HapticNotificationCapability> =
    CapabilityType::new("fission.haptics.notification");
pub const HAPTIC_SELECTION: CapabilityType<HapticSelectionCapability> =
    CapabilityType::new("fission.haptics.selection");
pub const HAPTIC_PATTERN: CapabilityType<HapticPatternCapability> =
    CapabilityType::new("fission.haptics.pattern");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haptic_requests_round_trip() {
        let request = HapticPatternRequest {
            steps: vec![
                HapticPatternStep {
                    duration_ms: 20,
                    intensity: 128,
                },
                HapticPatternStep {
                    duration_ms: 40,
                    intensity: 255,
                },
            ],
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: HapticPatternRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, request);
    }
}
