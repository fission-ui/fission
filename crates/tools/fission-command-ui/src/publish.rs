use anyhow::{Context, Result};
use fission::op::{AlignItems, Color, Fill, JustifyContent};
use fission::prelude::*;
use fission_command_core::{read_project_config, DistributionProvider, Target};
use fission_command_package::{
    publish_flow_snapshot, CheckSeverity, CheckStatus, PackageFormat, PublishShellOptions,
    ReadinessCheck,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

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
}

pub fn run_publish_tui(options: PublishUiOptions) -> Result<()> {
    let run_options = fission::terminal::TerminalRunOptions {
        width: options.width,
        height: options.height,
        screenshot: options.screenshot.clone(),
        exit_after_render: options.exit_after_render,
        ..fission::terminal::TerminalRunOptions::default()
    };
    let state = PublishUiState::load(options);
    fission::terminal::TerminalApp::with_state(PublishApp, state)
        .with_title("Fission publish")
        .with_env(|env| env.theme = fission::theme::Theme::dark())
        .with_sync_env(|state, env| env.theme = theme_for_mode(state.theme_mode))
        .with_state_update(|state, _runtime, _env| state.poll_task())
        .run_with_options(run_options)
}

pub fn run_publish_window(options: PublishUiOptions) -> Result<()> {
    let state = PublishUiState::load(options);
    fission::DesktopApp::<PublishUiState, _>::new_with_global_state(PublishApp, state)
        .with_title("Fission Publish")
        .with_sync_env(|state, env| env.theme = theme_for_mode(state.theme_mode))
        .with_frame_hook(|state| state.poll_task())
        .run()
}

fn theme_for_mode(mode: ThemeMode) -> fission::theme::Theme {
    match mode {
        ThemeMode::Dark => fission::theme::Theme::dark(),
        ThemeMode::Light => fission::theme::Theme::default(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PublishUiState {
    pub project_dir: PathBuf,
    pub app_name: String,
    pub app_id: String,
    pub board: PublishBoard,
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
    pub status_message: String,
    pub theme_mode: ThemeMode,
    pub file_picker: Option<FilePickerState>,
    pub selected_file: Option<FileSelection>,
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
            status_message: "Loading project".to_string(),
            theme_mode: ThemeMode::Dark,
            file_picker: None,
            selected_file: None,
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
            dry_run: false,
            yes: false,
            json: false,
            app: false,
        }
    }

    fn refresh_snapshot(&mut self) {
        match publish_flow_snapshot(&self.options()) {
            Ok(snapshot) => {
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
                self.status_message = "Preflight refreshed".to_string();
                if self.android_alias.trim().is_empty() {
                    self.android_alias = sanitize_workspace_name(&self.app_name).replace('.', "-");
                }
            }
            Err(err) => {
                self.status_message = format!("Preflight failed: {err}");
                self.package_checks = vec![UiCheck::failed(
                    "Project could not be loaded",
                    err.to_string(),
                )];
                self.distribution_checks.clear();
            }
        }
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

    fn select_board(&mut self, board: PublishBoard) {
        self.board = board;
        self.provider = board.provider();
        self.target = board.target();
        self.format = board.format();
        self.track = board.default_track().to_string();
        self.refresh_snapshot();
        self.load_release_env_values();
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
        self.refresh_snapshot();
    }

    fn save_values(&mut self, values: &[(&str, String)]) {
        let env_path = self.workspace.join("release.env");
        let result = values
            .iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .try_for_each(|(key, value)| upsert_env(&env_path, key, value.trim()));
        match result {
            Ok(()) => {
                self.status_message = format!("Saved release settings to {}", env_path.display())
            }
            Err(err) => self.status_message = format!("Failed to save release settings: {err}"),
        }
    }

    fn open_file_picker(&mut self, purpose: FilePurpose) {
        let current_dir = if self.project_dir.exists() {
            self.project_dir.clone()
        } else {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        self.file_picker = Some(FilePickerState::new(purpose, current_dir));
    }

    fn choose_file_entry(&mut self, index: usize) {
        let Some(picker) = &mut self.file_picker else {
            return;
        };
        picker.refresh();
        if index == 0 {
            if let Some(parent) = picker.current_dir.parent() {
                picker.current_dir = parent.to_path_buf();
                picker.refresh();
            }
            return;
        }
        let Some(entry) = picker.entries.get(index.saturating_sub(1)).cloned() else {
            return;
        };
        if entry.is_dir {
            picker.current_dir = entry.path;
            picker.refresh();
        } else {
            self.selected_file = Some(FileSelection {
                purpose: picker.purpose,
                path: entry.path,
            });
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
            FileAction::Reference => Ok(selection.path.clone()),
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
        let args = self.task_args(kind);
        self.start_task(kind, move || run_current_fission(args));
    }

    fn task_args(&self, kind: PublishTaskKind) -> Vec<String> {
        match kind {
            PublishTaskKind::Package => vec![
                "package".to_string(),
                "--target".to_string(),
                self.target.as_str().to_string(),
                "--format".to_string(),
                self.format.as_str().to_string(),
                "--release".to_string(),
                "--project-dir".to_string(),
                self.project_dir.display().to_string(),
            ],
            PublishTaskKind::DryRun => self.publish_task_args(true),
            PublishTaskKind::Publish => self.publish_task_args(false),
            PublishTaskKind::GenerateAndroidKey => Vec::new(),
        }
    }

    fn publish_task_args(&self, dry_run: bool) -> Vec<String> {
        let mut args = vec![
            "publish".to_string(),
            "--provider".to_string(),
            self.provider.as_str().to_string(),
            "--artifact".to_string(),
            self.artifact_manifest.display().to_string(),
            "--site".to_string(),
            self.site.clone(),
            "--yes".to_string(),
            "--project-dir".to_string(),
            self.project_dir.display().to_string(),
        ];
        if !self.track.trim().is_empty() {
            args.push("--track".to_string());
            args.push(self.track.clone());
        }
        if let Some(deploy) = self
            .deploy
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            args.push("--deploy".to_string());
            args.push(deploy.clone());
        }
        if dry_run {
            args.push("--dry-run".to_string());
        }
        args
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
                    data.output = output.lines().map(str::to_string).collect();
                    if data.output.is_empty() {
                        data.output.push("done".to_string());
                    }
                }
                Err(err) => {
                    data.status = TaskStatus::Failed;
                    data.output = err.to_string().lines().map(str::to_string).collect();
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
            self.refresh_snapshot();
        }
        true
    }

    fn is_ready_to_publish(&self) -> bool {
        self.package_checks.iter().all(UiCheck::is_non_blocking)
            && self
                .distribution_checks
                .iter()
                .all(UiCheck::is_non_blocking)
            && self.publish_confirmation.trim() == self.app_id
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
    const ALL: [PublishBoard; 4] = [Self::Android, Self::Ios, Self::Windows, Self::S3];

    fn from_provider(provider: DistributionProvider) -> Self {
        match provider {
            DistributionProvider::AppStore => Self::Ios,
            DistributionProvider::MicrosoftStore => Self::Windows,
            DistributionProvider::S3 => Self::S3,
            _ => Self::Android,
        }
    }

    fn provider(self) -> DistributionProvider {
        match self {
            Self::Android => DistributionProvider::PlayStore,
            Self::Ios => DistributionProvider::AppStore,
            Self::Windows => DistributionProvider::MicrosoftStore,
            Self::S3 => DistributionProvider::S3,
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

    fn title(self) -> &'static str {
        match self {
            Self::Android => "Android / Play",
            Self::Ios => "iOS / TestFlight",
            Self::Windows => "Windows / Store",
            Self::S3 => "S3 / Static",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Android => "AAB, upload key, Play service account",
            Self::Ios => "IPA, App Store Connect API, Apple account checks",
            Self::Windows => "MSIX, msstore, package identity and signing",
            Self::S3 => "Static artifact, bucket profile, region and endpoint",
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiCheck {
    pub severity: CheckSeverity,
    pub status: CheckStatus,
    pub summary: String,
    pub details: Option<String>,
    pub remediation: Vec<String>,
}

impl UiCheck {
    fn failed(summary: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
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
}

impl From<ReadinessCheck> for UiCheck {
    fn from(value: ReadinessCheck) -> Self {
        Self {
            severity: value.severity,
            status: value.status,
            summary: value.summary,
            details: value.details,
            remediation: value.remediation,
        }
    }
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
}

impl FilePickerState {
    fn new(purpose: FilePurpose, current_dir: PathBuf) -> Self {
        let mut state = Self {
            purpose,
            current_dir,
            entries: Vec::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        self.entries = fs::read_dir(&self.current_dir)
            .ok()
            .into_iter()
            .flatten()
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
        self.entries.truncate(28);
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
                .and_then(OsStr::to_str)
                .map(str::to_string),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileAction {
    Copy,
    Move,
    Reference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PublishTaskKind {
    Package,
    DryRun,
    Publish,
    GenerateAndroidKey,
}

impl PublishTaskKind {
    fn label(self) -> &'static str {
        match self {
            Self::Package => "package build",
            Self::DryRun => "dry-run publish",
            Self::Publish => "publish",
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
enum TaskStatus {
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

#[fission_reducer(PublishSelectBoard)]
fn publish_select_board(state: &mut PublishUiState, board: PublishBoard) {
    state.select_board(board);
}

#[fission_reducer(PublishRefresh)]
fn publish_refresh(state: &mut PublishUiState) {
    state.refresh_snapshot();
}

#[fission_reducer(PublishToggleTheme)]
fn publish_toggle_theme(state: &mut PublishUiState) {
    state.theme_mode = match state.theme_mode {
        ThemeMode::Light => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::Light,
    };
}

#[fission_reducer(PublishSetTrack)]
fn publish_set_track(state: &mut PublishUiState, value: String) {
    state.track = value;
    state.refresh_snapshot();
}

#[fission_reducer(PublishSetLocales)]
fn publish_set_locales(state: &mut PublishUiState, value: String) {
    state.locales_input = value;
    state.refresh_snapshot();
}

#[fission_reducer(PublishSetPlayJson)]
fn publish_set_play_json(state: &mut PublishUiState, value: String) {
    state.play_json_path = value;
}

#[fission_reducer(PublishSetAndroidJks)]
fn publish_set_android_jks(state: &mut PublishUiState, value: String) {
    state.android_jks_path = value;
}

#[fission_reducer(PublishSetAndroidAlias)]
fn publish_set_android_alias(state: &mut PublishUiState, value: String) {
    state.android_alias = value;
}

#[fission_reducer(PublishSetAndroidPassword)]
fn publish_set_android_password(state: &mut PublishUiState, value: String) {
    state.android_password = value;
}

#[fission_reducer(PublishSetAppStoreKeyPath)]
fn publish_set_app_store_key_path(state: &mut PublishUiState, value: String) {
    state.app_store_key_path = value;
}

#[fission_reducer(PublishSetAppStoreKeyId)]
fn publish_set_app_store_key_id(state: &mut PublishUiState, value: String) {
    state.app_store_key_id = value;
}

#[fission_reducer(PublishSetAppStoreIssuerId)]
fn publish_set_app_store_issuer_id(state: &mut PublishUiState, value: String) {
    state.app_store_issuer_id = value;
}

#[fission_reducer(PublishSetWindowsPfx)]
fn publish_set_windows_pfx(state: &mut PublishUiState, value: String) {
    state.windows_pfx_path = value;
}

#[fission_reducer(PublishSetWindowsPassword)]
fn publish_set_windows_password(state: &mut PublishUiState, value: String) {
    state.windows_password = value;
}

#[fission_reducer(PublishSetAzureTenant)]
fn publish_set_azure_tenant(state: &mut PublishUiState, value: String) {
    state.azure_tenant_id = value;
}

#[fission_reducer(PublishSetAzureClient)]
fn publish_set_azure_client(state: &mut PublishUiState, value: String) {
    state.azure_client_id = value;
}

#[fission_reducer(PublishSetMicrosoftSecret)]
fn publish_set_microsoft_secret(state: &mut PublishUiState, value: String) {
    state.microsoft_secret = value;
}

#[fission_reducer(PublishSetAwsProfile)]
fn publish_set_aws_profile(state: &mut PublishUiState, value: String) {
    state.aws_profile = value;
}

#[fission_reducer(PublishSetAwsRegion)]
fn publish_set_aws_region(state: &mut PublishUiState, value: String) {
    state.aws_region = value;
}

#[fission_reducer(PublishSetAwsEndpoint)]
fn publish_set_aws_endpoint(state: &mut PublishUiState, value: String) {
    state.aws_endpoint = value;
}

#[fission_reducer(PublishSetAwsAccessKey)]
fn publish_set_aws_access_key(state: &mut PublishUiState, value: String) {
    state.aws_access_key_id = value;
}

#[fission_reducer(PublishSetAwsSecretKey)]
fn publish_set_aws_secret_key(state: &mut PublishUiState, value: String) {
    state.aws_secret_access_key = value;
}

#[fission_reducer(PublishSetConfirmation)]
fn publish_set_confirmation(state: &mut PublishUiState, value: String) {
    state.publish_confirmation = value;
}

#[fission_reducer(PublishSaveCredentials)]
fn publish_save_credentials(state: &mut PublishUiState) {
    state.save_current_credentials();
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

#[derive(Clone)]
pub struct PublishApp;

impl From<PublishApp> for Widget {
    fn from(_component: PublishApp) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let size = view.env().viewport_size;
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let compact = size.width < 96.0;
        let body: Widget = if compact {
            Column {
                gap: Some(1.0),
                children: widgets![
                    PublishHero,
                    PublishTargetStrip,
                    PublishMainPanel,
                    PublishConsole
                ],
                ..Default::default()
            }
            .into()
        } else {
            Column {
                gap: Some(1.0),
                children: widgets![
                    PublishHero,
                    Row {
                        gap: Some(1.5),
                        align_items: AlignItems::Stretch,
                        children: widgets![PublishTargetRail { width: 26.0 }, PublishMainPanel,],
                        ..Default::default()
                    },
                    PublishConsole,
                ],
                ..Default::default()
            }
            .into()
        };
        let root = Container::new(body)
            .width(size.width.max(80.0))
            .height(size.height.max(36.0))
            .padding(if compact {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [2.0, 2.0, 1.0, 1.0]
            })
            .bg(palette.background)
            .into();
        if view.state().file_picker.is_some() {
            Overlay {
                content: root,
                overlay: FilePickerPanel.into(),
                ..Default::default()
            }
            .into()
        } else {
            root
        }
    }
}

#[derive(Clone)]
struct PublishHero;

impl From<PublishHero> for Widget {
    fn from(_component: PublishHero) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let refresh = with_reducer!(ctx, PublishRefresh, publish_refresh);
        let theme = with_reducer!(ctx, PublishToggleTheme, publish_toggle_theme);
        let progress = readiness_fraction(view.state());
        Container::new(Column {
            gap: Some(0.5),
            children: widgets![
                Text::new("Fission publish cockpit").color(palette.text),
                Text::new(format!(
                    "{}  -  {}  -  workspace {}",
                    view.state().app_name,
                    view.state().app_id,
                    view.state().workspace.display()
                ))
                .color(palette.muted),
                Row {
                    gap: Some(1.0),
                    align_items: AlignItems::Center,
                    children: widgets![
                        ProgressPill { value: progress },
                        PublishButton {
                            label: "Refresh".into(),
                            action: refresh,
                            tone: ButtonTone::Quiet,
                            width: 13.0
                        },
                        PublishButton {
                            label: "Theme".into(),
                            action: theme,
                            tone: ButtonTone::Quiet,
                            width: 11.0
                        },
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .height(7.0)
        .padding([2.0, 2.0, 1.0, 1.0])
        .bg(palette.hero)
        .border_radius(10.0)
        .into()
    }
}

#[derive(Clone)]
struct ProgressPill {
    value: f32,
}

impl From<ProgressPill> for Widget {
    fn from(component: ProgressPill) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let text = format!("{}% ready", (component.value * 100.0).round() as i32);
        Container::new(Text::new(text).color(palette.accent_text))
            .padding([1.2, 1.2, 0.4, 0.4])
            .bg(palette.accent)
            .border_radius(999.0)
            .into()
    }
}

#[derive(Clone)]
struct PublishTargetRail {
    width: f32,
}

impl From<PublishTargetRail> for Widget {
    fn from(component: PublishTargetRail) -> Widget {
        let (_ctx, _view) = fission::build::current::<PublishUiState>();
        Container::new(Column {
            gap: Some(1.0),
            children: PublishBoard::ALL
                .iter()
                .copied()
                .map(|board| {
                    TargetCard {
                        board,
                        width: component.width,
                    }
                    .into()
                })
                .collect(),
            ..Default::default()
        })
        .width(component.width)
        .into()
    }
}

#[derive(Clone)]
struct PublishTargetStrip;

impl From<PublishTargetStrip> for Widget {
    fn from(_component: PublishTargetStrip) -> Widget {
        Row {
            gap: Some(1.0),
            children: PublishBoard::ALL
                .iter()
                .copied()
                .map(|board| TargetCard { board, width: 22.0 }.into())
                .collect(),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct TargetCard {
    board: PublishBoard,
    width: f32,
}

impl From<TargetCard> for Widget {
    fn from(component: TargetCard) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let selected = view.state().board == component.board;
        let action = with_reducer!(
            ctx,
            PublishSelectBoard(component.board),
            publish_select_board
        );
        let bg = if selected {
            palette.selected
        } else {
            palette.surface
        };
        Button {
            on_press: Some(action),
            width: Some(component.width),
            height: Some(3.0),
            padding: Some([1.0, 1.0, 0.8, 0.8]),
            background_fill: Some(Fill::Solid(bg)),
            text_color: Some(palette.text),
            child: Some(
                Text::new(if selected {
                    format!("> {}", component.board.title())
                } else {
                    component.board.title().to_string()
                })
                .color(palette.text)
                .into(),
            ),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct PublishMainPanel;

impl From<PublishMainPanel> for Widget {
    fn from(_component: PublishMainPanel) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let board_title = view.state().board.title();
        Container::new(Scroll {
            direction: FlexDirection::Column,
            child: Some(
                Column {
                    gap: Some(1.0),
                    children: widgets![
                        Column {
                            gap: Some(0.3),
                            children: widgets![
                                Text::new(board_title).color(palette.text),
                                Text::new(view.state().board.subtitle()).color(palette.muted),
                            ],
                            ..Default::default()
                        },
                        LaunchSequence,
                        CredentialsPanel,
                        ReleaseOptionsPanel,
                        ActionDeck,
                        CheckSection {
                            title: "Project/package preflight".into(),
                            checks: view.state().package_checks.clone()
                        },
                        CheckSection {
                            title: "Provider/distribution preflight".into(),
                            checks: view.state().distribution_checks.clone()
                        },
                    ],
                    ..Default::default()
                }
                .into(),
            ),
            show_scrollbar: true,
            ..Default::default()
        })
        .flex_grow(1.0)
        .padding([1.5, 1.5, 1.0, 1.0])
        .bg(palette.panel)
        .border_radius(12.0)
        .into()
    }
}

#[derive(Clone)]
struct LaunchSequence;

impl From<LaunchSequence> for Widget {
    fn from(_component: LaunchSequence) -> Widget {
        Row {
            gap: Some(0.7),
            children: [
                "Preflight",
                "Credentials",
                "Signing",
                "Artifact",
                "Dry run",
                "Publish",
            ]
            .into_iter()
            .enumerate()
            .map(|(idx, label)| {
                StepPill {
                    index: idx + 1,
                    label: label.to_string(),
                }
                .into()
            })
            .collect(),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct StepPill {
    index: usize,
    label: String,
}

impl From<StepPill> for Widget {
    fn from(component: StepPill) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Container::new(
            Text::new(format!("{}  {}", component.index, component.label)).color(palette.text),
        )
        .padding([0.9, 0.9, 0.45, 0.45])
        .bg(palette.surface)
        .border_radius(999.0)
        .into()
    }
}

#[derive(Clone)]
struct ReleaseOptionsPanel;

impl From<ReleaseOptionsPanel> for Widget {
    fn from(_component: ReleaseOptionsPanel) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let set_track = with_reducer!(ctx, PublishSetTrack(String::new()), publish_set_track);
        let set_locales = with_reducer!(ctx, PublishSetLocales(String::new()), publish_set_locales);
        FormBand {
            title: "Release shape".into(),
            description: format!(
                "{} / {} artifact written to {}",
                view.state().target.as_str(),
                view.state().format.as_str(),
                view.state().artifact_manifest.display()
            ),
            children: widgets![
                PublishTextField {
                    id: "publish_track",
                    label: "Track / channel".into(),
                    value: view.state().track.clone(),
                    placeholder: view.state().board.default_track().into(),
                    on_change: set_track,
                    secret: false,
                    width: 28.0
                },
                PublishTextField {
                    id: "publish_locales",
                    label: "Locales".into(),
                    value: view.state().locales_input.clone(),
                    placeholder: "en-US, pl-PL".into(),
                    on_change: set_locales,
                    secret: false,
                    width: 28.0
                },
            ],
        }
        .into()
    }
}

#[derive(Clone)]
struct CredentialsPanel;

impl From<CredentialsPanel> for Widget {
    fn from(_component: CredentialsPanel) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        match view.state().board {
            PublishBoard::Android => AndroidCredentials.into(),
            PublishBoard::Ios => IosCredentials.into(),
            PublishBoard::Windows => WindowsCredentials.into(),
            PublishBoard::S3 => S3Credentials.into(),
        }
    }
}

#[derive(Clone)]
struct AndroidCredentials;

impl From<AndroidCredentials> for Widget {
    fn from(_component: AndroidCredentials) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let play = with_reducer!(
            ctx,
            PublishSetPlayJson(String::new()),
            publish_set_play_json
        );
        let jks = with_reducer!(
            ctx,
            PublishSetAndroidJks(String::new()),
            publish_set_android_jks
        );
        let alias = with_reducer!(
            ctx,
            PublishSetAndroidAlias(String::new()),
            publish_set_android_alias
        );
        let password = with_reducer!(
            ctx,
            PublishSetAndroidPassword(String::new()),
            publish_set_android_password
        );
        let pick_play = with_reducer!(
            ctx,
            PublishOpenFilePicker(FilePurpose::PlayServiceJson),
            publish_open_file_picker
        );
        let pick_jks = with_reducer!(
            ctx,
            PublishOpenFilePicker(FilePurpose::AndroidKeystore),
            publish_open_file_picker
        );
        let save = with_reducer!(ctx, PublishSaveCredentials, publish_save_credentials);
        let generate = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::GenerateAndroidKey),
            publish_start_task
        );
        FormBand {
            title: "Android credentials and signing".into(),
            description: "Create API access in Play Console, keep the JSON and upload key outside git, then save references into the local release workspace.".into(),
            children: widgets![
                InstructionList { items: vec![
                    "Play Console -> Setup -> API access -> link a Google Cloud project".into(),
                    "Enable Android Publisher API, create a service account, grant this app access".into(),
                    "Use an existing JKS or generate an upload key here from a password".into(),
                ]},
                Row { gap: Some(1.0), children: widgets![
                    PublishTextField { id: "publish_play_json", label: "Service account JSON".into(), value: view.state().play_json_path.clone(), placeholder: "~/.fission/app/play-service-account.json".into(), on_change: play, secret: false, width: 42.0 },
                    PublishButton { label: "Browse".into(), action: pick_play, tone: ButtonTone::Quiet, width: 12.0 },
                ], ..Default::default() },
                Row { gap: Some(1.0), children: widgets![
                    PublishTextField { id: "publish_android_jks", label: "Upload key JKS".into(), value: view.state().android_jks_path.clone(), placeholder: "~/.fission/app/upload-key.jks".into(), on_change: jks, secret: false, width: 42.0 },
                    PublishButton { label: "Browse".into(), action: pick_jks, tone: ButtonTone::Quiet, width: 12.0 },
                ], ..Default::default() },
                Row { gap: Some(1.0), children: widgets![
                    PublishTextField { id: "publish_android_alias", label: "Alias".into(), value: view.state().android_alias.clone(), placeholder: "upload".into(), on_change: alias, secret: false, width: 22.0 },
                    PublishTextField { id: "publish_android_password", label: "Key password".into(), value: view.state().android_password.clone(), placeholder: "stored in release.env".into(), on_change: password, secret: true, width: 24.0 },
                    PublishButton { label: "Generate JKS".into(), action: generate, tone: ButtonTone::Secondary, width: 17.0 },
                    PublishButton { label: "Save".into(), action: save, tone: ButtonTone::Primary, width: 10.0 },
                ], ..Default::default() },
            ],
        }
        .into()
    }
}

#[derive(Clone)]
struct IosCredentials;

impl From<IosCredentials> for Widget {
    fn from(_component: IosCredentials) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let key_path = with_reducer!(
            ctx,
            PublishSetAppStoreKeyPath(String::new()),
            publish_set_app_store_key_path
        );
        let key_id = with_reducer!(
            ctx,
            PublishSetAppStoreKeyId(String::new()),
            publish_set_app_store_key_id
        );
        let issuer = with_reducer!(
            ctx,
            PublishSetAppStoreIssuerId(String::new()),
            publish_set_app_store_issuer_id
        );
        let browse = with_reducer!(
            ctx,
            PublishOpenFilePicker(FilePurpose::AppStoreKey),
            publish_open_file_picker
        );
        let save = with_reducer!(ctx, PublishSaveCredentials, publish_save_credentials);
        FormBand {
            title: "App Store Connect access".into(),
            description: "Apple account approval can be incomplete; this flow still verifies local IPA/package prerequisites and stores API-key details once available.".into(),
            children: widgets![
                InstructionList { items: vec![
                    "App Store Connect -> Users and Access -> Integrations -> App Store Connect API".into(),
                    "Create an API key with App Manager access and download the .p8 once".into(),
                    "Capture Key ID and Issuer ID; provisioning/profile setup remains in Apple tooling".into(),
                ]},
                Row { gap: Some(1.0), children: widgets![
                    PublishTextField { id: "publish_app_store_key", label: ".p8 key".into(), value: view.state().app_store_key_path.clone(), placeholder: "~/.fission/app/ios/AuthKey_XXXX.p8".into(), on_change: key_path, secret: false, width: 42.0 },
                    PublishButton { label: "Browse".into(), action: browse, tone: ButtonTone::Quiet, width: 12.0 },
                ], ..Default::default() },
                Row { gap: Some(1.0), children: widgets![
                    PublishTextField { id: "publish_app_store_key_id", label: "Key ID".into(), value: view.state().app_store_key_id.clone(), placeholder: "ABC123DEFG".into(), on_change: key_id, secret: false, width: 20.0 },
                    PublishTextField { id: "publish_app_store_issuer", label: "Issuer ID".into(), value: view.state().app_store_issuer_id.clone(), placeholder: "UUID".into(), on_change: issuer, secret: false, width: 34.0 },
                    PublishButton { label: "Save".into(), action: save, tone: ButtonTone::Primary, width: 10.0 },
                ], ..Default::default() },
            ],
        }
        .into()
    }
}

#[derive(Clone)]
struct WindowsCredentials;

impl From<WindowsCredentials> for Widget {
    fn from(_component: WindowsCredentials) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let pfx = with_reducer!(
            ctx,
            PublishSetWindowsPfx(String::new()),
            publish_set_windows_pfx
        );
        let password = with_reducer!(
            ctx,
            PublishSetWindowsPassword(String::new()),
            publish_set_windows_password
        );
        let tenant = with_reducer!(
            ctx,
            PublishSetAzureTenant(String::new()),
            publish_set_azure_tenant
        );
        let client = with_reducer!(
            ctx,
            PublishSetAzureClient(String::new()),
            publish_set_azure_client
        );
        let secret = with_reducer!(
            ctx,
            PublishSetMicrosoftSecret(String::new()),
            publish_set_microsoft_secret
        );
        let browse = with_reducer!(
            ctx,
            PublishOpenFilePicker(FilePurpose::WindowsCertificate),
            publish_open_file_picker
        );
        let save = with_reducer!(ctx, PublishSaveCredentials, publish_save_credentials);
        FormBand {
            title: "Windows Store and signing".into(),
            description: "MSIX publishing uses Microsoft Store Developer CLI. EXE/MSI Partner Center submission can use Entra credentials.".into(),
            children: widgets![
                InstructionList { items: vec![
                    "Install and run msstore sign-in/reconfigure for MSIX uploads".into(),
                    "Reserve the product and configure package identity in Partner Center".into(),
                    "Select a PFX only when local signing is required".into(),
                ]},
                Row { gap: Some(1.0), children: widgets![
                    PublishTextField { id: "publish_windows_pfx", label: "PFX certificate".into(), value: view.state().windows_pfx_path.clone(), placeholder: "~/.fission/app/windows/signing.pfx".into(), on_change: pfx, secret: false, width: 42.0 },
                    PublishButton { label: "Browse".into(), action: browse, tone: ButtonTone::Quiet, width: 12.0 },
                    PublishTextField { id: "publish_windows_password", label: "PFX password".into(), value: view.state().windows_password.clone(), placeholder: "stored locally".into(), on_change: password, secret: true, width: 20.0 },
                ], ..Default::default() },
                Row { gap: Some(1.0), children: widgets![
                    PublishTextField { id: "publish_azure_tenant", label: "Azure tenant".into(), value: view.state().azure_tenant_id.clone(), placeholder: "tenant id".into(), on_change: tenant, secret: false, width: 24.0 },
                    PublishTextField { id: "publish_azure_client", label: "Azure client".into(), value: view.state().azure_client_id.clone(), placeholder: "client id".into(), on_change: client, secret: false, width: 24.0 },
                    PublishTextField { id: "publish_ms_secret", label: "Client secret".into(), value: view.state().microsoft_secret.clone(), placeholder: "optional for MSIX".into(), on_change: secret, secret: true, width: 24.0 },
                    PublishButton { label: "Save".into(), action: save, tone: ButtonTone::Primary, width: 10.0 },
                ], ..Default::default() },
            ],
        }
        .into()
    }
}

#[derive(Clone)]
struct S3Credentials;

impl From<S3Credentials> for Widget {
    fn from(_component: S3Credentials) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let profile = with_reducer!(
            ctx,
            PublishSetAwsProfile(String::new()),
            publish_set_aws_profile
        );
        let region = with_reducer!(
            ctx,
            PublishSetAwsRegion(String::new()),
            publish_set_aws_region
        );
        let endpoint = with_reducer!(
            ctx,
            PublishSetAwsEndpoint(String::new()),
            publish_set_aws_endpoint
        );
        let access = with_reducer!(
            ctx,
            PublishSetAwsAccessKey(String::new()),
            publish_set_aws_access_key
        );
        let secret = with_reducer!(
            ctx,
            PublishSetAwsSecretKey(String::new()),
            publish_set_aws_secret_key
        );
        let save = with_reducer!(ctx, PublishSaveCredentials, publish_save_credentials);
        FormBand {
            title: "S3 destination credentials".into(),
            description: "Use AWS_PROFILE locally when possible. Static keys are supported for local proof, but CI should prefer OIDC/web identity.".into(),
            children: widgets![
                InstructionList { items: vec![
                    "Configure [distribution.s3.production] bucket/path in fission.toml".into(),
                    "Use AWS_PROFILE for local publishing, or endpoint + static keys for S3-compatible storage".into(),
                    "CI should prefer AWS_WEB_IDENTITY_TOKEN_FILE or provider-native identity".into(),
                ]},
                Row { gap: Some(1.0), children: widgets![
                    PublishTextField { id: "publish_aws_profile", label: "AWS profile".into(), value: view.state().aws_profile.clone(), placeholder: "default".into(), on_change: profile, secret: false, width: 18.0 },
                    PublishTextField { id: "publish_aws_region", label: "Region".into(), value: view.state().aws_region.clone(), placeholder: "eu-west-2".into(), on_change: region, secret: false, width: 18.0 },
                    PublishTextField { id: "publish_aws_endpoint", label: "S3 endpoint".into(), value: view.state().aws_endpoint.clone(), placeholder: "optional".into(), on_change: endpoint, secret: false, width: 30.0 },
                ], ..Default::default() },
                Row { gap: Some(1.0), children: widgets![
                    PublishTextField { id: "publish_aws_access", label: "Access key".into(), value: view.state().aws_access_key_id.clone(), placeholder: "optional".into(), on_change: access, secret: false, width: 26.0 },
                    PublishTextField { id: "publish_aws_secret", label: "Secret key".into(), value: view.state().aws_secret_access_key.clone(), placeholder: "optional".into(), on_change: secret, secret: true, width: 28.0 },
                    PublishButton { label: "Save".into(), action: save, tone: ButtonTone::Primary, width: 10.0 },
                ], ..Default::default() },
            ],
        }
        .into()
    }
}

#[derive(Clone)]
struct ActionDeck;

impl From<ActionDeck> for Widget {
    fn from(_component: ActionDeck) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let package = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Package),
            publish_start_task
        );
        let dry = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::DryRun),
            publish_start_task
        );
        let publish = with_reducer!(
            ctx,
            PublishStartTask(PublishTaskKind::Publish),
            publish_start_task
        );
        let confirm = with_reducer!(
            ctx,
            PublishSetConfirmation(String::new()),
            publish_set_confirmation
        );
        let locked = !view.state().is_ready_to_publish();
        FormBand {
            title: "Release actions".into(),
            description: "Build first, run the provider dry-run, then type the app id to unlock the final publish button.".into(),
            children: widgets![
                Row { gap: Some(1.0), children: widgets![
                    PublishButton { label: "Build artifact".into(), action: package, tone: ButtonTone::Primary, width: 18.0 },
                    PublishButton { label: "Dry run".into(), action: dry, tone: ButtonTone::Secondary, width: 14.0 },
                    PublishButton { label: if locked { "Publish locked".into() } else { "Publish".into() }, action: publish, tone: if locked { ButtonTone::Quiet } else { ButtonTone::Danger }, width: 18.0 },
                ], ..Default::default() },
                PublishTextField { id: "publish_confirmation", label: format!("Type app id: {}", view.state().app_id), value: view.state().publish_confirmation.clone(), placeholder: view.state().app_id.clone(), on_change: confirm, secret: false, width: 46.0 },
            ],
        }
        .into()
    }
}

#[derive(Clone)]
struct CheckSection {
    title: String,
    checks: Vec<UiCheck>,
}

impl From<CheckSection> for Widget {
    fn from(component: CheckSection) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let children = component
            .checks
            .iter()
            .take(12)
            .cloned()
            .map(|check| CheckRow { check }.into())
            .collect::<Vec<_>>();
        Container::new(Column {
            gap: Some(0.55),
            children: std::iter::once(Text::new(component.title).color(palette.text).into())
                .chain(children)
                .collect(),
            ..Default::default()
        })
        .padding([1.0, 1.0, 0.9, 0.9])
        .bg(palette.surface)
        .border_radius(10.0)
        .into()
    }
}

#[derive(Clone)]
struct CheckRow {
    check: UiCheck,
}

impl From<CheckRow> for Widget {
    fn from(component: CheckRow) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let (marker, color) = match component.check.status {
            CheckStatus::Passed => ("OK", palette.success),
            CheckStatus::Warning => ("!", palette.warning),
            CheckStatus::Missing | CheckStatus::Failed => ("x", palette.error),
            CheckStatus::Skipped => (".", palette.muted),
        };
        let detail = component.check.details.unwrap_or_default();
        Row {
            gap: Some(0.8),
            align_items: AlignItems::Start,
            children: widgets![
                Text::new(marker).color(color),
                Column {
                    gap: Some(0.15),
                    children: widgets![
                        Text::new(component.check.summary).color(palette.text),
                        Text::new(detail).color(palette.muted),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct PublishConsole;

impl From<PublishConsole> for Widget {
    fn from(_component: PublishConsole) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let mut lines = vec![view.state().status_message.clone()];
        lines.extend(view.state().task_log.iter().rev().take(5).cloned());
        Container::new(Column {
            gap: Some(0.25),
            children: lines
                .into_iter()
                .rev()
                .map(|line| Text::new(line).color(palette.muted).into())
                .collect(),
            ..Default::default()
        })
        .padding([1.5, 1.5, 0.8, 0.8])
        .bg(palette.console)
        .border_radius(10.0)
        .into()
    }
}

#[derive(Clone)]
struct FilePickerPanel;

impl From<FilePickerPanel> for Widget {
    fn from(_component: FilePickerPanel) -> Widget {
        let (ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let Some(picker) = view.state().file_picker.clone() else {
            return Spacer::default().into();
        };
        let close = with_reducer!(ctx, PublishCloseFilePicker, publish_close_file_picker);
        let mut children = widgets![
            Row {
                justify_content: JustifyContent::SpaceBetween,
                children: widgets![
                    Text::new(picker.purpose.title()).color(palette.text),
                    PublishButton {
                        label: "Close".into(),
                        action: close,
                        tone: ButtonTone::Quiet,
                        width: 10.0
                    },
                ],
                ..Default::default()
            },
            Text::new(picker.current_dir.display().to_string()).color(palette.muted),
        ];
        let parent_action = with_reducer!(ctx, PublishPickFileEntry(0), publish_pick_file_entry);
        children.push(
            PublishButton {
                label: "../".into(),
                action: parent_action,
                tone: ButtonTone::Quiet,
                width: 52.0,
            }
            .into(),
        );
        for (idx, entry) in picker.entries.iter().enumerate() {
            let action = with_reducer!(ctx, PublishPickFileEntry(idx + 1), publish_pick_file_entry);
            let label = if entry.is_dir {
                format!("{}/", entry.label)
            } else {
                entry.label.clone()
            };
            children.push(
                PublishButton {
                    label,
                    action,
                    tone: if entry.is_dir {
                        ButtonTone::Quiet
                    } else {
                        ButtonTone::Secondary
                    },
                    width: 52.0,
                }
                .into(),
            );
        }
        if let Some(selection) = &view.state().selected_file {
            let copy = with_reducer!(ctx, PublishApplyFile(FileAction::Copy), publish_apply_file);
            let mv = with_reducer!(ctx, PublishApplyFile(FileAction::Move), publish_apply_file);
            let reference = with_reducer!(
                ctx,
                PublishApplyFile(FileAction::Reference),
                publish_apply_file
            );
            children.push(
                Text::new(format!("Selected {}", selection.path.display()))
                    .color(palette.text)
                    .into(),
            );
            children.push(
                Row {
                    gap: Some(1.0),
                    children: widgets![
                        PublishButton {
                            label: "Copy to workspace".into(),
                            action: copy,
                            tone: ButtonTone::Primary,
                            width: 22.0
                        },
                        PublishButton {
                            label: "Move".into(),
                            action: mv,
                            tone: ButtonTone::Secondary,
                            width: 10.0
                        },
                        PublishButton {
                            label: "Reference path".into(),
                            action: reference,
                            tone: ButtonTone::Quiet,
                            width: 18.0
                        },
                    ],
                    ..Default::default()
                }
                .into(),
            );
        }
        Container::new(Column {
            gap: Some(0.65),
            children,
            ..Default::default()
        })
        .width(62.0)
        .padding([1.5, 1.5, 1.0, 1.0])
        .bg(palette.panel)
        .border(palette.accent, 1.0)
        .border_radius(12.0)
        .into()
    }
}

#[derive(Clone)]
struct FormBand {
    title: String,
    description: String,
    children: Vec<Widget>,
}

impl From<FormBand> for Widget {
    fn from(component: FormBand) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let mut children = widgets![
            Text::new(component.title).color(palette.text),
            Text::new(component.description).color(palette.muted),
        ];
        children.extend(component.children);
        Container::new(Column {
            gap: Some(0.8),
            children,
            ..Default::default()
        })
        .padding([1.1, 1.1, 0.9, 0.9])
        .bg(palette.surface)
        .border_radius(10.0)
        .into()
    }
}

#[derive(Clone)]
struct InstructionList {
    items: Vec<String>,
}

impl From<InstructionList> for Widget {
    fn from(component: InstructionList) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        Column {
            gap: Some(0.25),
            children: component
                .items
                .into_iter()
                .enumerate()
                .map(|(idx, item)| {
                    Text::new(format!("{}. {}", idx + 1, item))
                        .color(palette.muted)
                        .into()
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct PublishTextField {
    id: &'static str,
    label: String,
    value: String,
    placeholder: String,
    on_change: ActionEnvelope,
    secret: bool,
    width: f32,
}

impl From<PublishTextField> for Widget {
    fn from(component: PublishTextField) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        TextInput {
            id: Some(WidgetId::explicit(component.id)),
            label: Some(component.label.into()),
            value: component.value,
            placeholder: Some(component.placeholder.into()),
            on_change: Some(component.on_change),
            width: Some(component.width),
            height: Some(3.0),
            padding: Some([0.6, 0.6, 0.25, 0.25]),
            obscure_text: component.secret,
            background_fill: Some(Fill::Solid(palette.input)),
            border_color: Some(palette.hairline),
            focus_border_color: Some(palette.accent),
            text_color: Some(palette.text),
            label_color: Some(palette.muted),
            placeholder_color: Some(palette.muted),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone)]
struct PublishButton {
    label: String,
    action: ActionEnvelope,
    tone: ButtonTone,
    width: f32,
}

impl From<PublishButton> for Widget {
    fn from(component: PublishButton) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let fill = match component.tone {
            ButtonTone::Primary => palette.accent,
            ButtonTone::Secondary => palette.secondary,
            ButtonTone::Danger => palette.error,
            ButtonTone::Quiet => palette.surface_alt,
        };
        Button {
            on_press: Some(component.action),
            width: Some(component.width),
            height: Some(3.0),
            padding: Some([0.8, 0.8, 0.35, 0.35]),
            background_fill: Some(Fill::Solid(fill)),
            text_color: Some(palette.text),
            child: Some(Text::new(component.label).color(palette.text).into()),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Copy)]
enum ButtonTone {
    Primary,
    Secondary,
    Danger,
    Quiet,
}

#[derive(Clone, Copy)]
struct PublishPalette {
    background: Color,
    hero: Color,
    panel: Color,
    surface: Color,
    surface_alt: Color,
    selected: Color,
    console: Color,
    input: Color,
    hairline: Color,
    text: Color,
    muted: Color,
    accent: Color,
    accent_text: Color,
    secondary: Color,
    success: Color,
    warning: Color,
    error: Color,
}

impl PublishPalette {
    fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                background: rgb(7, 12, 18),
                hero: rgb(18, 42, 50),
                panel: rgb(14, 23, 34),
                surface: rgb(23, 34, 47),
                surface_alt: rgb(33, 46, 62),
                selected: rgb(26, 70, 72),
                console: rgb(10, 16, 24),
                input: rgb(10, 18, 28),
                hairline: rgb(63, 83, 101),
                text: rgb(238, 245, 244),
                muted: rgb(158, 177, 187),
                accent: rgb(36, 176, 146),
                accent_text: rgb(3, 24, 21),
                secondary: rgb(80, 123, 196),
                success: rgb(54, 210, 141),
                warning: rgb(237, 164, 64),
                error: rgb(236, 89, 91),
            },
            ThemeMode::Light => Self {
                background: rgb(239, 243, 239),
                hero: rgb(217, 236, 226),
                panel: rgb(251, 251, 247),
                surface: rgb(244, 247, 241),
                surface_alt: rgb(231, 238, 231),
                selected: rgb(205, 232, 220),
                console: rgb(225, 231, 226),
                input: rgb(255, 255, 251),
                hairline: rgb(175, 191, 181),
                text: rgb(18, 31, 33),
                muted: rgb(78, 95, 98),
                accent: rgb(26, 145, 115),
                accent_text: rgb(245, 255, 252),
                secondary: rgb(87, 119, 174),
                success: rgb(27, 141, 89),
                warning: rgb(184, 106, 24),
                error: rgb(196, 60, 63),
            },
        }
    }
}

fn readiness_fraction(state: &PublishUiState) -> f32 {
    let checks = state
        .package_checks
        .iter()
        .chain(state.distribution_checks.iter());
    let mut total = 0.0;
    let mut passed = 0.0;
    for check in checks {
        total += 1.0;
        if check.status == CheckStatus::Passed || check.severity != CheckSeverity::Error {
            passed += 1.0;
        }
    }
    if total == 0.0 {
        0.0
    } else {
        passed / total
    }
}

fn run_current_fission(args: Vec<String>) -> Result<String> {
    let exe = env::current_exe().context("failed to find current fission executable")?;
    let mut child = Command::new(exe)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start fission command")?;
    let stdout = child.stdout.take().context("missing stdout pipe")?;
    let stderr = child.stderr.take().context("missing stderr pipe")?;
    let out_handle = thread::spawn(move || {
        BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .collect::<Vec<_>>()
    });
    let err_handle = thread::spawn(move || {
        BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .collect::<Vec<_>>()
    });
    let status = child.wait().context("failed to wait for fission command")?;
    let mut output = out_handle.join().unwrap_or_default();
    output.extend(err_handle.join().unwrap_or_default());
    let text = output.join("\n");
    if status.success() {
        Ok(text)
    } else {
        anyhow::bail!("command failed with {status}:\n{text}")
    }
}

fn copy_or_move_selected_file(
    selected: &Path,
    workspace: &Path,
    default_name: &str,
    move_file: bool,
) -> Result<PathBuf> {
    fs::create_dir_all(workspace)
        .with_context(|| format!("failed to create {}", workspace.display()))?;
    let dest = workspace.join(default_name);
    if move_file {
        fs::rename(selected, &dest).with_context(|| {
            format!(
                "failed to move {} to {}",
                selected.display(),
                dest.display()
            )
        })?;
    } else {
        fs::copy(selected, &dest).with_context(|| {
            format!(
                "failed to copy {} to {}",
                selected.display(),
                dest.display()
            )
        })?;
    }
    set_private_file_permissions(&dest)?;
    Ok(dest)
}

fn read_env_entries(path: &Path) -> Result<std::collections::BTreeMap<String, String>> {
    let mut entries = std::collections::BTreeMap::new();
    if !path.exists() {
        return Ok(entries);
    }
    let text = fs::read_to_string(path)?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some((key, value)) = line.split_once('=') {
            entries.insert(key.trim().to_string(), unquote_env_value(value.trim()));
        }
    }
    Ok(entries)
}

fn upsert_env(path: &Path, key: &str, value: &str) -> Result<()> {
    let mut entries = read_env_entries(path)?;
    entries.insert(key.to_string(), value.to_string());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        set_private_dir_permissions(parent)?;
    }
    let mut out = String::new();
    out.push_str("# Fission local release environment. Do not commit this file.\n");
    out.push_str("# Generated by the publish cockpit.\n");
    for (key, value) in entries {
        out.push_str("export ");
        out.push_str(&key);
        out.push('=');
        out.push_str(&quote_env_value(&value));
        out.push('\n');
    }
    fs::write(path, out)?;
    set_private_file_permissions(path)?;
    Ok(())
}

fn quote_env_value(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
    )
}

fn unquote_env_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        let inner = &trimmed[1..trimmed.len() - 1];
        inner
            .replace("\\\"", "\"")
            .replace("\\$", "$")
            .replace("\\\\", "\\")
    } else {
        trimmed.to_string()
    }
}

fn sanitize_workspace_name(value: &str) -> String {
    let out = value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>()
        .trim_matches(['-', '.', '_'])
        .to_string();
    if out.is_empty() {
        "app".to_string()
    } else {
        out
    }
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
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
    }
}

pub(crate) fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_ui_renders_android_board() {
        let state = PublishUiState::load(PublishUiOptions {
            project_dir: PathBuf::from("."),
            provider: DistributionProvider::PlayStore,
            target: Some(Target::Android),
            format: Some(PackageFormat::Aab),
            artifact: None,
            site: "production".to_string(),
            deploy: None,
            track: Some("internal".to_string()),
            locales: vec!["en-US".to_string()],
            screenshot: None,
            exit_after_render: false,
            width: None,
            height: None,
        });
        let mut app = fission::terminal::TerminalApp::with_state(PublishApp, state);
        let frame = app.render_frame(160, 80).expect("publish UI renders");
        let text = frame.as_plain_text();
        assert!(text.contains("Fission publish cockpit"));
        assert!(text.contains("Android credentials and signing"));
    }
}
