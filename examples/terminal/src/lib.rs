#[cfg(not(target_arch = "wasm32"))]
mod layout;
mod palette;
mod window_dot;

#[cfg(not(target_arch = "wasm32"))]
use crate::layout::{
    CHROME_VERTICAL_RESERVE, MIN_TERMINAL_HEIGHT, MIN_TERMINAL_WIDTH, TERMINAL_FONT_SIZE,
    TERMINAL_LINE_HEIGHT,
};
use crate::palette::{CHROME_BG, GREEN, MUTED, RED, TEXT, WINDOW_BG, YELLOW};
use crate::window_dot::WindowDot;
use fission::core::ui::{Container, Text, Widget};
use fission::core::{Action, ActionId, GlobalState, ReducerContext, ResourceKey, TimerResource};
#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
use fission::prelude::DesktopApp;
use fission::widgets::{HStack, Spacer, VStack};
#[cfg(not(target_arch = "wasm32"))]
use fission::widgets::{TerminalLaunchConfig, TerminalSession, TerminalView};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct TerminalSession;

#[cfg(target_arch = "wasm32")]
impl TerminalSession {
    fn take_dirty(&self) -> bool {
        false
    }

    fn title(&self) -> String {
        "Browser preview".into()
    }
}

#[derive(Clone, Debug, Default)]
pub struct TerminalExampleState {
    cwd: PathBuf,
    session: Option<Arc<TerminalSession>>,
    redraw_epoch: u64,
}

impl GlobalState for TerminalExampleState {}

pub fn embedded_state() -> TerminalExampleState {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    #[cfg(not(target_arch = "wasm32"))]
    let session = TerminalSession::spawn(TerminalLaunchConfig {
        cwd: Some(cwd.clone()),
        program: std::env::var("SHELL").ok(),
        ..Default::default()
    })
    .ok();
    #[cfg(target_arch = "wasm32")]
    let session = None;
    TerminalExampleState {
        cwd,
        session,
        redraw_epoch: 0,
    }
}

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
    _state: &mut TerminalExampleState,
    _: StartTerminal,
    _: &mut ReducerContext<TerminalExampleState>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if _state.session.is_some() {
            return;
        }

        _state.session = TerminalSession::spawn(TerminalLaunchConfig {
            cwd: Some(_state.cwd.clone()),
            program: std::env::var("SHELL").ok(),
            ..Default::default()
        })
        .ok();
    }
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
pub struct TerminalExampleApp;

impl From<TerminalExampleApp> for Widget {
    fn from(_component: TerminalExampleApp) -> Self {
        let (ctx, view) = fission::build::current::<TerminalExampleState>();
        let tokens = &view.env().theme.tokens;
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
        ctx.with_resources(|resources| {
            resources.timer(
                TimerResource::new(
                    ResourceKey::new("terminal-session-poll"),
                    Duration::from_millis(16),
                    PollTerminalTick,
                )
                .on_tick(poll_terminal_action),
            );
        });

        let title = view
            .state()
            .session
            .as_ref()
            .map(|session| format_terminal_title(&session.title()))
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Shell".into());

        let chrome = Container::new(HStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                WindowDot { color: RED }.into(),
                WindowDot { color: YELLOW }.into(),
                WindowDot { color: GREEN }.into(),
                Spacer {
                    width: Some(tokens.spacing.m),
                    ..Default::default()
                }
                .into(),
                VStack {
                    spacing: Some(tokens.spacing.xs),
                    children: vec![
                        Text::new(title)
                            .size(tokens.typography.font_size_sm)
                            .color(TEXT)
                            .into(),
                        Text::new(view.state().cwd.display().to_string())
                            .size(tokens.typography.font_size_xs)
                            .color(MUTED)
                            .into(),
                    ],
                }
                .into(),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Text::new("Fission Terminal")
                    .size(tokens.typography.font_size_xs)
                    .color(MUTED)
                    .into(),
            ],
        })
        .bg(CHROME_BG)
        .padding_all(tokens.spacing.m)
        .into();

        #[cfg(not(target_arch = "wasm32"))]
        let terminal_height =
            (view.viewport_size().height - CHROME_VERTICAL_RESERVE).max(MIN_TERMINAL_HEIGHT);
        #[cfg(not(target_arch = "wasm32"))]
        let terminal_width = view.viewport_size().width.max(MIN_TERMINAL_WIDTH);
        #[cfg(not(target_arch = "wasm32"))]
        let body = if let Some(session) = view.state().session.clone() {
            TerminalView::new(session, terminal_width, terminal_height)
                .font_size(TERMINAL_FONT_SIZE)
                .line_height(TERMINAL_LINE_HEIGHT)
                .padding(tokens.spacing.m, tokens.spacing.m)
                .into()
        } else {
            Container::new(
                Text::new("Failed to start shell")
                    .size(tokens.typography.body_medium_size)
                    .color(TEXT),
            )
            .padding_all(tokens.spacing.l)
            .bg(WINDOW_BG)
            .into()
        };
        #[cfg(target_arch = "wasm32")]
        let body = Container::new(
            Text::new("The terminal example requires native process access.")
                .size(tokens.typography.body_medium_size)
                .color(TEXT),
        )
        .padding_all(tokens.spacing.l)
        .bg(WINDOW_BG)
        .into();

        Container::new(VStack {
            spacing: Some(tokens.spacing.none),
            children: vec![chrome, body],
        })
        .bg(WINDOW_BG)
        .into()
    }
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

pub fn configure_embedded_env(_state: &TerminalExampleState, env: &mut fission::core::Env) {
    env.theme = fission::theme::Theme::dark();
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    DesktopApp::<TerminalExampleState, _>::new(TerminalExampleApp)
        .with_title("Fission Terminal")
        .with_state_init(move |state: &mut TerminalExampleState| state.cwd = cwd.clone())
        .with_startup_action(StartTerminal)
        .with_sync_env(configure_embedded_env)
        .run()
}
