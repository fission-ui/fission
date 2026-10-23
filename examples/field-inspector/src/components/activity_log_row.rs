use crate::components::ui::{MutedText, StatusPill};
use crate::model::{CapabilityLog, FieldInspectorState};
use fission::prelude::*;
fission_component! {
    pub ActivityLogRow<FieldInspectorState> { log: CapabilityLog }
    |row, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Row(
            gap: tokens.spacing.s,
            align_items: ir_op::AlignItems::Start,
            children: [
                StatusPill(row.log.state.label().to_string(), row.log.state),
                Column(
                    gap: tokens.spacing.xs,
                    flex_grow: 1.0,
                    children: [
                        Text(row.log.title).size(typography.font_size_base).weight(typography.font_weight_bold),
                        MutedText(row.log.detail),
                    ],
                ),
            ],
        )
}
