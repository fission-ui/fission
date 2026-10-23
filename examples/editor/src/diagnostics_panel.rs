use crate::diagnostic_item::DiagnosticItem;
use crate::model::{DiagSeverity, EditorState};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub DiagnosticsPanel<EditorState> |_panel, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let mut diagnostics: Vec<(&String, &crate::model::Diagnostic)> = state
            .diagnostics
            .iter()
            .flat_map(|(path, diags)| diags.iter().map(move |d| (path, d)))
            .collect();
        diagnostics.sort_by(|a, b| {
            let sev_ord = |s: &DiagSeverity| match s {
                DiagSeverity::Error => 0,
                DiagSeverity::Warning => 1,
                DiagSeverity::Info => 2,
                DiagSeverity::Hint => 3,
            };
            sev_ord(&a.1.severity).cmp(&sev_ord(&b.1.severity))
        });
    } =>
        if diagnostics.is_empty() {
            Container(
                Text("No problems detected")
                    .size(tokens.typography.font_size_sm)
                    .color(palette.dim_text),
            )
            .bg(palette.terminal_bg)
            .padding_all(tokens.spacing.s)
            .flex_grow(1.0)
        } else {
            Container(
                Scroll(
                    direction: FlexDirection::Column,
                    child: VStack(
                        spacing: tokens.spacing.xs,
                        children: [
                            for (path, diagnostic) in diagnostics {
                                DiagnosticItem(
                                    path.clone(),
                                    diagnostic.clone(),
                                    key: format!("{path}:{}:{}", diagnostic.line, diagnostic.col),
                                )
                            },
                        ],
                    ),
                    show_scrollbar: true,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                ),
            )
            .bg(palette.terminal_bg)
            .padding_all(tokens.spacing.xs)
            .flex_grow(1.0)
        }
}
