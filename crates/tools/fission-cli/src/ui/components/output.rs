use crate::ui::commands::CommandStatus;
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
    fn build(&self, _ctx: &mut BuildCtx<UiState>, view: &View<UiState>) -> Node {
        let palette = UiPalette::for_mode(view.state.theme_mode);
        let density = UiDensity::new(view.state.compact_mode);
        let log_height = density.output_log_height(self.height);
        let log_width = (self.width - 4.0).max(10.0);
        let (title, status, output) = match view.state.last_command.as_ref() {
            Some(record) => (
                record.title.clone(),
                record.status,
                log_scrollback(record, view, log_height),
            ),
            None => (
                "Ready".to_string(),
                CommandStatus::Ready,
                ScrollbackView {
                    total_lines: 1,
                    visible_text: "Choose a screen, select an action, and review results here."
                        .to_string(),
                    start_line: 0,
                    visible_lines: 1,
                },
            ),
        };
        let status_color = match status {
            CommandStatus::Ready => palette.muted,
            CommandStatus::Running => palette.warning,
            CommandStatus::Ok => palette.success,
            CommandStatus::Failed => palette.error,
            CommandStatus::Started => palette.warning,
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
                    Scroll {
                        id: Some(log_scroll_node_id()),
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
    record: &crate::ui::commands::CommandRecord,
    view: &View<UiState>,
    log_height: f32,
) -> ScrollbackView {
    let visible_lines = (log_height.floor() as usize).max(1);
    let offset = view
        .runtime
        .scroll
        .get_offset(log_scroll_node_id())
        .max(0.0);
    let total_lines = record.output.display_line_count().max(1);
    let start_line = (offset.floor() as usize).min(total_lines.saturating_sub(1));
    let visible_text = record
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
