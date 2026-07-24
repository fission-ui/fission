use super::{MutedText, SoftPanel};
use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct Metric {
    pub label: String,
    pub value: String,
}

impl Metric {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

impl From<Metric> for Widget {
    fn from(metric: Metric) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        SoftPanel::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                MutedText::new(metric.label),
                Text::new(metric.value)
                    .size(typography.body_large_size)
                    .line_height(typography.body_large_size * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
            ],
            ..Default::default()
        })
        .into()
    }
}
