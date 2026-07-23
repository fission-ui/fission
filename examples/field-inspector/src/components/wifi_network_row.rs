use crate::components::ui::{MutedText, SoftPanel, StatusPill};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;

pub struct WifiNetworkRow {
    pub network: WifiNetwork,
}

impl From<WifiNetworkRow> for Widget {
    fn from(row: WifiNetworkRow) -> Self {
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
                        Text::new(row.network.ssid.clone())
                            .size(typography.label_large_size)
                            .weight(typography.font_weight_bold),
                        MutedText::new(format!(
                            "RSSI {:?}, security {:?}",
                            row.network.rssi, row.network.security
                        )),
                    ],
                    ..Default::default()
                },
                StatusPill::new(
                    if row.network.connected {
                        "Connected"
                    } else {
                        "Visible"
                    },
                    CapabilityState::Ready,
                ),
            ],
            ..Default::default()
        })
        .into()
    }
}
