use crate::diagnostic_item::DiagnosticItem;
use crate::model::{DiagSeverity, EditorState, OpenFile};
use crate::palette::{DIM_TEXT, TERMINAL_BG};
use fission::core::ui::{Container, Scroll, Text, Widget};
use fission::core::{reduce_with, FlexDirection};
use fission::widgets::VStack;

pub struct DiagnosticsPanel;

impl From<DiagnosticsPanel> for Widget {
    fn from(_component: DiagnosticsPanel) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;

        let open_id = ctx
            .bind(
                OpenFile(String::new()),
                reduce_with!((|s: &mut EditorState, a: OpenFile, _| s.open_file(a.0))),
            )
            .id;

        let mut all_diags: Vec<(&String, &crate::model::Diagnostic)> = view
            .state()
            .diagnostics
            .iter()
            .flat_map(|(path, diags)| diags.iter().map(move |d| (path, d)))
            .collect();
        all_diags.sort_by(|a, b| {
            let sev_ord = |s: &DiagSeverity| match s {
                DiagSeverity::Error => 0,
                DiagSeverity::Warning => 1,
                DiagSeverity::Info => 2,
                DiagSeverity::Hint => 3,
            };
            sev_ord(&a.1.severity).cmp(&sev_ord(&b.1.severity))
        });

        if all_diags.is_empty() {
            return Container::new(
                Text::new("No problems detected")
                    .size(tokens.typography.font_size_sm)
                    .color(DIM_TEXT),
            )
            .bg(TERMINAL_BG)
            .padding_all(tokens.spacing.s)
            .flex_grow(1.0)
            .into();
        }

        let items = all_diags
            .into_iter()
            .map(|(path, diagnostic)| {
                DiagnosticItem {
                    path: path.clone(),
                    diagnostic: diagnostic.clone(),
                    open_id,
                }
                .into()
            })
            .collect();

        Container::new(Scroll {
            direction: FlexDirection::Column,
            child: Some(
                VStack {
                    spacing: Some(tokens.spacing.xs),
                    children: items,
                }
                .into(),
            ),
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        })
        .bg(TERMINAL_BG)
        .padding_all(tokens.spacing.xs)
        .flex_grow(1.0)
        .into()
    }
}
