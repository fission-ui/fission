use crate::components::ui::{MutedText, SoftPanel, StatusPill};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;
fission_component! {
    pub BluetoothDeviceRow<FieldInspectorState> { device: BluetoothDevice }
    |row, _state, cx| { let tokens = &cx.view.env().theme.tokens; let typography = &tokens.typography; } =>
        SoftPanel(Row(
            gap: tokens.spacing.s,
            children: [
                Column(
                    gap: tokens.spacing.xs, flex_grow: 1.0,
                    children: [
                        Text(row.device.name.clone().unwrap_or_else(|| row.device.id.clone())).size(typography.label_large_size).weight(typography.font_weight_bold),
                        MutedText(format!("RSSI {:?}, paired {}", row.device.rssi, row.device.paired)),
                    ],
                ),
                StatusPill("Bluetooth".to_string(), CapabilityState::Ready),
            ],
        ))
}
