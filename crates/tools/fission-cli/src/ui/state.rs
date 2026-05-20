use super::commands::{CommandRecord, CommandRuntime};
use super::routes::UiRoute;
use super::theme::UiThemeMode;
use crate::{read_project_config, workflow, Target};
use fission::prelude::AppState;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct UiState {
    pub(crate) project_dir: PathBuf,
    pub(crate) project_name: String,
    pub(crate) app_id: String,
    pub(crate) project_status: String,
    pub(crate) targets: Vec<Target>,
    pub(crate) devices: Vec<UiDevice>,
    pub(crate) route: UiRoute,
    pub(crate) theme_mode: UiThemeMode,
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
    pub(crate) last_command: Option<CommandRecord>,
    pub(crate) command_runtime: CommandRuntime,
    pub(crate) last_command_generation: u64,
    pub(crate) last_refreshed_command_generation: u64,
}

impl AppState for UiState {}

impl UiState {
    pub(crate) fn load(project_dir: PathBuf) -> Self {
        let mut state = Self {
            project_dir,
            route: UiRoute::Dashboard,
            theme_mode: UiThemeMode::Dark,
            host: "127.0.0.1".to_string(),
            port: "8123".to_string(),
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

    pub(crate) fn poll_command_status(&mut self) -> bool {
        let Some(snapshot) = self.command_runtime.snapshot() else {
            return false;
        };
        let mut changed = false;
        if self.last_command_generation != snapshot.generation
            || self.last_command.as_ref() != Some(&snapshot.record)
        {
            self.last_command = Some(snapshot.record);
            self.last_command_generation = snapshot.generation;
            changed = true;
        }
        if snapshot.finished && self.last_refreshed_command_generation != snapshot.generation {
            self.last_refreshed_command_generation = snapshot.generation;
            self.refresh();
            changed = true;
        }
        changed
    }
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
