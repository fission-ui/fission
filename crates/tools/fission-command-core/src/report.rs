//! Common finite CLI command outcomes; serialization never selects execution.

pub use crate::process_report::process_failure;
use crate::{FissionProject, Target};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "fission.cli-result.v1";
pub type Result<T> = std::result::Result<T, Failure>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Init,
    AddTarget,
    Build,
    SiteBuild,
    SiteCheck,
    SiteRoutes,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub schema: String,
    pub action: Action,
    /// Null when the project path cannot be resolved/represented in JSON.
    pub project_dir: Option<PathBuf>,
    pub selected_targets: Vec<Target>,
    pub state: Option<ProjectState>,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Success { data: Data },
    Failure { error: Failure },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectState {
    pub name: String,
    pub config_path: PathBuf,
    pub configured_targets: Vec<Target>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Data {
    /// Setup files verified after the command completes.
    pub artifacts: Vec<PathBuf>,
    pub targets: Vec<TargetData>,
    /// Read these immediately after init, before changing the project.
    pub instructions: Vec<PathBuf>,
    pub next_steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TargetData {
    pub target: Target,
    /// Verified build output directory, when the authority knows it.
    pub artifact_dir: Option<PathBuf>,
    /// Planned path; check/routes do not claim files exist.
    pub planned_output_dir: Option<PathBuf>,
    pub artifacts: Vec<PathBuf>,
    pub routes: Vec<Route>,
}

impl TargetData {
    pub fn completed(target: Target) -> Self {
        Self {
            target,
            artifact_dir: None,
            planned_output_dir: None,
            artifacts: Vec::new(),
            routes: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Route {
    /// Public route path; no server or deployment URL is implied.
    pub path: String,
    pub title: String,
    pub source: PathBuf,
    /// Planned output for check/routes, verified file for build.
    pub output: PathBuf,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ProjectNotFound,
    InvalidConfiguration,
    InvalidTarget,
    TargetNotConfigured,
    ScaffoldMissing,
    MissingToolchain,
    CompileFailed,
    SiteFailed,
    ArtifactMissing,
    ReportUnavailable,
    IoFailed,
    Interrupted,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Failure {
    pub code: ErrorCode,
    pub message: String,
    pub diagnostics: String,
    pub recovery: Vec<Step>,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

impl Failure {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            diagnostics: String::new(),
            recovery: Vec::new(),
        }
    }

    pub fn recover(mut self, step: Step) -> Self {
        self.recovery.push(step);
        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Step {
    pub instruction: String,
    pub invocation: Invocation,
}

/// Execute directly, without shell interpolation. A prerequisite may require a source edit.
#[derive(Debug, Serialize, Deserialize)]
pub struct Invocation {
    pub cwd: PathBuf,
    pub program: String,
    pub argv: Vec<String>,
}

pub fn step(root: &Path, instruction: impl Into<String>, argv: &[&str]) -> Step {
    let mut argv = argv.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    if argv.first().is_some_and(|arg| arg == "init") {
        if argv.get(1).is_some_and(|arg| arg == ".") {
            argv[1] = root.to_string_lossy().into_owned();
        }
    } else if !argv.iter().any(|arg| arg == "--help") {
        argv.extend(["--project-dir".into(), root.to_string_lossy().into_owned()]);
    }
    Step {
        instruction: instruction.into(),
        invocation: Invocation {
            cwd: std::env::current_dir().unwrap_or_else(|_| root.to_path_buf()),
            program: "fission".into(),
            argv,
        },
    }
}

pub fn absolute(root: &Path) -> Result<PathBuf> {
    if root.is_absolute() {
        return Ok(root.to_path_buf());
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(root))
        .map_err(|_| Failure::new(ErrorCode::IoFailed, "Cannot resolve the current directory."))
}

pub fn load_project(root: &Path) -> Result<FissionProject> {
    if !root.join("fission.toml").exists() {
        return Err(Failure::new(
            ErrorCode::ProjectNotFound,
            "The project has no fission.toml.",
        )
        .recover(step(
            root,
            "Initialize the project and read its instruction files.",
            &["init", ".", "--json"],
        )));
    }
    crate::read_project_config(root).map_err(|_| {
        Failure::new(
            ErrorCode::InvalidConfiguration,
            "Cannot read or parse fission.toml; repair the configuration file.",
        )
    })
}

pub fn state(root: &Path) -> Option<ProjectState> {
    let project = load_project(root).ok()?;
    Some(ProjectState {
        name: project.app.name,
        config_path: root.join("fission.toml"),
        configured_targets: project.targets.into_iter().collect(),
    })
}

pub(crate) fn verified_files(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    if paths.iter().any(|path| !path.is_file()) {
        return Err(Failure::new(
            ErrorCode::ArtifactMissing,
            "Project setup did not produce the expected configuration/scaffold files.",
        ));
    }
    Ok(paths)
}

pub fn operation_failure(error: &anyhow::Error) -> Failure {
    if let Some(error) =
        error.downcast_ref::<fission_command_process::diagnostic::DiagnosticFailure>()
    {
        return process_failure(
            fission_command_process::diagnostic::DiagnosticFailure {
                kind: error.kind,
                diagnostics: error.diagnostics.clone(),
            },
            ErrorCode::InvalidConfiguration,
            "cargo",
        );
    }
    if let Some(failure) = error.downcast_ref::<Failure>() {
        return Failure {
            code: failure.code,
            message: failure.message.clone(),
            diagnostics: failure.diagnostics.clone(),
            recovery: Vec::new(),
        };
    }
    if error.downcast_ref::<std::io::Error>().is_some() {
        Failure::new(
            ErrorCode::IoFailed,
            "Project files could not be read or updated; check paths and permissions.",
        )
    } else {
        Failure::new(ErrorCode::InvalidConfiguration, "Project configuration could not be loaded or updated; check Cargo.toml, fission.toml and dependency paths.")
    }
}
