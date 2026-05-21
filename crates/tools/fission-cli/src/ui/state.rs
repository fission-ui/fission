use super::commands::{
    CommandRuntime, CommandSessionId, CommandSnapshot, UiCommand, DEFAULT_SCROLLBACK_LINES,
};
use super::density::UiDensity;
use super::routes::UiRoute;
use super::theme::UiThemeMode;
use crate::{read_project_config, workflow, Target};
use fission::core::{Env, RuntimeState};
use fission::ir::NodeId;
use fission::prelude::AppState;
use std::path::PathBuf;

const LOG_SCROLL_NODE_ID_PREFIX: &str = "cli_ui_log_scrollback";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiState {
    pub(crate) project_dir: PathBuf,
    pub(crate) project_name: String,
    pub(crate) app_id: String,
    pub(crate) project_status: String,
    pub(crate) targets: Vec<Target>,
    pub(crate) devices: Vec<UiDevice>,
    pub(crate) route: UiRoute,
    pub(crate) theme_mode: UiThemeMode,
    pub(crate) compact_mode: bool,
    pub(crate) selected_target: Option<Target>,
    pub(crate) selected_device: Option<String>,
    pub(crate) init_name: String,
    pub(crate) init_app_id: String,
    pub(crate) init_local_path: String,
    pub(crate) host: String,
    pub(crate) port: String,
    pub(crate) strict: bool,
    pub(crate) release: bool,
    pub(crate) detach: bool,
    pub(crate) no_open: bool,
    pub(crate) headless: bool,
    pub(crate) command_runtime: CommandRuntime,
    pub(crate) command_sessions: Vec<CommandSnapshot>,
    pub(crate) active_command_session_id: Option<CommandSessionId>,
    pub(crate) last_active_log_line_count: usize,
    pub(crate) refreshed_finished_sessions: Vec<CommandSessionId>,
    pub(crate) scrollback_limit: usize,
    pub(crate) scrollback_limit_input: String,
    pub(crate) pending_dialog: Option<UiDialog>,
    pub(crate) exit_confirmed: bool,
}

impl AppState for UiState {}

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
    pub(crate) fn load(project_dir: PathBuf) -> Self {
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

    pub(crate) fn refresh(&mut self) {
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

    pub(crate) fn selected_target_label(&self) -> String {
        self.selected_target
            .map(Target::as_str)
            .unwrap_or("none")
            .to_string()
    }

    pub(crate) fn selected_device_label(&self) -> String {
        self.selected_device
            .as_deref()
            .unwrap_or("auto")
            .to_string()
    }

    pub(crate) fn target_devices(&self) -> Vec<&UiDevice> {
        self.devices
            .iter()
            .filter(|device| {
                self.selected_target
                    .map(|target| target == device.target)
                    .unwrap_or(true)
            })
            .collect()
    }

    pub(crate) fn poll_command_status(&mut self, runtime: &mut RuntimeState, env: &Env) -> bool {
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

    pub(crate) fn sync_command_sessions(&mut self) {
        let snapshot = self.command_runtime.snapshot();
        self.active_command_session_id = snapshot.active_session_id;
        self.last_active_log_line_count = snapshot
            .active_session_id
            .and_then(|id| snapshot.sessions.iter().find(|item| item.id == id))
            .map(|item| item.record.output.display_line_count())
            .unwrap_or(0);
        self.command_sessions = snapshot.sessions;
    }

    pub(crate) fn active_command_session(&self) -> Option<&CommandSnapshot> {
        self.active_command_session_id
            .and_then(|id| self.command_sessions.iter().find(|item| item.id == id))
            .or_else(|| self.command_sessions.last())
    }

    pub(crate) fn select_command_session(&mut self, session_id: CommandSessionId) {
        self.command_runtime.set_active(session_id);
        self.sync_command_sessions();
    }

    pub(crate) fn request_command_confirmation(&mut self, command: UiCommand) {
        let label = command.label();
        let message = command.confirmation_message();
        self.pending_dialog = Some(UiDialog::Command {
            command,
            title: format!("Confirm: {label}"),
            message,
        });
    }

    pub(crate) fn request_exit_confirmation(&mut self) {
        if self.exit_confirmed {
            return;
        }
        self.pending_dialog = Some(UiDialog::Exit {
            title: "Exit Fission CLI?".to_string(),
            message: "Running commands are not stopped automatically. You can cancel and inspect their output before leaving.".to_string(),
        });
    }

    pub(crate) fn set_scrollback_limit(&mut self, limit: usize) {
        let limit = limit.max(1);
        self.scrollback_limit = limit;
        self.scrollback_limit_input = limit.to_string();
        self.command_runtime.set_limit(limit);
        self.sync_command_sessions();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum UiDialog {
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

pub(crate) fn log_scroll_node_id(session_id: CommandSessionId) -> NodeId {
    NodeId::explicit(&format!("{LOG_SCROLL_NODE_ID_PREFIX}_{session_id}"))
}

pub(crate) fn log_visible_rows_for_height(height: f32, compact: bool) -> usize {
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
        .set_offset(log_scroll_node_id(session_id), max_offset);
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
        .get_offset(log_scroll_node_id(next_session_id));
    current + 2.0 >= old_max || current + 2.0 >= new_max
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiDevice {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) target: Target,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) detail: String,
    pub(crate) available: bool,
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

pub(crate) fn target_label(target: Target) -> &'static str {
    match target {
        Target::Android => "Android",
        Target::Ios => "iOS",
        Target::Linux => "Linux",
        Target::Macos => "macOS",
        Target::Site => "Static site",
        Target::Web => "Web",
        Target::Windows => "Windows",
    }
}

pub(crate) fn all_targets() -> [Target; 7] {
    [
        Target::Android,
        Target::Ios,
        Target::Linux,
        Target::Macos,
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
