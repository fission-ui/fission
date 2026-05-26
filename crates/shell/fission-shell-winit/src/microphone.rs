use fission_core::{
    MicrophoneAvailability, MicrophoneCapture, MicrophoneCaptureRequest, MicrophoneDevice,
    MicrophoneError, MicrophonePermission, MicrophonePermissionRequest, CANCEL_MICROPHONE_CAPTURE,
    CAPTURE_MICROPHONE_AUDIO, GET_MICROPHONE_AVAILABILITY, REQUEST_MICROPHONE_PERMISSION,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::Arc;

/// Host-side microphone provider.
pub trait MicrophoneHost: Send + Sync + 'static {
    /// Returns microphone permission state and available input devices.
    fn availability(&self) -> Result<MicrophoneAvailability, MicrophoneError>;
    /// Requests microphone permission and returns the resulting state.
    fn request_permission(
        &self,
        request: MicrophonePermissionRequest,
    ) -> Result<MicrophonePermission, MicrophoneError>;
    /// Captures bounded audio using the requested device and audio format preferences.
    fn capture_audio(
        &self,
        request: MicrophoneCaptureRequest,
    ) -> Result<MicrophoneCapture, MicrophoneError>;
    /// Cancels an active microphone capture flow.
    fn cancel_capture(&self) -> Result<(), MicrophoneError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedMicrophoneHost;

impl MicrophoneHost for UnsupportedMicrophoneHost {
    fn availability(&self) -> Result<MicrophoneAvailability, MicrophoneError> {
        Ok(MicrophoneAvailability {
            permission: MicrophonePermission::Denied,
            devices: Vec::new(),
        })
    }

    fn request_permission(
        &self,
        _request: MicrophonePermissionRequest,
    ) -> Result<MicrophonePermission, MicrophoneError> {
        Err(MicrophoneError::unsupported("request_permission"))
    }

    fn capture_audio(
        &self,
        _request: MicrophoneCaptureRequest,
    ) -> Result<MicrophoneCapture, MicrophoneError> {
        Err(MicrophoneError::unsupported("capture_audio"))
    }

    fn cancel_capture(&self) -> Result<(), MicrophoneError> {
        Err(MicrophoneError::unsupported("cancel_capture"))
    }
}

#[derive(Debug, Clone)]
pub struct MemoryMicrophoneHost {
    availability: MicrophoneAvailability,
    capture: MicrophoneCapture,
}

impl MemoryMicrophoneHost {
    pub fn new(availability: MicrophoneAvailability, capture: MicrophoneCapture) -> Self {
        Self {
            availability,
            capture,
        }
    }
}

impl Default for MemoryMicrophoneHost {
    fn default() -> Self {
        Self::new(
            MicrophoneAvailability {
                permission: MicrophonePermission::Granted,
                devices: vec![MicrophoneDevice {
                    id: "memory-mic".into(),
                    label: Some("Memory microphone".into()),
                    is_default: true,
                }],
            },
            MicrophoneCapture {
                bytes: vec![0, 1, 2, 3],
                content_type: "audio/pcm".into(),
                sample_rate_hz: 48_000,
                channels: 1,
                duration_ms: 1_000,
                device_id: Some("memory-mic".into()),
            },
        )
    }
}

impl MicrophoneHost for MemoryMicrophoneHost {
    fn availability(&self) -> Result<MicrophoneAvailability, MicrophoneError> {
        Ok(self.availability.clone())
    }

    fn request_permission(
        &self,
        _request: MicrophonePermissionRequest,
    ) -> Result<MicrophonePermission, MicrophoneError> {
        Ok(self.availability.permission)
    }

    fn capture_audio(
        &self,
        _request: MicrophoneCaptureRequest,
    ) -> Result<MicrophoneCapture, MicrophoneError> {
        Ok(self.capture.clone())
    }

    fn cancel_capture(&self) -> Result<(), MicrophoneError> {
        Ok(())
    }
}

pub(crate) fn register_microphone_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn MicrophoneHost>,
) {
    let availability_host = host.clone();
    async_registry.register_operation_capability(GET_MICROPHONE_AVAILABILITY, move |(), _| {
        let host = availability_host.clone();
        async move { host.availability() }
    });

    let permission_host = host.clone();
    async_registry.register_operation_capability(
        REQUEST_MICROPHONE_PERMISSION,
        move |request, _| {
            let host = permission_host.clone();
            async move { host.request_permission(request) }
        },
    );

    let capture_host = host.clone();
    async_registry.register_operation_capability(CAPTURE_MICROPHONE_AUDIO, move |request, _| {
        let host = capture_host.clone();
        async move { host.capture_audio(request) }
    });

    async_registry.register_operation_capability(CANCEL_MICROPHONE_CAPTURE, move |(), _| {
        let host = host.clone();
        async move { host.cancel_capture() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_host_reports_errors() {
        let host = UnsupportedMicrophoneHost;
        assert!(host
            .capture_audio(MicrophoneCaptureRequest::default())
            .is_err());
    }

    #[test]
    fn memory_host_returns_audio_capture() {
        let host = MemoryMicrophoneHost::default();
        let availability = host.availability().unwrap();
        assert_eq!(availability.permission, MicrophonePermission::Granted);

        let capture = host
            .capture_audio(MicrophoneCaptureRequest::default())
            .unwrap();
        assert_eq!(capture.content_type, "audio/pcm");
        assert_eq!(capture.sample_rate_hz, 48_000);
    }
}
