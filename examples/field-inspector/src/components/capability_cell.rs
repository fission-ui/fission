use crate::components::ui::{BodyText, StatusPill};
use crate::model::{CapabilityLine, FieldInspectorState};
use fission::prelude::*;
fission_component! {
    pub CapabilityCell<FieldInspectorState> { line: CapabilityLine }
    |cell, _state, cx| {
        let tokens = &cx.view.env().theme.tokens; let typography = &tokens.typography;
        let semantic_identifier = format!("field-inspector.capability.{}", cell.line.title.to_ascii_lowercase().replace([' ', '/'], "-"));
        let semantic_label = cell.line.title;
    } =>
        SemanticsRegion(
            Container(Column(
                gap: tokens.spacing.s,
                children: [
                    Row(
                        gap: tokens.spacing.s,
                        children: [
                            Text(cell.line.title).size(typography.font_size_base).line_height(typography.font_size_base * typography.line_height_snug).weight(typography.font_weight_bold).color(tokens.colors.text_primary),
                            Spacer(flex_grow: 1.0),
                            StatusPill(cell.line.state.label().to_string(), cell.line.state),
                        ],
                    ),
                    BodyText(cell.line.detail),
                ],
            )).bg(tokens.colors.background.with_alpha(150)).border(tokens.colors.border.with_alpha(110), 1.0).border_radius(tokens.radii.xl).padding_all(tokens.spacing.s),
        ).identifier(semantic_identifier).label(semantic_label)
}
