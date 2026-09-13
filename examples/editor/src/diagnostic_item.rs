use crate::layout::DIAGNOSTIC_MESSAGE_PREVIEW_CHARS;
use crate::model::{DiagSeverity, Diagnostic, EditorState, OpenFile};
use crate::palette::EditorPalette;
use fission::prelude::*;
use fission::widgets::VStack;

pub(crate) struct DiagnosticItem {
    pub path: String,
    pub diagnostic: Diagnostic,
    pub open_id: ActionId,
}

impl From<DiagnosticItem> for Widget {
    fn from(item: DiagnosticItem) -> Self {
        let (_ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
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

        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(
                VStack {
                    spacing: Some(tokens.spacing.xs),
                    children: widgets![
                        Text::new(label)
                            .size(tokens.typography.font_size_sm)
                            .color(color),
                        Text::new(
                            item.diagnostic
                                .message
                                .chars()
                                .take(DIAGNOSTIC_MESSAGE_PREVIEW_CHARS)
                                .collect::<String>(),
                        )
                        .size(tokens.typography.font_size_xs)
                        .color(palette.panel_text),
                    ],
                }
                .into(),
            ),
            on_press: Some(ActionEnvelope {
                id: item.open_id,
                payload: serde_json::to_vec(&OpenFile(item.path)).unwrap(),
            }),
            padding: Some([
                tokens.spacing.xs,
                tokens.spacing.xs,
                tokens.spacing.none,
                tokens.spacing.none,
            ]),
            ..Default::default()
        }
        .into()
    }
}
