use fission_core::{
    WifiAvailability, WifiConnectRequest, WifiConnection, WifiDisconnectRequest, WifiError,
    WifiNetwork, WifiPermission, WifiPermissionRequest, WifiScanRequest, WifiScanResult,
    WifiSecurity, CONNECT_WIFI_NETWORK, DISCONNECT_WIFI_NETWORK, GET_WIFI_AVAILABILITY,
    REQUEST_WIFI_PERMISSION, SCAN_WIFI_NETWORKS,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::Arc;

/// Host-side Wi-Fi provider.
pub trait WifiHost: Send + Sync + 'static {
    /// Returns Wi-Fi adapter, permission, and current connection state.
    fn availability(&self) -> Result<WifiAvailability, WifiError>;
    /// Requests Wi-Fi, nearby-network, or related location permission from the host.
    fn request_permission(
        &self,
        request: WifiPermissionRequest,
    ) -> Result<WifiPermission, WifiError>;
    /// Scans for nearby Wi-Fi networks using the supplied filters.
    fn scan_networks(&self, request: WifiScanRequest) -> Result<WifiScanResult, WifiError>;
    /// Requests connection to one Wi-Fi network.
    fn connect_network(&self, request: WifiConnectRequest) -> Result<WifiConnection, WifiError>;
    /// Requests disconnection from a Wi-Fi network.
    fn disconnect_network(&self, request: WifiDisconnectRequest) -> Result<(), WifiError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedWifiHost;

impl WifiHost for UnsupportedWifiHost {
    fn availability(&self) -> Result<WifiAvailability, WifiError> {
        Ok(WifiAvailability {
            permission: WifiPermission::Denied,
            enabled: false,
            connected_network: None,
        })
    }

    fn request_permission(
        &self,
        _request: WifiPermissionRequest,
    ) -> Result<WifiPermission, WifiError> {
        Err(WifiError::unsupported("request_permission"))
    }

    fn scan_networks(&self, _request: WifiScanRequest) -> Result<WifiScanResult, WifiError> {
        Err(WifiError::unsupported("scan_networks"))
    }

    fn connect_network(&self, _request: WifiConnectRequest) -> Result<WifiConnection, WifiError> {
        Err(WifiError::unsupported("connect_network"))
    }

    fn disconnect_network(&self, _request: WifiDisconnectRequest) -> Result<(), WifiError> {
        Err(WifiError::unsupported("disconnect_network"))
    }
}

#[derive(Debug, Clone)]
pub struct MemoryWifiHost {
    availability: WifiAvailability,
    networks: Vec<WifiNetwork>,
}

impl MemoryWifiHost {
    pub fn new(availability: WifiAvailability, networks: Vec<WifiNetwork>) -> Self {
        Self {
            availability,
            networks,
        }
    }
}

impl Default for MemoryWifiHost {
    fn default() -> Self {
        let network = WifiNetwork {
            ssid: "Fission".into(),
            bssid: Some("00:11:22:33:44:55".into()),
            rssi: Some(-45),
            frequency_mhz: Some(5_200),
            security: WifiSecurity::Wpa3,
            connected: true,
        };
        Self::new(
            WifiAvailability {
                permission: WifiPermission::Granted,
                enabled: true,
                connected_network: Some(network.clone()),
            },
            vec![network],
        )
    }
}

impl WifiHost for MemoryWifiHost {
    fn availability(&self) -> Result<WifiAvailability, WifiError> {
        Ok(self.availability.clone())
    }

    fn request_permission(
        &self,
        _request: WifiPermissionRequest,
    ) -> Result<WifiPermission, WifiError> {
        Ok(self.availability.permission)
    }

    fn scan_networks(&self, _request: WifiScanRequest) -> Result<WifiScanResult, WifiError> {
        Ok(WifiScanResult {
            networks: self.networks.clone(),
        })
    }

    fn connect_network(&self, request: WifiConnectRequest) -> Result<WifiConnection, WifiError> {
        let network = self
            .networks
            .iter()
            .find(|network| network.ssid == request.ssid)
            .cloned()
            .ok_or_else(|| WifiError::new("not_found", "Wi-Fi network not found"))?;
        Ok(WifiConnection { network })
    }

    fn disconnect_network(&self, _request: WifiDisconnectRequest) -> Result<(), WifiError> {
        Ok(())
    }
}

pub(crate) fn register_wifi_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn WifiHost>,
) {
    let availability_host = host.clone();
    async_registry.register_operation_capability(GET_WIFI_AVAILABILITY, move |(), _| {
        let host = availability_host.clone();
        async move { host.availability() }
    });

    let permission_host = host.clone();
    async_registry.register_operation_capability(REQUEST_WIFI_PERMISSION, move |request, _| {
        let host = permission_host.clone();
        async move { host.request_permission(request) }
    });

    let scan_host = host.clone();
    async_registry.register_operation_capability(SCAN_WIFI_NETWORKS, move |request, _| {
        let host = scan_host.clone();
        async move { host.scan_networks(request) }
    });

    let connect_host = host.clone();
    async_registry.register_operation_capability(CONNECT_WIFI_NETWORK, move |request, _| {
        let host = connect_host.clone();
        async move { host.connect_network(request) }
    });

    async_registry.register_operation_capability(DISCONNECT_WIFI_NETWORK, move |request, _| {
        let host = host.clone();
        async move { host.disconnect_network(request) }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_host_reports_errors() {
        let host = UnsupportedWifiHost;
        assert!(host.scan_networks(WifiScanRequest::default()).is_err());
    }

    #[test]
    fn memory_host_scans_and_connects() {
        let host = MemoryWifiHost::default();
        let scan = host.scan_networks(WifiScanRequest::default()).unwrap();
        assert_eq!(scan.networks.len(), 1);

        let connection = host
            .connect_network(WifiConnectRequest {
                ssid: "Fission".into(),
                security: WifiSecurity::Wpa3,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(connection.network.ssid, "Fission");
    }
}
