use crate::components::device_list::DeviceList;
use crate::components::section_header::SectionHeader;
use crate::components::ui::{ActionButton, Metric, PanelCard, ResponsiveGrid};
use crate::model::{
    on_connect_sensor, on_read_sensor, on_scan_sensors, ConnectSensor, FieldInspectorState,
    ReadSensor, ScanSensors,
};
use fission::prelude::*;

pub struct SensorsPanel;

impl From<SensorsPanel> for Widget {
    fn from(_: SensorsPanel) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let scan = with_reducer!(ctx, ScanSensors, on_scan_sensors);
        let read = with_reducer!(ctx, ReadSensor, on_read_sensor);
        let connect =
            view.state().bluetooth_devices.first().map(|device| {
                with_reducer!(ctx, ConnectSensor(device.id.clone()), on_connect_sensor)
            });
        let mut actions = widgets![ActionButton::new(
            "field-inspector.sensors.scan",
            "Scan nearby devices",
            scan,
            ButtonVariant::Primary,
        )];
        if let Some(action) = connect {
            actions.push(
                ActionButton::new(
                    "field-inspector.sensors.connect",
                    "Connect bridge",
                    action,
                    ButtonVariant::SecondaryColor,
                )
                .into(),
            );
        }
        actions.push(
            ActionButton::new(
                "field-inspector.sensors.read",
                "Read telemetry",
                read,
                ButtonVariant::SecondaryGray,
            )
            .into(),
        );
        let spacing = &view.env().theme.tokens.spacing;

        PanelCard::new(Column {
            gap: Some(spacing.m),
            children: widgets![
                SectionHeader {
                    title: "Read local context",
                    body: "Nearby Bluetooth and Wi-Fi data belong behind host capabilities because hardware and permissions vary across platforms.",
                },
                Row {
                    gap: Some(spacing.s),
                    wrap: ir_op::FlexWrap::Wrap,
                    children: actions,
                    ..Default::default()
                },
                ResponsiveGrid::new(widgets![
                    Metric::new(
                        "Bluetooth devices",
                        view.state().bluetooth_devices.len().to_string(),
                    ),
                    Metric::new(
                        "Wi-Fi networks",
                        view.state().wifi_networks.len().to_string(),
                    ),
                    Metric::new(
                        "Sensor reading",
                        view.state()
                            .sensor_reading
                            .clone()
                            .unwrap_or_else(|| "Pending".to_string()),
                    ),
                ]),
                DeviceList,
            ],
            ..Default::default()
        })
        .into()
    }
}
