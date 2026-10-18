#[cfg(not(target_arch = "wasm32"))]
use crate::layout::{
    ACTIVITY_BAR_WIDTH, DIVIDER_THICKNESS, EDITOR_HORIZONTAL_RESERVE, MIN_TERMINAL_CONTENT_HEIGHT,
    MIN_TERMINAL_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH,
};
use crate::layout::{MIN_TERMINAL_HEIGHT, PANEL_HEADER_HEIGHT, TERMINAL_HEIGHT_FRACTION};
use crate::model::{on_set_bottom_panel_tab, BottomPanelTab, EditorState, SetBottomPanelTab};
use crate::palette::EditorPalette;
use crate::terminal_panel_tab::TerminalPanelTab;
use fission::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use fission::widgets::TerminalView;
use std::path::Path;

fission_component! {
/// The bottom panel: terminal and problems tabs above the chosen tab's content.
    pub TerminalPanel<EditorState> |_panel, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let is_terminal = state.bottom_panel_tab == BottomPanelTab::Terminal;
        let content_id = if is_terminal {
            "editor_terminal_tab_content"
        } else {
            "editor_problems_tab_content"
        };
    } =>
        Container(
            Column(
                flex_grow: 1.0,
                children: [
                    BottomPanelHeader(),
                    Container(BottomPanelBody())
                        .flex_grow(1.0)
                        .id(WidgetId::explicit(content_id)),
                ],
            ),
        )
        .height(panel_height(state, cx.view.viewport_size().height))
        .bg(palette.terminal_bg)
        .flex_shrink(0.0)
}

/// The panel's height: the user's chosen height, kept within a fraction of the window.
fn panel_height(state: &EditorState, viewport_height: f32) -> f32 {
    state
        .terminal_height
        .min((viewport_height * TERMINAL_HEIGHT_FRACTION).max(MIN_TERMINAL_HEIGHT))
}

fission_component! {
/// The tab buttons and the running terminal's title.
    BottomPanelHeader<EditorState> |_header, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let title = state
            .terminal_session
            .as_ref()
            .map(|session| format_terminal_title(&session.title()))
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Terminal".into());
    } =>
        Container(
            HStack(
                spacing: tokens.spacing.none,
                children: [
                    TerminalPanelTab(
                        "TERMINAL",
                        state.bottom_panel_tab == BottomPanelTab::Terminal,
                        cx.bind(
                            SetBottomPanelTab(BottomPanelTab::Terminal),
                            reduce_with!(on_set_bottom_panel_tab),
                        ),
                        "editor_terminal_tab_button",
                    ),
                    TerminalPanelTab(
                        "PROBLEMS",
                        state.bottom_panel_tab == BottomPanelTab::Problems,
                        cx.bind(
                            SetBottomPanelTab(BottomPanelTab::Problems),
                            reduce_with!(on_set_bottom_panel_tab),
                        ),
                        "editor_problems_tab_button",
                    ),
                    Spacer(flex_grow: 1.0),
                    Container(
                        Text(title)
                            .size(tokens.typography.font_size_xs)
                            .color(palette.dim_text),
                    ).padding_all(tokens.spacing.s),
                ],
            ),
        )
        .bg(palette.surface_bg)
        .height(PANEL_HEADER_HEIGHT)
        .border(palette.border_color, 1.0)
        .flex_shrink(0.0)
}

fission_component! {
/// The terminal, or the problems list, filling the space under the header.
    BottomPanelBody<EditorState> |_body, state, _cx| {} =>
        if state.bottom_panel_tab == BottomPanelTab::Terminal {
            TerminalContent()
        } else {
            crate::diagnostics_panel::DiagnosticsPanel::new()
        }
}

#[cfg(not(target_arch = "wasm32"))]
fission_component! {
    TerminalContent<EditorState> |_content, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let viewport = cx.view.viewport_size();
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
    } =>
        if let Some(session) = state.terminal_session.clone() {
            TerminalView::new(session, width, height)
                .font_size(tokens.typography.font_size_sm)
                .line_height(
                    tokens.typography.font_size_sm * tokens.typography.line_height_normal,
                )
                .padding(tokens.spacing.s, tokens.spacing.s)
        } else {
            TerminalMessage("Terminal session unavailable")
        }
}

#[cfg(target_arch = "wasm32")]
fission_component! {
    TerminalContent<EditorState> |_content, _state, _cx| {} =>
        TerminalMessage("The integrated terminal requires native process access.")
}

fission_component! {
/// Explains why no terminal is shown.
    TerminalMessage<EditorState> { text: &'static str } |message, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Text(message.text)
                .size(tokens.typography.font_size_sm)
                .color(palette.dim_text),
        )
        .padding_all(tokens.spacing.m)
        .bg(palette.terminal_bg)
        .flex_grow(1.0)
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
