//! Wi-Fi host capabilities.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WifiPermission {
    #[default]
    Unknown,
    Granted,
    Denied,
    Restricted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WifiSecurity {
    Open,
    Wep,
    Wpa,
    #[default]
    Wpa2,
    Wpa3,
    Enterprise,
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiNetwork {
    pub ssid: String,
    pub bssid: Option<String>,
    pub rssi: Option<i16>,
    pub frequency_mhz: Option<u32>,
    pub security: WifiSecurity,
    pub connected: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiAvailability {
    pub permission: WifiPermission,
    pub enabled: bool,
    pub connected_network: Option<WifiNetwork>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiPermissionRequest {
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiScanRequest {
    pub ssid_prefix: Option<String>,
    pub include_hidden: bool,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiScanResult {
    pub networks: Vec<WifiNetwork>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiConnectRequest {
    pub ssid: String,
    pub passphrase: Option<String>,
    pub security: WifiSecurity,
    pub hidden: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiConnection {
    pub network: WifiNetwork,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiDisconnectRequest {
    pub ssid: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WifiError {
    pub code: String,
    pub message: String,
}

impl WifiError {
    /// Creates a portable wi-fi error payload.
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
    /// `operation` should name the attempted wi-fi operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "Wi-Fi operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct GetWifiAvailabilityCapability;
impl OperationCapability for GetWifiAvailabilityCapability {
    type Request = ();
    type Ok = WifiAvailability;
    type Err = WifiError;
}

pub struct RequestWifiPermissionCapability;
impl OperationCapability for RequestWifiPermissionCapability {
    type Request = WifiPermissionRequest;
    type Ok = WifiPermission;
    type Err = WifiError;
}

pub struct ScanWifiNetworksCapability;
impl OperationCapability for ScanWifiNetworksCapability {
    type Request = WifiScanRequest;
    type Ok = WifiScanResult;
    type Err = WifiError;
}

pub struct ConnectWifiNetworkCapability;
impl OperationCapability for ConnectWifiNetworkCapability {
    type Request = WifiConnectRequest;
    type Ok = WifiConnection;
    type Err = WifiError;
}

pub struct DisconnectWifiNetworkCapability;
impl OperationCapability for DisconnectWifiNetworkCapability {
    type Request = WifiDisconnectRequest;
    type Ok = ();
    type Err = WifiError;
}

pub const GET_WIFI_AVAILABILITY: CapabilityType<GetWifiAvailabilityCapability> =
    CapabilityType::new("fission.wifi.get_availability");
pub const REQUEST_WIFI_PERMISSION: CapabilityType<RequestWifiPermissionCapability> =
    CapabilityType::new("fission.wifi.request_permission");
pub const SCAN_WIFI_NETWORKS: CapabilityType<ScanWifiNetworksCapability> =
    CapabilityType::new("fission.wifi.scan_networks");
pub const CONNECT_WIFI_NETWORK: CapabilityType<ConnectWifiNetworkCapability> =
    CapabilityType::new("fission.wifi.connect_network");
pub const DISCONNECT_WIFI_NETWORK: CapabilityType<DisconnectWifiNetworkCapability> =
    CapabilityType::new("fission.wifi.disconnect_network");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_scan_result_round_trips() {
        let result = WifiScanResult {
            networks: vec![WifiNetwork {
                ssid: "Fission".into(),
                bssid: Some("00:11:22:33:44:55".into()),
                rssi: Some(-50),
                frequency_mhz: Some(5_200),
                security: WifiSecurity::Wpa3,
                connected: false,
            }],
        };

        let bytes = serde_json::to_vec(&result).unwrap();
        let decoded: WifiScanResult = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, result);
    }
}
