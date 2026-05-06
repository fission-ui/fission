use crate::model::{BottomPanelTab, EditorState};
use fission_core::op::Color;
use fission_core::ui::{Button, ButtonVariant, Container, Node, Text};
use fission_core::{BuildCtx, Handler, View, Widget};
use fission_ir::NodeId;
use fission_widgets::{HStack, Spacer, TerminalView, VStack};
use std::path::Path;

pub struct TerminalPanel;

const BG: Color = Color {
    r: 24,
    g: 24,
    b: 24,
    a: 255,
};
const HEADER_BG: Color = Color {
    r: 37,
    g: 37,
    b: 38,
    a: 255,
};
const BORDER: Color = Color {
    r: 48,
    g: 48,
    b: 49,
    a: 255,
};
const TEXT: Color = Color {
    r: 204,
    g: 204,
    b: 204,
    a: 255,
};
const MUTED: Color = Color {
    r: 150,
    g: 150,
    b: 150,
    a: 255,
};

impl Widget<EditorState> for TerminalPanel {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let is_terminal = view.state.bottom_panel_tab == BottomPanelTab::Terminal;
        let is_problems = view.state.bottom_panel_tab == BottomPanelTab::Problems;
        let set_terminal = ctx.bind(
            crate::model::SetBottomPanelTab(BottomPanelTab::Terminal),
            (|s: &mut EditorState, a: crate::model::SetBottomPanelTab, _| {
                s.bottom_panel_tab = a.0;
                if a.0 == BottomPanelTab::Terminal {
                    s.ensure_terminal_session();
                }
            }) as Handler<EditorState, crate::model::SetBottomPanelTab>,
        );
        let set_problems = ctx.bind(
            crate::model::SetBottomPanelTab(BottomPanelTab::Problems),
            (|s: &mut EditorState, a: crate::model::SetBottomPanelTab, _| {
                s.bottom_panel_tab = a.0;
            }) as Handler<EditorState, crate::model::SetBottomPanelTab>,
        );

        let tab =
            |label: &str, active: bool, action: fission_core::ActionEnvelope, id: &str| -> Node {
                Button {
                    id: Some(NodeId::explicit(id)),
                    variant: ButtonVariant::Ghost,
                    child: Some(Box::new(
                        VStack {
                            spacing: Some(0.0),
                            children: vec![
                                Container::new(
                                    Text::new(label)
                                        .size(11.0)
                                        .color(if active { TEXT } else { MUTED })
                                        .into_node(),
                                )
                                .padding_all(6.0)
                                .into_node(),
                                Container::new(Spacer::default().into_node())
                                    .height(2.0)
                                    .bg(if active {
                                        TEXT
                                    } else {
                                        Color {
                                            r: 0,
                                            g: 0,
                                            b: 0,
                                            a: 0,
                                        }
                                    })
                                    .into_node(),
                            ],
                        }
                        .into_node(),
                    )),
                    on_press: Some(action),
                    padding: Some([0.0; 4]),
                    ..Default::default()
                }
                .into_node()
            };

        let title = view
            .state
            .terminal_session
            .as_ref()
            .map(|session| format_terminal_title(&session.title()))
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Terminal".into());

        let header = Container::new(
            HStack {
                spacing: Some(0.0),
                children: vec![
                    tab(
                        "TERMINAL",
                        is_terminal,
                        set_terminal,
                        "editor_terminal_tab_button",
                    ),
                    tab(
                        "PROBLEMS",
                        is_problems,
                        set_problems,
                        "editor_problems_tab_button",
                    ),
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                    Container::new(Text::new(title).size(11.0).color(MUTED).into_node())
                        .padding_all(8.0)
                        .into_node(),
                ],
            }
            .into_node(),
        )
        .bg(HEADER_BG)
        .height(28.0)
        .border(BORDER, 1.0)
        .flex_shrink(0.0)
        .into_node();

        let sidebar_width = view
            .state
            .sidebar_width
            .min((view.viewport_size().width - 160.0).clamp(180.0, 360.0));
        let panel_width = (view.viewport_size().width
            - 48.0
            - if view.state.sidebar_visible {
                sidebar_width + 1.0
            } else {
                0.0
            })
        .max(280.0);
        let terminal_height = (view
            .state
            .terminal_height
            .min((view.viewport_size().height * 0.33).max(96.0))
            - 28.0)
            .max(72.0);

        let content = if is_terminal {
            if let Some(session) = view.state.terminal_session.clone() {
                TerminalView::new(session, panel_width, terminal_height)
                    .font_size(13.0)
                    .line_height(18.0)
                    .padding(10.0, 8.0)
                    .build(ctx, view)
            } else {
                Container::new(
                    Text::new("Terminal session unavailable")
                        .size(13.0)
                        .color(MUTED)
                        .into_node(),
                )
                .padding_all(12.0)
                .bg(BG)
                .flex_grow(1.0)
                .into_node()
            }
        } else {
            crate::diagnostics_panel::DiagnosticsPanel.build(ctx, view)
        };
        let content = Container::new(content)
            .id(NodeId::explicit(if is_terminal {
                "editor_terminal_tab_content"
            } else {
                "editor_problems_tab_content"
            }))
            .flex_grow(1.0)
            .into_node();

        Container::new(
            fission_core::ui::Column {
                children: vec![header, content],
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
        )
        .height(
            view.state
                .terminal_height
                .min((view.viewport_size().height * 0.33).max(96.0)),
        )
        .bg(BG)
        .flex_shrink(0.0)
        .into_node()
    }
}

fn format_terminal_title(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return "Terminal".into();
    }

    let path = Path::new(trimmed);
    if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
        if let Some(parent) = path
            .parent()
            .and_then(|value| value.file_name())
            .and_then(|value| value.to_str())
        {
            return format!(".../{parent}/{name}");
        }
        return name.to_string();
    }

    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 28 {
        trimmed.to_string()
    } else {
        format!(
            "...{}",
            chars[chars.len() - 25..].iter().collect::<String>()
        )
    }
}
