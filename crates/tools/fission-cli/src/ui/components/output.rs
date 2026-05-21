use crate::ui::actions::{select_command_session, SelectCommandSession};
use crate::ui::commands::CommandStatus;
use crate::ui::commands::{CommandSessionId, CommandSnapshot};
use crate::ui::density::UiDensity;
use crate::ui::state::{log_scroll_node_id, UiState};
use crate::ui::theme::UiPalette;
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct OutputPanel {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl Widget<UiState> for OutputPanel {
    fn build(&self, ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let density = UiDensity::new(view.state.compact_mode);
        let log_height = density.output_log_height(self.height);
        let log_width = (self.width - 4.0).max(10.0);
        let active = view.state.active_command_session();
        let (title, status, output, scroll_id) = match active {
            Some(session) => (
                session.record.title.clone(),
                session.record.status,
                log_scrollback(session, view, log_height),
                log_scroll_node_id(session.id),
            ),
            None => (
                "Output".to_string(),
                CommandStatus::Ready,
                ScrollbackView {
                    total_lines: 1,
                    visible_text:
                        "Choose a workflow, confirm the command, and review results here."
                            .to_string(),
                    start_line: 0,
                    visible_lines: 1,
                },
                NodeId::explicit("cli_ui_log_scrollback_empty"),
            ),
        };
        let status_color = match status {
            CommandStatus::Ready => palette.muted,
            CommandStatus::Running => palette.warning,
            CommandStatus::Ok => palette.success,
            CommandStatus::Failed => palette.error,
        };
        Container::new(
            Column {
                gap: Some(0.0),
                children: vec![
                    Row {
                        gap: Some(2.0),
                        children: vec![
                            Text::new(title).color(palette.text).into_node(),
                            Text::new(status.label()).color(status_color).into_node(),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                    command_tabs(ctx, view, self.width - 4.0),
                    Scroll {
                        id: Some(scroll_id),
                        direction: FlexDirection::Column,
                        width: Some(log_width),
                        height: Some(log_height),
                        show_scrollbar: true,
                        child: Some(Box::new(scrollback_content(output, palette.muted))),
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .width(self.width)
        .height(self.height)
        .padding(density.sidebar_padding())
        .bg(palette.raised)
        .border(palette.border, 1.0)
        .into_node()
    }
}

struct ScrollbackView {
    total_lines: usize,
    visible_text: String,
    start_line: usize,
    visible_lines: usize,
}

fn log_scrollback(
    session: &CommandSnapshot,
    view: &View<UiState>,
    log_height: f32,
) -> ScrollbackView {
    let visible_lines = (log_height.floor() as usize).max(1);
    let offset = view
        .runtime
        .scroll
        .get_offset(log_scroll_node_id(session.id))
        .max(0.0);
    let total_lines = session.record.output.display_line_count().max(1);
    let start_line = (offset.floor() as usize).min(total_lines.saturating_sub(1));
    let visible_text = session
        .record
        .output
        .visible_lines(start_line, visible_lines)
        .join("\n");
    ScrollbackView {
        total_lines,
        visible_text,
        start_line,
        visible_lines,
    }
}

fn command_tabs(ctx: &mut BuildCtx<UiState>, view: &View<UiState>, width: f32) -> Node {
    if view.state.command_sessions.is_empty() {
        return Spacer {
            height: Some(0.0),
            ..Default::default()
        }
        .into_node();
    }

    let palette = UiPalette::for_mode(view.state.theme_mode);
    let max_tabs = 5usize;
    let mut tabs = Vec::new();
    for session in view
        .state
        .command_sessions
        .iter()
        .rev()
        .take(max_tabs)
        .rev()
    {
        tabs.push(command_tab(session.id, ctx, view));
    }
    if view.state.command_sessions.len() > max_tabs {
        tabs.insert(
            0,
            Text::new(format!("+{}", view.state.command_sessions.len() - max_tabs))
                .color(palette.muted)
                .into_node(),
        );
    }

    Scroll {
        id: Some(NodeId::explicit("cli_ui_command_tabs")),
        direction: FlexDirection::Row,
        width: Some(width),
        height: Some(1.0),
        show_scrollbar: false,
        child: Some(Box::new(
            Row {
                gap: Some(1.0),
                children: tabs,
                ..Default::default()
            }
            .into_node(),
        )),
        ..Default::default()
    }
    .into_node()
}

fn command_tab(
    session_id: CommandSessionId,
    ctx: &mut BuildCtx<UiState>,
    view: &View<UiState>,
) -> Node {
    let Some(session) = view
        .state
        .command_sessions
        .iter()
        .find(|item| item.id == session_id)
    else {
        return Spacer::default().into_node();
    };
    let palette = UiPalette::for_mode(view.state.theme_mode);
    let active = view.state.active_command_session_id == Some(session.id);
    let status_marker = match session.record.status {
        CommandStatus::Ready => "-",
        CommandStatus::Running => "*",
        CommandStatus::Ok => "+",
        CommandStatus::Failed => "!",
    };
    let label = format!("{status_marker} {}", session.record.title);
    let action = with_reducer!(
        ctx,
        SelectCommandSession(session.id),
        select_command_session
    );
    Button {
        on_press: Some(action),
        height: Some(1.0),
        padding: Some([0.0; 4]),
        background_fill: Some(Fill::Solid(if active {
            palette.accent
        } else {
            palette.subtle
        })),
        text_color: Some(if active {
            palette.accent_text
        } else {
            palette.text
        }),
        child: Some(Box::new(Text::new(label).into_node())),
        ..Default::default()
    }
    .into_node()
}

fn scrollback_content(output: ScrollbackView, color: Color) -> Node {
    let bottom_lines = output
        .total_lines
        .saturating_sub(output.start_line.saturating_add(output.visible_lines));
    Column {
        gap: Some(0.0),
        children: vec![
            Spacer {
                height: Some(output.start_line as f32),
                ..Default::default()
            }
            .into_node(),
            Text::new(output.visible_text).color(color).into_node(),
            Spacer {
                height: Some(bottom_lines as f32),
                ..Default::default()
            }
            .into_node(),
        ],
        ..Default::default()
    }
    .into_node()
}
