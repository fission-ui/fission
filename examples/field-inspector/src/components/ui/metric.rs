use super::{MutedText, SoftPanel};
use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! {
    pub Metric<FieldInspectorState> { label: String, value: String }
    |metric, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        SoftPanel(
            Column(
                gap: tokens.spacing.xs,
                children: [
                    MutedText(metric.label),
                    Text(metric.value)
                        .size(typography.body_large_size)
                        .line_height(typography.body_large_size * typography.line_height_snug)
                        .weight(typography.font_weight_bold)
                        .color(tokens.colors.text_primary),
                ],
            ),
        )
}
