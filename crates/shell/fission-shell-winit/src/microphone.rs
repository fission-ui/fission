use fission_core::{
    single_chunk_data_stream, CapabilityCtx, MicrophoneAvailability, MicrophoneCapture,
    MicrophoneCaptureRequest, MicrophoneDevice, MicrophoneError, MicrophonePermission,
    MicrophonePermissionRequest, CANCEL_MICROPHONE_CAPTURE, CAPTURE_MICROPHONE_AUDIO,
    GET_MICROPHONE_AVAILABILITY, REQUEST_MICROPHONE_PERMISSION,
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
        ctx: &CapabilityCtx,
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
        _ctx: &CapabilityCtx,
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
    capture_bytes: Vec<u8>,
    content_type: String,
    sample_rate_hz: u32,
    channels: u16,
    duration_ms: u64,
    device_id: Option<String>,
}

impl MemoryMicrophoneHost {
    pub fn new(
        availability: MicrophoneAvailability,
        capture_bytes: Vec<u8>,
        content_type: impl Into<String>,
        sample_rate_hz: u32,
        channels: u16,
        duration_ms: u64,
        device_id: Option<String>,
    ) -> Self {
        Self {
            availability,
            capture_bytes,
            content_type: content_type.into(),
            sample_rate_hz,
            channels,
            duration_ms,
            device_id,
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
            vec![0, 1, 2, 3],
            "audio/pcm",
            48_000,
            1,
            1_000,
            Some("memory-mic".into()),
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
        ctx: &CapabilityCtx,
    ) -> Result<MicrophoneCapture, MicrophoneError> {
        let stream = ctx.register_data_stream(single_chunk_data_stream(self.capture_bytes.clone()));
        Ok(MicrophoneCapture {
            stream,
            byte_len: Some(self.capture_bytes.len() as u64),
            content_type: self.content_type.clone(),
            sample_rate_hz: self.sample_rate_hz,
            channels: self.channels,
            duration_ms: self.duration_ms,
            device_id: self.device_id.clone(),
        })
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
    async_registry.register_operation_capability(CAPTURE_MICROPHONE_AUDIO, move |request, ctx| {
        let host = capture_host.clone();
        async move { host.capture_audio(request, &ctx) }
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
            .capture_audio(
                MicrophoneCaptureRequest::default(),
                &CapabilityCtx::new_runtime(1, fission_core::DataStreamRegistry::new()),
            )
            .is_err());
    }

    #[test]
    fn memory_host_returns_audio_capture() {
        let host = MemoryMicrophoneHost::default();
        let availability = host.availability().unwrap();
        assert_eq!(availability.permission, MicrophonePermission::Granted);

        let capture = host
            .capture_audio(
                MicrophoneCaptureRequest::default(),
                &CapabilityCtx::new_runtime(1, fission_core::DataStreamRegistry::new()),
            )
            .unwrap();
        assert_eq!(capture.content_type, "audio/pcm");
        assert_eq!(capture.sample_rate_hz, 48_000);
    }
}
