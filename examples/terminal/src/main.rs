use fission::core::op::Color;
use fission::core::ui::{Container, Node, Text};
use fission::core::{
    Action, ActionId, AppState, BuildCtx, ReducerContext, ResourceKey, TimerResource, View, Widget,
};
use fission::prelude::DesktopApp;
use fission::widgets::{
    HStack, Spacer, TerminalLaunchConfig, TerminalSession, TerminalView, VStack,
};
use fission::IntoWidget;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const WINDOW_BG: Color = Color {
    r: 24,
    g: 24,
    b: 24,
    a: 255,
};
const CHROME_BG: Color = Color {
    r: 40,
    g: 40,
    b: 40,
    a: 255,
};
const TEXT: Color = Color {
    r: 214,
    g: 214,
    b: 214,
    a: 255,
};
const MUTED: Color = Color {
    r: 143,
    g: 143,
    b: 143,
    a: 255,
};
const RED: Color = Color {
    r: 255,
    g: 95,
    b: 86,
    a: 255,
};
const YELLOW: Color = Color {
    r: 255,
    g: 189,
    b: 46,
    a: 255,
};
const GREEN: Color = Color {
    r: 39,
    g: 201,
    b: 63,
    a: 255,
};

#[derive(Clone, Debug, Default)]
struct TerminalExampleState {
    cwd: PathBuf,
    session: Option<Arc<TerminalSession>>,
    redraw_epoch: u64,
}

impl AppState for TerminalExampleState {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct StartTerminal;

impl Action for StartTerminal {
    fn static_id() -> ActionId {
        ActionId::from_name("examples::terminal::StartTerminal")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct PollTerminal;

impl Action for PollTerminal {
    fn static_id() -> ActionId {
        ActionId::from_name("examples::terminal::PollTerminal")
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct PollTerminalTick;

fn start_terminal(
    state: &mut TerminalExampleState,
    _: StartTerminal,
    _: &mut ReducerContext<TerminalExampleState>,
) {
    if state.session.is_some() {
        return;
    }

    state.session = TerminalSession::spawn(TerminalLaunchConfig {
        cwd: Some(state.cwd.clone()),
        program: std::env::var("SHELL").ok(),
        ..Default::default()
    })
    .ok();
}

fn poll_terminal(
    state: &mut TerminalExampleState,
    _: PollTerminal,
    ctx: &mut ReducerContext<TerminalExampleState>,
) {
    let _tick: PollTerminalTick = ctx.input.timer_tick().unwrap_or_default();

    if state
        .session
        .as_ref()
        .map(|session| session.take_dirty())
        .unwrap_or(false)
    {
        state.redraw_epoch = state.redraw_epoch.wrapping_add(1);
    }
}

#[derive(Clone)]

struct TerminalExampleApp;

impl Widget<TerminalExampleState> for TerminalExampleApp {
    fn build(
        &self,
        ctx: &mut BuildCtx<TerminalExampleState>,
        view: &View<TerminalExampleState>,
    ) -> impl fission::IntoWidget<TerminalExampleState> {
        fission::core::view::internal_node_widget({
            {
                ctx.register(
                    start_terminal
                        as fn(
                            &mut TerminalExampleState,
                            StartTerminal,
                            &mut ReducerContext<TerminalExampleState>,
                        ),
                );
                let poll_terminal_action = ctx.bind(
                    PollTerminal,
                    poll_terminal
                        as fn(
                            &mut TerminalExampleState,
                            PollTerminal,
                            &mut ReducerContext<TerminalExampleState>,
                        ),
                );
                ctx.resources.timer(
                    TimerResource::new(
                        ResourceKey::new("terminal-session-poll"),
                        Duration::from_millis(16),
                        PollTerminalTick,
                    )
                    .on_tick(poll_terminal_action),
                );

                let title = view
                    .state
                    .session
                    .as_ref()
                    .map(|session| format_terminal_title(&session.title()))
                    .filter(|title| !title.trim().is_empty())
                    .unwrap_or_else(|| "Shell".into());

                let chrome = Container::<fission::Node>::lowered(
                    HStack {
                        spacing: Some(8.0),
                        children: vec![
                            dot(RED),
                            dot(YELLOW),
                            dot(GREEN),
                            Spacer {
                                width: Some(12.0),
                                ..Default::default()
                            }
                            .into_node(),
                            VStack {
                                spacing: Some(2.0),
                                children: vec![
                                    Text::new(title).size(12.0).color(TEXT).into_node(),
                                    Text::new(view.state.cwd.display().to_string())
                                        .size(10.0)
                                        .color(MUTED)
                                        .into_node(),
                                ],
                            }
                            .into_node(),
                            Spacer {
                                flex_grow: 1.0,
                                ..Default::default()
                            }
                            .into_node(),
                            Text::new("Fission Terminal")
                                .size(11.0)
                                .color(MUTED)
                                .into_node(),
                        ],
                    }
                    .into_node(),
                )
                .bg(CHROME_BG)
                .padding_all(10.0)
                .into_node();

                let terminal_height = (view.viewport_size().height - 52.0).max(180.0);
                let terminal_width = view.viewport_size().width.max(320.0);
                let body = if let Some(session) = view.state.session.clone() {
                    TerminalView::new(session, terminal_width, terminal_height)
                        .font_size(13.0)
                        .line_height(18.0)
                        .padding(10.0, 10.0)
                        .build(ctx, view)
                        .into_widget()
                        .lower_to_node(ctx, view)
                } else {
                    Container::<fission::Node>::lowered(
                        Text::new("Failed to start shell")
                            .size(13.0)
                            .color(TEXT)
                            .into_node(),
                    )
                    .padding_all(16.0)
                    .bg(WINDOW_BG)
                    .into_node()
                };

                Container::<fission::Node>::lowered(
                    VStack {
                        spacing: Some(0.0),
                        children: vec![chrome, body],
                    }
                    .into_node(),
                )
                .bg(WINDOW_BG)
                .into_node()
            }
        })
    }
}

fn dot(color: Color) -> Node {
    Container::<Node>::lowered(
        Spacer {
            width: Some(12.0),
            height: Some(12.0),
            ..Default::default()
        }
        .into_node(),
    )
    .width(12.0)
    .height(12.0)
    .bg(color)
    .border_radius(999.0)
    .into_node()
}

fn format_terminal_title(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return "Shell".into();
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

fn main() -> anyhow::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    DesktopApp::new(TerminalExampleApp)
        .with_title("Fission Terminal")
        .with_state_init(move |state: &mut TerminalExampleState| state.cwd = cwd.clone())
        .with_startup_action(StartTerminal)
        .with_sync_env(
            |_state: &TerminalExampleState, env: &mut fission::core::Env| {
                env.theme = fission::theme::Theme::dark();
            },
        )
        .run()
}
