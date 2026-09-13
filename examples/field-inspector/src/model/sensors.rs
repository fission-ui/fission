//! Reducers for discovering, connecting to and reading Bluetooth sensors.

use super::*;

#[fission_reducer(ScanSensors)]
pub fn on_scan_sensors(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Sensors;
    state.log(
        "Sensor scan",
        "Scanning Bluetooth and Wi-Fi context",
        CapabilityState::Pending,
    );
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .bluetooth()
        .request_permission(BluetoothPermissionRequest {
            reason: Some("Find the asset sensor bridge".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .bluetooth()
        .scan_devices(BluetoothScanRequest {
            service_uuids: vec![state.selected_order().asset.sensor_service_uuid.to_string()],
            timeout_ms: Some(3_000),
            include_paired: true,
            allow_duplicates: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .wifi()
        .request_permission(WifiPermissionRequest {
            reason: Some("Confirm the technician is on a site network".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .wifi()
        .scan_networks(WifiScanRequest {
            ssid_prefix: None,
            include_hidden: false,
            timeout_ms: Some(3_000),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ConnectSensor)]
pub fn on_connect_sensor(
    state: &mut FieldInspectorState,
    device_id: String,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Sensors;
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .bluetooth()
        .connect_device(BluetoothConnectRequest {
            device_id,
            service_uuids: vec![state.selected_order().asset.sensor_service_uuid.to_string()],
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ReadSensor)]
pub fn on_read_sensor(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Sensors;
    let Some(connection) = &state.bluetooth_connection else {
        state.log(
            "Sensor read",
            "Connect to the sensor bridge first",
            CapabilityState::Warning,
        );
        return;
    };
    let ok: ActionEnvelope = CapabilitySucceeded.into();
    let err: ActionEnvelope = CapabilityFailed.into();
    ctx.effects
        .bluetooth()
        .read_characteristic(BluetoothReadRequest {
            connection_id: connection.connection_id.clone(),
            service_uuid: READ_SERVICE_UUID.into(),
            characteristic_uuid: READ_CHARACTERISTIC_UUID.into(),
        })
        .on_ok(ok)
        .on_err(err);
}
