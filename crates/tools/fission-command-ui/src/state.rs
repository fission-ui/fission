use super::commands::{
    CommandRuntime, CommandSessionId, CommandSnapshot, UiCommand, DEFAULT_SCROLLBACK_LINES,
};
use super::density::UiDensity;
use super::routes::UiRoute;
use super::theme::UiThemeMode;
use fission::core::{Env, RuntimeState};
use fission::prelude::GlobalState;
use fission::WidgetId;
use fission_command_core::{read_project_config, Target};
use fission_command_run as workflow;
use std::path::PathBuf;

const LOG_SCROLL_NODE_ID_PREFIX: &str = "cli_ui_log_scrollback";

#[derive(Clone, Debug, PartialEq)]
pub struct UiState {
    pub project_dir: PathBuf,
    pub project_name: String,
    pub app_id: String,
    pub project_status: String,
    pub targets: Vec<Target>,
    pub devices: Vec<UiDevice>,
    pub route: UiRoute,
    pub theme_mode: UiThemeMode,
    pub compact_mode: bool,
    pub selected_target: Option<Target>,
    pub selected_device: Option<String>,
    pub init_name: String,
    pub init_app_id: String,
    pub init_local_path: String,
    pub host: String,
    pub port: String,
    pub strict: bool,
    pub release: bool,
    pub detach: bool,
    pub no_open: bool,
    pub headless: bool,
    pub command_runtime: CommandRuntime,
    pub command_sessions: Vec<CommandSnapshot>,
    pub active_command_session_id: Option<CommandSessionId>,
    pub last_active_log_line_count: usize,
    pub refreshed_finished_sessions: Vec<CommandSessionId>,
    pub scrollback_limit: usize,
    pub scrollback_limit_input: String,
    pub pending_dialog: Option<UiDialog>,
    pub exit_confirmed: bool,
}

impl GlobalState for UiState {}

impl Default for UiState {
    fn default() -> Self {
        Self {
            project_dir: PathBuf::new(),
            project_name: String::new(),
            app_id: String::new(),
            project_status: String::new(),
            targets: Vec::new(),
            devices: Vec::new(),
            route: UiRoute::default(),
            theme_mode: UiThemeMode::default(),
            compact_mode: true,
            selected_target: None,
            selected_device: None,
            init_name: String::new(),
            init_app_id: String::new(),
            init_local_path: String::new(),
            host: String::new(),
            port: String::new(),
            strict: false,
            release: false,
            detach: false,
            no_open: false,
            headless: false,
            command_runtime: CommandRuntime::default(),
            command_sessions: Vec::new(),
            active_command_session_id: None,
            last_active_log_line_count: 0,
            refreshed_finished_sessions: Vec::new(),
            scrollback_limit: DEFAULT_SCROLLBACK_LINES,
            scrollback_limit_input: DEFAULT_SCROLLBACK_LINES.to_string(),
            pending_dialog: None,
            exit_confirmed: false,
        }
    }
}

impl UiState {
    pub fn load(project_dir: PathBuf) -> Self {
        let mut state = Self {
            project_dir,
            route: UiRoute::Dashboard,
            theme_mode: UiThemeMode::Dark,
            host: "127.0.0.1".to_string(),
            port: "8123".to_string(),
            scrollback_limit: DEFAULT_SCROLLBACK_LINES,
            scrollback_limit_input: DEFAULT_SCROLLBACK_LINES.to_string(),
            detach: true,
            ..Default::default()
        };
        state.refresh();
        state
    }

    pub fn refresh(&mut self) {
        match read_project_config(&self.project_dir) {
            Ok(project) => {
                self.project_name = project.app.name;
                self.app_id = project.app.app_id;
                self.targets = project.targets.iter().copied().collect();
                self.project_status = "Project loaded".to_string();
                if self.selected_target.is_none()
                    || self
                        .selected_target
                        .is_some_and(|target| !self.targets.contains(&target))
                {
                    self.selected_target = preferred_target(&self.targets);
                }
            }
            Err(error) => {
                self.project_name = self
                    .project_dir
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("workspace")
                    .to_string();
                self.app_id = "Not initialised".to_string();
                self.targets.clear();
                self.selected_target = None;
                self.project_status = format!("Project not initialised: {error}");
            }
        }

        self.devices = workflow::discover_devices(&self.project_dir)
            .into_iter()
            .map(UiDevice::from)
            .collect();
        if self.selected_device.is_none()
            || self
                .selected_device
                .as_ref()
                .is_some_and(|selected| !self.devices.iter().any(|device| &device.id == selected))
        {
            self.selected_device = self
                .devices
                .iter()
                .find(|device| {
                    self.selected_target
                        .map(|target| target == device.target)
                        .unwrap_or(true)
                        && device.available
                })
                .map(|device| device.id.clone());
        }
    }

    pub fn selected_target_label(&self) -> String {
        self.selected_target
            .map(Target::as_str)
            .unwrap_or("none")
            .to_string()
    }

    pub fn selected_device_label(&self) -> String {
        self.selected_device
            .as_deref()
            .unwrap_or("auto")
            .to_string()
    }

    pub fn target_devices(&self) -> Vec<&UiDevice> {
        self.devices
            .iter()
            .filter(|device| {
                self.selected_target
                    .map(|target| target == device.target)
                    .unwrap_or(true)
            })
            .collect()
    }

    pub fn poll_command_status(&mut self, runtime: &mut RuntimeState, env: &Env) -> bool {
        let snapshot = self.command_runtime.snapshot();
        let mut changed = false;

        let active_session = snapshot
            .active_session_id
            .and_then(|id| snapshot.sessions.iter().find(|item| item.id == id));
        let active_line_count = active_session
            .map(|item| item.record.output.display_line_count())
            .unwrap_or(0);
        if self.active_command_session_id != snapshot.active_session_id
            || self.command_sessions != snapshot.sessions
        {
            let should_follow = should_follow_log_output(
                self,
                runtime,
                env,
                snapshot.active_session_id,
                active_line_count,
            );
            self.command_sessions = snapshot.sessions.clone();
            self.active_command_session_id = snapshot.active_session_id;
            self.last_active_log_line_count = active_line_count;
            if should_follow {
                stick_log_scroll_to_bottom(
                    runtime,
                    env,
                    snapshot.active_session_id,
                    active_line_count,
                    self.compact_mode,
                );
            }
            changed = true;
        }

        for session in snapshot.sessions.iter().filter(|session| session.finished) {
            if !self.refreshed_finished_sessions.contains(&session.id) {
                self.refreshed_finished_sessions.push(session.id);
                self.refresh();
                changed = true;
            }
        }
        changed
    }

    pub fn sync_command_sessions(&mut self) {
        let snapshot = self.command_runtime.snapshot();
        self.active_command_session_id = snapshot.active_session_id;
        self.last_active_log_line_count = snapshot
            .active_session_id
            .and_then(|id| snapshot.sessions.iter().find(|item| item.id == id))
            .map(|item| item.record.output.display_line_count())
            .unwrap_or(0);
        self.command_sessions = snapshot.sessions;
    }

    pub fn active_command_session(&self) -> Option<&CommandSnapshot> {
        self.active_command_session_id
            .and_then(|id| self.command_sessions.iter().find(|item| item.id == id))
            .or_else(|| self.command_sessions.last())
    }

    pub fn select_command_session(&mut self, session_id: CommandSessionId) {
        self.command_runtime.set_active(session_id);
        self.sync_command_sessions();
    }

    pub fn request_command_confirmation(&mut self, command: UiCommand) {
        let label = command.label();
        let message = command.confirmation_message();
        self.pending_dialog = Some(UiDialog::Command {
            command,
            title: format!("Confirm: {label}"),
            message,
        });
    }

    pub fn request_exit_confirmation(&mut self) {
        if self.exit_confirmed {
            return;
        }
        self.pending_dialog = Some(UiDialog::Exit {
            title: "Exit Fission command?".to_string(),
            message: "Running commands are not stopped automatically. You can cancel and inspect their output before leaving.".to_string(),
        });
    }

    pub fn set_scrollback_limit(&mut self, limit: usize) {
        let limit = limit.max(1);
        self.scrollback_limit = limit;
        self.scrollback_limit_input = limit.to_string();
        self.command_runtime.set_limit(limit);
        self.sync_command_sessions();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiDialog {
    Command {
        command: UiCommand,
        title: String,
        message: String,
    },
    Exit {
        title: String,
        message: String,
    },
}

pub fn log_scroll_widget_id(session_id: CommandSessionId) -> WidgetId {
    WidgetId::explicit(&format!("{LOG_SCROLL_NODE_ID_PREFIX}_{session_id}"))
}

pub fn log_visible_rows_for_height(height: f32, compact: bool) -> usize {
    let density = UiDensity::new(compact);
    let metrics = density.shell_metrics(height);
    density.output_log_height(metrics.footer_h).floor().max(1.0) as usize
}

fn stick_log_scroll_to_bottom(
    runtime: &mut RuntimeState,
    env: &Env,
    session_id: Option<CommandSessionId>,
    line_count: usize,
    compact: bool,
) {
    let Some(session_id) = session_id else {
        return;
    };
    let visible_rows = log_visible_rows_for_height(env.viewport_size.height, compact);
    let max_offset = line_count.saturating_sub(visible_rows).max(0) as f32;
    runtime
        .scroll
        .set_offset(log_scroll_widget_id(session_id).into(), max_offset);
}

fn should_follow_log_output(
    state: &UiState,
    runtime: &RuntimeState,
    env: &Env,
    next_session_id: Option<CommandSessionId>,
    next_line_count: usize,
) -> bool {
    let Some(next_session_id) = next_session_id else {
        return false;
    };
    if state.active_command_session_id != Some(next_session_id) {
        return true;
    }
    let visible_rows = log_visible_rows_for_height(env.viewport_size.height, state.compact_mode);
    let old_max = state
        .last_active_log_line_count
        .saturating_sub(visible_rows) as f32;
    let new_max = next_line_count.saturating_sub(visible_rows) as f32;
    let current = runtime
        .scroll
        .get_offset(log_scroll_widget_id(next_session_id).into());
    current + 2.0 >= old_max || current + 2.0 >= new_max
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiDevice {
    pub id: String,
    pub name: String,
    pub target: Target,
    pub kind: String,
    pub status: String,
    pub detail: String,
    pub available: bool,
}

impl From<workflow::Device> for UiDevice {
    fn from(device: workflow::Device) -> Self {
        Self {
            id: device.id,
            name: device.name,
            target: device.target,
            kind: device.kind,
            status: device.status,
            detail: device.detail,
            available: device.available,
        }
    }
}

pub fn target_label(target: Target) -> &'static str {
    match target {
        Target::Android => "Android",
        Target::Ios => "iOS",
        Target::Linux => "Linux",
        Target::Macos => "macOS",
        Target::Server => "Server",
        Target::Site => "Static site",
        Target::Web => "Web",
        Target::Windows => "Windows",
    }
}

pub fn all_targets() -> [Target; 8] {
    [
        Target::Android,
        Target::Ios,
        Target::Linux,
        Target::Macos,
        Target::Server,
        Target::Site,
        Target::Web,
        Target::Windows,
    ]
}

fn preferred_target(targets: &[Target]) -> Option<Target> {
    let host = if cfg!(target_os = "windows") {
        Target::Windows
    } else if cfg!(target_os = "macos") {
        Target::Macos
    } else {
        Target::Linux
    };
    targets
        .iter()
        .copied()
        .find(|target| *target == host)
        .or_else(|| {
            targets
                .iter()
                .copied()
                .find(|target| *target == Target::Web)
        })
        .or_else(|| targets.first().copied())
}
