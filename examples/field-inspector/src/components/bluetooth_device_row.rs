use crate::components::ui::{MutedText, SoftPanel, StatusPill};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;

pub struct BluetoothDeviceRow {
    pub device: BluetoothDevice,
}

impl From<BluetoothDeviceRow> for Widget {
    fn from(row: BluetoothDeviceRow) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        SoftPanel::new(Row {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Column {
                    gap: Some(tokens.spacing.xs),
                    flex_grow: 1.0,
                    children: widgets![
                        Text::new(
                            row.device
                                .name
                                .clone()
                                .unwrap_or_else(|| row.device.id.clone())
                        )
                        .size(typography.label_large_size)
                        .weight(typography.font_weight_bold),
                        MutedText::new(format!(
                            "RSSI {:?}, paired {}",
                            row.device.rssi, row.device.paired
                        )),
                    ],
                    ..Default::default()
                },
                StatusPill::new("Bluetooth", CapabilityState::Ready),
            ],
            ..Default::default()
        })
        .into()
    }
}
