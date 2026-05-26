use fission_core::{
    BluetoothAdvertiseReceipt, BluetoothAdvertiseRequest, BluetoothAvailability,
    BluetoothConnectRequest, BluetoothConnection, BluetoothDevice, BluetoothDisconnectRequest,
    BluetoothError, BluetoothMode, BluetoothPermission, BluetoothPermissionRequest,
    BluetoothReadRequest, BluetoothReadResult, BluetoothScanRequest, BluetoothScanResult,
    BluetoothStopAdvertiseRequest, BluetoothWriteRequest, CONNECT_BLUETOOTH_DEVICE,
    DISCONNECT_BLUETOOTH_DEVICE, GET_BLUETOOTH_AVAILABILITY, READ_BLUETOOTH_CHARACTERISTIC,
    REQUEST_BLUETOOTH_PERMISSION, SCAN_BLUETOOTH_DEVICES, START_BLUETOOTH_ADVERTISING,
    STOP_BLUETOOTH_ADVERTISING, WRITE_BLUETOOTH_CHARACTERISTIC,
};
use fission_shell::async_host::AsyncRegistry;
use std::sync::Arc;

/// Host-side Bluetooth provider.
pub trait BluetoothHost: Send + Sync + 'static {
    /// Returns adapter, permission, and Bluetooth mode availability.
    fn availability(&self) -> Result<BluetoothAvailability, BluetoothError>;
    /// Requests Bluetooth or nearby-device permission from the host.
    fn request_permission(
        &self,
        request: BluetoothPermissionRequest,
    ) -> Result<BluetoothPermission, BluetoothError>;
    /// Scans for Bluetooth devices matching the supplied filters.
    fn scan_devices(
        &self,
        request: BluetoothScanRequest,
    ) -> Result<BluetoothScanResult, BluetoothError>;
    /// Connects to a Bluetooth device and returns a connection handle.
    fn connect_device(
        &self,
        request: BluetoothConnectRequest,
    ) -> Result<BluetoothConnection, BluetoothError>;
    /// Disconnects a previously opened Bluetooth connection.
    fn disconnect_device(&self, request: BluetoothDisconnectRequest) -> Result<(), BluetoothError>;
    /// Reads bytes from a characteristic on an active connection.
    fn read_characteristic(
        &self,
        request: BluetoothReadRequest,
    ) -> Result<BluetoothReadResult, BluetoothError>;
    /// Writes bytes to a characteristic on an active connection.
    fn write_characteristic(&self, request: BluetoothWriteRequest) -> Result<(), BluetoothError>;
    /// Starts Bluetooth advertising where the platform permits it.
    fn start_advertising(
        &self,
        request: BluetoothAdvertiseRequest,
    ) -> Result<BluetoothAdvertiseReceipt, BluetoothError>;
    /// Stops a previously started Bluetooth advertisement.
    fn stop_advertising(
        &self,
        request: BluetoothStopAdvertiseRequest,
    ) -> Result<(), BluetoothError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedBluetoothHost;

impl BluetoothHost for UnsupportedBluetoothHost {
    fn availability(&self) -> Result<BluetoothAvailability, BluetoothError> {
        Ok(BluetoothAvailability {
            permission: BluetoothPermission::Denied,
            enabled: false,
            supports_classic: false,
            supports_low_energy: false,
        })
    }

    fn request_permission(
        &self,
        _request: BluetoothPermissionRequest,
    ) -> Result<BluetoothPermission, BluetoothError> {
        Err(BluetoothError::unsupported("request_permission"))
    }

    fn scan_devices(
        &self,
        _request: BluetoothScanRequest,
    ) -> Result<BluetoothScanResult, BluetoothError> {
        Err(BluetoothError::unsupported("scan_devices"))
    }

    fn connect_device(
        &self,
        _request: BluetoothConnectRequest,
    ) -> Result<BluetoothConnection, BluetoothError> {
        Err(BluetoothError::unsupported("connect_device"))
    }

    fn disconnect_device(
        &self,
        _request: BluetoothDisconnectRequest,
    ) -> Result<(), BluetoothError> {
        Err(BluetoothError::unsupported("disconnect_device"))
    }

    fn read_characteristic(
        &self,
        _request: BluetoothReadRequest,
    ) -> Result<BluetoothReadResult, BluetoothError> {
        Err(BluetoothError::unsupported("read_characteristic"))
    }

    fn write_characteristic(&self, _request: BluetoothWriteRequest) -> Result<(), BluetoothError> {
        Err(BluetoothError::unsupported("write_characteristic"))
    }

    fn start_advertising(
        &self,
        _request: BluetoothAdvertiseRequest,
    ) -> Result<BluetoothAdvertiseReceipt, BluetoothError> {
        Err(BluetoothError::unsupported("start_advertising"))
    }

    fn stop_advertising(
        &self,
        _request: BluetoothStopAdvertiseRequest,
    ) -> Result<(), BluetoothError> {
        Err(BluetoothError::unsupported("stop_advertising"))
    }
}

#[derive(Debug, Clone)]
pub struct MemoryBluetoothHost {
    availability: BluetoothAvailability,
    devices: Vec<BluetoothDevice>,
    read_result: BluetoothReadResult,
}

impl MemoryBluetoothHost {
    pub fn new(
        availability: BluetoothAvailability,
        devices: Vec<BluetoothDevice>,
        read_result: BluetoothReadResult,
    ) -> Self {
        Self {
            availability,
            devices,
            read_result,
        }
    }
}

impl Default for MemoryBluetoothHost {
    fn default() -> Self {
        let device = BluetoothDevice {
            id: "memory-bluetooth".into(),
            name: Some("Memory Bluetooth".into()),
            address: Some("00:11:22:33:44:55".into()),
            rssi: Some(-42),
            paired: true,
            modes: vec![BluetoothMode::Classic, BluetoothMode::LowEnergy],
        };
        Self::new(
            BluetoothAvailability {
                permission: BluetoothPermission::Granted,
                enabled: true,
                supports_classic: true,
                supports_low_energy: true,
            },
            vec![device],
            BluetoothReadResult {
                value: b"fission".to_vec(),
            },
        )
    }
}

impl BluetoothHost for MemoryBluetoothHost {
    fn availability(&self) -> Result<BluetoothAvailability, BluetoothError> {
        Ok(self.availability.clone())
    }

    fn request_permission(
        &self,
        _request: BluetoothPermissionRequest,
    ) -> Result<BluetoothPermission, BluetoothError> {
        Ok(self.availability.permission)
    }

    fn scan_devices(
        &self,
        _request: BluetoothScanRequest,
    ) -> Result<BluetoothScanResult, BluetoothError> {
        Ok(BluetoothScanResult {
            devices: self.devices.clone(),
        })
    }

    fn connect_device(
        &self,
        request: BluetoothConnectRequest,
    ) -> Result<BluetoothConnection, BluetoothError> {
        let device = self
            .devices
            .iter()
            .find(|device| device.id == request.device_id)
            .cloned()
            .ok_or_else(|| BluetoothError::new("not_found", "Bluetooth device not found"))?;
        Ok(BluetoothConnection {
            connection_id: format!("memory:{}", device.id),
            device,
        })
    }

    fn disconnect_device(
        &self,
        _request: BluetoothDisconnectRequest,
    ) -> Result<(), BluetoothError> {
        Ok(())
    }

    fn read_characteristic(
        &self,
        _request: BluetoothReadRequest,
    ) -> Result<BluetoothReadResult, BluetoothError> {
        Ok(self.read_result.clone())
    }

    fn write_characteristic(&self, _request: BluetoothWriteRequest) -> Result<(), BluetoothError> {
        Ok(())
    }

    fn start_advertising(
        &self,
        _request: BluetoothAdvertiseRequest,
    ) -> Result<BluetoothAdvertiseReceipt, BluetoothError> {
        Ok(BluetoothAdvertiseReceipt {
            advertisement_id: "memory-advertisement".into(),
        })
    }

    fn stop_advertising(
        &self,
        _request: BluetoothStopAdvertiseRequest,
    ) -> Result<(), BluetoothError> {
        Ok(())
    }
}

pub(crate) fn register_bluetooth_capabilities(
    async_registry: &mut AsyncRegistry,
    host: Arc<dyn BluetoothHost>,
) {
    let availability_host = host.clone();
    async_registry.register_operation_capability(GET_BLUETOOTH_AVAILABILITY, move |(), _| {
        let host = availability_host.clone();
        async move { host.availability() }
    });

    let permission_host = host.clone();
    async_registry.register_operation_capability(
        REQUEST_BLUETOOTH_PERMISSION,
        move |request, _| {
            let host = permission_host.clone();
            async move { host.request_permission(request) }
        },
    );

    let scan_host = host.clone();
    async_registry.register_operation_capability(SCAN_BLUETOOTH_DEVICES, move |request, _| {
        let host = scan_host.clone();
        async move { host.scan_devices(request) }
    });

    let connect_host = host.clone();
    async_registry.register_operation_capability(CONNECT_BLUETOOTH_DEVICE, move |request, _| {
        let host = connect_host.clone();
        async move { host.connect_device(request) }
    });

    let disconnect_host = host.clone();
    async_registry.register_operation_capability(DISCONNECT_BLUETOOTH_DEVICE, move |request, _| {
        let host = disconnect_host.clone();
        async move { host.disconnect_device(request) }
    });

    let read_host = host.clone();
    async_registry.register_operation_capability(
        READ_BLUETOOTH_CHARACTERISTIC,
        move |request, _| {
            let host = read_host.clone();
            async move { host.read_characteristic(request) }
        },
    );

    let write_host = host.clone();
    async_registry.register_operation_capability(
        WRITE_BLUETOOTH_CHARACTERISTIC,
        move |request, _| {
            let host = write_host.clone();
            async move { host.write_characteristic(request) }
        },
    );

    let advertise_host = host.clone();
    async_registry.register_operation_capability(START_BLUETOOTH_ADVERTISING, move |request, _| {
        let host = advertise_host.clone();
        async move { host.start_advertising(request) }
    });

    async_registry.register_operation_capability(STOP_BLUETOOTH_ADVERTISING, move |request, _| {
        let host = host.clone();
        async move { host.stop_advertising(request) }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_host_reports_errors() {
        let host = UnsupportedBluetoothHost;
        assert!(host.scan_devices(BluetoothScanRequest::default()).is_err());
    }

    #[test]
    fn memory_host_scans_connects_and_reads() {
        let host = MemoryBluetoothHost::default();
        let scan = host.scan_devices(BluetoothScanRequest::default()).unwrap();
        assert_eq!(scan.devices.len(), 1);

        let connection = host
            .connect_device(BluetoothConnectRequest {
                device_id: scan.devices[0].id.clone(),
                service_uuids: Vec::new(),
            })
            .unwrap();
        assert_eq!(connection.device.id, "memory-bluetooth");

        let read = host
            .read_characteristic(BluetoothReadRequest::default())
            .unwrap();
        assert_eq!(read.value, b"fission".to_vec());
    }
}
