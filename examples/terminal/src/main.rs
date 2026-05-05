use fission_core::op::Color;
use fission_core::ui::{Container, Node, Text};
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_shell_desktop::DesktopApp;
use fission_widgets::{HStack, Spacer, TerminalLaunchConfig, TerminalSession, TerminalView, VStack};
use std::path::PathBuf;
use std::sync::Arc;

const WINDOW_BG: Color = Color { r: 24, g: 24, b: 24, a: 255 };
const CHROME_BG: Color = Color { r: 40, g: 40, b: 40, a: 255 };
const TEXT: Color = Color { r: 214, g: 214, b: 214, a: 255 };
const MUTED: Color = Color { r: 143, g: 143, b: 143, a: 255 };
const RED: Color = Color { r: 255, g: 95, b: 86, a: 255 };
const YELLOW: Color = Color { r: 255, g: 189, b: 46, a: 255 };
const GREEN: Color = Color { r: 39, g: 201, b: 63, a: 255 };

#[derive(Clone, Debug, Default)]
struct TerminalExampleState {
    cwd: PathBuf,
    session: Option<Arc<TerminalSession>>,
}

impl AppState for TerminalExampleState {}

struct TerminalExampleApp;

impl Widget<TerminalExampleState> for TerminalExampleApp {
    fn build(&self, _ctx: &mut BuildCtx<TerminalExampleState>, view: &View<TerminalExampleState>) -> Node {
        let title = view
            .state
            .session
            .as_ref()
            .map(|session| session.title())
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Shell".into());

        let chrome = Container::new(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    dot(RED),
                    dot(YELLOW),
                    dot(GREEN),
                    Spacer { width: Some(12.0), ..Default::default() }.into_node(),
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
                    Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
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
                .build(_ctx, view)
        } else {
            Container::new(Text::new("Failed to start shell").size(13.0).color(TEXT).into_node())
                .padding_all(16.0)
                .bg(WINDOW_BG)
                .into_node()
        };

        Container::new(
            VStack {
                spacing: Some(0.0),
                children: vec![chrome, body],
            }
            .into_node(),
        )
        .bg(WINDOW_BG)
        .into_node()
    }
}

fn dot(color: Color) -> Node {
    Container::new(Spacer { width: Some(12.0), height: Some(12.0), ..Default::default() }.into_node())
        .width(12.0)
        .height(12.0)
        .bg(color)
        .border_radius(999.0)
        .into_node()
}

fn main() -> anyhow::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    DesktopApp::new(TerminalExampleApp)
        .with_title("Fission Terminal")
        .with_state_init(move |state: &mut TerminalExampleState| {
            state.cwd = cwd.clone();
            state.session = TerminalSession::spawn(TerminalLaunchConfig {
                cwd: Some(cwd.clone()),
                ..Default::default()
            })
            .ok();
        })
        .with_sync_env(|_state: &TerminalExampleState, env: &mut fission_core::Env| {
            env.theme = fission_theme::Theme::dark();
        })
        .with_frame_hook(|state: &mut TerminalExampleState| {
            state.session.as_ref().map(|session| session.take_dirty()).unwrap_or(false)
        })
        .run()
}
