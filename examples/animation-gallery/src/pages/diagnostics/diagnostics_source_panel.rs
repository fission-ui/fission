use super::diagnostics_case::DiagnosticsCase;
use crate::state::AnimationGalleryState;
use crate::style;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub(super) DiagnosticsSourcePanel<AnimationGalleryState> {
        diagnostics: DiagnosticsCase,
    } |panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
            gap: tokens.spacing.s,
            children: [
                SectionTitle(panel.diagnostics.primary_title.to_string()),
                CodeBlock(panel.diagnostics.primary_source.to_string()),
                SectionTitle(panel.diagnostics.secondary_title.to_string()),
                CodeBlock(panel.diagnostics.secondary_source.to_string()),
            ],
        ))
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
