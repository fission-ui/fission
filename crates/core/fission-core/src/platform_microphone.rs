//! Microphone host capabilities.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MicrophonePermission {
    #[default]
    Unknown,
    Granted,
    Denied,
    Restricted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioSampleFormat {
    U8,
    I16,
    #[default]
    F32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicrophoneDevice {
    pub id: String,
    pub label: Option<String>,
    pub is_default: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicrophoneAvailability {
    pub permission: MicrophonePermission,
    pub devices: Vec<MicrophoneDevice>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicrophonePermissionRequest {
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicrophoneCaptureRequest {
    pub device_id: Option<String>,
    pub duration_ms: u64,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
    pub sample_format: AudioSampleFormat,
}

impl Default for MicrophoneCaptureRequest {
    fn default() -> Self {
        Self {
            device_id: None,
            duration_ms: 1_000,
            sample_rate_hz: None,
            channels: None,
            sample_format: AudioSampleFormat::F32,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicrophoneCapture {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub duration_ms: u64,
    pub device_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicrophoneError {
    pub code: String,
    pub message: String,
}

impl MicrophoneError {
    /// Creates a portable microphone error payload.
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
    /// `operation` should name the attempted microphone operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "microphone operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct GetMicrophoneAvailabilityCapability;
impl OperationCapability for GetMicrophoneAvailabilityCapability {
    type Request = ();
    type Ok = MicrophoneAvailability;
    type Err = MicrophoneError;
}

pub struct RequestMicrophonePermissionCapability;
impl OperationCapability for RequestMicrophonePermissionCapability {
    type Request = MicrophonePermissionRequest;
    type Ok = MicrophonePermission;
    type Err = MicrophoneError;
}

pub struct CaptureMicrophoneAudioCapability;
impl OperationCapability for CaptureMicrophoneAudioCapability {
    type Request = MicrophoneCaptureRequest;
    type Ok = MicrophoneCapture;
    type Err = MicrophoneError;
}

pub struct CancelMicrophoneCaptureCapability;
impl OperationCapability for CancelMicrophoneCaptureCapability {
    type Request = ();
    type Ok = ();
    type Err = MicrophoneError;
}

pub const GET_MICROPHONE_AVAILABILITY: CapabilityType<GetMicrophoneAvailabilityCapability> =
    CapabilityType::new("fission.microphone.get_availability");
pub const REQUEST_MICROPHONE_PERMISSION: CapabilityType<RequestMicrophonePermissionCapability> =
    CapabilityType::new("fission.microphone.request_permission");
pub const CAPTURE_MICROPHONE_AUDIO: CapabilityType<CaptureMicrophoneAudioCapability> =
    CapabilityType::new("fission.microphone.capture_audio");
pub const CANCEL_MICROPHONE_CAPTURE: CapabilityType<CancelMicrophoneCaptureCapability> =
    CapabilityType::new("fission.microphone.cancel_capture");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microphone_capture_request_round_trips() {
        let request = MicrophoneCaptureRequest {
            device_id: Some("default".into()),
            duration_ms: 5_000,
            sample_rate_hz: Some(48_000),
            channels: Some(2),
            sample_format: AudioSampleFormat::I16,
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: MicrophoneCaptureRequest = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, request);
    }
}
