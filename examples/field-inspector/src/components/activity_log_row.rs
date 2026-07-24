use crate::components::ui::{MutedText, StatusPill};
use crate::model::{CapabilityLog, FieldInspectorState};
use fission::prelude::*;

pub struct ActivityLogRow {
    pub log: CapabilityLog,
}

impl From<ActivityLogRow> for Widget {
    fn from(row: ActivityLogRow) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Row {
            gap: Some(tokens.spacing.s),
            align_items: ir_op::AlignItems::Start,
            children: widgets![
                StatusPill::new(row.log.state.label(), row.log.state),
                Column {
                    gap: Some(tokens.spacing.xs),
                    flex_grow: 1.0,
                    children: widgets![
                        Text::new(row.log.title)
                            .size(typography.font_size_base)
                            .weight(typography.font_weight_bold),
                        MutedText::new(row.log.detail),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
