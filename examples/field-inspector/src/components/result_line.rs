use crate::components::ui::{MutedText, SoftPanel, StatusPill};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;
fission_component! {
    pub ResultLine<FieldInspectorState> { label: &'static str, value: Option<String>, matched: bool }
    |result, _state, cx| { let tokens = &cx.view.env().theme.tokens; let typography = &tokens.typography; } =>
        SoftPanel(Row(
            gap: tokens.spacing.s,
            children: [
                Column(gap: tokens.spacing.xs, flex_grow: 1.0, children: [
                    Text(result.label).size(typography.font_size_base).weight(typography.font_weight_bold),
                    MutedText(result.value.clone().unwrap_or_else(|| "Waiting for scan".to_string())),
                ]),
                StatusPill(
                    if result.matched { "Matched" } else { "Pending" }.to_string(),
                    if result.matched { CapabilityState::Complete } else { CapabilityState::Idle },
                ),
            ],
        ))
}
