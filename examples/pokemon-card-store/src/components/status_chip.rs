use crate::app::StoreState;
use crate::components::palette::{TEXT_BODY, TEXT_MUTED};
use fission::prelude::*;

pub struct StatusChip {
    pub label: &'static str,
    pub identifier: &'static str,
    pub status: &'static str,
}

impl From<StatusChip> for Widget {
    fn from(chip: StatusChip) -> Self {
        let (_, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new(chip.label)
                    .size(typography.font_size_xs)
                    .line_height(typography.font_size_xs * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(TEXT_MUTED),
                Text::new(chip.status)
                    .size(typography.body_medium_size)
                    .line_height(typography.body_medium_size * typography.line_height_snug)
                    .weight(typography.font_weight_bold)
                    .color(TEXT_BODY)
                    .semantics_identifier(chip.identifier),
            ],
            ..Default::default()
        }
        .into()
    }
}
