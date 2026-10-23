use crate::components::ui::{MutedText, SoftPanel, StatusPill};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;
fission_component! {
    pub WifiNetworkRow<FieldInspectorState> { network: WifiNetwork }
    |row, _state, cx| { let tokens = &cx.view.env().theme.tokens; let typography = &tokens.typography; } =>
        SoftPanel(Row(
            gap: tokens.spacing.s,
            children: [
                Column(gap: tokens.spacing.xs, flex_grow: 1.0, children: [
                    Text(row.network.ssid.clone()).size(typography.label_large_size).weight(typography.font_weight_bold),
                    MutedText(format!("RSSI {:?}, security {:?}", row.network.rssi, row.network.security)),
                ]),
                StatusPill(if row.network.connected { "Connected" } else { "Visible" }.to_string(), CapabilityState::Ready),
            ],
        ))
}
