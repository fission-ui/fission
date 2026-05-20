use crate::{read_project_config, workflow, Target};
use anyhow::Result;
use fission::ir::op::{AlignItems, Color, JustifyContent};
use fission::prelude::*;
use fission::terminal::TerminalRunOptions;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct TuiOptions {
    pub(crate) project_dir: PathBuf,
    pub(crate) screenshot: Option<PathBuf>,
    pub(crate) exit_after_render: bool,
    pub(crate) width: Option<u16>,
    pub(crate) height: Option<u16>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct CliTuiState {
    project_name: String,
    app_id: String,
    project_dir: PathBuf,
    project_status: String,
    targets: Vec<String>,
    devices: Vec<TuiDevice>,
}

impl AppState for CliTuiState {}

#[derive(Clone, Debug, Default, PartialEq)]
struct TuiDevice {
    id: String,
    target: String,
    kind: String,
    status: String,
    name: String,
}

pub(crate) fn run_tui(options: TuiOptions) -> Result<()> {
    let state = load_state(options.project_dir.clone());
    let run_options = TerminalRunOptions {
        width: options.width,
        height: options.height,
        screenshot: options.screenshot,
        exit_after_render: options.exit_after_render,
        ..TerminalRunOptions::default()
    };
    TerminalApp::with_state(CliTuiApp, state)
        .with_title("Fission CLI")
        .run_with_options(run_options)
}

fn load_state(project_dir: PathBuf) -> CliTuiState {
    let (project_name, app_id, targets, project_status) = match read_project_config(&project_dir) {
        Ok(project) => (
            project.app.name,
            project.app.app_id,
            project
                .targets
                .iter()
                .copied()
                .map(Target::as_str)
                .map(ToOwned::to_owned)
                .collect(),
            "fission.toml loaded".to_string(),
        ),
        Err(error) => (
            project_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("workspace")
                .to_string(),
            "unregistered".to_string(),
            Vec::new(),
            format!("project not registered: {error}"),
        ),
    };
    let devices = workflow::discover_devices(&project_dir)
        .into_iter()
        .map(|device| TuiDevice {
            id: device.id,
            target: device.target.as_str().to_string(),
            kind: device.kind,
            status: device.status,
            name: device.name,
        })
        .collect();
    CliTuiState {
        project_name,
        app_id,
        project_dir,
        project_status,
        targets,
        devices,
    }
}

struct CliTuiApp;

impl Widget<CliTuiState> for CliTuiApp {
    fn build(&self, _ctx: &mut BuildCtx<CliTuiState>, view: &View<CliTuiState>) -> Node {
        let colors = TuiColors::from_env(view.env);
        let viewport = view.env.viewport_size;
        let width = viewport.width.max(80.0);
        let height = viewport.height.max(24.0);
        let body_height = (height - 8.0).max(12.0);
        let left_width = (width * 0.34).clamp(30.0, 44.0);
        let right_width = (width - left_width - 5.0).max(40.0);

        Container::new(
            Column {
                gap: Some(1.0),
                children: vec![
                    header(&view.state, colors, width - 4.0),
                    Row {
                        gap: Some(2.0),
                        align_items: AlignItems::Stretch,
                        children: vec![
                            left_panel(&view.state, colors, left_width, body_height),
                            right_panel(&view.state, colors, right_width, body_height),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                    footer(colors, width - 4.0),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .width(width)
        .height(height)
        .padding([2.0, 2.0, 1.0, 1.0])
        .bg(colors.background)
        .into_node()
    }
}

#[derive(Clone, Copy)]
struct TuiColors {
    background: Color,
    surface: Color,
    raised: Color,
    border: Color,
    text: Color,
    muted: Color,
    accent: Color,
    accent_text: Color,
    success: Color,
    warning: Color,
}

impl TuiColors {
    fn from_env(env: &Env) -> Self {
        let tokens = &env.theme.tokens.colors;
        Self {
            background: tokens.background,
            surface: tokens.surface,
            raised: tokens.surface_raised,
            border: tokens.border,
            text: tokens.text_primary,
            muted: tokens.text_secondary,
            accent: tokens.primary,
            accent_text: tokens.on_primary,
            success: tokens.success,
            warning: tokens.warning,
        }
    }
}

fn header(state: &CliTuiState, colors: TuiColors, width: f32) -> Node {
    Container::new(
        Row {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            children: vec![
                Column {
                    gap: Some(0.0),
                    children: vec![
                        Text::new("Fission CLI")
                            .color(colors.accent_text)
                            .into_node(),
                        Text::new(format!(
                            "{}  •  {}",
                            state.project_name, state.project_status
                        ))
                        .color(colors.accent_text)
                        .into_node(),
                    ],
                    ..Default::default()
                }
                .into_node(),
                Text::new("q / Esc exits")
                    .color(colors.accent_text)
                    .into_node(),
            ],
            ..Default::default()
        }
        .into_node(),
    )
    .width(width)
    .height(4.0)
    .padding([2.0, 2.0, 1.0, 1.0])
    .bg(colors.accent)
    .border(colors.accent, 1.0)
    .into_node()
}

fn left_panel(state: &CliTuiState, colors: TuiColors, width: f32, height: f32) -> Node {
    let mut children = vec![
        section_title("Project", colors),
        key_value(
            "Directory",
            &state.project_dir.display().to_string(),
            colors,
        ),
        key_value("App id", &state.app_id, colors),
        section_title("Targets", colors),
    ];
    if state.targets.is_empty() {
        children.push(
            Text::new("No fission.toml targets registered yet.")
                .color(colors.warning)
                .into_node(),
        );
    } else {
        for target in &state.targets {
            children.push(chip(target, colors));
        }
    }
    children.extend([
        section_title("Workflow", colors),
        command_line("fission doctor --project-dir .", colors),
        command_line("fission devices --project-dir .", colors),
        command_line("fission run --target <target>", colors),
        command_line("fission site check --project-dir documentation", colors),
    ]);
    panel(
        width,
        height,
        colors,
        Column {
            gap: Some(1.0),
            children,
            ..Default::default()
        }
        .into_node(),
    )
}

fn right_panel(state: &CliTuiState, colors: TuiColors, width: f32, height: f32) -> Node {
    let mut children = vec![
        section_title("Detected devices", colors),
        table_header(colors),
    ];
    for device in state
        .devices
        .iter()
        .take((height as usize).saturating_sub(6))
    {
        children.push(device_row(device, colors));
    }
    if state.devices.is_empty() {
        children.push(
            Text::new("No runnable devices detected.")
                .color(colors.warning)
                .into_node(),
        );
    }
    children.extend([
        section_title("Architecture check", colors),
        Text::new(
            "This screen is a normal Fission widget tree rendered by fission-shell-terminal.",
        )
        .color(colors.muted)
        .into_node(),
        Text::new(
            "The shell builds widgets, lowers Core IR, lays out nodes, then paints terminal cells.",
        )
        .color(colors.muted)
        .into_node(),
    ]);
    panel(
        width,
        height,
        colors,
        Column {
            gap: Some(1.0),
            children,
            ..Default::default()
        }
        .into_node(),
    )
}

fn panel(width: f32, height: f32, colors: TuiColors, child: Node) -> Node {
    Container::new(child)
        .width(width)
        .height(height)
        .padding([2.0, 2.0, 1.0, 1.0])
        .bg(colors.surface)
        .border(colors.border, 1.0)
        .into_node()
}

fn footer(colors: TuiColors, width: f32) -> Node {
    Container::new(
        Text::new("Terminal support is derived from Core IR and semantics. Unsupported graphical operations fail before render.")
            .color(colors.muted)
            .into_node(),
    )
    .width(width)
    .height(2.0)
    .padding([1.0, 1.0, 0.0, 0.0])
    .bg(colors.raised)
    .border(colors.border, 1.0)
    .into_node()
}

fn section_title(title: &str, colors: TuiColors) -> Node {
    Text::new(title).color(colors.accent).into_node()
}

fn key_value(key: &str, value: &str, colors: TuiColors) -> Node {
    Row {
        gap: Some(1.0),
        children: vec![
            Text::new(format!("{key}:"))
                .color(colors.muted)
                .width(10.0)
                .into_node(),
            Text::new(value.to_string()).color(colors.text).into_node(),
        ],
        ..Default::default()
    }
    .into_node()
}

fn chip(label: &str, colors: TuiColors) -> Node {
    Container::new(
        Text::new(label.to_string())
            .color(colors.accent_text)
            .into_node(),
    )
    .padding([1.0, 1.0, 0.0, 0.0])
    .bg(colors.accent)
    .border(colors.accent, 1.0)
    .into_node()
}

fn command_line(command: &str, colors: TuiColors) -> Node {
    Text::new(command.to_string())
        .color(colors.muted)
        .into_node()
}

fn table_header(colors: TuiColors) -> Node {
    Row {
        gap: Some(1.0),
        children: vec![
            Text::new("target")
                .color(colors.muted)
                .width(10.0)
                .into_node(),
            Text::new("kind")
                .color(colors.muted)
                .width(14.0)
                .into_node(),
            Text::new("status")
                .color(colors.muted)
                .width(12.0)
                .into_node(),
            Text::new("name").color(colors.muted).into_node(),
        ],
        ..Default::default()
    }
    .into_node()
}

fn device_row(device: &TuiDevice, colors: TuiColors) -> Node {
    let status_color = if device.status == "available" {
        colors.success
    } else {
        colors.warning
    };
    Row {
        gap: Some(1.0),
        children: vec![
            Text::new(device.target.clone())
                .color(colors.text)
                .width(10.0)
                .into_node(),
            Text::new(device.kind.clone())
                .color(colors.text)
                .width(14.0)
                .into_node(),
            Text::new(device.status.clone())
                .color(status_color)
                .width(12.0)
                .into_node(),
            Text::new(format!("{} ({})", device.name, device.id))
                .color(colors.text)
                .into_node(),
        ],
        ..Default::default()
    }
    .into_node()
}
