//! Volume-control host capabilities.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeStream {
    #[default]
    Media,
    Ring,
    Alarm,
    Notification,
    Call,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeAdjustDirection {
    Up,
    Down,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeLevel {
    pub stream: VolumeStream,
    pub level: u8,
    pub muted: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeSetRequest {
    pub stream: VolumeStream,
    pub level: u8,
    pub muted: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeAdjustRequest {
    pub stream: VolumeStream,
    pub direction: VolumeAdjustDirection,
    pub step: u8,
}

impl Default for VolumeAdjustDirection {
    fn default() -> Self {
        Self::Up
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeError {
    pub code: String,
    pub message: String,
}

impl VolumeError {
    /// Creates a portable volume error payload.
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
    /// `operation` should name the attempted volume operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "volume operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct GetVolumeLevelCapability;
impl OperationCapability for GetVolumeLevelCapability {
    type Request = VolumeStream;
    type Ok = VolumeLevel;
    type Err = VolumeError;
}

pub struct SetVolumeLevelCapability;
impl OperationCapability for SetVolumeLevelCapability {
    type Request = VolumeSetRequest;
    type Ok = VolumeLevel;
    type Err = VolumeError;
}

pub struct AdjustVolumeLevelCapability;
impl OperationCapability for AdjustVolumeLevelCapability {
    type Request = VolumeAdjustRequest;
    type Ok = VolumeLevel;
    type Err = VolumeError;
}

pub const GET_VOLUME_LEVEL: CapabilityType<GetVolumeLevelCapability> =
    CapabilityType::new("fission.volume.get_level");
pub const SET_VOLUME_LEVEL: CapabilityType<SetVolumeLevelCapability> =
    CapabilityType::new("fission.volume.set_level");
pub const ADJUST_VOLUME_LEVEL: CapabilityType<AdjustVolumeLevelCapability> =
    CapabilityType::new("fission.volume.adjust_level");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_set_request_round_trips() {
        let request = VolumeSetRequest {
            stream: VolumeStream::Media,
            level: 42,
            muted: Some(false),
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: VolumeSetRequest = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, request);
    }
}
