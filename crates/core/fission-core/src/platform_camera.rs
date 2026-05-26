//! Camera and flashlight host capabilities.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraPermission {
    #[default]
    Unknown,
    Granted,
    Denied,
    Restricted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraFacing {
    Front,
    Back,
    External,
    #[default]
    Unspecified,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraImageFormat {
    #[default]
    Jpeg,
    Png,
    Heif,
    Raw,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraFlashMode {
    #[default]
    Off,
    On,
    Auto,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraResolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraDevice {
    pub id: String,
    pub label: Option<String>,
    pub facing: CameraFacing,
    pub has_flashlight: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraAvailability {
    pub permission: CameraPermission,
    pub devices: Vec<CameraDevice>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraPermissionRequest {
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraCaptureRequest {
    pub camera_id: Option<String>,
    pub facing: CameraFacing,
    pub resolution: Option<CameraResolution>,
    pub format: CameraImageFormat,
    pub flash: CameraFlashMode,
    pub quality: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraCapture {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub width: u32,
    pub height: u32,
    pub camera_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraFlashlightRequest {
    pub camera_id: Option<String>,
    pub enabled: bool,
    pub intensity: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraError {
    pub code: String,
    pub message: String,
}

impl CameraError {
    /// Creates a portable camera error payload.
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
    /// `operation` should name the attempted camera operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "camera operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct GetCameraAvailabilityCapability;
impl OperationCapability for GetCameraAvailabilityCapability {
    type Request = ();
    type Ok = CameraAvailability;
    type Err = CameraError;
}

pub struct RequestCameraPermissionCapability;
impl OperationCapability for RequestCameraPermissionCapability {
    type Request = CameraPermissionRequest;
    type Ok = CameraPermission;
    type Err = CameraError;
}

pub struct CapturePhotoCapability;
impl OperationCapability for CapturePhotoCapability {
    type Request = CameraCaptureRequest;
    type Ok = CameraCapture;
    type Err = CameraError;
}

pub struct SetCameraFlashlightCapability;
impl OperationCapability for SetCameraFlashlightCapability {
    type Request = CameraFlashlightRequest;
    type Ok = ();
    type Err = CameraError;
}

pub struct CancelCameraCaptureCapability;
impl OperationCapability for CancelCameraCaptureCapability {
    type Request = ();
    type Ok = ();
    type Err = CameraError;
}

pub const GET_CAMERA_AVAILABILITY: CapabilityType<GetCameraAvailabilityCapability> =
    CapabilityType::new("fission.camera.get_availability");
pub const REQUEST_CAMERA_PERMISSION: CapabilityType<RequestCameraPermissionCapability> =
    CapabilityType::new("fission.camera.request_permission");
pub const CAPTURE_PHOTO: CapabilityType<CapturePhotoCapability> =
    CapabilityType::new("fission.camera.capture_photo");
pub const SET_CAMERA_FLASHLIGHT: CapabilityType<SetCameraFlashlightCapability> =
    CapabilityType::new("fission.camera.set_flashlight");
pub const CANCEL_CAMERA_CAPTURE: CapabilityType<CancelCameraCaptureCapability> =
    CapabilityType::new("fission.camera.cancel_capture");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_capture_request_round_trips() {
        let request = CameraCaptureRequest {
            camera_id: Some("back".into()),
            facing: CameraFacing::Back,
            resolution: Some(CameraResolution {
                width: 1920,
                height: 1080,
            }),
            format: CameraImageFormat::Heif,
            flash: CameraFlashMode::Auto,
            quality: Some(90),
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: CameraCaptureRequest = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, request);
    }

    #[test]
    fn camera_availability_round_trips() {
        let availability = CameraAvailability {
            permission: CameraPermission::Granted,
            devices: vec![CameraDevice {
                id: "front".into(),
                label: Some("Front camera".into()),
                facing: CameraFacing::Front,
                has_flashlight: false,
            }],
        };

        let bytes = serde_json::to_vec(&availability).unwrap();
        let decoded: CameraAvailability = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, availability);
    }
}
