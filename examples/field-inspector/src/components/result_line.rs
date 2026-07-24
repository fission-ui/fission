use crate::components::ui::{MutedText, SoftPanel, StatusPill};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;

pub struct ResultLine {
    pub label: &'static str,
    pub value: Option<String>,
    pub matched: bool,
}

impl From<ResultLine> for Widget {
    fn from(result: ResultLine) -> Self {
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
                        Text::new(result.label)
                            .size(typography.font_size_base)
                            .weight(typography.font_weight_bold),
                        MutedText::new(
                            result
                                .value
                                .unwrap_or_else(|| "Waiting for scan".to_string())
                        ),
                    ],
                    ..Default::default()
                },
                StatusPill::new(
                    if result.matched { "Matched" } else { "Pending" },
                    if result.matched {
                        CapabilityState::Complete
                    } else {
                        CapabilityState::Idle
                    },
                ),
            ],
            ..Default::default()
        })
        .into()
    }
}
