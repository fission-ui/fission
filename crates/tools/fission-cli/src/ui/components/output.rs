use crate::ui::commands::CommandStatus;
use crate::ui::state::UiState;
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
        let (title, status, output) = match view.state.last_command.as_ref() {
            Some(record) => (
                record.title.clone(),
                record.status,
                last_lines(&record.output, (self.height as usize).saturating_sub(3)),
            ),
            None => (
                "Ready".to_string(),
                CommandStatus::Ready,
                "Choose a screen, select an action, and review results here.".to_string(),
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
                    Text::new(output).color(palette.muted).into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .width(self.width)
        .height(self.height)
        .padding([1.0, 1.0, 0.0, 0.0])
        .bg(palette.raised)
        .border(palette.border, 1.0)
        .into_node()
    }
}

fn last_lines(value: &str, max_lines: usize) -> String {
    let max_lines = max_lines.max(1);
    let lines = value.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}
