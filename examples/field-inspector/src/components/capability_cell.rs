use crate::components::ui::{BodyText, StatusPill};
use crate::model::{CapabilityLine, FieldInspectorState};
use fission::prelude::*;

pub struct CapabilityCell {
    pub line: CapabilityLine,
}

impl From<CapabilityCell> for Widget {
    fn from(cell: CapabilityCell) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let semantic_identifier = format!(
            "field-inspector.capability.{}",
            cell.line
                .title
                .to_ascii_lowercase()
                .replace([' ', '/'], "-")
        );
        let semantic_label = cell.line.title;

        SemanticsRegion::new(
            Container::new(Column {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    Row {
                        gap: Some(tokens.spacing.s),
                        children: widgets![
                            Text::new(cell.line.title)
                                .size(typography.font_size_base)
                                .line_height(
                                    typography.font_size_base * typography.line_height_snug
                                )
                                .weight(typography.font_weight_bold)
                                .color(tokens.colors.text_primary),
                            Spacer {
                                flex_grow: 1.0,
                                ..Default::default()
                            },
                            StatusPill::new(cell.line.state.label(), cell.line.state),
                        ],
                        ..Default::default()
                    },
                    BodyText::new(cell.line.detail),
                ],
                ..Default::default()
            })
            .bg(tokens.colors.background.with_alpha(150))
            .border(tokens.colors.border.with_alpha(110), 1.0)
            .border_radius(tokens.radii.xl)
            .padding_all(tokens.spacing.s),
        )
        .identifier(semantic_identifier)
        .label(semantic_label)
        .into()
    }
}
