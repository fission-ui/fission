use anyhow::Result;
use fission::op::{Color, Fill};
use fission::prelude::*;
use fission_command_core::{read_project_config, DistributionProvider, Target};
use fission_command_package::{CheckSeverity, CheckStatus, ReadinessCheck};
use fission_command_release::{release_config_readiness_checks, release_content_readiness_checks};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ReleaseConfigEditorOptions {
    pub project_dir: PathBuf,
    pub provider: Option<DistributionProvider>,
    pub screenshot: Option<PathBuf>,
    pub exit_after_render: bool,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseConfigEditorState {
    pub project_dir: PathBuf,
    pub app_name: String,
    pub app_id: String,
    pub provider: DistributionProvider,
    pub current_step: usize,
    pub config_checks: Vec<EditorCheck>,
    pub content_checks: Vec<EditorCheck>,
    pub status_message: String,
    pub theme_mode: EditorThemeMode,
}

impl GlobalState for ReleaseConfigEditorState {}

impl ReleaseConfigEditorState {
    pub fn load(options: ReleaseConfigEditorOptions) -> Self {
        let project = read_project_config(&options.project_dir).ok();
        let provider = options
            .provider
            .or_else(|| project.as_ref().and_then(default_provider_for_project))
            .unwrap_or(DistributionProvider::PlayStore);
        let mut state = Self {
            project_dir: options.project_dir,
            app_name: project
                .as_ref()
                .map(|project| project.app.name.clone())
                .unwrap_or_else(|| "workspace".to_string()),
            app_id: project
                .as_ref()
                .map(|project| project.app.app_id.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            provider,
            current_step: 1,
            config_checks: Vec::new(),
            content_checks: Vec::new(),
            status_message: "Loading release configuration".to_string(),
            theme_mode: EditorThemeMode::Dark,
        };
        state.refresh_checks();
        state
    }

    pub fn refresh_checks(&mut self) {
        self.config_checks =
            release_config_readiness_checks(&self.project_dir, Some(self.provider))
                .unwrap_or_else(|err| vec![readiness_error("release_config.editor.config", err)])
                .into_iter()
                .map(EditorCheck::from)
                .collect();
        self.content_checks =
            release_content_readiness_checks(&self.project_dir, Some(self.provider))
                .unwrap_or_else(|err| vec![readiness_error("release_config.editor.content", err)])
                .into_iter()
                .map(EditorCheck::from)
                .collect();
        self.status_message = "Release checks refreshed".to_string();
    }

    pub fn next_step(&mut self) {
        self.current_step = (self.current_step + 1).min(RELEASE_CONFIG_STEPS.len());
    }

    pub fn previous_step(&mut self) {
        self.current_step = self.current_step.saturating_sub(1).max(1);
    }

    pub fn handle_key(&mut self, code: &fission::KeyCode) -> bool {
        match code {
            fission::KeyCode::Left | fission::KeyCode::Up => {
                self.previous_step();
                true
            }
            fission::KeyCode::Right | fission::KeyCode::Down | fission::KeyCode::Enter => {
                self.next_step();
                true
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorCheck {
    pub id: String,
    pub severity: CheckSeverity,
    pub status: CheckStatus,
    pub summary: String,
    pub details: Option<String>,
    pub remediation: Vec<String>,
}

impl From<ReadinessCheck> for EditorCheck {
    fn from(check: ReadinessCheck) -> Self {
        Self {
            id: check.id,
            severity: check.severity,
            status: check.status,
            summary: check.summary,
            details: check.details,
            remediation: check.remediation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorThemeMode {
    Dark,
    Light,
}

impl EditorThemeMode {
    fn toggle(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

#[derive(Clone)]
pub struct ReleaseConfigEditorApp;

impl From<ReleaseConfigEditorApp> for Widget {
    fn from(_app: ReleaseConfigEditorApp) -> Self {
        let (_ctx, view) = fission::build::current::<ReleaseConfigEditorState>();
        let palette = EditorPalette::for_mode(view.state().theme_mode);
        let viewport = view.env().viewport_size;
        let width = viewport.width.max(100.0);
        let height = viewport.height.max(34.0);
        let body_h = (height - 9.0).max(22.0);
        Container::new(Column {
            gap: Some(1.0),
            children: widgets![
                EditorHeader,
                Row {
                    gap: Some(1.0),
                    children: widgets![
                        EditorSidebar {
                            width: 28.0,
                            height: body_h
                        },
                        EditorBody {
                            width: (width - 33.0).max(68.0),
                            height: body_h
                        },
                    ],
                    ..Default::default()
                },
                EditorFooter,
            ],
            ..Default::default()
        })
        .width(width)
        .height(height)
        .padding([1.0, 1.0, 1.0, 1.0])
        .bg(palette.background)
        .into()
    }
}

#[derive(Clone)]
struct EditorHeader;

impl From<EditorHeader> for Widget {
    fn from(_header: EditorHeader) -> Self {
        let (ctx, view) = fission::build::current::<ReleaseConfigEditorState>();
        let palette = EditorPalette::for_mode(view.state().theme_mode);
        let refresh = with_reducer!(ctx, ReleaseConfigRefresh, release_config_refresh);
        let theme = with_reducer!(ctx, ReleaseConfigToggleTheme, release_config_toggle_theme);
        Container::new(Row {
            gap: Some(2.0),
            children: widgets![
                Column {
                    gap: Some(0.0),
                    children: widgets![
                        Text::new("Fission Release Config").color(palette.accent_text),
                        Text::new(format!(
                            "{} - {}",
                            view.state().app_name,
                            view.state().app_id
                        ))
                        .color(palette.accent_text),
                    ],
                    ..Default::default()
                },
                Spacer {
                    width: Some(2.0),
                    ..Default::default()
                },
                EditorButton {
                    label: "Refresh".into(),
                    action: Some(refresh),
                    tone: ButtonTone::Quiet,
                    width: 12.0
                },
                EditorButton {
                    label: "Theme".into(),
                    action: Some(theme),
                    tone: ButtonTone::Quiet,
                    width: 10.0
                },
            ],
            ..Default::default()
        })
        .height(4.0)
        .padding([1.0, 1.0, 1.0, 1.0])
        .bg(palette.accent)
        .into()
    }
}

#[derive(Clone)]
struct EditorSidebar {
    width: f32,
    height: f32,
}

impl From<EditorSidebar> for Widget {
    fn from(sidebar: EditorSidebar) -> Self {
        let (_ctx, view) = fission::build::current::<ReleaseConfigEditorState>();
        let palette = EditorPalette::for_mode(view.state().theme_mode);
        let mut children = widgets![Text::new("Workflow").color(palette.accent)];
        for (index, step) in RELEASE_CONFIG_STEPS.iter().enumerate() {
            let selected = view.state().current_step == index + 1;
            let marker = if selected { ">" } else { " " };
            children.push(
                Text::new(format!("{marker} {}. {}", index + 1, step.title))
                    .color(if selected {
                        palette.accent
                    } else {
                        palette.text
                    })
                    .into(),
            );
        }
        Container::new(Column {
            gap: Some(1.0),
            children,
            ..Default::default()
        })
        .width(sidebar.width)
        .height(sidebar.height)
        .padding([1.0, 1.0, 1.0, 1.0])
        .bg(palette.panel)
        .border(palette.border, 1.0)
        .into()
    }
}

#[derive(Clone)]
struct EditorBody {
    width: f32,
    height: f32,
}

impl From<EditorBody> for Widget {
    fn from(body: EditorBody) -> Self {
        let (_ctx, view) = fission::build::current::<ReleaseConfigEditorState>();
        let palette = EditorPalette::for_mode(view.state().theme_mode);
        let step = RELEASE_CONFIG_STEPS[view.state().current_step.saturating_sub(1)];
        let checks = checks_for_current_step(view.state());
        Container::new(Scroll {
            direction: FlexDirection::Column,
            width: Some((body.width - 2.0).max(1.0)),
            height: Some((body.height - 2.0).max(1.0)),
            show_scrollbar: true,
            child: Some(
                Column {
                    gap: Some(1.0),
                    children: widgets![
                        Text::new(format!(
                            "Step {} of {} - {}",
                            view.state().current_step,
                            RELEASE_CONFIG_STEPS.len(),
                            step.title
                        ))
                        .color(palette.accent),
                        Text::new(step.description).color(palette.text),
                        KeyValue {
                            label: "Provider".into(),
                            value: view.state().provider.as_str().into()
                        },
                        KeyValue {
                            label: "Project".into(),
                            value: view.state().project_dir.display().to_string()
                        },
                        CommandList {
                            commands: step.commands(view.state().provider)
                        },
                        CheckList {
                            title: step.check_title.into(),
                            checks
                        },
                    ],
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        })
        .width(body.width)
        .height(body.height)
        .padding([1.0, 1.0, 1.0, 1.0])
        .bg(palette.surface)
        .border(palette.border, 1.0)
        .into()
    }
}

#[derive(Clone)]
struct EditorFooter;

impl From<EditorFooter> for Widget {
    fn from(_footer: EditorFooter) -> Self {
        let (ctx, view) = fission::build::current::<ReleaseConfigEditorState>();
        let palette = EditorPalette::for_mode(view.state().theme_mode);
        let previous = with_reducer!(ctx, ReleaseConfigPreviousStep, release_config_previous_step);
        let next = with_reducer!(ctx, ReleaseConfigNextStep, release_config_next_step);
        Container::new(Row {
            gap: Some(1.0),
            children: widgets![
                Text::new(view.state().status_message.clone()).color(palette.muted),
                Spacer {
                    width: Some(2.0),
                    ..Default::default()
                },
                EditorButton {
                    label: "Previous".into(),
                    action: Some(previous),
                    tone: ButtonTone::Quiet,
                    width: 12.0
                },
                EditorButton {
                    label: "Next".into(),
                    action: Some(next),
                    tone: ButtonTone::Primary,
                    width: 10.0
                },
                Text::new("Arrows move steps").color(palette.muted),
            ],
            ..Default::default()
        })
        .height(3.0)
        .padding([0.0, 1.0, 0.0, 1.0])
        .bg(palette.panel)
        .into()
    }
}

#[derive(Clone)]
struct KeyValue {
    label: String,
    value: String,
}

impl From<KeyValue> for Widget {
    fn from(row: KeyValue) -> Self {
        let (_ctx, view) = fission::build::current::<ReleaseConfigEditorState>();
        let palette = EditorPalette::for_mode(view.state().theme_mode);
        Row {
            gap: Some(1.0),
            children: widgets![
                Text::new(format!("{}:", row.label))
                    .color(palette.muted)
                    .width(14.0),
                Text::new(row.value).color(palette.text),
            ],
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct CommandList {
    commands: Vec<String>,
}

impl From<CommandList> for Widget {
    fn from(list: CommandList) -> Self {
        let (_ctx, view) = fission::build::current::<ReleaseConfigEditorState>();
        let palette = EditorPalette::for_mode(view.state().theme_mode);
        let mut children = widgets![Text::new("Equivalent CLI").color(palette.accent)];
        for command in list.commands {
            children.push(Text::new(command).color(palette.text).into());
        }
        Column {
            gap: Some(0.0),
            children,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct CheckList {
    title: String,
    checks: Vec<EditorCheck>,
}

impl From<CheckList> for Widget {
    fn from(list: CheckList) -> Self {
        let (_ctx, view) = fission::build::current::<ReleaseConfigEditorState>();
        let palette = EditorPalette::for_mode(view.state().theme_mode);
        let mut children = widgets![Text::new(list.title).color(palette.accent)];
        if list.checks.is_empty() {
            children.push(
                Text::new("No checks available for this step yet.")
                    .color(palette.muted)
                    .into(),
            );
        }
        for check in list.checks.into_iter().take(12) {
            let color = match check.status {
                CheckStatus::Passed => palette.success,
                CheckStatus::Warning | CheckStatus::Skipped => palette.warning,
                CheckStatus::Missing | CheckStatus::Failed => palette.error,
            };
            children.push(
                Text::new(format!(
                    "[{}] {} - {}",
                    status_label(check.status),
                    check.id,
                    check.summary
                ))
                .color(color)
                .into(),
            );
            if let Some(details) = check.details {
                children.push(
                    Text::new(format!("  {details}"))
                        .color(palette.muted)
                        .into(),
                );
            }
        }
        Column {
            gap: Some(0.0),
            children,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct EditorButton {
    label: String,
    action: Option<ActionEnvelope>,
    tone: ButtonTone,
    width: f32,
}

impl From<EditorButton> for Widget {
    fn from(button: EditorButton) -> Self {
        let (_ctx, view) = fission::build::current::<ReleaseConfigEditorState>();
        let palette = EditorPalette::for_mode(view.state().theme_mode);
        let bg = match button.tone {
            ButtonTone::Primary => palette.accent,
            ButtonTone::Quiet => palette.panel,
        };
        let fg = match button.tone {
            ButtonTone::Primary => palette.accent_text,
            ButtonTone::Quiet => palette.text,
        };
        Button {
            on_press: button.action,
            width: Some(button.width),
            height: Some(3.0),
            padding: Some([0.0, 1.0, 0.0, 1.0]),
            background_fill: Some(Fill::Solid(bg)),
            text_color: Some(fg),
            child: Some(Text::new(button.label).color(fg).into()),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Copy)]
enum ButtonTone {
    Primary,
    Quiet,
}

#[derive(Clone, Copy)]
struct EditorStep {
    title: &'static str,
    description: &'static str,
    check_title: &'static str,
}

impl EditorStep {
    fn commands(self, provider: DistributionProvider) -> Vec<String> {
        match self.title {
            "Manifest & identity" => vec![
                "fission release-config validate --provider <provider>".to_string(),
                "fission release-config set app.version <version> --dry-run --json".to_string(),
            ],
            "Release files" => vec![
                "fission release-config write-file --release <id> --kind notes --locale en-US --from-file notes.md --dry-run --json".to_string(),
                "fission release-content validate --provider <provider>".to_string(),
            ],
            "Provider baseline" => vec![
                format!("fission release-config import --provider {} --dry-run --json", provider.as_str()),
                format!("fission release-config lock --provider {} --yes", provider.as_str()),
            ],
            "Content assets" => vec![
                format!("fission release-content render --provider {}", provider.as_str()),
                format!("fission release-content push --provider {} --dry-run --json", provider.as_str()),
            ],
            _ => vec![
                format!("fission release-config push --provider {} --dry-run --json", provider.as_str()),
                "fission publish --provider <provider> --dry-run --json".to_string(),
            ],
        }
    }
}

const RELEASE_CONFIG_STEPS: &[EditorStep] = &[
    EditorStep {
        title: "Manifest & identity",
        description: "Check that fission.toml contains reviewable app identity, version/build, provider ids, and no secret material.",
        check_title: "Release config checks",
    },
    EditorStep {
        title: "Release files",
        description: "Review the active release entry and sidecar files for notes, review info, privacy, and localized content.",
        check_title: "Release config checks",
    },
    EditorStep {
        title: "Provider baseline",
        description: "Import or lock provider state before pushing local metadata, so remote changes are not overwritten accidentally.",
        check_title: "Release config checks",
    },
    EditorStep {
        title: "Content assets",
        description: "Validate screenshots, videos, feature graphics, logos, and provider-specific asset directories before upload.",
        check_title: "Release content checks",
    },
    EditorStep {
        title: "Push & publish",
        description: "Dry-run metadata and artifact publish paths, then use the shared publish workflow for the final mutation.",
        check_title: "Release content checks",
    },
];

#[derive(Clone, Copy)]
struct EditorPalette {
    background: Color,
    surface: Color,
    panel: Color,
    border: Color,
    text: Color,
    muted: Color,
    accent: Color,
    accent_text: Color,
    success: Color,
    warning: Color,
    error: Color,
}

impl EditorPalette {
    fn for_mode(mode: EditorThemeMode) -> Self {
        match mode {
            EditorThemeMode::Dark => Self {
                background: rgb(5, 10, 15),
                surface: rgb(13, 22, 31),
                panel: rgb(18, 30, 40),
                border: rgb(53, 69, 82),
                text: rgb(242, 247, 246),
                muted: rgb(162, 177, 187),
                accent: rgb(93, 213, 112),
                accent_text: rgb(5, 22, 12),
                success: rgb(93, 213, 112),
                warning: rgb(244, 193, 70),
                error: rgb(246, 84, 80),
            },
            EditorThemeMode::Light => Self {
                background: rgb(238, 243, 240),
                surface: rgb(250, 252, 249),
                panel: rgb(235, 242, 238),
                border: rgb(178, 194, 184),
                text: rgb(17, 29, 33),
                muted: rgb(80, 95, 101),
                accent: rgb(24, 148, 72),
                accent_text: rgb(245, 255, 248),
                success: rgb(24, 148, 72),
                warning: rgb(184, 117, 22),
                error: rgb(200, 54, 55),
            },
        }
    }
}

pub fn run_release_config_tui(options: ReleaseConfigEditorOptions) -> Result<()> {
    let run_options = fission::terminal::TerminalRunOptions {
        width: options.width,
        height: options.height,
        screenshot: options.screenshot.clone(),
        exit_after_render: options.exit_after_render,
        ..fission::terminal::TerminalRunOptions::default()
    };
    let state = ReleaseConfigEditorState::load(options);
    fission::terminal::TerminalApp::with_state(ReleaseConfigEditorApp, state)
        .with_title("Fission release config")
        .configure_env(|env| env.theme = fission::theme::Theme::dark())
        .with_sync_env(|state, env| {
            env.theme = match state.theme_mode {
                EditorThemeMode::Dark => fission::theme::Theme::dark(),
                EditorThemeMode::Light => fission::theme::Theme::default(),
            };
        })
        .with_key_handler(release_config_key_handler)
        .run_with_options(run_options)
}

fn release_config_key_handler(
    state: &mut ReleaseConfigEditorState,
    code: &fission::KeyCode,
    _modifiers: u8,
) -> bool {
    state.handle_key(code)
}

#[fission_reducer(ReleaseConfigRefresh)]
fn release_config_refresh(state: &mut ReleaseConfigEditorState) {
    state.refresh_checks();
}

#[fission_reducer(ReleaseConfigNextStep)]
fn release_config_next_step(state: &mut ReleaseConfigEditorState) {
    state.next_step();
}

#[fission_reducer(ReleaseConfigPreviousStep)]
fn release_config_previous_step(state: &mut ReleaseConfigEditorState) {
    state.previous_step();
}

#[fission_reducer(ReleaseConfigToggleTheme)]
fn release_config_toggle_theme(state: &mut ReleaseConfigEditorState) {
    state.theme_mode = state.theme_mode.toggle();
}

fn checks_for_current_step(state: &ReleaseConfigEditorState) -> Vec<EditorCheck> {
    match state.current_step {
        4 | 5 => state.content_checks.clone(),
        _ => state.config_checks.clone(),
    }
}

fn default_provider_for_project(
    project: &fission_command_core::FissionProject,
) -> Option<DistributionProvider> {
    if project.targets.contains(&Target::Android) {
        Some(DistributionProvider::PlayStore)
    } else if project.targets.contains(&Target::Ios) {
        Some(DistributionProvider::AppStore)
    } else if project.targets.contains(&Target::Windows) {
        Some(DistributionProvider::MicrosoftStore)
    } else if project.targets.contains(&Target::Site) {
        Some(DistributionProvider::GithubPages)
    } else {
        None
    }
}

fn readiness_error(id: &str, error: anyhow::Error) -> ReadinessCheck {
    ReadinessCheck {
        id: id.to_string(),
        severity: CheckSeverity::Error,
        status: CheckStatus::Failed,
        summary: "release-config editor check failed".to_string(),
        details: Some(error.to_string()),
        remediation: vec!["Fix the reported error and refresh the editor.".to_string()],
    }
}

fn status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Passed => "ok",
        CheckStatus::Missing => "missing",
        CheckStatus::Failed => "failed",
        CheckStatus::Warning => "warning",
        CheckStatus::Skipped => "skipped",
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(project_dir: PathBuf) -> ReleaseConfigEditorOptions {
        ReleaseConfigEditorOptions {
            project_dir,
            provider: Some(DistributionProvider::PlayStore),
            screenshot: None,
            exit_after_render: false,
            width: None,
            height: None,
        }
    }

    #[test]
    fn release_config_editor_renders_release_config_flow() {
        let state = ReleaseConfigEditorState::load(options(PathBuf::from(".")));
        let mut app = fission::terminal::TerminalApp::with_state(ReleaseConfigEditorApp, state);
        let frame = app
            .render_frame(160, 60)
            .expect("release config editor renders");
        let text = frame.as_plain_text();
        assert!(text.contains("Fission Release Config"));
        assert!(text.contains("Step 1 of 5"));
        assert!(text.contains("Manifest & identity"));
        assert!(text.contains("Provider"));
        assert!(text.contains("play-store"));
    }

    #[test]
    fn release_config_editor_arrow_keys_move_between_steps() {
        let mut state = ReleaseConfigEditorState::load(options(PathBuf::from(".")));
        assert_eq!(state.current_step, 1);
        assert!(state.handle_key(&fission::KeyCode::Right));
        assert_eq!(state.current_step, 2);
        assert!(state.handle_key(&fission::KeyCode::Down));
        assert_eq!(state.current_step, 3);
        assert!(state.handle_key(&fission::KeyCode::Left));
        assert_eq!(state.current_step, 2);
        assert!(state.handle_key(&fission::KeyCode::Up));
        assert_eq!(state.current_step, 1);
    }
}
