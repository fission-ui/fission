use crate::components::device_list::DeviceList;
use crate::components::section_header::SectionHeader;
use crate::components::ui::{
    ActionButton, Metric, PanelCard, ResponsiveGrid, DEFAULT_ITEM_MIN_WIDTH,
};
use crate::model::{
    on_connect_sensor, on_read_sensor, on_scan_sensors, ConnectSensor, FieldInspectorState,
    ReadSensor, ScanSensors,
};
use fission::prelude::*;
fission_component! {
    pub SensorsPanel<FieldInspectorState> {}
    |_panel, state, cx| {
        let scan = with_reducer!(cx.build, ScanSensors, on_scan_sensors);
        let connect = state.bluetooth_devices.first().map(|device| with_reducer!(cx.build, ConnectSensor(device.id.clone()), on_connect_sensor));
        let spacing = &cx.view.env().theme.tokens.spacing;
    } =>
        PanelCard(Column(
            gap: spacing.m,
            children: [
                SectionHeader("Read local context", "Nearby Bluetooth and Wi-Fi data belong behind host capabilities because hardware and permissions vary across platforms."),
                Row(gap: spacing.s, wrap: ir_op::FlexWrap::Wrap, children: [
                    ActionButton("field-inspector.sensors.scan".to_string(), "Scan nearby devices".to_string(), scan, ButtonVariant::Primary),
                    if let Some(action) = connect {
                        ActionButton("field-inspector.sensors.connect".to_string(), "Connect bridge".to_string(), action, ButtonVariant::SecondaryColor)
                    }
                    ActionButton("field-inspector.sensors.read".to_string(), "Read telemetry".to_string(), with_reducer!(cx.build, ReadSensor, on_read_sensor), ButtonVariant::SecondaryGray),
                ]),
                ResponsiveGrid(
                    [
                        Metric("Bluetooth devices".to_string(), state.bluetooth_devices.len().to_string()),
                        Metric("Wi-Fi networks".to_string(), state.wifi_networks.len().to_string()),
                        Metric("Sensor reading".to_string(), state.sensor_reading.clone().unwrap_or_else(|| "Pending".to_string())),
                    ],
                    DEFAULT_ITEM_MIN_WIDTH,
                ),
                DeviceList(),
            ],
        ))
}
