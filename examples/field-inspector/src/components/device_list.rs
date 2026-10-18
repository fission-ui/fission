use crate::components::bluetooth_device_row::BluetoothDeviceRow;
use crate::components::ui::BodyText;
use crate::components::wifi_network_row::WifiNetworkRow;
use crate::model::FieldInspectorState;
use fission::prelude::*;
fission_component! {
    pub DeviceList<FieldInspectorState> {}
    |_list, state, cx| {} => Column(gap: cx.view.env().theme.tokens.spacing.s, children: [
        if state.bluetooth_devices.is_empty() && state.wifi_networks.is_empty() {
            BodyText("No nearby device data yet. Run a sensor scan.".to_string())
        }
        for device in state.bluetooth_devices.iter().cloned() {
            BluetoothDeviceRow(device.clone(), key: device.id.clone())
        }
        for network in state.wifi_networks.iter().cloned() {
            WifiNetworkRow(network.clone(), key: network.ssid.clone())
        }
    ])
}
