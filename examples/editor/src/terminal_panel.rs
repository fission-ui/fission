#[cfg(not(target_arch = "wasm32"))]
use crate::layout::{
    ACTIVITY_BAR_WIDTH, DIVIDER_THICKNESS, EDITOR_HORIZONTAL_RESERVE, MIN_TERMINAL_CONTENT_HEIGHT,
    MIN_TERMINAL_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH,
};
use crate::layout::{MIN_TERMINAL_HEIGHT, PANEL_HEADER_HEIGHT, TERMINAL_HEIGHT_FRACTION};
use crate::model::{on_set_bottom_panel_tab, BottomPanelTab, EditorState, SetBottomPanelTab};
use crate::palette::EditorPalette;
use crate::terminal_panel_tab::TerminalPanelTab;
use fission::core::reduce_with;
use fission::core::ui::{Column, Container, Text, Widget};
#[cfg(not(target_arch = "wasm32"))]
use fission::widgets::TerminalView;
use fission::widgets::{HStack, Spacer};
use fission::{WidgetId, WidgetIdExt};
use std::path::Path;

/// The bottom panel: terminal and problems tabs above the chosen tab's content.
pub struct TerminalPanel;

impl From<TerminalPanel> for Widget {
    fn from(_component: TerminalPanel) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let state = view.state();
        let is_terminal = state.bottom_panel_tab == BottomPanelTab::Terminal;
        let content_id = if is_terminal {
            "editor_terminal_tab_content"
        } else {
            "editor_problems_tab_content"
        };

        Container::new(Column {
            children: vec![
                BottomPanelHeader.into(),
                Container::new(BottomPanelBody)
                    .flex_grow(1.0)
                    .id(WidgetId::explicit(content_id)),
            ],
            flex_grow: 1.0,
            ..Default::default()
        })
        .height(panel_height(state, view.viewport_size().height))
        .bg(palette.terminal_bg)
        .flex_shrink(0.0)
        .into()
    }
}

/// The panel's height: the user's chosen height, kept within a fraction of the window.
fn panel_height(state: &EditorState, viewport_height: f32) -> f32 {
    state
        .terminal_height
        .min((viewport_height * TERMINAL_HEIGHT_FRACTION).max(MIN_TERMINAL_HEIGHT))
}

/// The tab buttons and the running terminal's title.
struct BottomPanelHeader;

impl From<BottomPanelHeader> for Widget {
    fn from(_header: BottomPanelHeader) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        let title = state
            .terminal_session
            .as_ref()
            .map(|session| format_terminal_title(&session.title()))
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Terminal".into());

        Container::new(HStack {
            spacing: Some(tokens.spacing.none),
            children: vec![
                TerminalPanelTab {
                    label: "TERMINAL",
                    active: state.bottom_panel_tab == BottomPanelTab::Terminal,
                    action: ctx.bind(
                        SetBottomPanelTab(BottomPanelTab::Terminal),
                        reduce_with!(on_set_bottom_panel_tab),
                    ),
                    id: "editor_terminal_tab_button",
                }
                .into(),
                TerminalPanelTab {
                    label: "PROBLEMS",
                    active: state.bottom_panel_tab == BottomPanelTab::Problems,
                    action: ctx.bind(
                        SetBottomPanelTab(BottomPanelTab::Problems),
                        reduce_with!(on_set_bottom_panel_tab),
                    ),
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
                        .color(palette.dim_text),
                )
                .padding_all(tokens.spacing.s)
                .into(),
            ],
        })
        .bg(palette.surface_bg)
        .height(PANEL_HEADER_HEIGHT)
        .border(palette.border_color, 1.0)
        .flex_shrink(0.0)
        .into()
    }
}

/// The terminal, or the problems list, filling the space under the header.
struct BottomPanelBody;

impl From<BottomPanelBody> for Widget {
    fn from(_body: BottomPanelBody) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        if view.state().bottom_panel_tab != BottomPanelTab::Terminal {
            return crate::diagnostics_panel::DiagnosticsPanel.into();
        }
        TerminalContent.into()
    }
}

struct TerminalContent;

#[cfg(not(target_arch = "wasm32"))]
impl From<TerminalContent> for Widget {
    fn from(_content: TerminalContent) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        let viewport = view.viewport_size();
        let Some(session) = state.terminal_session.clone() else {
            return TerminalMessage {
                text: "Terminal session unavailable",
            }
            .into();
        };

        let sidebar_width = state.sidebar_width.min(
            (viewport.width - EDITOR_HORIZONTAL_RESERVE)
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
        );
        let sidebar_space = if state.sidebar_visible {
            sidebar_width + DIVIDER_THICKNESS
        } else {
            tokens.spacing.none
        };
        let width = (viewport.width - ACTIVITY_BAR_WIDTH - sidebar_space).max(MIN_TERMINAL_WIDTH);
        let height = (panel_height(state, viewport.height) - PANEL_HEADER_HEIGHT)
            .max(MIN_TERMINAL_CONTENT_HEIGHT);

        TerminalView::new(session, width, height)
            .font_size(tokens.typography.font_size_sm)
            .line_height(tokens.typography.font_size_sm * tokens.typography.line_height_normal)
            .padding(tokens.spacing.s, tokens.spacing.s)
            .into()
    }
}

#[cfg(target_arch = "wasm32")]
impl From<TerminalContent> for Widget {
    fn from(_content: TerminalContent) -> Self {
        TerminalMessage {
            text: "The integrated terminal requires native process access.",
        }
        .into()
    }
}

/// Explains why no terminal is shown.
struct TerminalMessage {
    text: &'static str,
}

impl From<TerminalMessage> for Widget {
    fn from(message: TerminalMessage) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(message.text)
                .size(tokens.typography.font_size_sm)
                .color(palette.dim_text),
        )
        .padding_all(tokens.spacing.m)
        .bg(palette.terminal_bg)
        .flex_grow(1.0)
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
