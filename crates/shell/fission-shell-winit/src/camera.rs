use fission_core::{
    CameraAvailability, CameraCapture, CameraCaptureRequest, CameraDevice, CameraError,
    CameraFacing, CameraFlashlightRequest, CameraPermission, CameraPermissionRequest,
    CANCEL_CAMERA_CAPTURE, CAPTURE_PHOTO, GET_CAMERA_AVAILABILITY, REQUEST_CAMERA_PERMISSION,
    SET_CAMERA_FLASHLIGHT,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::{Arc, Mutex};

/// Host-side camera and flashlight provider.
pub trait CameraHost: Send + Sync + 'static {
    /// Returns camera permission state and host-visible camera devices.
    fn availability(&self) -> Result<CameraAvailability, CameraError>;
    /// Requests camera permission and returns the resulting permission state.
    fn request_permission(
        &self,
        request: CameraPermissionRequest,
    ) -> Result<CameraPermission, CameraError>;
    /// Captures a still image according to the selected camera, format, flash, and quality request.
    fn capture_photo(&self, request: CameraCaptureRequest) -> Result<CameraCapture, CameraError>;
    /// Enables, disables, or adjusts the selected camera flashlight where available.
    fn set_flashlight(&self, request: CameraFlashlightRequest) -> Result<(), CameraError>;
    /// Cancels an active camera capture flow.
    fn cancel_capture(&self) -> Result<(), CameraError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedCameraHost;

impl CameraHost for UnsupportedCameraHost {
    fn availability(&self) -> Result<CameraAvailability, CameraError> {
        Ok(CameraAvailability {
            permission: CameraPermission::Denied,
            devices: Vec::new(),
        })
    }

    fn request_permission(
        &self,
        _request: CameraPermissionRequest,
    ) -> Result<CameraPermission, CameraError> {
        Err(CameraError::unsupported("request_permission"))
    }

    fn capture_photo(&self, _request: CameraCaptureRequest) -> Result<CameraCapture, CameraError> {
        Err(CameraError::unsupported("capture_photo"))
    }

    fn set_flashlight(&self, _request: CameraFlashlightRequest) -> Result<(), CameraError> {
        Err(CameraError::unsupported("set_flashlight"))
    }

    fn cancel_capture(&self) -> Result<(), CameraError> {
        Err(CameraError::unsupported("cancel_capture"))
    }
}

#[derive(Debug)]
pub struct MemoryCameraHost {
    availability: CameraAvailability,
    capture: CameraCapture,
    flashlight_calls: Arc<Mutex<Vec<CameraFlashlightRequest>>>,
}

impl MemoryCameraHost {
    pub fn new(availability: CameraAvailability, capture: CameraCapture) -> Self {
        Self {
            availability,
            capture,
            flashlight_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn flashlight_calls(&self) -> Vec<CameraFlashlightRequest> {
        self.flashlight_calls
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }
}

impl Default for MemoryCameraHost {
    fn default() -> Self {
        Self::new(
            CameraAvailability {
                permission: CameraPermission::Granted,
                devices: vec![CameraDevice {
                    id: "memory-camera".into(),
                    label: Some("Memory camera".into()),
                    facing: CameraFacing::Back,
                    has_flashlight: true,
                }],
            },
            CameraCapture {
                bytes: vec![0xff, 0xd8, 0xff, 0xd9],
                content_type: "image/jpeg".into(),
                width: 1,
                height: 1,
                camera_id: Some("memory-camera".into()),
            },
        )
    }
}

impl CameraHost for MemoryCameraHost {
    fn availability(&self) -> Result<CameraAvailability, CameraError> {
        Ok(self.availability.clone())
    }

    fn request_permission(
        &self,
        _request: CameraPermissionRequest,
    ) -> Result<CameraPermission, CameraError> {
        Ok(self.availability.permission)
    }

    fn capture_photo(&self, _request: CameraCaptureRequest) -> Result<CameraCapture, CameraError> {
        Ok(self.capture.clone())
    }

    fn set_flashlight(&self, request: CameraFlashlightRequest) -> Result<(), CameraError> {
        self.flashlight_calls.lock().unwrap().push(request);
        Ok(())
    }

    fn cancel_capture(&self) -> Result<(), CameraError> {
        Ok(())
    }
}

pub(crate) fn register_camera_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn CameraHost>,
) {
    let availability_host = host.clone();
    async_registry.register_operation_capability(GET_CAMERA_AVAILABILITY, move |(), _| {
        let host = availability_host.clone();
        async move { host.availability() }
    });

    let permission_host = host.clone();
    async_registry.register_operation_capability(REQUEST_CAMERA_PERMISSION, move |request, _| {
        let host = permission_host.clone();
        async move { host.request_permission(request) }
    });

    let capture_host = host.clone();
    async_registry.register_operation_capability(CAPTURE_PHOTO, move |request, _| {
        let host = capture_host.clone();
        async move { host.capture_photo(request) }
    });

    let flashlight_host = host.clone();
    async_registry.register_operation_capability(SET_CAMERA_FLASHLIGHT, move |request, _| {
        let host = flashlight_host.clone();
        async move { host.set_flashlight(request) }
    });

    async_registry.register_operation_capability(CANCEL_CAMERA_CAPTURE, move |(), _| {
        let host = host.clone();
        async move { host.cancel_capture() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::CameraImageFormat;

    #[test]
    fn unsupported_host_reports_errors() {
        let host = UnsupportedCameraHost;
        assert!(host
            .capture_photo(CameraCaptureRequest {
                format: CameraImageFormat::Jpeg,
                ..Default::default()
            })
            .is_err());
        assert!(host
            .set_flashlight(CameraFlashlightRequest {
                enabled: true,
                ..Default::default()
            })
            .is_err());
    }

    #[test]
    fn memory_host_returns_capture_and_records_flashlight() {
        let host = MemoryCameraHost::default();
        let availability = host.availability().unwrap();
        assert_eq!(availability.permission, CameraPermission::Granted);

        let capture = host.capture_photo(CameraCaptureRequest::default()).unwrap();
        assert_eq!(capture.content_type, "image/jpeg");

        let request = CameraFlashlightRequest {
            enabled: true,
            intensity: Some(128),
            ..Default::default()
        };
        host.set_flashlight(request.clone()).unwrap();
        assert_eq!(host.flashlight_calls(), vec![request]);
    }
}
