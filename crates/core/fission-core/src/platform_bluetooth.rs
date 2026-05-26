//! Bluetooth host capabilities.

use crate::capability::{CapabilityType, OperationCapability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothPermission {
    #[default]
    Unknown,
    Granted,
    Denied,
    Restricted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothMode {
    Classic,
    LowEnergy,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothDevice {
    pub id: String,
    pub name: Option<String>,
    pub address: Option<String>,
    pub rssi: Option<i16>,
    pub paired: bool,
    pub modes: Vec<BluetoothMode>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothAvailability {
    pub permission: BluetoothPermission,
    pub enabled: bool,
    pub supports_classic: bool,
    pub supports_low_energy: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothPermissionRequest {
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothScanRequest {
    pub service_uuids: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub include_paired: bool,
    pub allow_duplicates: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothScanResult {
    pub devices: Vec<BluetoothDevice>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothConnectRequest {
    pub device_id: String,
    pub service_uuids: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothConnection {
    pub connection_id: String,
    pub device: BluetoothDevice,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothDisconnectRequest {
    pub connection_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothReadRequest {
    pub connection_id: String,
    pub service_uuid: String,
    pub characteristic_uuid: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothReadResult {
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothWriteRequest {
    pub connection_id: String,
    pub service_uuid: String,
    pub characteristic_uuid: String,
    pub value: Vec<u8>,
    pub with_response: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothAdvertiseRequest {
    pub service_uuid: String,
    pub service_data: Vec<u8>,
    pub local_name: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothAdvertiseReceipt {
    pub advertisement_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothStopAdvertiseRequest {
    pub advertisement_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BluetoothError {
    pub code: String,
    pub message: String,
}

impl BluetoothError {
    /// Creates a portable bluetooth error payload.
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
    /// `operation` should name the attempted bluetooth operation. Use this
    /// from hosts that implement the capability contract but cannot provide this
    /// operation on the current platform or hardware.
    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            "unsupported",
            format!(
                "Bluetooth operation `{}` is not supported by this host",
                operation.into()
            ),
        )
    }
}

pub struct GetBluetoothAvailabilityCapability;
impl OperationCapability for GetBluetoothAvailabilityCapability {
    type Request = ();
    type Ok = BluetoothAvailability;
    type Err = BluetoothError;
}

pub struct RequestBluetoothPermissionCapability;
impl OperationCapability for RequestBluetoothPermissionCapability {
    type Request = BluetoothPermissionRequest;
    type Ok = BluetoothPermission;
    type Err = BluetoothError;
}

pub struct ScanBluetoothDevicesCapability;
impl OperationCapability for ScanBluetoothDevicesCapability {
    type Request = BluetoothScanRequest;
    type Ok = BluetoothScanResult;
    type Err = BluetoothError;
}

pub struct ConnectBluetoothDeviceCapability;
impl OperationCapability for ConnectBluetoothDeviceCapability {
    type Request = BluetoothConnectRequest;
    type Ok = BluetoothConnection;
    type Err = BluetoothError;
}

pub struct DisconnectBluetoothDeviceCapability;
impl OperationCapability for DisconnectBluetoothDeviceCapability {
    type Request = BluetoothDisconnectRequest;
    type Ok = ();
    type Err = BluetoothError;
}

pub struct ReadBluetoothCharacteristicCapability;
impl OperationCapability for ReadBluetoothCharacteristicCapability {
    type Request = BluetoothReadRequest;
    type Ok = BluetoothReadResult;
    type Err = BluetoothError;
}

pub struct WriteBluetoothCharacteristicCapability;
impl OperationCapability for WriteBluetoothCharacteristicCapability {
    type Request = BluetoothWriteRequest;
    type Ok = ();
    type Err = BluetoothError;
}

pub struct StartBluetoothAdvertisingCapability;
impl OperationCapability for StartBluetoothAdvertisingCapability {
    type Request = BluetoothAdvertiseRequest;
    type Ok = BluetoothAdvertiseReceipt;
    type Err = BluetoothError;
}

pub struct StopBluetoothAdvertisingCapability;
impl OperationCapability for StopBluetoothAdvertisingCapability {
    type Request = BluetoothStopAdvertiseRequest;
    type Ok = ();
    type Err = BluetoothError;
}

pub const GET_BLUETOOTH_AVAILABILITY: CapabilityType<GetBluetoothAvailabilityCapability> =
    CapabilityType::new("fission.bluetooth.get_availability");
pub const REQUEST_BLUETOOTH_PERMISSION: CapabilityType<RequestBluetoothPermissionCapability> =
    CapabilityType::new("fission.bluetooth.request_permission");
pub const SCAN_BLUETOOTH_DEVICES: CapabilityType<ScanBluetoothDevicesCapability> =
    CapabilityType::new("fission.bluetooth.scan_devices");
pub const CONNECT_BLUETOOTH_DEVICE: CapabilityType<ConnectBluetoothDeviceCapability> =
    CapabilityType::new("fission.bluetooth.connect_device");
pub const DISCONNECT_BLUETOOTH_DEVICE: CapabilityType<DisconnectBluetoothDeviceCapability> =
    CapabilityType::new("fission.bluetooth.disconnect_device");
pub const READ_BLUETOOTH_CHARACTERISTIC: CapabilityType<ReadBluetoothCharacteristicCapability> =
    CapabilityType::new("fission.bluetooth.read_characteristic");
pub const WRITE_BLUETOOTH_CHARACTERISTIC: CapabilityType<WriteBluetoothCharacteristicCapability> =
    CapabilityType::new("fission.bluetooth.write_characteristic");
pub const START_BLUETOOTH_ADVERTISING: CapabilityType<StartBluetoothAdvertisingCapability> =
    CapabilityType::new("fission.bluetooth.start_advertising");
pub const STOP_BLUETOOTH_ADVERTISING: CapabilityType<StopBluetoothAdvertisingCapability> =
    CapabilityType::new("fission.bluetooth.stop_advertising");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bluetooth_scan_request_round_trips() {
        let request = BluetoothScanRequest {
            service_uuids: vec!["180D".into()],
            timeout_ms: Some(10_000),
            include_paired: true,
            allow_duplicates: false,
        };

        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: BluetoothScanRequest = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, request);
    }

    #[test]
    fn bluetooth_connection_round_trips() {
        let connection = BluetoothConnection {
            connection_id: "connection-1".into(),
            device: BluetoothDevice {
                id: "device-1".into(),
                name: Some("Heart monitor".into()),
                address: Some("00:11:22:33:44:55".into()),
                rssi: Some(-42),
                paired: true,
                modes: vec![BluetoothMode::LowEnergy],
            },
        };

        let bytes = serde_json::to_vec(&connection).unwrap();
        let decoded: BluetoothConnection = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded, connection);
    }
}
