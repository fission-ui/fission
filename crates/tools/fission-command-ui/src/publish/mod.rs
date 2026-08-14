use anyhow::{Context, Result};
use fission::op::{Color, Fill};
use fission::prelude::*;
use fission_command_core::{read_project_config, DistributionProvider, Target};
use fission_command_package::{
    package_silent, CheckSeverity, CheckStatus, PackageFormat, PackageOptions, PublishShellOptions,
    ReadinessCheck,
};
use fission_command_release::{publish_workflow, PublishWorkflowOptions, ReleasePlanSnapshot};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

mod fission_toml;
mod fs_ops;
mod snapshot;
mod style;
#[cfg(test)]
mod tests;
mod widgets;

use fission_toml::*;
use fs_ops::*;
use snapshot::*;
use style::theme_for_mode;
pub use widgets::PublishApp;

#[derive(Clone, Debug)]
pub struct PublishUiOptions {
    pub project_dir: PathBuf,
    pub provider: DistributionProvider,
    pub target: Option<Target>,
    pub format: Option<PackageFormat>,
    pub artifact: Option<PathBuf>,
    pub site: String,
    pub deploy: Option<String>,
    pub track: Option<String>,
    pub locales: Vec<String>,
    pub screenshot: Option<PathBuf>,
    pub exit_after_render: bool,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub native_file_dialog: bool,
}

pub fn run_publish_tui(options: PublishUiOptions) -> Result<()> {
    let run_options = fission::terminal::TerminalRunOptions {
        width: options.width,
        height: options.height,
        screenshot: options.screenshot.clone(),
        exit_after_render: options.exit_after_render,
        ..fission::terminal::TerminalRunOptions::default()
    };
    let mut options = options;
    options.native_file_dialog = false;
    let state = PublishUiState::load(options);
    fission::terminal::TerminalApp::with_state(PublishApp, state)
        .with_title("Fission publish")
        .with_env(|env| env.theme = fission::theme::Theme::dark())
        .with_sync_env(|state, env| env.theme = theme_for_mode(state.theme_mode))
        .with_key_handler(publish_key_handler)
        .with_state_update(|state, _runtime, _env| state.poll_background_tasks())
        .run_with_options(run_options)
}

pub fn run_publish_window(options: PublishUiOptions) -> Result<()> {
    let mut options = options;
    options.native_file_dialog = true;
    let state = PublishUiState::load(options);
    fission::DesktopApp::<PublishUiState, _>::new_with_global_state(PublishApp, state)
        .with_title("Fission Publish")
        .with_sync_env(|state, env| env.theme = theme_for_mode(state.theme_mode))
        .with_key_handler(publish_key_handler)
        .with_frame_hook(|state| state.poll_background_tasks())
        .run()
}

fn publish_key_handler(
    state: &mut PublishUiState,
    code: &fission::KeyCode,
    _modifiers: u8,
) -> bool {
    state.handle_key(code)
}

#[derive(Clone, Debug, PartialEq)]
pub struct PublishUiState {
    pub project_dir: PathBuf,
    pub app_name: String,
    pub app_id: String,
    pub board: PublishBoard,
    pub current_step: usize,
    pub target: Target,
    pub format: PackageFormat,
    pub provider: DistributionProvider,
    pub site: String,
    pub deploy: Option<String>,
    pub track: String,
    pub locales_input: String,
    pub workspace: PathBuf,
    pub artifact_manifest: PathBuf,
    pub package_checks: Vec<UiCheck>,
    pub distribution_checks: Vec<UiCheck>,
    pub release_checks: Vec<UiCheck>,
    pub release_plan: Option<ReleasePlanSnapshot>,
    pub status_message: String,
    pub theme_mode: ThemeMode,
    pub file_picker: Option<FilePickerState>,
    pub selected_file: Option<FileSelection>,
    pub(crate) config_editor: Option<FissionTomlEditorState>,
    pub play_json_path: String,
    pub android_jks_path: String,
    pub android_alias: String,
    pub android_password: String,
    pub app_store_key_path: String,
    pub app_store_key_id: String,
    pub app_store_issuer_id: String,
    pub windows_pfx_path: String,
    pub windows_password: String,
    pub azure_tenant_id: String,
    pub azure_client_id: String,
    pub microsoft_secret: String,
    pub aws_profile: String,
    pub aws_region: String,
    pub aws_endpoint: String,
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub publish_confirmation: String,
    pub task: Option<PublishTaskState>,
    pub task_revision_seen: u64,
    pub task_log: Vec<String>,
    pub(crate) snapshot_task: Option<SnapshotRefreshState>,
    pub snapshot_task_revision_seen: u64,
    pub native_file_dialog: bool,
}

impl GlobalState for PublishUiState {}

impl Default for PublishUiState {
    fn default() -> Self {
        Self::load(default_publish_options(PathBuf::from(".")))
    }
}

impl PublishUiState {
    pub fn load(options: PublishUiOptions) -> Self {
        let board = PublishBoard::from_provider(options.provider);
        let target = options.target.unwrap_or_else(|| board.target());
        let format = options.format.unwrap_or_else(|| board.format());
        let provider = options.provider;
        let track = options
            .track
            .clone()
            .unwrap_or_else(|| board.default_track().to_string());
        let locales_input = if options.locales.is_empty() {
            "".to_string()
        } else {
            options.locales.join(", ")
        };
        let mut state = Self {
            project_dir: options.project_dir.clone(),
            app_name: "workspace".to_string(),
            app_id: "unknown".to_string(),
            board,
            current_step: 1,
            target,
            format,
            provider,
            site: options.site,
            deploy: options.deploy,
            track,
            locales_input,
            workspace: PathBuf::new(),
            artifact_manifest: options.artifact.unwrap_or_default(),
            package_checks: Vec::new(),
            distribution_checks: Vec::new(),
            release_checks: Vec::new(),
            release_plan: None,
            status_message: "Loading project".to_string(),
            theme_mode: ThemeMode::Dark,
            file_picker: None,
            selected_file: None,
            config_editor: None,
            play_json_path: String::new(),
            android_jks_path: String::new(),
            android_alias: String::new(),
            android_password: String::new(),
            app_store_key_path: String::new(),
            app_store_key_id: String::new(),
            app_store_issuer_id: String::new(),
            windows_pfx_path: String::new(),
            windows_password: String::new(),
            azure_tenant_id: String::new(),
            azure_client_id: String::new(),
            microsoft_secret: String::new(),
            aws_profile: String::new(),
            aws_region: String::new(),
            aws_endpoint: String::new(),
            aws_access_key_id: String::new(),
            aws_secret_access_key: String::new(),
            publish_confirmation: String::new(),
            task: None,
            task_revision_seen: 0,
            task_log: Vec::new(),
            snapshot_task: None,
            snapshot_task_revision_seen: 0,
            native_file_dialog: options.native_file_dialog,
        };
        state.refresh_snapshot();
        state.load_release_env_values();
        state
    }

    fn options(&self) -> PublishShellOptions {
        PublishShellOptions {
            project_dir: self.project_dir.clone(),
            provider: self.provider,
            target: Some(self.target),
            format: Some(self.format),
            artifact: if self.artifact_manifest.as_os_str().is_empty() {
                None
            } else {
                Some(self.artifact_manifest.clone())
            },
            site: self.site.clone(),
            deploy: self.deploy.clone(),
            track: Some(self.track.clone()).filter(|value| !value.trim().is_empty()),
            locales: self
                .locales_input
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
            overwrite_remote: false,
            dry_run: false,
            yes: false,
            json: false,
            app: false,
        }
    }

    fn refresh_snapshot(&mut self) {
        match collect_refresh_snapshot(self.options()) {
            Ok(result) => self.apply_refresh_result(result),
            Err(err) => self.apply_refresh_error(err.to_string()),
        }
    }

    fn apply_refresh_result(&mut self, result: SnapshotRefreshResult) {
        let snapshot = result.snapshot;
        self.app_name = snapshot.app_name;
        self.app_id = snapshot.app_id;
        self.provider = snapshot.provider;
        self.target = snapshot.target;
        self.format = snapshot.format;
        self.site = snapshot.site;
        self.track = snapshot
            .track
            .unwrap_or_else(|| self.board.default_track().to_string());
        if !snapshot.locales.is_empty() && self.locales_input.trim().is_empty() {
            self.locales_input = snapshot.locales.join(", ");
        }
        self.workspace = snapshot.workspace;
        self.artifact_manifest = snapshot.artifact_manifest;
        self.package_checks = snapshot
            .package_checks
            .into_iter()
            .map(UiCheck::from)
            .collect();
        self.distribution_checks = snapshot
            .distribution_checks
            .into_iter()
            .map(UiCheck::from)
            .collect();
        self.release_plan = result.release_plan;
        self.release_checks = result
            .release_checks
            .into_iter()
            .map(UiCheck::from)
            .collect();
        self.status_message = "Preflight refreshed".to_string();
        if self.android_alias.trim().is_empty() {
            self.android_alias = sanitize_workspace_name(&self.app_name).replace('.', "-");
        }
    }

    fn apply_refresh_error(&mut self, err: String) {
        self.status_message = format!("Preflight failed: {err}");
        self.package_checks = vec![UiCheck::failed("Project could not be loaded", err)];
        self.distribution_checks.clear();
        self.release_checks.clear();
        self.release_plan = None;
    }

    fn start_snapshot_refresh(&mut self) {
        if self
            .snapshot_task
            .as_ref()
            .is_some_and(|task| task.status() == TaskStatus::Running)
        {
            self.status_message = "Preflight refresh is already running".to_string();
            return;
        }
        let options = self.options();
        let task = SnapshotRefreshState::new();
        let shared = task.shared.clone();
        thread::spawn(move || {
            let result = collect_refresh_snapshot(options).map_err(|err| err.to_string());
            let mut data = shared.lock().expect("snapshot refresh lock poisoned");
            data.status = if result.is_ok() {
                TaskStatus::Ok
            } else {
                TaskStatus::Failed
            };
            data.message = match &result {
                Ok(_) => "Preflight refreshed".to_string(),
                Err(err) => format!("Preflight failed: {err}"),
            };
            data.result = Some(result);
            data.revision = data.revision.saturating_add(1);
        });
        self.snapshot_task = Some(task);
        self.snapshot_task_revision_seen = 0;
        self.status_message = "Refreshing preflight...".to_string();
    }

    fn load_release_env_values(&mut self) {
        let env_path = self.workspace.join("release.env");
        let Ok(entries) = read_env_entries(&env_path) else {
            return;
        };
        self.play_json_path = entries
            .get("GOOGLE_APPLICATION_CREDENTIALS")
            .cloned()
            .unwrap_or_default();
        self.android_jks_path = entries.get("ANDROID_KEYSTORE").cloned().unwrap_or_default();
        self.android_alias = entries
            .get("ANDROID_KEYSTORE_ALIAS")
            .cloned()
            .unwrap_or_else(|| self.android_alias.clone());
        self.app_store_key_path = entries
            .get("APP_STORE_CONNECT_API_KEY_PATH")
            .cloned()
            .unwrap_or_default();
        self.app_store_key_id = entries
            .get("APP_STORE_CONNECT_KEY_ID")
            .cloned()
            .unwrap_or_default();
        self.app_store_issuer_id = entries
            .get("APP_STORE_CONNECT_ISSUER_ID")
            .cloned()
            .unwrap_or_default();
        self.windows_pfx_path = entries
            .get("WINDOWS_CERTIFICATE")
            .cloned()
            .unwrap_or_default();
        self.azure_tenant_id = entries.get("AZURE_TENANT_ID").cloned().unwrap_or_default();
        self.azure_client_id = entries.get("AZURE_CLIENT_ID").cloned().unwrap_or_default();
        self.aws_profile = entries.get("AWS_PROFILE").cloned().unwrap_or_default();
        self.aws_region = entries.get("AWS_REGION").cloned().unwrap_or_default();
        self.aws_endpoint = entries
            .get("AWS_ENDPOINT_URL_S3")
            .cloned()
            .unwrap_or_default();
        self.aws_access_key_id = entries
            .get("AWS_ACCESS_KEY_ID")
            .cloned()
            .unwrap_or_default();
    }

    fn save_env_value(&mut self, key: &str, value: &str) {
        if value.trim().is_empty() {
            self.status_message = format!("Skipped empty {key}");
            return;
        }
        let env_path = self.workspace.join("release.env");
        match upsert_env(&env_path, key, value.trim()) {
            Ok(()) => self.status_message = format!("Saved {key} to {}", env_path.display()),
            Err(err) => self.status_message = format!("Failed to save {key}: {err}"),
        }
    }

    fn save_current_credentials(&mut self) {
        match self.board {
            PublishBoard::Android => {
                let values = [
                    (
                        "GOOGLE_APPLICATION_CREDENTIALS",
                        self.play_json_path.clone(),
                    ),
                    ("ANDROID_KEYSTORE", self.android_jks_path.clone()),
                    ("ANDROID_KEYSTORE_ALIAS", self.android_alias.clone()),
                    ("ANDROID_KEYSTORE_PASSWORD", self.android_password.clone()),
                    ("ANDROID_KEY_PASSWORD", self.android_password.clone()),
                ];
                self.save_values(&values);
            }
            PublishBoard::Ios => {
                let values = [
                    (
                        "APP_STORE_CONNECT_API_KEY_PATH",
                        self.app_store_key_path.clone(),
                    ),
                    ("APP_STORE_CONNECT_KEY_ID", self.app_store_key_id.clone()),
                    (
                        "APP_STORE_CONNECT_ISSUER_ID",
                        self.app_store_issuer_id.clone(),
                    ),
                ];
                self.save_values(&values);
            }
            PublishBoard::Windows => {
                let values = [
                    ("WINDOWS_CERTIFICATE", self.windows_pfx_path.clone()),
                    (
                        "WINDOWS_CERTIFICATE_PASSWORD",
                        self.windows_password.clone(),
                    ),
                    ("AZURE_TENANT_ID", self.azure_tenant_id.clone()),
                    ("AZURE_CLIENT_ID", self.azure_client_id.clone()),
                    (
                        "MICROSOFT_STORE_CLIENT_SECRET",
                        self.microsoft_secret.clone(),
                    ),
                ];
                self.save_values(&values);
            }
            PublishBoard::S3 => {
                let values = [
                    ("AWS_PROFILE", self.aws_profile.clone()),
                    ("AWS_REGION", self.aws_region.clone()),
                    ("AWS_ENDPOINT_URL_S3", self.aws_endpoint.clone()),
                    ("AWS_ACCESS_KEY_ID", self.aws_access_key_id.clone()),
                    ("AWS_SECRET_ACCESS_KEY", self.aws_secret_access_key.clone()),
                ];
                self.save_values(&values);
            }
        }
        self.start_snapshot_refresh();
    }

    fn save_values(&mut self, values: &[(&str, String)]) {
        let env_path = self.workspace.join("release.env");
        let saved = values
            .iter()
            .filter_map(|(key, value)| {
                let value = value.trim();
                (!value.is_empty()).then_some((*key, value.to_string()))
            })
            .collect::<Vec<_>>();
        let result = saved
            .iter()
            .try_for_each(|(key, value)| upsert_env(&env_path, key, value));
        match result {
            Ok(()) => {
                for (key, value) in saved {
                    env::set_var(key, value);
                }
                self.status_message = format!("Saved release settings to {}", env_path.display())
            }
            Err(err) => self.status_message = format!("Failed to save release settings: {err}"),
        }
    }

    fn open_file_picker(&mut self, purpose: FilePurpose) {
        if self.native_file_dialog {
            self.open_native_file_dialog(purpose);
            return;
        }
        let current_dir = if self.project_dir.exists() {
            self.project_dir.clone()
        } else {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        self.file_picker = Some(FilePickerState::new(purpose, current_dir));
    }

    fn open_native_file_dialog(&mut self, purpose: FilePurpose) {
        let mut dialog = rfd::FileDialog::new().set_title(purpose.title());
        if self.project_dir.exists() {
            dialog = dialog.set_directory(&self.project_dir);
        }
        if let Some((name, extensions)) = purpose.file_filter() {
            dialog = dialog.add_filter(name, extensions);
        }
        match dialog.pick_file() {
            Some(path) => {
                self.selected_file = Some(FileSelection { purpose, path });
                self.file_picker = None;
                self.status_message = format!("Selected {}", purpose.title());
            }
            None => {
                self.status_message = format!("No file selected for {}", purpose.title());
            }
        }
    }

    fn choose_file_entry(&mut self, index: usize) {
        let Some(picker) = &mut self.file_picker else {
            return;
        };
        picker.refresh();
        picker.selected_index = index.min(picker.entries.len());
        if index == 0 {
            if let Some(parent) = picker.current_dir.parent() {
                picker.current_dir = parent.to_path_buf();
                picker.refresh();
                picker.selected_index = 0;
            }
            return;
        }
        let Some(entry) = picker.entries.get(index.saturating_sub(1)).cloned() else {
            return;
        };
        if entry.is_dir {
            picker.current_dir = entry.path;
            picker.refresh();
            picker.selected_index = 0;
        } else {
            self.selected_file = Some(FileSelection {
                purpose: picker.purpose,
                path: entry.path,
            });
        }
    }

    fn handle_key(&mut self, code: &fission::KeyCode) -> bool {
        if self.config_editor.is_some() && matches!(code, fission::KeyCode::Escape) {
            self.config_editor = None;
            return true;
        }
        if self.file_picker.is_some() {
            return self.handle_file_picker_key(code);
        }
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

    fn handle_file_picker_key(&mut self, code: &fission::KeyCode) -> bool {
        let Some(picker) = &mut self.file_picker else {
            return false;
        };
        match code {
            fission::KeyCode::Up => {
                picker.selected_index = picker.selected_index.saturating_sub(1);
                true
            }
            fission::KeyCode::Down => {
                picker.selected_index = (picker.selected_index + 1).min(picker.entries.len());
                true
            }
            fission::KeyCode::Enter | fission::KeyCode::Right => {
                let index = picker.selected_index;
                self.choose_file_entry(index);
                true
            }
            fission::KeyCode::Escape | fission::KeyCode::Left => {
                self.file_picker = None;
                true
            }
            _ => false,
        }
    }

    fn apply_selected_file(&mut self, action: FileAction) {
        let Some(selection) = self.selected_file.take() else {
            return;
        };
        let Some(dest_name) = selection.purpose.default_name(&selection.path) else {
            return;
        };
        let folder = match selection.purpose {
            FilePurpose::AppStoreKey => self.workspace.join("ios"),
            FilePurpose::WindowsCertificate => self.workspace.join("windows"),
            _ => self.workspace.clone(),
        };
        let result = match action {
            FileAction::Reference => {
                if path_is_inside_project(&selection.path, &self.project_dir) {
                    Err(anyhow::anyhow!(
                        "refusing to reference a secret file inside the project tree; copy or move it to {} instead",
                        folder.display()
                    ))
                } else {
                    Ok(selection.path.clone())
                }
            }
            FileAction::Copy => {
                copy_or_move_selected_file(&selection.path, &folder, &dest_name, false)
            }
            FileAction::Move => {
                copy_or_move_selected_file(&selection.path, &folder, &dest_name, true)
            }
        };
        match result {
            Ok(path) => {
                let value = path.display().to_string();
                match selection.purpose {
                    FilePurpose::PlayServiceJson => self.play_json_path = value.clone(),
                    FilePurpose::AndroidKeystore => self.android_jks_path = value.clone(),
                    FilePurpose::AppStoreKey => self.app_store_key_path = value.clone(),
                    FilePurpose::WindowsCertificate => self.windows_pfx_path = value.clone(),
                }
                self.save_env_value(selection.purpose.env_key(), &value);
                self.file_picker = None;
            }
            Err(err) => self.status_message = format!("File selection failed: {err}"),
        }
    }

    fn generate_android_key(&mut self) {
        if self.android_password.trim().is_empty() {
            self.status_message = "Enter an Android keystore password first".to_string();
            return;
        }
        let alias = if self.android_alias.trim().is_empty() {
            sanitize_workspace_name(&self.app_name).replace('.', "-")
        } else {
            self.android_alias.clone()
        };
        let dest = self.workspace.join("upload-key.jks");
        let task_dest = dest.clone();
        let password = self.android_password.clone();
        let task_alias = alias.clone();
        self.start_task(PublishTaskKind::GenerateAndroidKey, move || {
            let status = Command::new("keytool")
                .arg("-genkeypair")
                .arg("-v")
                .arg("-keystore")
                .arg(&task_dest)
                .arg("-storepass")
                .arg(&password)
                .arg("-keypass")
                .arg(&password)
                .arg("-alias")
                .arg(&task_alias)
                .arg("-keyalg")
                .arg("RSA")
                .arg("-keysize")
                .arg("2048")
                .arg("-validity")
                .arg("9125")
                .arg("-dname")
                .arg("CN=Fission Upload, OU=Fission Local Publish, O=Fission, L=Local, ST=Local, C=US")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context("failed to run keytool")?
                .wait_with_output()
                .context("failed to wait for keytool")?;
            let mut out = String::new();
            out.push_str(&String::from_utf8_lossy(&status.stdout));
            out.push_str(&String::from_utf8_lossy(&status.stderr));
            if !status.status.success() {
                anyhow::bail!("keytool failed\n{out}");
            }
            Ok(out)
        });
        self.android_jks_path = dest.display().to_string();
        let values = [
            ("ANDROID_KEYSTORE", self.android_jks_path.clone()),
            ("ANDROID_KEYSTORE_ALIAS", alias),
            ("ANDROID_KEYSTORE_PASSWORD", self.android_password.clone()),
            ("ANDROID_KEY_PASSWORD", self.android_password.clone()),
        ];
        self.save_values(&values);
    }

    fn start_cli_task(&mut self, kind: PublishTaskKind) {
        let task = self.task_request(kind);
        self.start_task(kind, move || task.run())
    }

    fn skip_requirement(&mut self, id: String) {
        match fission_command_release::skip_release_requirement(&self.project_dir, &id, true) {
            Ok(()) => {
                self.status_message = format!("Skipped recommended release check {id}");
                self.start_snapshot_refresh();
            }
            Err(err) => self.status_message = format!("Failed to skip {id}: {err}"),
        }
    }

    fn task_request(&self, kind: PublishTaskKind) -> PublishTaskRequest {
        PublishTaskRequest {
            kind,
            project_dir: self.project_dir.clone(),
            provider: self.provider,
            target: self.target,
            format: self.format,
            artifact: if self.artifact_manifest.as_os_str().is_empty() {
                None
            } else {
                Some(self.artifact_manifest.clone())
            },
            site: self.site.clone(),
            deploy: self.deploy.clone(),
            track: Some(self.track.clone()).filter(|value| !value.trim().is_empty()),
            locales: self
                .locales_input
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
        }
    }

    fn start_task<F>(&mut self, kind: PublishTaskKind, run: F)
    where
        F: FnOnce() -> Result<String> + Send + 'static,
    {
        if self
            .task
            .as_ref()
            .is_some_and(|task| task.status() == TaskStatus::Running)
        {
            self.status_message = "A publish task is already running".to_string();
            return;
        }
        let task = PublishTaskState::new(kind);
        let shared = task.shared.clone();
        thread::spawn(move || {
            let result = run();
            let mut data = shared.lock().expect("publish task lock poisoned");
            match result {
                Ok(output) => {
                    data.status = TaskStatus::Ok;
                    data.output = redact_output_lines(&output);
                    if data.output.is_empty() {
                        data.output.push("done".to_string());
                    }
                }
                Err(err) => {
                    data.status = TaskStatus::Failed;
                    data.output = redact_output_lines(&err.to_string());
                }
            }
            data.revision = data.revision.saturating_add(1);
        });
        self.task = Some(task);
        self.status_message = format!("Started {}", kind.label());
    }

    fn poll_task(&mut self) -> bool {
        let Some(task) = &self.task else {
            return false;
        };
        let revision = task.revision();
        if self.task_revision_seen == revision {
            return false;
        }
        self.task_revision_seen = revision;
        self.task_log = task.output();
        if task.status() != TaskStatus::Running {
            self.status_message = format!("{}: {}", task.kind.label(), task.status().label());
            self.start_snapshot_refresh();
        }
        true
    }

    fn poll_snapshot_task(&mut self) -> bool {
        let Some(task) = &self.snapshot_task else {
            return false;
        };
        let revision = task.revision();
        if self.snapshot_task_revision_seen == revision {
            return false;
        }
        self.snapshot_task_revision_seen = revision;
        if task.status() == TaskStatus::Running {
            self.status_message = task.message();
            return true;
        }
        let result = task.result();
        self.snapshot_task = None;
        self.snapshot_task_revision_seen = 0;
        match result {
            Some(Ok(result)) => {
                self.apply_refresh_result(result);
                if let Some(editor) = &mut self.config_editor {
                    editor.status_message = "Saved and readiness refreshed.".to_string();
                }
            }
            Some(Err(err)) => {
                self.apply_refresh_error(err.clone());
                if let Some(editor) = &mut self.config_editor {
                    editor.status_message = format!("Saved, but readiness refresh failed: {err}");
                }
            }
            None => self.status_message = "Preflight refresh finished without a result".to_string(),
        }
        true
    }

    fn poll_background_tasks(&mut self) -> bool {
        let task_changed = self.poll_task();
        let snapshot_changed = self.poll_snapshot_task();
        task_changed || snapshot_changed
    }

    fn is_ready_to_publish(&self) -> bool {
        Self::check_group_ready(&self.package_checks)
            && Self::check_group_ready(&self.distribution_checks)
            && Self::check_group_ready(&self.release_checks)
            && self.publish_confirmation.trim() == self.app_id
    }

    fn check_group_ready(checks: &[UiCheck]) -> bool {
        !checks.is_empty() && checks.iter().all(UiCheck::is_non_blocking)
    }

    fn next_step(&mut self) {
        self.current_step = (self.current_step + 1).min(self.board.step_count());
    }

    fn previous_step(&mut self) {
        self.current_step = self.current_step.saturating_sub(1).max(1);
    }

    fn go_to_step(&mut self, step: usize) {
        self.current_step = step.clamp(1, self.board.step_count());
    }

    fn open_config_editor(&mut self, field: Option<String>) {
        self.config_editor = Some(FissionTomlEditorState::load(&self.project_dir, field));
    }

    fn select_config_field(&mut self, field: String) {
        if self.config_editor.is_none() {
            self.open_config_editor(Some(field.clone()));
        }
        let value = read_fission_toml_field(&self.project_dir, &field).unwrap_or_default();
        if let Some(editor) = &mut self.config_editor {
            editor.selected_preset = field_specs()
                .iter()
                .position(|spec| spec.path == field)
                .unwrap_or(editor.selected_preset);
            editor.field_path = field;
            editor.value = value;
            editor.status_message = "Loaded current field value from fission.toml.".to_string();
        }
    }

    fn apply_config_editor_field(&mut self) {
        let Some((field_path, value)) = self
            .config_editor
            .as_ref()
            .map(|editor| (editor.field_path.clone(), editor.value.clone()))
        else {
            return;
        };
        match apply_fission_toml_field(&self.project_dir, &field_path, &value) {
            Ok(message) => {
                if let Some(editor) = &mut self.config_editor {
                    editor.status_message = format!("{message}. Refreshing readiness...");
                }
                self.start_snapshot_refresh();
            }
            Err(err) => {
                if let Some(editor) = &mut self.config_editor {
                    editor.status_message = format!("Failed to update fission.toml: {err}");
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PublishBoard {
    Android,
    Ios,
    Windows,
    S3,
}

impl PublishBoard {
    fn from_provider(provider: DistributionProvider) -> Self {
        match provider {
            DistributionProvider::AppStore => Self::Ios,
            DistributionProvider::MicrosoftStore => Self::Windows,
            DistributionProvider::S3 => Self::S3,
            _ => Self::Android,
        }
    }

    fn target(self) -> Target {
        match self {
            Self::Android => Target::Android,
            Self::Ios => Target::Ios,
            Self::Windows => Target::Windows,
            Self::S3 => Target::Site,
        }
    }

    fn format(self) -> PackageFormat {
        match self {
            Self::Android => PackageFormat::Aab,
            Self::Ios => PackageFormat::Ipa,
            Self::Windows => PackageFormat::Msix,
            Self::S3 => PackageFormat::Static,
        }
    }

    fn default_track(self) -> &'static str {
        match self {
            Self::Android => "internal",
            Self::Ios => "testflight",
            Self::Windows => "private",
            Self::S3 => "",
        }
    }

    fn step_count(self) -> usize {
        7
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiCheck {
    pub id: String,
    pub severity: CheckSeverity,
    pub status: CheckStatus,
    pub summary: String,
    pub details: Option<String>,
    pub remediation: Vec<String>,
}

impl UiCheck {
    fn failed(summary: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            id: "publish.ui.failed".to_string(),
            severity: CheckSeverity::Error,
            status: CheckStatus::Failed,
            summary: summary.into(),
            details: Some(details.into()),
            remediation: Vec::new(),
        }
    }

    fn is_non_blocking(&self) -> bool {
        self.severity != CheckSeverity::Error || self.status == CheckStatus::Passed
    }

    fn needs_attention(&self) -> bool {
        matches!(
            self.status,
            CheckStatus::Missing | CheckStatus::Failed | CheckStatus::Warning
        )
    }

    fn action_hints(&self, board: PublishBoard, current_step: usize) -> Vec<PublishCheckAction> {
        check_action_hints(self, board, current_step)
    }
}

impl From<ReadinessCheck> for UiCheck {
    fn from(value: ReadinessCheck) -> Self {
        Self {
            id: value.id,
            severity: value.severity,
            status: value.status,
            summary: value.summary,
            details: value.details,
            remediation: value.remediation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishCheckAction {
    pub label: String,
    pub kind: PublishCheckActionKind,
    pub primary: bool,
}

impl PublishCheckAction {
    fn primary(label: impl Into<String>, kind: PublishCheckActionKind) -> Self {
        Self {
            label: label.into(),
            kind,
            primary: true,
        }
    }

    fn secondary(label: impl Into<String>, kind: PublishCheckActionKind) -> Self {
        Self {
            label: label.into(),
            kind,
            primary: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublishCheckActionKind {
    GoToStep(usize),
    OpenFilePicker(FilePurpose),
    OpenConfigEditor(String),
    SaveCredentials,
    GenerateAndroidKey,
    StartTask(PublishTaskKind),
    SkipRequirement(String),
    Refresh,
}

fn check_action_hints(
    check: &UiCheck,
    board: PublishBoard,
    current_step: usize,
) -> Vec<PublishCheckAction> {
    if !check.needs_attention() {
        return Vec::new();
    }
    let haystack = check_search_text(check);
    let mut actions = Vec::new();

    if mentions_any(
        &haystack,
        &[
            "google_application_credentials",
            "service account",
            "play service",
            "play store credential",
        ],
    ) {
        actions.push(PublishCheckAction::primary(
            "Select service JSON",
            PublishCheckActionKind::OpenFilePicker(FilePurpose::PlayServiceJson),
        ));
        actions.push(PublishCheckAction::secondary(
            "Credential step",
            PublishCheckActionKind::GoToStep(3),
        ));
    }

    if mentions_any(
        &haystack,
        &["android_keystore", "keystore", "upload key", ".jks", "jks"],
    ) {
        actions.push(PublishCheckAction::primary(
            "Select JKS",
            PublishCheckActionKind::OpenFilePicker(FilePurpose::AndroidKeystore),
        ));
        actions.push(PublishCheckAction::secondary(
            "Generate key",
            PublishCheckActionKind::GenerateAndroidKey,
        ));
        actions.push(PublishCheckAction::secondary(
            "Signing step",
            PublishCheckActionKind::GoToStep(4),
        ));
    }

    if mentions_any(
        &haystack,
        &[
            "app_store_connect_api_key_path",
            "app store connect",
            ".p8",
            "issuer id",
            "key id",
        ],
    ) {
        if mentions_any(&haystack, &[".p8", "api_key_path", "key path"]) {
            actions.push(PublishCheckAction::primary(
                "Select .p8 key",
                PublishCheckActionKind::OpenFilePicker(FilePurpose::AppStoreKey),
            ));
        }
        actions.push(PublishCheckAction::secondary(
            "Credential step",
            PublishCheckActionKind::GoToStep(4),
        ));
        actions.push(PublishCheckAction::secondary(
            "Save settings",
            PublishCheckActionKind::SaveCredentials,
        ));
    }

    if mentions_any(
        &haystack,
        &[
            "windows_certificate",
            "certificate",
            ".pfx",
            ".p12",
            "signtool",
        ],
    ) {
        if mentions_any(&haystack, &["certificate", ".pfx", ".p12"]) {
            actions.push(PublishCheckAction::primary(
                "Select certificate",
                PublishCheckActionKind::OpenFilePicker(FilePurpose::WindowsCertificate),
            ));
        }
        actions.push(PublishCheckAction::secondary(
            "Signing step",
            PublishCheckActionKind::GoToStep(3),
        ));
    }

    if mentions_any(
        &haystack,
        &[
            "azure_tenant_id",
            "azure_client_id",
            "microsoft_store_client_secret",
            "client secret",
            "tenant id",
            "seller id",
        ],
    ) {
        actions.push(PublishCheckAction::primary(
            "Store credential step",
            PublishCheckActionKind::GoToStep(4),
        ));
        actions.push(PublishCheckAction::secondary(
            "Save settings",
            PublishCheckActionKind::SaveCredentials,
        ));
    }

    if mentions_any(
        &haystack,
        &[
            "aws_profile",
            "aws_region",
            "aws_access_key_id",
            "aws_secret_access_key",
            "s3",
            "bucket",
        ],
    ) {
        actions.push(PublishCheckAction::primary(
            "S3 settings step",
            PublishCheckActionKind::GoToStep(4),
        ));
        actions.push(PublishCheckAction::secondary(
            "Save settings",
            PublishCheckActionKind::SaveCredentials,
        ));
    }

    if check.id.starts_with("release.package.")
        || mentions_any(
            &haystack,
            &[
                "artifact",
                "package",
                "rebuild",
                "build the",
                "manifest",
                "stale",
            ],
        )
    {
        actions.push(PublishCheckAction::primary(
            "Build artifact",
            PublishCheckActionKind::StartTask(PublishTaskKind::Package),
        ));
        actions.push(PublishCheckAction::secondary(
            "Build step",
            PublishCheckActionKind::GoToStep(match board {
                PublishBoard::Android => 6,
                PublishBoard::Ios => 6,
                PublishBoard::Windows => 6,
                PublishBoard::S3 => 5,
            }),
        ));
    }

    if mentions_any(
        &haystack,
        &[
            "version code",
            "version_code",
            "build number",
            "build_number",
            "already been used",
            "release.build",
        ],
    ) {
        actions.push(PublishCheckAction::primary(
            "Bump build",
            PublishCheckActionKind::StartTask(PublishTaskKind::BumpBuild),
        ));
        actions.push(PublishCheckAction::secondary(
            "Rebuild artifact",
            PublishCheckActionKind::StartTask(PublishTaskKind::Package),
        ));
    }

    if let Some(field) = config_field_for_check(check, board) {
        actions.push(PublishCheckAction::primary(
            format!("Configure {field}"),
            PublishCheckActionKind::OpenConfigEditor(field),
        ));
    } else if check.id.starts_with("release_config.") || check.id.starts_with("release_content.") {
        actions.push(PublishCheckAction::primary(
            "Open config editor",
            PublishCheckActionKind::OpenConfigEditor(String::new()),
        ));
    }

    if actions.is_empty() {
        actions.push(PublishCheckAction::secondary(
            "Refresh after manual fix",
            PublishCheckActionKind::Refresh,
        ));
    }

    if is_skippable_ui_check(check) {
        actions.push(PublishCheckAction::secondary(
            format!("Skip {}", short_check_action_label(&check.id)),
            PublishCheckActionKind::SkipRequirement(check.id.clone()),
        ));
    }

    let actions = actions
        .into_iter()
        .filter(|action| !matches!(action.kind, PublishCheckActionKind::GoToStep(step) if step == current_step))
        .collect();
    dedupe_check_actions(actions)
}

fn is_skippable_ui_check(check: &UiCheck) -> bool {
    check.severity != CheckSeverity::Error
        && matches!(
            check.status,
            CheckStatus::Missing | CheckStatus::Failed | CheckStatus::Warning
        )
}

fn check_search_text(check: &UiCheck) -> String {
    let mut text = String::new();
    text.push_str(&check.id);
    text.push(' ');
    text.push_str(&check.summary);
    if let Some(details) = &check.details {
        text.push(' ');
        text.push_str(details);
    }
    for remediation in &check.remediation {
        text.push(' ');
        text.push_str(remediation);
    }
    text.to_ascii_lowercase()
}

fn mentions_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn short_check_action_label(id: &str) -> String {
    id.rsplit('.')
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(id)
        .replace('_', " ")
}

fn dedupe_check_actions(actions: Vec<PublishCheckAction>) -> Vec<PublishCheckAction> {
    let mut deduped = Vec::new();
    for action in actions {
        if !deduped
            .iter()
            .any(|existing: &PublishCheckAction| existing.kind == action.kind)
        {
            deduped.push(action);
        }
    }
    deduped
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    #[default]
    Dark,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilePickerState {
    pub purpose: FilePurpose,
    pub current_dir: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected_index: usize,
    pub error: Option<String>,
    pub truncated: bool,
}

impl FilePickerState {
    fn new(purpose: FilePurpose, current_dir: PathBuf) -> Self {
        let mut state = Self {
            purpose,
            current_dir,
            entries: Vec::new(),
            selected_index: 0,
            error: None,
            truncated: false,
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        self.error = None;
        self.truncated = false;
        let entries = match fs::read_dir(&self.current_dir) {
            Ok(entries) => entries,
            Err(err) => {
                self.entries.clear();
                self.error = Some(err.to_string());
                return;
            }
        };
        self.entries = entries
            .filter_map(Result::ok)
            .map(|entry| {
                let path = entry.path();
                FileEntry {
                    label: entry.file_name().to_string_lossy().to_string(),
                    is_dir: path.is_dir(),
                    path,
                }
            })
            .collect();
        self.entries
            .sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.label.cmp(&b.label)));
        if self.entries.len() > 200 {
            self.entries.truncate(200);
            self.truncated = true;
        }
        self.selected_index = self.selected_index.min(self.entries.len());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileEntry {
    pub label: String,
    pub is_dir: bool,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSelection {
    pub purpose: FilePurpose,
    pub path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FilePurpose {
    PlayServiceJson,
    AndroidKeystore,
    AppStoreKey,
    WindowsCertificate,
}

impl FilePurpose {
    fn title(self) -> &'static str {
        match self {
            Self::PlayServiceJson => "Select Play service account JSON",
            Self::AndroidKeystore => "Select Android upload key JKS",
            Self::AppStoreKey => "Select App Store Connect .p8 key",
            Self::WindowsCertificate => "Select Windows signing certificate PFX",
        }
    }

    fn env_key(self) -> &'static str {
        match self {
            Self::PlayServiceJson => "GOOGLE_APPLICATION_CREDENTIALS",
            Self::AndroidKeystore => "ANDROID_KEYSTORE",
            Self::AppStoreKey => "APP_STORE_CONNECT_API_KEY_PATH",
            Self::WindowsCertificate => "WINDOWS_CERTIFICATE",
        }
    }

    fn default_name(self, selected: &Path) -> Option<String> {
        match self {
            Self::PlayServiceJson => Some("play-service-account.json".to_string()),
            Self::AndroidKeystore => Some("upload-key.jks".to_string()),
            Self::AppStoreKey | Self::WindowsCertificate => selected
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::to_string),
        }
    }

    fn file_filter(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Self::PlayServiceJson => Some(("JSON", &["json"])),
            Self::AndroidKeystore => Some(("Java keystore", &["jks", "keystore"])),
            Self::AppStoreKey => Some(("App Store Connect key", &["p8"])),
            Self::WindowsCertificate => Some(("Windows certificate", &["pfx", "p12"])),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileAction {
    Copy,
    Move,
    Reference,
}

#[derive(Clone, Debug, PartialEq)]
struct PublishTaskRequest {
    kind: PublishTaskKind,
    project_dir: PathBuf,
    provider: DistributionProvider,
    target: Target,
    format: PackageFormat,
    artifact: Option<PathBuf>,
    site: String,
    deploy: Option<String>,
    track: Option<String>,
    locales: Vec<String>,
}

impl PublishTaskRequest {
    fn run(self) -> Result<String> {
        match self.kind {
            PublishTaskKind::Package => {
                let path = package_silent(PackageOptions {
                    project_dir: self.project_dir,
                    target: self.target,
                    format: self.format,
                    release: true,
                    variant: None,
                    json: false,
                })?;
                Ok(format!("artifact manifest: {}", path.display()))
            }
            PublishTaskKind::DryRun | PublishTaskKind::Publish => {
                let dry_run = self.kind == PublishTaskKind::DryRun;
                publish_workflow(PublishWorkflowOptions {
                    project_dir: self.project_dir,
                    provider: self.provider,
                    target: Some(self.target),
                    format: Some(self.format),
                    artifact: self.artifact,
                    site: self.site,
                    deploy: self.deploy,
                    track: self.track,
                    locales: self.locales,
                    overwrite_remote: false,
                    dry_run,
                    yes: !dry_run,
                    json: false,
                })?;
                Ok("release workflow completed".to_string())
            }
            PublishTaskKind::BumpBuild => {
                fission_command_release::bump_release_build(
                    &self.project_dir,
                    Some(self.target),
                    1,
                    true,
                )?;
                Ok("release build bumped".to_string())
            }
            PublishTaskKind::GenerateAndroidKey => Ok("Android upload key generated".to_string()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PublishTaskKind {
    Package,
    DryRun,
    Publish,
    BumpBuild,
    GenerateAndroidKey,
}

impl PublishTaskKind {
    fn label(self) -> &'static str {
        match self {
            Self::Package => "package build",
            Self::DryRun => "dry-run publish",
            Self::Publish => "publish",
            Self::BumpBuild => "build number bump",
            Self::GenerateAndroidKey => "Android upload key generation",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PublishTaskState {
    kind: PublishTaskKind,
    shared: Arc<Mutex<PublishTaskData>>,
}

impl PublishTaskState {
    fn new(kind: PublishTaskKind) -> Self {
        Self {
            kind,
            shared: Arc::new(Mutex::new(PublishTaskData {
                status: TaskStatus::Running,
                revision: 1,
                output: vec![format!("Running {}...", kind.label())],
            })),
        }
    }

    fn status(&self) -> TaskStatus {
        self.shared
            .lock()
            .expect("publish task lock poisoned")
            .status
    }

    fn revision(&self) -> u64 {
        self.shared
            .lock()
            .expect("publish task lock poisoned")
            .revision
    }

    fn output(&self) -> Vec<String> {
        self.shared
            .lock()
            .expect("publish task lock poisoned")
            .output
            .clone()
    }
}

impl PartialEq for PublishTaskState {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.revision() == other.revision()
    }
}

#[derive(Clone, Debug)]
struct PublishTaskData {
    status: TaskStatus,
    revision: u64,
    output: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TaskStatus {
    Running,
    Ok,
    Failed,
}

impl TaskStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Ok => "passed",
            Self::Failed => "failed",
        }
    }
}

#[fission_reducer(PublishNextStep)]
fn publish_next_step(state: &mut PublishUiState) {
    state.next_step();
}

#[fission_reducer(PublishPreviousStep)]
fn publish_previous_step(state: &mut PublishUiState) {
    state.previous_step();
}

#[fission_reducer(PublishGoToStep)]
fn publish_go_to_step(state: &mut PublishUiState, step: usize) {
    state.go_to_step(step);
}

#[fission_reducer(PublishRefresh)]
fn publish_refresh(state: &mut PublishUiState) {
    state.start_snapshot_refresh();
}

#[fission_reducer(PublishToggleTheme)]
fn publish_toggle_theme(state: &mut PublishUiState) {
    state.theme_mode = match state.theme_mode {
        ThemeMode::Light => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::Light,
    };
}

fn text_input_value(ctx: &ReducerContext<PublishUiState>) -> Option<String> {
    ctx.input
        .text_change()
        .map(|change| change.new_text.clone())
}

#[fission_reducer(PublishSetTrack)]
fn publish_set_track(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    let Some(value) = text_input_value(ctx) else {
        return;
    };
    state.track = value;
    state.status_message =
        "Track updated; refresh preflight to re-check provider readiness.".into();
}

#[fission_reducer(PublishSetLocales)]
fn publish_set_locales(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    let Some(value) = text_input_value(ctx) else {
        return;
    };
    state.locales_input = value;
    state.status_message =
        "Locales updated; refresh preflight to re-check release readiness.".into();
}

#[fission_reducer(PublishSetPlayJson)]
fn publish_set_play_json(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.play_json_path = value;
    }
}

#[fission_reducer(PublishSetAndroidJks)]
fn publish_set_android_jks(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.android_jks_path = value;
    }
}

#[fission_reducer(PublishSetAndroidAlias)]
fn publish_set_android_alias(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.android_alias = value;
    }
}

#[fission_reducer(PublishSetAndroidPassword)]
fn publish_set_android_password(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    if let Some(value) = text_input_value(ctx) {
        state.android_password = value;
    }
}

#[fission_reducer(PublishSetAppStoreKeyPath)]
fn publish_set_app_store_key_path(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    if let Some(value) = text_input_value(ctx) {
        state.app_store_key_path = value;
    }
}

#[fission_reducer(PublishSetAppStoreKeyId)]
fn publish_set_app_store_key_id(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    if let Some(value) = text_input_value(ctx) {
        state.app_store_key_id = value;
    }
}

#[fission_reducer(PublishSetAppStoreIssuerId)]
fn publish_set_app_store_issuer_id(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    if let Some(value) = text_input_value(ctx) {
        state.app_store_issuer_id = value;
    }
}

#[fission_reducer(PublishSetWindowsPfx)]
fn publish_set_windows_pfx(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.windows_pfx_path = value;
    }
}

#[fission_reducer(PublishSetWindowsPassword)]
fn publish_set_windows_password(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    if let Some(value) = text_input_value(ctx) {
        state.windows_password = value;
    }
}

#[fission_reducer(PublishSetAzureTenant)]
fn publish_set_azure_tenant(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.azure_tenant_id = value;
    }
}

#[fission_reducer(PublishSetAzureClient)]
fn publish_set_azure_client(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.azure_client_id = value;
    }
}

#[fission_reducer(PublishSetMicrosoftSecret)]
fn publish_set_microsoft_secret(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    if let Some(value) = text_input_value(ctx) {
        state.microsoft_secret = value;
    }
}

#[fission_reducer(PublishSetAwsProfile)]
fn publish_set_aws_profile(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.aws_profile = value;
    }
}

#[fission_reducer(PublishSetAwsRegion)]
fn publish_set_aws_region(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.aws_region = value;
    }
}

#[fission_reducer(PublishSetAwsEndpoint)]
fn publish_set_aws_endpoint(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.aws_endpoint = value;
    }
}

#[fission_reducer(PublishSetAwsAccessKey)]
fn publish_set_aws_access_key(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    if let Some(value) = text_input_value(ctx) {
        state.aws_access_key_id = value;
    }
}

#[fission_reducer(PublishSetAwsSecretKey)]
fn publish_set_aws_secret_key(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    if let Some(value) = text_input_value(ctx) {
        state.aws_secret_access_key = value;
    }
}

#[fission_reducer(PublishSetConfirmation)]
fn publish_set_confirmation(state: &mut PublishUiState, ctx: &mut ReducerContext<PublishUiState>) {
    if let Some(value) = text_input_value(ctx) {
        state.publish_confirmation = value;
    }
}

#[fission_reducer(PublishSaveCredentials)]
fn publish_save_credentials(state: &mut PublishUiState) {
    state.save_current_credentials();
}

#[fission_reducer(PublishOpenConfigEditor)]
fn publish_open_config_editor(state: &mut PublishUiState, field: String) {
    let field = (!field.trim().is_empty()).then_some(field);
    state.open_config_editor(field);
}

#[fission_reducer(PublishCloseConfigEditor)]
fn publish_close_config_editor(state: &mut PublishUiState) {
    state.config_editor = None;
}

#[fission_reducer(PublishSetConfigFieldPath)]
fn publish_set_config_field_path(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    let Some(value) = text_input_value(ctx) else {
        return;
    };
    if let Some(editor) = &mut state.config_editor {
        editor.field_path = value;
    }
}

#[fission_reducer(PublishSetConfigFieldValue)]
fn publish_set_config_field_value(
    state: &mut PublishUiState,
    ctx: &mut ReducerContext<PublishUiState>,
) {
    let Some(value) = text_input_value(ctx) else {
        return;
    };
    if let Some(editor) = &mut state.config_editor {
        editor.value = value;
    }
}

#[fission_reducer(PublishSelectConfigField)]
fn publish_select_config_field(state: &mut PublishUiState, field: String) {
    state.select_config_field(field);
}

#[fission_reducer(PublishApplyConfigField)]
fn publish_apply_config_field(state: &mut PublishUiState) {
    state.apply_config_editor_field();
}

#[fission_reducer(PublishOpenFilePicker)]
fn publish_open_file_picker(state: &mut PublishUiState, purpose: FilePurpose) {
    state.open_file_picker(purpose);
}

#[fission_reducer(PublishPickFileEntry)]
fn publish_pick_file_entry(state: &mut PublishUiState, index: usize) {
    state.choose_file_entry(index);
}

#[fission_reducer(PublishApplyFile)]
fn publish_apply_file(state: &mut PublishUiState, action: FileAction) {
    state.apply_selected_file(action);
}

#[fission_reducer(PublishCloseFilePicker)]
fn publish_close_file_picker(state: &mut PublishUiState) {
    state.file_picker = None;
    state.selected_file = None;
}

#[fission_reducer(PublishStartTask)]
fn publish_start_task(state: &mut PublishUiState, kind: PublishTaskKind) {
    match kind {
        PublishTaskKind::Publish if !state.is_ready_to_publish() => {
            state.status_message =
                "Publish is locked until checks pass and the app id is typed exactly".to_string();
        }
        PublishTaskKind::GenerateAndroidKey => state.generate_android_key(),
        _ => state.start_cli_task(kind),
    }
}

#[fission_reducer(PublishSkipRequirement)]
fn publish_skip_requirement(state: &mut PublishUiState, id: String) {
    state.skip_requirement(id);
}

pub fn default_publish_options(project_dir: PathBuf) -> PublishUiOptions {
    let provider = read_project_config(&project_dir)
        .ok()
        .and_then(|project| {
            if project.targets.contains(&Target::Android) {
                Some(DistributionProvider::PlayStore)
            } else if project.targets.contains(&Target::Ios) {
                Some(DistributionProvider::AppStore)
            } else if project.targets.contains(&Target::Windows) {
                Some(DistributionProvider::MicrosoftStore)
            } else {
                None
            }
        })
        .unwrap_or(DistributionProvider::PlayStore);
    PublishUiOptions {
        project_dir,
        provider,
        target: None,
        format: None,
        artifact: None,
        site: "production".to_string(),
        deploy: None,
        track: None,
        locales: Vec::new(),
        screenshot: None,
        exit_after_render: false,
        width: None,
        height: None,
        native_file_dialog: false,
    }
}
