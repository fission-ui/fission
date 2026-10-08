//! Versioned results for the finite website workflow. Other CLI protocols are independent.

use crate::{FissionProject, Target};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "fission.website-result.v1";
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
    /// Only directories verified to exist after a successful build.
    pub artifact_dir: Option<PathBuf>,
    /// May be returned by check/routes without creating files.
    pub planned_output_dir: Option<PathBuf>,
    /// Only files verified to exist after this operation.
    pub artifacts: Vec<PathBuf>,
    pub routes: Vec<Route>,
    /// Read these immediately after init, before changing the project.
    pub instructions: Vec<PathBuf>,
    pub next_steps: Vec<Step>,
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
        return Err(Failure::new(ErrorCode::ProjectNotFound, "The project has no fission.toml.")
            .recover(step(root, "Register the project, then read generated AGENTS.md and any referenced AGENTS.fission.md.", &["init", ".", "--json"])));
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

pub fn require_website_target(target: Target) -> Result<()> {
    if !matches!(target, Target::Web | Target::Site) {
        return Err(Failure::new(
            ErrorCode::InvalidTarget,
            "Structured website operations support web and static-site targets.",
        ));
    }
    Ok(())
}

pub fn require_target(root: &Path, project: &FissionProject, target: Target) -> Result<()> {
    let repair = || {
        step(
            root,
            "Use the supported target command to add or repair the scaffold, then retry.",
            &["add-target", target.as_str(), "--json"],
        )
    };
    if !project.targets.contains(&target) {
        return Err(Failure::new(
            ErrorCode::TargetNotConfigured,
            "The selected target is not configured.",
        )
        .recover(repair()));
    }
    if !root.join(target.scaffold_relative_path()).is_file() {
        return Err(Failure::new(
            ErrorCode::ScaffoldMissing,
            "The selected target scaffold is missing.",
        )
        .recover(repair()));
    }
    Ok(())
}

pub fn init(
    root: &Path,
    name: Option<String>,
    app_id: Option<String>,
    local_path: Option<PathBuf>,
) -> Result<Data> {
    if local_path
        .as_ref()
        .is_some_and(|path| path.to_str().is_none())
    {
        return Err(Failure::new(
            ErrorCode::InvalidConfiguration,
            "Structured init requires a UTF-8 --local-path.",
        ));
    }
    if root.join("fission.toml").exists() {
        load_project(root)?;
    }
    crate::init_project(root, name, app_id, local_path)
        .map_err(|error| operation_failure(&error))?;
    load_project(root)?;
    let artifacts = verified_files(vec![root.join("fission.toml"), root.join("Cargo.toml")])?;
    let instructions_root = crate::find_git_root(root).unwrap_or_else(|| root.to_path_buf());
    let mut instructions = ["AGENTS.md", "AGENTS.fission.md"]
        .into_iter()
        .map(|name| instructions_root.join(name))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    if root != instructions_root && root.join("AGENTS.md").is_file() {
        instructions.push(root.join("AGENTS.md"));
    }
    Ok(Data { instructions, artifacts,
        next_steps: vec![step(root, "Read the instructions immediately; retain fission.toml and add the intended website target.",
            &["add-target", "static-site", "--json"])], ..Data::default() })
}

pub fn add_targets(root: &Path, targets: &[Target]) -> Result<Data> {
    if targets.is_empty() {
        return Err(Failure::new(
            ErrorCode::InvalidTarget,
            "Select at least one website target.",
        ));
    }
    for target in targets {
        require_website_target(*target)?;
    }
    load_project(root)?;
    crate::add_targets(root, targets).map_err(|error| operation_failure(&error))?;
    let mut data = Data {
        artifacts: vec![root.join("fission.toml")],
        ..Data::default()
    };
    for target in targets {
        data.artifacts
            .push(root.join(target.scaffold_relative_path()));
        data.next_steps.push(step(
            root,
            "Build the configured website target.",
            &["build", "--target", target.as_str(), "--json"],
        ));
    }
    data.artifacts = verified_files(data.artifacts)?;
    Ok(data)
}

fn verified_files(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    if paths.iter().any(|path| !path.is_file()) {
        return Err(Failure::new(ErrorCode::ArtifactMissing, "Project setup did not produce the expected configuration/scaffold files; check for directories or inaccessible paths where files are expected."));
    }
    Ok(paths)
}

fn operation_failure(error: &anyhow::Error) -> Failure {
    // Do not echo TOML source lines, dependency URLs, app identifiers or environment values.
    if error.downcast_ref::<std::io::Error>().is_some() {
        Failure::new(
            ErrorCode::IoFailed,
            "Project files could not be created or updated; check file permissions and paths.",
        )
    } else {
        Failure::new(ErrorCode::InvalidConfiguration, "Project configuration could not be updated; check Cargo.toml and fission.toml, including the package name and dependency paths.")
    }
}
