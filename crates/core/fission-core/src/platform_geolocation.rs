//! Geolocation host capabilities.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeolocationPermission {
    Granted,
    Denied,
    Prompt,
    #[default]
    Unknown,
    Unsupported,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeolocationPermissionRequest {
    pub precise: bool,
    pub background: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeolocationPositionRequest {
    pub high_accuracy: bool,
    pub timeout_ms: Option<u64>,
    pub maximum_age_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeolocationPosition {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_meters: Option<f64>,
    pub accuracy_meters: f64,
    pub altitude_accuracy_meters: Option<f64>,
    pub heading_degrees: Option<f64>,
    pub speed_mps: Option<f64>,
    pub timestamp_unix_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeolocationError {
    pub code: String,
    pub message: String,
}

impl GeolocationError {
    /// Creates a portable geolocation error payload.
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
    /// `operation` should name the attempted geolocation operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "geolocation operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct GetGeolocationPermissionCapability;
impl OperationCapability for GetGeolocationPermissionCapability {
    type Request = ();
    type Ok = GeolocationPermission;
    type Err = GeolocationError;
}

pub struct RequestGeolocationPermissionCapability;
impl OperationCapability for RequestGeolocationPermissionCapability {
    type Request = GeolocationPermissionRequest;
    type Ok = GeolocationPermission;
    type Err = GeolocationError;
}

pub struct GetCurrentPositionCapability;
impl OperationCapability for GetCurrentPositionCapability {
    type Request = GeolocationPositionRequest;
    type Ok = GeolocationPosition;
    type Err = GeolocationError;
}

pub const GET_GEOLOCATION_PERMISSION: CapabilityType<GetGeolocationPermissionCapability> =
    CapabilityType::new("fission.geolocation.get_permission");
pub const REQUEST_GEOLOCATION_PERMISSION: CapabilityType<RequestGeolocationPermissionCapability> =
    CapabilityType::new("fission.geolocation.request_permission");
pub const GET_CURRENT_POSITION: CapabilityType<GetCurrentPositionCapability> =
    CapabilityType::new("fission.geolocation.get_current_position");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geolocation_request_round_trips() {
        let request = GeolocationPositionRequest {
            high_accuracy: true,
            timeout_ms: Some(5_000),
            maximum_age_ms: Some(30_000),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: GeolocationPositionRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn geolocation_position_round_trips() {
        let position = GeolocationPosition {
            latitude: 51.5074,
            longitude: -0.1278,
            altitude_meters: None,
            accuracy_meters: 8.0,
            altitude_accuracy_meters: None,
            heading_degrees: None,
            speed_mps: None,
            timestamp_unix_ms: 1_774_000_000_000,
        };
        let bytes = serde_json::to_vec(&position).unwrap();
        let decoded: GeolocationPosition = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, position);
    }
}
