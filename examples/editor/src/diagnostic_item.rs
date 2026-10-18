use crate::layout::DIAGNOSTIC_MESSAGE_PREVIEW_CHARS;
use crate::model::{on_open_file, DiagSeverity, Diagnostic, EditorState, OpenFile};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) DiagnosticItem<EditorState> {
        path: String,
        diagnostic: Diagnostic,
    } |item, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let (icon, color) = match item.diagnostic.severity {
            DiagSeverity::Error => ("x", palette.error_text),
            DiagSeverity::Warning => ("!", palette.warning_text),
            DiagSeverity::Info => ("i", palette.info_text),
            DiagSeverity::Hint => ("?", palette.dim_text),
        };
        let filename = item.path.rsplit('/').next().unwrap_or(&item.path);
        let label = format!(
            "{} {}:{}:{}",
            icon, filename, item.diagnostic.line, item.diagnostic.col
        );
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: VStack(
                    spacing: tokens.spacing.xs,
                    children: [
                        Text(label)
                            .size(tokens.typography.font_size_sm)
                            .color(color),
                        Text(
                            item.diagnostic
                                .message
                                .chars()
                                .take(DIAGNOSTIC_MESSAGE_PREVIEW_CHARS)
                                .collect::<String>(),
                        )
                        .size(tokens.typography.font_size_xs)
                        .color(palette.panel_text),
                    ],
            ),
            on_press: cx.bind(OpenFile(item.path.clone()), reduce_with!(on_open_file)),
            padding: [
                tokens.spacing.xs,
                tokens.spacing.xs,
                tokens.spacing.none,
                tokens.spacing.none,
            ],
        )
}
