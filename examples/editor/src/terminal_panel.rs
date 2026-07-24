use crate::layout::{
    ACTIVITY_BAR_WIDTH, DIVIDER_THICKNESS, EDITOR_HORIZONTAL_RESERVE, MIN_TERMINAL_CONTENT_HEIGHT,
    MIN_TERMINAL_HEIGHT, MIN_TERMINAL_WIDTH, PANEL_HEADER_HEIGHT, SIDEBAR_MAX_WIDTH,
    SIDEBAR_MIN_WIDTH, TERMINAL_HEIGHT_FRACTION,
};
use crate::model::{BottomPanelTab, EditorState};
use crate::palette::{BORDER_COLOR, DIM_TEXT, SURFACE_BG, TERMINAL_BG};
use crate::terminal_panel_tab::TerminalPanelTab;
use fission::core::reduce_with;
use fission::core::ui::{Container, Text, Widget};
use fission::widgets::{HStack, Spacer, TerminalView};
use fission::{WidgetId, WidgetIdExt};
use std::path::Path;

pub struct TerminalPanel;

impl From<TerminalPanel> for Widget {
    fn from(_component: TerminalPanel) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let is_terminal = view.state().bottom_panel_tab == BottomPanelTab::Terminal;
        let is_problems = view.state().bottom_panel_tab == BottomPanelTab::Problems;
        let set_terminal = ctx.bind(
            crate::model::SetBottomPanelTab(BottomPanelTab::Terminal),
            reduce_with!(
                (|s: &mut EditorState, a: crate::model::SetBottomPanelTab, _| {
                    s.bottom_panel_tab = a.0;
                    if a.0 == BottomPanelTab::Terminal {
                        s.ensure_terminal_session();
                    }
                })
            ),
        );
        let set_problems = ctx.bind(
            crate::model::SetBottomPanelTab(BottomPanelTab::Problems),
            reduce_with!(
                (|s: &mut EditorState, a: crate::model::SetBottomPanelTab, _| {
                    s.bottom_panel_tab = a.0;
                })
            ),
        );

        let title = view
            .state()
            .terminal_session
            .as_ref()
            .map(|session| format_terminal_title(&session.title()))
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Terminal".into());

        let header = Container::new(HStack {
            spacing: Some(tokens.spacing.none),
            children: vec![
                TerminalPanelTab {
                    label: "TERMINAL",
                    active: is_terminal,
                    action: set_terminal,
                    id: "editor_terminal_tab_button",
                }
                .into(),
                TerminalPanelTab {
                    label: "PROBLEMS",
                    active: is_problems,
                    action: set_problems,
                    id: "editor_problems_tab_button",
                }
                .into(),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Container::new(
                    Text::new(title)
                        .size(tokens.typography.font_size_xs)
                        .color(DIM_TEXT),
                )
                .padding_all(tokens.spacing.s)
                .into(),
            ],
        })
        .bg(SURFACE_BG)
        .height(PANEL_HEADER_HEIGHT)
        .border(BORDER_COLOR, 1.0)
        .flex_shrink(0.0)
        .into();

        let sidebar_width = view.state().sidebar_width.min(
            (view.viewport_size().width - EDITOR_HORIZONTAL_RESERVE)
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
        );
        let panel_width = (view.viewport_size().width
            - ACTIVITY_BAR_WIDTH
            - if view.state().sidebar_visible {
                sidebar_width + DIVIDER_THICKNESS
            } else {
                tokens.spacing.none
            })
        .max(MIN_TERMINAL_WIDTH);
        let terminal_height = (view.state().terminal_height.min(
            (view.viewport_size().height * TERMINAL_HEIGHT_FRACTION).max(MIN_TERMINAL_HEIGHT),
        ) - PANEL_HEADER_HEIGHT)
            .max(MIN_TERMINAL_CONTENT_HEIGHT);

        let content: Widget = if is_terminal {
            if let Some(session) = view.state().terminal_session.clone() {
                TerminalView::new(session, panel_width, terminal_height)
                    .font_size(tokens.typography.font_size_sm)
                    .line_height(
                        tokens.typography.font_size_sm * tokens.typography.line_height_normal,
                    )
                    .padding(tokens.spacing.s, tokens.spacing.s)
                    .into()
            } else {
                Container::new(
                    Text::new("Terminal session unavailable")
                        .size(tokens.typography.font_size_sm)
                        .color(DIM_TEXT),
                )
                .padding_all(tokens.spacing.m)
                .bg(TERMINAL_BG)
                .flex_grow(1.0)
                .into()
            }
        } else {
            crate::diagnostics_panel::DiagnosticsPanel.into()
        };
        let content = Container::new(content)
            .flex_grow(1.0)
            .id(WidgetId::explicit(if is_terminal {
                "editor_terminal_tab_content"
            } else {
                "editor_problems_tab_content"
            }));

        Container::new(fission::core::ui::Column {
            children: vec![header, content],
            flex_grow: 1.0,
            ..Default::default()
        })
        .height(
            view.state().terminal_height.min(
                (view.viewport_size().height * TERMINAL_HEIGHT_FRACTION).max(MIN_TERMINAL_HEIGHT),
            ),
        )
        .bg(TERMINAL_BG)
        .flex_shrink(0.0)
        .into()
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
