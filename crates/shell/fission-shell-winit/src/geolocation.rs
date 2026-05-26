use fission_core::{
    GeolocationError, GeolocationPermission, GeolocationPermissionRequest, GeolocationPosition,
    GeolocationPositionRequest, GET_CURRENT_POSITION, GET_GEOLOCATION_PERMISSION,
    REQUEST_GEOLOCATION_PERMISSION,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::Arc;

/// Host-side geolocation provider used by shell capability registration.
pub trait GeolocationHost: Send + Sync + 'static {
    /// Returns the current location permission state without showing a prompt.
    fn permission(&self) -> Result<GeolocationPermission, GeolocationError>;
    /// Requests location permission with precision and background preferences.
    fn request_permission(
        &self,
        request: GeolocationPermissionRequest,
    ) -> Result<GeolocationPermission, GeolocationError>;
    /// Returns the current position according to accuracy, timeout, and cache rules.
    fn current_position(
        &self,
        request: GeolocationPositionRequest,
    ) -> Result<GeolocationPosition, GeolocationError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedGeolocationHost;

impl GeolocationHost for UnsupportedGeolocationHost {
    fn permission(&self) -> Result<GeolocationPermission, GeolocationError> {
        Ok(GeolocationPermission::Unsupported)
    }

    fn request_permission(
        &self,
        _request: GeolocationPermissionRequest,
    ) -> Result<GeolocationPermission, GeolocationError> {
        Ok(GeolocationPermission::Unsupported)
    }

    fn current_position(
        &self,
        _request: GeolocationPositionRequest,
    ) -> Result<GeolocationPosition, GeolocationError> {
        Err(GeolocationError::unsupported("current_position"))
    }
}

#[derive(Debug, Clone)]
pub struct MemoryGeolocationHost {
    position: GeolocationPosition,
}

impl MemoryGeolocationHost {
    pub fn new(position: GeolocationPosition) -> Self {
        Self { position }
    }
}

impl Default for MemoryGeolocationHost {
    fn default() -> Self {
        Self {
            position: GeolocationPosition {
                latitude: 51.5074,
                longitude: -0.1278,
                altitude_meters: None,
                accuracy_meters: 10.0,
                altitude_accuracy_meters: None,
                heading_degrees: None,
                speed_mps: None,
                timestamp_unix_ms: 1_774_000_000_000,
            },
        }
    }
}

impl GeolocationHost for MemoryGeolocationHost {
    fn permission(&self) -> Result<GeolocationPermission, GeolocationError> {
        Ok(GeolocationPermission::Granted)
    }

    fn request_permission(
        &self,
        _request: GeolocationPermissionRequest,
    ) -> Result<GeolocationPermission, GeolocationError> {
        Ok(GeolocationPermission::Granted)
    }

    fn current_position(
        &self,
        _request: GeolocationPositionRequest,
    ) -> Result<GeolocationPosition, GeolocationError> {
        Ok(self.position.clone())
    }
}

pub(crate) fn register_geolocation_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn GeolocationHost>,
) {
    let permission_host = host.clone();
    async_registry.register_operation_capability(GET_GEOLOCATION_PERMISSION, move |(), _| {
        let host = permission_host.clone();
        async move { host.permission() }
    });

    let request_host = host.clone();
    async_registry.register_operation_capability(
        REQUEST_GEOLOCATION_PERMISSION,
        move |request, _| {
            let host = request_host.clone();
            async move { host.request_permission(request) }
        },
    );

    async_registry.register_operation_capability(GET_CURRENT_POSITION, move |request, _| {
        let host = host.clone();
        async move { host.current_position(request) }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_host_reports_unsupported() {
        let host = UnsupportedGeolocationHost;
        assert_eq!(
            host.permission().unwrap(),
            GeolocationPermission::Unsupported
        );
        assert!(host
            .current_position(GeolocationPositionRequest::default())
            .is_err());
    }

    #[test]
    fn memory_host_returns_position() {
        let host = MemoryGeolocationHost::default();
        assert_eq!(host.permission().unwrap(), GeolocationPermission::Granted);
        assert_eq!(
            host.current_position(GeolocationPositionRequest::default())
                .unwrap()
                .latitude,
            51.5074
        );
    }
}
