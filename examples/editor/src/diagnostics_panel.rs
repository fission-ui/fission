use crate::model::{DiagSeverity, EditorState, NavigateDiagnostic, OpenFile};
use fission_core::op::Color;
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, Node, Scroll, Text};
use fission_core::{ActionEnvelope, BuildCtx, FlexDirection, Handler, View, Widget};
use fission_widgets::{VStack, HStack, Spacer};
use serde_json;

pub struct DiagnosticsPanel;

impl Widget<EditorState> for DiagnosticsPanel {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let text_color = Color { r: 204, g: 204, b: 204, a: 255 };
        let error_color = Color { r: 244, g: 71, b: 71, a: 255 };
        let warn_color = Color { r: 255, g: 193, b: 7, a: 255 };
        let info_color = Color { r: 66, g: 133, b: 244, a: 255 };
        let dim_color = Color { r: 140, g: 140, b: 140, a: 255 };

        let open_id = ctx.bind(
            OpenFile(String::new()),
            (|s: &mut EditorState, a: OpenFile, _| s.open_file(a.0))
                as Handler<EditorState, OpenFile>,
        ).id;

        let mut all_diags: Vec<(&String, &crate::model::Diagnostic)> = view.state.diagnostics
            .iter()
            .flat_map(|(path, diags)| diags.iter().map(move |d| (path, d)))
            .collect();
        all_diags.sort_by(|a, b| {
            let sev_ord = |s: &DiagSeverity| match s { DiagSeverity::Error => 0, DiagSeverity::Warning => 1, DiagSeverity::Info => 2, DiagSeverity::Hint => 3 };
            sev_ord(&a.1.severity).cmp(&sev_ord(&b.1.severity))
        });

        let bg = Color { r: 24, g: 24, b: 24, a: 255 };

        if all_diags.is_empty() {
            return Container::new(
                Text::new("No problems detected")
                    .size(12.0)
                    .color(dim_color)
                    .into_node(),
            )
            .bg(bg)
            .padding_all(8.0)
            .flex_grow(1.0)
            .into_node();
        }

        let mut items = Vec::new();
        for (path, diag) in &all_diags {
            let (icon, color) = match diag.severity {
                DiagSeverity::Error => ("✕", error_color),
                DiagSeverity::Warning => ("⚠", warn_color),
                DiagSeverity::Info => ("ℹ", info_color),
                DiagSeverity::Hint => ("💡", dim_color),
            };
            let filename = path.rsplit('/').next().unwrap_or(path);
            let label = format!("{} {}:{}:{}", icon, filename, diag.line, diag.col);

            items.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Some(Box::new(
                        VStack {
                            spacing: Some(1.0),
                            children: vec![
                                Text::new(label).size(12.0).color(color).into_node(),
                                Text::new(diag.message.chars().take(80).collect::<String>())
                                    .size(11.0)
                                    .color(text_color)
                                    .into_node(),
                            ],
                        }.into_node(),
                    )),
                    on_press: Some(ActionEnvelope {
                        id: open_id,
                        payload: serde_json::to_vec(&OpenFile(path.to_string())).unwrap(),
                    }),
                    padding: Some([4.0, 4.0, 0.0, 0.0]),
                    ..Default::default()
                }.into_node(),
            );
        }

        Container::new(
            Scroll {
                direction: FlexDirection::Column,
                child: Some(Box::new(
                    VStack { spacing: Some(2.0), children: items }.into_node(),
                )),
                show_scrollbar: true,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            }.into_node(),
        )
        .bg(bg)
        .padding_all(4.0)
        .flex_grow(1.0)
        .into_node()
    }
}
