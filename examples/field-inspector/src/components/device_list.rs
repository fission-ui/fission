use crate::components::bluetooth_device_row::BluetoothDeviceRow;
use crate::components::ui::BodyText;
use crate::components::wifi_network_row::WifiNetworkRow;
use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct DeviceList;

impl From<DeviceList> for Widget {
    fn from(_: DeviceList) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let mut children: Vec<Widget> = view
            .state()
            .bluetooth_devices
            .iter()
            .cloned()
            .map(|device| BluetoothDeviceRow { device }.into())
            .collect();

        children.extend(
            view.state()
                .wifi_networks
                .iter()
                .cloned()
                .map(|network| WifiNetworkRow { network }.into()),
        );

        if children.is_empty() {
            children.push(BodyText::new("No nearby device data yet. Run a sensor scan.").into());
        }

        Column {
            gap: Some(view.env().theme.tokens.spacing.s),
            children,
            ..Default::default()
        }
        .into()
    }
}
