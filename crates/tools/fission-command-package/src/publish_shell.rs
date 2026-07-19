#[path = "publish_shell_env.rs"]
mod env_file;

use super::{
    readiness_distribute, readiness_package, report_status, CheckStatus, PackageFormat,
    ReadinessCheck, ARTIFACT_MANIFEST,
};
use anyhow::{bail, Context, Result};
use env_file::{
    set_private_dir_permissions, set_private_file_permissions, unquote_env_value, upsert_env,
    write_env_file,
};
use fission_command_core::{read_project_config, DistributionProvider, Target};
use rpassword::read_password;
use serde::Deserialize;
use std::collections::VecDeque;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead, BufReader, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const GUIDED_LOG_LINES: usize = 10;

#[derive(Clone, Debug)]
pub struct PublishShellOptions {
    pub project_dir: PathBuf,
    pub provider: DistributionProvider,
    pub target: Option<Target>,
    pub format: Option<PackageFormat>,
    pub artifact: Option<PathBuf>,
    pub site: String,
    pub deploy: Option<String>,
    pub track: Option<String>,
    pub locales: Vec<String>,
    pub overwrite_remote: bool,
    pub dry_run: bool,
    pub yes: bool,
    pub json: bool,
    pub app: bool,
}

#[derive(Clone, Debug)]
pub struct PublishShellWorkflowRequest {
    pub project_dir: PathBuf,
    pub provider: DistributionProvider,
    pub target: Option<Target>,
    pub format: Option<PackageFormat>,
    pub artifact: Option<PathBuf>,
    pub site: String,
    pub deploy: Option<String>,
    pub track: Option<String>,
    pub locales: Vec<String>,
    pub overwrite_remote: bool,
    pub dry_run: bool,
    pub yes: bool,
    pub json: bool,
}

#[derive(Clone, Copy)]
pub struct PublishShellHooks {
    pub release_plan: fn(PublishShellWorkflowRequest) -> Result<PublishShellReleasePlan>,
    pub release_checks: fn(PublishShellWorkflowRequest) -> Result<Vec<ReadinessCheck>>,
    pub publish: fn(PublishShellWorkflowRequest) -> Result<String>,
    pub skip_requirement: fn(PublishShellWorkflowRequest, String) -> Result<String>,
    pub bump_build: fn(PublishShellWorkflowRequest) -> Result<String>,
}

#[derive(Clone, Debug)]
pub struct PublishFlowSnapshot {
    pub project_dir: PathBuf,
    pub app_name: String,
    pub app_id: String,
    pub provider: DistributionProvider,
    pub target: Target,
    pub format: PackageFormat,
    pub site: String,
    pub track: Option<String>,
    pub locales: Vec<String>,
    pub workspace: PathBuf,
    pub artifact_manifest: PathBuf,
    pub package_checks: Vec<ReadinessCheck>,
    pub distribution_checks: Vec<ReadinessCheck>,
    pub release_checks: Vec<ReadinessCheck>,
}

#[derive(Clone, Debug)]
pub struct PublishShellReleasePlan {
    pub status: String,
    pub provider: String,
    pub target: Option<String>,
    pub format: Option<String>,
    pub track: Option<String>,
    pub locales: Vec<String>,
    pub steps: Vec<PublishShellReleaseStep>,
    pub capabilities: Vec<PublishShellProviderCapability>,
    pub requirements: Vec<ReadinessCheck>,
}

#[derive(Clone, Debug)]
pub struct PublishShellReleaseStep {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct PublishShellProviderCapability {
    pub id: String,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Deserialize)]
struct ReleasePlanJson {
    #[serde(default)]
    status: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    track: Option<String>,
    #[serde(default)]
    locales: Vec<String>,
    #[serde(default)]
    steps: Vec<ReleaseStepJson>,
    #[serde(default)]
    capabilities: Vec<ProviderCapabilityJson>,
    #[serde(default)]
    requirements: Vec<ReleaseRequirementJson>,
}

#[derive(Debug, Deserialize)]
struct ReleaseStepJson {
    id: String,
    title: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct ProviderCapabilityJson {
    id: String,
    status: String,
    summary: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseRequirementJson {
    id: String,
    level: String,
    status: String,
    summary: String,
    details: Option<String>,
    #[serde(default)]
    remediation: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct PublishContext {
    pub(crate) project_dir: PathBuf,
    pub(crate) app_name: String,
    pub(crate) app_id: String,
    pub(crate) provider: DistributionProvider,
    pub(crate) target: Target,
    pub(crate) format: PackageFormat,
    pub(crate) site: String,
    pub(crate) track: Option<String>,
    pub(crate) locales: Vec<String>,
    pub(crate) overwrite_remote: bool,
    pub(crate) workspace: PathBuf,
    pub(crate) artifact: Option<PathBuf>,
}

pub fn run_publish_shell(options: PublishShellOptions) -> Result<()> {
    run_publish_shell_with_hooks(options, current_fission_hooks())
}

pub fn run_publish_shell_with_hooks(
    options: PublishShellOptions,
    hooks: PublishShellHooks,
) -> Result<()> {
    if options.json {
        bail!("interactive publish does not support --json; use fission readiness/package/distribute for machine-readable CI flows");
    }

    let mut cx = PublishContext::load(&options)?;
    ensure_local_workspace_files(&cx)?;
    load_local_env(&cx.workspace.join("release.env"))?;

    if options.app {
        println!("Fission publish app mode");
        println!("Windowed publish will use the same local workspace and checks as terminal mode.");
        println!("Opening terminal flow in this build so the release can be completed safely.\n");
    }

    if options.dry_run {
        render_dashboard(&cx, hooks)?;
        dry_run_publish(&cx, &options, hooks, false)?;
        return Ok(());
    }

    if options.yes {
        render_dashboard(&cx, hooks)?;
        package_artifact(&mut cx, hooks, false, false)?;
        dry_run_publish(&cx, &options, hooks, false)?;
        publish_artifact(&cx, &options, hooks)?;
        return Ok(());
    }

    if !io::stdin().is_terminal() {
        render_dashboard(&cx, hooks)?;
        bail!("interactive publish requires a terminal; pass --dry-run/--yes or use package/distribute directly in CI");
    }

    loop {
        render_dashboard(&cx, hooks)?;
        println!();
        println!("Actions");
        println!("  1  Create/update local workspace");
        println!("  2  Configure provider credentials and signing");
        println!("  3  Select track/locales/artifact options");
        println!("  4  Review release metadata, screenshots, and skips");
        println!("  5  Build/package artifact");
        println!("  6  Dry-run publish");
        println!("  7  Publish");
        println!("  8  Export CI checklist");
        println!("  r  Refresh");
        println!("  q  Quit");
        match prompt("Choose action")?.trim() {
            "1" => ensure_local_workspace(&cx)?,
            "2" => configure_provider_and_signing(&mut cx)?,
            "3" => configure_release_options(&mut cx)?,
            "4" => review_release_metadata_content(&cx, hooks)?,
            "5" => package_artifact(&mut cx, hooks, true, true)?,
            "6" => dry_run_publish(&cx, &options, hooks, true)?,
            "7" => publish_artifact_interactive(&cx, &options, hooks)?,
            "8" => export_ci_checklist(&cx),
            "r" | "R" => load_local_env(&cx.workspace.join("release.env"))?,
            "q" | "Q" => break,
            _ => pause("Unknown action."),
        }
    }
    Ok(())
}

pub fn publish_flow_snapshot(options: &PublishShellOptions) -> Result<PublishFlowSnapshot> {
    let cx = PublishContext::load(options)?;
    ensure_local_workspace_files(&cx)?;
    load_local_env(&cx.workspace.join("release.env"))?;
    let package_checks = readiness_package(&cx.project_dir, Some(cx.target), Some(cx.format), true)
        .unwrap_or_else(|err| readiness_error("release.package.readiness_failed", err));
    let distribution_checks = match super::load_publish_manifest(&cx.project_dir) {
        Ok(config) => readiness_distribute(
            &cx.project_dir,
            cx.provider,
            &cx.site,
            cx.track.as_deref(),
            Some(cx.format),
            Some(&cx.artifact_manifest_path()),
            &config,
        )
        .unwrap_or_else(|err| readiness_error("release.distribution.readiness_failed", err)),
        Err(err) => readiness_error("release.distribution.config_failed", err),
    };
    let artifact_manifest = cx.artifact_manifest_path();
    let release_checks = Vec::new();
    Ok(PublishFlowSnapshot {
        project_dir: cx.project_dir,
        app_name: cx.app_name,
        app_id: cx.app_id,
        provider: cx.provider,
        target: cx.target,
        format: cx.format,
        site: cx.site,
        track: cx.track,
        locales: cx.locales,
        workspace: cx.workspace,
        artifact_manifest,
        package_checks,
        distribution_checks,
        release_checks,
    })
}

pub fn ensure_publish_workspace(options: &PublishShellOptions) -> Result<PathBuf> {
    let cx = PublishContext::load(options)?;
    ensure_local_workspace_files(&cx)?;
    Ok(cx.workspace)
}

impl PublishContext {
    fn load(options: &PublishShellOptions) -> Result<Self> {
        let project = read_project_config(&options.project_dir).with_context(|| {
            format!(
                "failed to read {}",
                options.project_dir.join("fission.toml").display()
            )
        })?;
        let target = options
            .target
            .unwrap_or_else(|| default_target_for_provider(options.provider));
        let format = options
            .format
            .unwrap_or_else(|| default_format_for_target_provider(target, options.provider));
        let workspace = local_workspace_for_app(&project.app.name)?;
        let track = options
            .track
            .clone()
            .or_else(|| default_track_for_provider(options.provider).map(str::to_string));
        let locales = if options.locales.is_empty() {
            release_default_locales(&options.project_dir).unwrap_or_default()
        } else {
            options.locales.clone()
        };
        Ok(Self {
            project_dir: options.project_dir.clone(),
            app_name: project.app.name,
            app_id: project.app.app_id,
            provider: options.provider,
            target,
            format,
            site: options.site.clone(),
            track,
            locales,
            overwrite_remote: options.overwrite_remote,
            workspace,
            artifact: options.artifact.clone(),
        })
    }

    fn artifact_manifest_path(&self) -> PathBuf {
        self.artifact
            .clone()
            .unwrap_or_else(|| self.default_artifact_manifest_path())
    }

    fn default_artifact_manifest_path(&self) -> PathBuf {
        self.project_dir
            .join("target/fission/release")
            .join(self.target.as_str())
            .join(self.format.as_str())
            .join(ARTIFACT_MANIFEST)
    }
}

pub fn default_target_for_provider(provider: DistributionProvider) -> Target {
    match provider {
        DistributionProvider::PlayStore => Target::Android,
        DistributionProvider::AppStore => Target::Ios,
        DistributionProvider::MicrosoftStore => Target::Windows,
        DistributionProvider::S3
        | DistributionProvider::GithubPages
        | DistributionProvider::CloudflarePages
        | DistributionProvider::Netlify => Target::Site,
        DistributionProvider::DockerRegistry => Target::Server,
        DistributionProvider::GithubReleases
        | DistributionProvider::GoogleDrive
        | DistributionProvider::OneDrive
        | DistributionProvider::Dropbox => Target::Linux,
    }
}

pub fn default_format_for_target_provider(
    target: Target,
    provider: DistributionProvider,
) -> PackageFormat {
    match (target, provider) {
        (Target::Android, _) => PackageFormat::Aab,
        (Target::Ios, _) => PackageFormat::Ipa,
        (Target::Windows, DistributionProvider::MicrosoftStore) => PackageFormat::Msix,
        (Target::Windows, _) => PackageFormat::Exe,
        (Target::Linux | Target::Terminal, _) => PackageFormat::Run,
        (Target::Macos, _) => PackageFormat::App,
        (Target::Server, _) => PackageFormat::DockerImage,
        (Target::Site | Target::Web, _) => PackageFormat::Static,
    }
}

pub fn default_track_for_provider(provider: DistributionProvider) -> Option<&'static str> {
    match provider {
        DistributionProvider::PlayStore => Some("internal"),
        DistributionProvider::AppStore => Some("testflight"),
        DistributionProvider::MicrosoftStore => Some("private"),
        _ => None,
    }
}

fn release_default_locales(project_dir: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(project_dir.join("fission.toml"))?;
    let value: toml::Value = toml::from_str(&text)?;
    Ok(value
        .get("release")
        .and_then(|release| release.get("default_locales"))
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default())
}

fn local_workspace_for_app(app_name: &str) -> Result<PathBuf> {
    let home = env::var_os("HOME").context("HOME is not set; cannot resolve ~/.fission")?;
    Ok(PathBuf::from(home)
        .join(".fission")
        .join(sanitize_workspace_name(app_name)))
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

fn render_dashboard(cx: &PublishContext, hooks: PublishShellHooks) -> Result<()> {
    println!("\x1b[2J\x1b[H");
    println!("Fission Local Publish");
    println!("─────────────────────");
    println!("App        {} ({})", cx.app_name, cx.app_id);
    println!(
        "Target     {} / {}",
        cx.target.as_str(),
        cx.provider.as_str()
    );
    println!("Format     {}", cx.format.as_str());
    println!("Track      {}", cx.track.as_deref().unwrap_or("not set"));
    println!(
        "Locales    {}",
        if cx.locales.is_empty() {
            "not set".to_string()
        } else {
            cx.locales.join(", ")
        }
    );
    println!("Workspace  {}", cx.workspace.display());
    println!("Artifact   {}", cx.artifact_manifest_path().display());
    println!("\nSecrets never written to fission.toml. Local release files belong under ~/.fission/<app-name>.");
    println!();

    let package = readiness_package(&cx.project_dir, Some(cx.target), Some(cx.format), true)
        .unwrap_or_else(|err| readiness_error("release.package.readiness_failed", err));
    let distribute_checks = match super::load_publish_manifest(&cx.project_dir) {
        Ok(config) => readiness_distribute(
            &cx.project_dir,
            cx.provider,
            &cx.site,
            cx.track.as_deref(),
            Some(cx.format),
            Some(&cx.artifact_manifest_path()),
            &config,
        )
        .unwrap_or_else(|err| readiness_error("release.distribution.readiness_failed", err)),
        Err(err) => readiness_error("release.distribution.config_failed", err),
    };
    render_check_group("Project/package preflight", &package);
    render_check_group("Provider/distribution preflight", &distribute_checks);
    let release_plan = (hooks.release_checks)(workflow_request(cx, None, true, true, true))
        .unwrap_or_else(|err| readiness_error("release.plan.readiness_failed", err));
    render_check_group("Release plan", &release_plan);
    if let Ok(plan) = (hooks.release_plan)(workflow_request(cx, None, true, true, true)) {
        render_release_plan_summary(&plan);
    }
    println!(
        "Overall   package={} provider={} release={}",
        report_status(&package),
        report_status(&distribute_checks),
        report_status(&release_plan)
    );
    Ok(())
}

fn release_plan_checks_via_current_fission_request(
    request: PublishShellWorkflowRequest,
) -> Result<Vec<ReadinessCheck>> {
    let plan = release_plan_json_via_current_fission_request(request)?;
    Ok(plan
        .requirements
        .into_iter()
        .map(release_requirement_check)
        .collect())
}

fn release_plan_via_current_fission_request(
    request: PublishShellWorkflowRequest,
) -> Result<PublishShellReleasePlan> {
    release_plan_json_via_current_fission_request(request).map(publish_shell_release_plan)
}

fn release_plan_json_via_current_fission_request(
    request: PublishShellWorkflowRequest,
) -> Result<ReleasePlanJson> {
    let exe = env::current_exe().context("failed to resolve current fission executable")?;
    let mut args = vec![
        "readiness".to_string(),
        "release".to_string(),
        "--target".to_string(),
        request
            .target
            .context("release readiness requires a target")?
            .as_str()
            .to_string(),
        "--format".to_string(),
        request
            .format
            .context("release readiness requires a package format")?
            .as_str()
            .to_string(),
        "--provider".to_string(),
        request.provider.as_str().to_string(),
        "--site".to_string(),
        request.site,
        "--project-dir".to_string(),
        request.project_dir.display().to_string(),
        "--json".to_string(),
    ];
    if let Some(track) = request
        .track
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        args.push("--track".to_string());
        args.push(track.clone());
    }
    if let Some(artifact) = request.artifact.as_ref() {
        args.push("--artifact".to_string());
        args.push(artifact.display().to_string());
    }

    let output = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to run release readiness")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("release readiness produced no JSON output: {stderr}");
    }
    let plan: ReleasePlanJson = serde_json::from_str(&stdout)
        .with_context(|| format!("failed to parse release readiness JSON: {stdout}"))?;
    Ok(plan)
}

fn workflow_request(
    cx: &PublishContext,
    deploy: Option<String>,
    dry_run: bool,
    yes: bool,
    json: bool,
) -> PublishShellWorkflowRequest {
    PublishShellWorkflowRequest {
        project_dir: cx.project_dir.clone(),
        provider: cx.provider,
        target: Some(cx.target),
        format: Some(cx.format),
        artifact: cx.artifact.clone(),
        site: cx.site.clone(),
        deploy,
        track: cx.track.clone(),
        locales: cx.locales.clone(),
        overwrite_remote: cx.overwrite_remote,
        dry_run,
        yes,
        json,
    }
}

fn release_requirement_check(requirement: ReleaseRequirementJson) -> ReadinessCheck {
    ReadinessCheck {
        id: requirement.id,
        severity: match requirement.level.as_str() {
            "provider-required" => super::CheckSeverity::Error,
            "fission-recommended" => super::CheckSeverity::Warning,
            _ => super::CheckSeverity::Info,
        },
        status: match requirement.status.as_str() {
            "passed" => CheckStatus::Passed,
            "missing" => CheckStatus::Missing,
            "failed" => CheckStatus::Failed,
            "warning" => CheckStatus::Warning,
            "skipped" => CheckStatus::Skipped,
            _ => CheckStatus::Warning,
        },
        summary: requirement.summary,
        details: requirement.details,
        remediation: requirement.remediation,
    }
}

fn publish_shell_release_plan(plan: ReleasePlanJson) -> PublishShellReleasePlan {
    PublishShellReleasePlan {
        status: plan.status,
        provider: plan.provider,
        target: plan.target,
        format: plan.format,
        track: plan.track,
        locales: plan.locales,
        steps: plan
            .steps
            .into_iter()
            .map(|step| PublishShellReleaseStep {
                id: step.id,
                title: step.title,
                status: step.status,
            })
            .collect(),
        capabilities: plan
            .capabilities
            .into_iter()
            .map(|capability| PublishShellProviderCapability {
                id: capability.id,
                status: capability.status,
                summary: capability.summary,
            })
            .collect(),
        requirements: plan
            .requirements
            .into_iter()
            .map(release_requirement_check)
            .collect(),
    }
}

fn render_release_plan_summary(plan: &PublishShellReleasePlan) {
    println!();
    println!("Release plan details");
    println!("  status    {}", empty_dash(&plan.status));
    println!("  provider  {}", empty_dash(&plan.provider));
    println!(
        "  target    {} / {}",
        plan.target.as_deref().unwrap_or("-"),
        plan.format.as_deref().unwrap_or("-")
    );
    println!("  track     {}", plan.track.as_deref().unwrap_or("-"));
    println!(
        "  locales   {}",
        if plan.locales.is_empty() {
            "-".to_string()
        } else {
            plan.locales.join(", ")
        }
    );
    println!("  steps");
    for step in plan.steps.iter().take(7) {
        println!("    {:<18} {:<10} {}", step.id, step.status, step.title);
    }
    if !plan.capabilities.is_empty() {
        println!("  capabilities");
        for capability in plan.capabilities.iter().take(5) {
            println!(
                "    {:<18} {:<14} {}",
                capability.id, capability.status, capability.summary
            );
        }
    }
}

fn empty_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "-"
    } else {
        value
    }
}

fn current_fission_hooks() -> PublishShellHooks {
    PublishShellHooks {
        release_plan: release_plan_via_current_fission_request,
        release_checks: release_plan_checks_via_current_fission_request,
        publish: publish_via_current_fission_request,
        skip_requirement: skip_requirement_via_current_fission_request,
        bump_build: bump_build_via_current_fission_request,
    }
}

fn publish_via_current_fission_request(request: PublishShellWorkflowRequest) -> Result<String> {
    let label = if request.dry_run {
        "Running publish dry-run"
    } else {
        "Publishing artifact"
    };
    run_current_fission_logged(publish_command_args_from_request(request), label)?;
    Ok("publish command completed".to_string())
}

fn skip_requirement_via_current_fission_request(
    request: PublishShellWorkflowRequest,
    id: String,
) -> Result<String> {
    let args = vec![
        "release-config".to_string(),
        "skip-requirement".to_string(),
        "--project-dir".to_string(),
        request.project_dir.display().to_string(),
        "--id".to_string(),
        id.clone(),
        "--yes".to_string(),
    ];
    run_current_fission_logged(args, "Recording release skip")?;
    Ok(format!("recorded explicit skip for {id}"))
}

fn bump_build_via_current_fission_request(request: PublishShellWorkflowRequest) -> Result<String> {
    let mut args = vec![
        "release-config".to_string(),
        "bump-build".to_string(),
        "--project-dir".to_string(),
        request.project_dir.display().to_string(),
        "--yes".to_string(),
    ];
    if let Some(target) = request.target {
        args.push("--target".to_string());
        args.push(target.as_str().to_string());
    }
    run_current_fission_logged(args, "Bumping release build")?;
    Ok("bumped release build".to_string())
}

fn readiness_error(id: &str, err: anyhow::Error) -> Vec<ReadinessCheck> {
    vec![ReadinessCheck {
        id: id.to_string(),
        severity: super::CheckSeverity::Error,
        status: CheckStatus::Failed,
        summary: "readiness check could not complete".to_string(),
        details: Some(err.to_string()),
        remediation: vec![
            "Fix the configuration error above, then refresh this publish flow.".to_string(),
        ],
    }]
}

fn render_check_group(title: &str, checks: &[ReadinessCheck]) {
    println!("{title}");
    for check in checks {
        let mark = match check.status {
            CheckStatus::Passed => "✓",
            CheckStatus::Warning => "!",
            CheckStatus::Missing | CheckStatus::Failed => "✕",
            CheckStatus::Skipped => "·",
        };
        let detail = check
            .details
            .as_deref()
            .map(|value| format!(" — {value}"))
            .unwrap_or_default();
        println!("  {mark} {}{}", check.summary, detail);
        if check.status != CheckStatus::Passed {
            for fix in check.remediation.iter().take(1) {
                println!("    fix: {fix}");
            }
        }
    }
    println!();
}

fn ensure_local_workspace(cx: &PublishContext) -> Result<()> {
    ensure_local_workspace_files(cx)?;
    println!("Local workspace ready: {}", cx.workspace.display());
    println!(
        "Local env file: {}",
        cx.workspace.join("release.env").display()
    );
    pause("Press Enter to continue.");
    Ok(())
}

fn ensure_local_workspace_files(cx: &PublishContext) -> Result<()> {
    if let Some(parent) = cx.workspace.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        set_private_dir_permissions(parent)?;
    }
    fs::create_dir_all(&cx.workspace)
        .with_context(|| format!("failed to create {}", cx.workspace.display()))?;
    set_private_dir_permissions(&cx.workspace)?;
    let env_path = cx.workspace.join("release.env");
    if !env_path.exists() {
        write_env_file(&env_path, &[])?;
    } else {
        set_private_file_permissions(&env_path)?;
    }
    Ok(())
}

fn configure_provider_and_signing(cx: &mut PublishContext) -> Result<()> {
    ensure_local_workspace(cx)?;
    match cx.provider {
        DistributionProvider::PlayStore => configure_android_play(cx),
        DistributionProvider::AppStore => configure_ios_app_store(cx),
        DistributionProvider::MicrosoftStore => configure_windows_store(cx),
        DistributionProvider::S3 => configure_s3(cx),
        _ => configure_generic_provider(cx),
    }
}

fn configure_android_play(cx: &mut PublishContext) -> Result<()> {
    println!("\nConnect Google Play Console");
    println!("1. Open Google Play Console -> Setup -> API access");
    println!("2. Link or create a Google Cloud project");
    println!("3. Enable the Android Publisher API");
    println!("4. Create a service account");
    println!("5. Grant app access for package {}", cx.app_id);
    println!("6. Download the JSON key and keep it outside the repository");
    println!("Accepted local env: GOOGLE_APPLICATION_CREDENTIALS");
    println!("Accepted CI env: PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64");
    if confirm("Enter path to Play service-account JSON now?", true)? {
        let selected = select_file(&cx.project_dir, Some("json"))?;
        let chosen = handle_selected_file(
            &selected,
            &cx.workspace,
            "play-service-account.json",
            &cx.project_dir,
        )?;
        upsert_env(
            &cx.workspace.join("release.env"),
            "GOOGLE_APPLICATION_CREDENTIALS",
            &chosen.display().to_string(),
        )?;
        env::set_var("GOOGLE_APPLICATION_CREDENTIALS", &chosen);
    }

    println!("\nAndroid upload signing");
    println!("Use an existing JKS or generate a new upload key. Fission only asks for the keystore password.");
    println!("  1  Use existing JKS");
    println!("  2  Generate upload key");
    println!("  s  Skip");
    match prompt("Choose signing action")?.trim() {
        "1" => {
            let selected = select_file(&cx.project_dir, Some("jks"))?;
            let chosen =
                handle_selected_file(&selected, &cx.workspace, "upload-key.jks", &cx.project_dir)?;
            let alias = prompt_default("Keystore alias", &default_android_alias(cx))?;
            upsert_env(
                &cx.workspace.join("release.env"),
                "ANDROID_KEYSTORE",
                &chosen.display().to_string(),
            )?;
            upsert_env(
                &cx.workspace.join("release.env"),
                "ANDROID_KEYSTORE_ALIAS",
                &alias,
            )?;
            env::set_var("ANDROID_KEYSTORE", &chosen);
            env::set_var("ANDROID_KEYSTORE_ALIAS", &alias);
            configure_android_passwords(cx)?;
        }
        "2" => generate_android_jks(cx)?,
        _ => {}
    }
    Ok(())
}

fn configure_android_passwords(cx: &PublishContext) -> Result<()> {
    let password = prompt_password_confirmed("Android keystore password")?;
    let env_path = cx.workspace.join("release.env");
    upsert_env(&env_path, "ANDROID_KEYSTORE_PASSWORD", &password)?;
    upsert_env(&env_path, "ANDROID_KEY_PASSWORD", &password)?;
    env::set_var("ANDROID_KEYSTORE_PASSWORD", &password);
    env::set_var("ANDROID_KEY_PASSWORD", &password);
    Ok(())
}

fn generate_android_jks(cx: &PublishContext) -> Result<()> {
    let destination = cx.workspace.join("upload-key.jks");
    if destination.exists() && !confirm("upload-key.jks already exists. Replace it?", false)? {
        return Ok(());
    }
    let password = prompt_password_confirmed("New Android upload-key password")?;
    let alias = default_android_alias(cx);
    let mut command = Command::new("keytool");
    command
        .arg("-genkeypair")
        .arg("-v")
        .arg("-keystore")
        .arg(&destination)
        .arg("-storepass")
        .arg(&password)
        .arg("-keypass")
        .arg(&password)
        .arg("-alias")
        .arg(&alias)
        .arg("-keyalg")
        .arg("RSA")
        .arg("-keysize")
        .arg("2048")
        .arg("-validity")
        .arg("9125")
        .arg("-dname")
        .arg(format!(
            "CN={}, OU=Fission Local Publish, O=Fission, L=Local, ST=Local, C=US",
            cx.app_name
        ));
    run_logged_command(
        &mut command,
        &format!("Generating Android upload key at {}", destination.display()),
    )
    .context("failed to run keytool; install a JDK to generate Android upload keys")?;
    set_private_file_permissions(&destination)?;
    let env_path = cx.workspace.join("release.env");
    upsert_env(
        &env_path,
        "ANDROID_KEYSTORE",
        &destination.display().to_string(),
    )?;
    upsert_env(&env_path, "ANDROID_KEYSTORE_ALIAS", &alias)?;
    upsert_env(&env_path, "ANDROID_KEYSTORE_PASSWORD", &password)?;
    upsert_env(&env_path, "ANDROID_KEY_PASSWORD", &password)?;
    env::set_var("ANDROID_KEYSTORE", &destination);
    env::set_var("ANDROID_KEYSTORE_ALIAS", &alias);
    env::set_var("ANDROID_KEYSTORE_PASSWORD", &password);
    env::set_var("ANDROID_KEY_PASSWORD", &password);
    println!("Generated {}", destination.display());
    pause("Press Enter to continue.");
    Ok(())
}

fn default_android_alias(cx: &PublishContext) -> String {
    sanitize_workspace_name(&cx.app_name).replace('.', "-")
}

fn configure_ios_app_store(cx: &PublishContext) -> Result<()> {
    println!("\nConnect App Store Connect");
    println!("1. App Store Connect -> Users and Access -> Integrations -> App Store Connect API");
    println!("2. Create an API key with App Manager or equivalent role");
    println!("3. Download the .p8 key once");
    println!("4. Capture Key ID and Issuer ID");
    println!("Accepted local env: APP_STORE_CONNECT_API_KEY_PATH, APP_STORE_CONNECT_KEY_ID, APP_STORE_CONNECT_ISSUER_ID");
    println!("Accepted CI env: APP_STORE_CONNECT_API_KEY_BASE64");
    if confirm("Enter path to App Store Connect .p8 key now?", true)? {
        let selected = select_file(&cx.project_dir, Some("p8"))?;
        let selected_name = selected
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("AuthKey.p8")
            .to_string();
        let chosen = handle_selected_file(
            &selected,
            &cx.workspace.join("ios"),
            &selected_name,
            &cx.project_dir,
        )?;
        upsert_env(
            &cx.workspace.join("release.env"),
            "APP_STORE_CONNECT_API_KEY_PATH",
            &chosen.display().to_string(),
        )?;
        env::set_var("APP_STORE_CONNECT_API_KEY_PATH", &chosen);
    }
    let key_id = prompt_optional("App Store Connect key id")?;
    let issuer = prompt_optional("App Store Connect issuer id")?;
    if let Some(value) = key_id {
        upsert_env(
            &cx.workspace.join("release.env"),
            "APP_STORE_CONNECT_KEY_ID",
            &value,
        )?;
        env::set_var("APP_STORE_CONNECT_KEY_ID", value);
    }
    if let Some(value) = issuer {
        upsert_env(
            &cx.workspace.join("release.env"),
            "APP_STORE_CONNECT_ISSUER_ID",
            &value,
        )?;
        env::set_var("APP_STORE_CONNECT_ISSUER_ID", value);
    }
    Ok(())
}

fn configure_windows_store(cx: &PublishContext) -> Result<()> {
    println!("\nMicrosoft Store / Partner Center credentials");
    println!("MSIX publishing uses the Microsoft Store Developer CLI (`msstore`).");
    println!(
        "Run `msstore reconfigure` or the equivalent msstore sign-in flow before publishing MSIX."
    );
    println!("EXE/MSI submissions use Partner Center API credentials instead.");
    println!(
        "Accepted EXE/MSI env: AZURE_TENANT_ID, AZURE_CLIENT_ID, MICROSOFT_STORE_CLIENT_SECRET"
    );
    println!("Signing accepts WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_BASE64, WINDOWS_CERTIFICATE_PASSWORD, WINDOWS_CERTIFICATE_THUMBPRINT");
    if confirm("Enter path to a PFX signing certificate now?", false)? {
        let selected = select_file(&cx.project_dir, Some("pfx"))?;
        let selected_name = selected
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("signing.pfx")
            .to_string();
        let chosen = handle_selected_file(
            &selected,
            &cx.workspace.join("windows"),
            &selected_name,
            &cx.project_dir,
        )?;
        upsert_env(
            &cx.workspace.join("release.env"),
            "WINDOWS_CERTIFICATE",
            &chosen.display().to_string(),
        )?;
        env::set_var("WINDOWS_CERTIFICATE", &chosen);
        let password = prompt_password_confirmed("Windows certificate password")?;
        upsert_env(
            &cx.workspace.join("release.env"),
            "WINDOWS_CERTIFICATE_PASSWORD",
            &password,
        )?;
        env::set_var("WINDOWS_CERTIFICATE_PASSWORD", password);
    }
    if cx.format != PackageFormat::Msix
        && confirm(
            "Configure Partner Center API credentials for EXE/MSI submission?",
            false,
        )?
    {
        for key in ["AZURE_TENANT_ID", "AZURE_CLIENT_ID"] {
            if let Some(value) = prompt_optional(key)? {
                upsert_env(&cx.workspace.join("release.env"), key, &value)?;
                env::set_var(key, value);
            }
        }
        if confirm("Enter Partner Center client secret now?", false)? {
            let secret = prompt_password_confirmed("Partner Center client secret")?;
            upsert_env(
                &cx.workspace.join("release.env"),
                "MICROSOFT_STORE_CLIENT_SECRET",
                &secret,
            )?;
            env::set_var("MICROSOFT_STORE_CLIENT_SECRET", secret);
        }
    }
    Ok(())
}

fn configure_s3(cx: &PublishContext) -> Result<()> {
    println!("\nS3 credential mode");
    println!("Use AWS_PROFILE for local development where possible. CI should use OIDC/web identity or secret env vars.");
    println!("Accepted env: AWS_PROFILE, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_SESSION_TOKEN, AWS_WEB_IDENTITY_TOKEN_FILE, AWS_REGION, AWS_ENDPOINT_URL_S3");
    if let Some(profile) = prompt_optional("AWS_PROFILE")? {
        upsert_env(&cx.workspace.join("release.env"), "AWS_PROFILE", &profile)?;
        env::set_var("AWS_PROFILE", profile);
    }
    if let Some(region) = prompt_optional("AWS_REGION")? {
        upsert_env(&cx.workspace.join("release.env"), "AWS_REGION", &region)?;
        env::set_var("AWS_REGION", region);
    }
    if let Some(endpoint) = prompt_optional("AWS_ENDPOINT_URL_S3 for S3-compatible storage")? {
        upsert_env(
            &cx.workspace.join("release.env"),
            "AWS_ENDPOINT_URL_S3",
            &endpoint,
        )?;
        env::set_var("AWS_ENDPOINT_URL_S3", endpoint);
    }
    if confirm(
        "Enter static AWS access keys for this local workspace?",
        false,
    )? {
        if let Some(access_key) = prompt_optional("AWS_ACCESS_KEY_ID")? {
            upsert_env(
                &cx.workspace.join("release.env"),
                "AWS_ACCESS_KEY_ID",
                &access_key,
            )?;
            env::set_var("AWS_ACCESS_KEY_ID", access_key);
        }
        let secret_key = prompt_password_confirmed("AWS_SECRET_ACCESS_KEY")?;
        upsert_env(
            &cx.workspace.join("release.env"),
            "AWS_SECRET_ACCESS_KEY",
            &secret_key,
        )?;
        env::set_var("AWS_SECRET_ACCESS_KEY", secret_key);
        if let Some(session) = prompt_optional("AWS_SESSION_TOKEN")? {
            upsert_env(
                &cx.workspace.join("release.env"),
                "AWS_SESSION_TOKEN",
                &session,
            )?;
            env::set_var("AWS_SESSION_TOKEN", session);
        }
    }
    Ok(())
}

fn configure_generic_provider(cx: &PublishContext) -> Result<()> {
    println!(
        "Provider-specific interactive setup is not specialized yet for {}.",
        cx.provider.as_str()
    );
    println!("Use the readiness fixes shown above; Fission will still use env/provider-owned credentials only.");
    pause("Press Enter to continue.");
    Ok(())
}

fn configure_release_options(cx: &mut PublishContext) -> Result<()> {
    if let Some(default) = default_track_for_provider(cx.provider) {
        let value = prompt_default("Track/channel", cx.track.as_deref().unwrap_or(default))?;
        cx.track = (!value.trim().is_empty()).then_some(value);
    }
    let locale_default = if cx.locales.is_empty() {
        "pl-PL,en-US".to_string()
    } else {
        cx.locales.join(",")
    };
    let locales = prompt_default("Locales, comma-separated", &locale_default)?;
    cx.locales = locales
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    if confirm(
        "Reuse an existing artifact manifest instead of building a new one?",
        false,
    )? {
        let selected = select_file(&cx.project_dir.join("target/fission"), Some("json"))?;
        cx.artifact = Some(selected);
    }
    Ok(())
}

fn review_release_metadata_content(cx: &PublishContext, hooks: PublishShellHooks) -> Result<()> {
    println!("\nRelease metadata and content");
    println!("Provider: {}", cx.provider.as_str());
    println!("Track: {}", cx.track.as_deref().unwrap_or("not set"));
    println!(
        "Locales: {}",
        if cx.locales.is_empty() {
            "not set".to_string()
        } else {
            cx.locales.join(", ")
        }
    );
    println!();

    let checks = (hooks.release_checks)(workflow_request(cx, None, true, true, false))?;
    let relevant = checks
        .iter()
        .filter(|check| {
            check.id.starts_with("release_config.") || check.id.starts_with("release_content.")
        })
        .cloned()
        .collect::<Vec<_>>();
    if relevant.is_empty() {
        println!("No release metadata/content checks are available for this provider yet.");
        pause("Press Enter to continue.");
        return Ok(());
    }

    render_check_group("Release metadata/content checks", &relevant);
    println!("Helpful commands");
    println!(
        "  fission release-config edit-file --project-dir {} --release <release-id> --kind notes --locale <locale>",
        cx.project_dir.display()
    );
    println!(
        "  fission release-content capture --project-dir {} --target {} --set {}",
        cx.project_dir.display(),
        cx.target.as_str(),
        cx.provider.as_str()
    );
    println!(
        "  fission release-content render --project-dir {} --provider {}",
        cx.project_dir.display(),
        cx.provider.as_str()
    );
    println!(
        "  fission release-content validate --project-dir {} --provider {}",
        cx.project_dir.display(),
        cx.provider.as_str()
    );
    println!();

    let skippable = relevant
        .iter()
        .filter(|check| {
            matches!(
                check.status,
                CheckStatus::Missing | CheckStatus::Failed | CheckStatus::Warning
            ) && check.severity != super::CheckSeverity::Error
        })
        .collect::<Vec<_>>();
    if skippable.is_empty() {
        pause("Release metadata/content checks do not have skippable warnings. Press Enter to continue.");
        return Ok(());
    }

    println!(
        "Provider-required items are not skippable. Recommended items can be explicitly skipped."
    );
    for check in skippable {
        println!();
        println!("{} — {}", check.id, check.summary);
        if let Some(details) = &check.details {
            println!("  {details}");
        }
        for fix in &check.remediation {
            println!("  fix: {fix}");
        }
        if confirm("Add this id to [release].skip_requirements?", false)? {
            let output = (hooks.skip_requirement)(
                workflow_request(cx, None, true, true, false),
                check.id.clone(),
            )?;
            if !output.trim().is_empty() {
                println!("{output}");
            }
        }
    }
    pause("Release metadata/content review finished. Press Enter to continue.");
    Ok(())
}

fn package_artifact(
    cx: &mut PublishContext,
    hooks: PublishShellHooks,
    pause_after: bool,
    offer_interactive_fixes: bool,
) -> Result<()> {
    if offer_interactive_fixes {
        offer_build_bump_if_needed(cx, hooks)?;
    }
    println!("Building release package...");
    let artifact = super::package_silent(super::PackageOptions {
        project_dir: cx.project_dir.clone(),
        target: cx.target,
        format: cx.format,
        release: true,
        variant: None,
        json: false,
    })?;
    println!("Artifact manifest: {}", artifact.display());
    cx.artifact = Some(artifact);
    if pause_after {
        pause("Package step finished. Press Enter to continue.");
    }
    Ok(())
}

fn offer_build_bump_if_needed(cx: &mut PublishContext, hooks: PublishShellHooks) -> Result<()> {
    if cx.provider != DistributionProvider::PlayStore {
        return Ok(());
    }
    let checks = (hooks.release_checks)(workflow_request(cx, None, true, true, false))?;
    let Some(check) = checks.iter().find(|check| {
        check.id == "release.play_store.version_code_available"
            && check.status == CheckStatus::Failed
    }) else {
        return Ok(());
    };
    println!("\nGoogle Play version/build issue");
    println!("{}", check.summary);
    if let Some(details) = &check.details {
        println!("{details}");
    }
    for fix in &check.remediation {
        println!("fix: {fix}");
    }
    if confirm(
        "Increment the Android build/version code before packaging?",
        true,
    )? {
        let output = (hooks.bump_build)(workflow_request(cx, None, true, true, false))?;
        if !output.trim().is_empty() {
            println!("{output}");
        }
        cx.artifact = None;
    }
    Ok(())
}

fn dry_run_publish(
    cx: &PublishContext,
    options: &PublishShellOptions,
    hooks: PublishShellHooks,
    pause_after: bool,
) -> Result<()> {
    let output = (hooks.publish)(workflow_request(
        cx,
        options.deploy.clone(),
        true,
        true,
        false,
    ))?;
    if !output.trim().is_empty() {
        println!("{output}");
    }
    if pause_after {
        pause("Dry run finished. Press Enter to continue.");
    }
    Ok(())
}

fn publish_artifact_interactive(
    cx: &PublishContext,
    options: &PublishShellOptions,
    hooks: PublishShellHooks,
) -> Result<()> {
    println!("\nFinal publish confirmation");
    println!("Target: {} / {}", cx.target.as_str(), cx.provider.as_str());
    println!("Track: {}", cx.track.as_deref().unwrap_or("not set"));
    println!("Artifact: {}", cx.artifact_manifest_path().display());
    println!("Type the package/app id to publish: {}", cx.app_id);
    let typed = prompt("Confirm package/app id")?;
    if typed.trim() != cx.app_id {
        bail!(
            "confirmation did not match {}; publish cancelled",
            cx.app_id
        );
    }
    publish_artifact(cx, options, hooks)
}

fn publish_artifact(
    cx: &PublishContext,
    options: &PublishShellOptions,
    hooks: PublishShellHooks,
) -> Result<()> {
    let output = (hooks.publish)(workflow_request(
        cx,
        options.deploy.clone(),
        false,
        true,
        false,
    ))?;
    if !output.trim().is_empty() {
        println!("{output}");
    }
    Ok(())
}

fn publish_command_args_from_request(request: PublishShellWorkflowRequest) -> Vec<String> {
    let mut args = vec![
        "publish".to_string(),
        "--provider".to_string(),
        request.provider.as_str().to_string(),
        "--site".to_string(),
        request.site,
        "--project-dir".to_string(),
        request.project_dir.display().to_string(),
    ];
    if request.yes {
        args.push("--yes".to_string());
    }
    if let Some(target) = request.target {
        args.push("--target".to_string());
        args.push(target.as_str().to_string());
    }
    if let Some(format) = request.format {
        args.push("--format".to_string());
        args.push(format.as_str().to_string());
    }
    if let Some(artifact) = request.artifact {
        args.push("--artifact".to_string());
        args.push(artifact.display().to_string());
    }
    if let Some(track) = request
        .track
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        args.push("--track".to_string());
        args.push(track.clone());
    }
    for locale in request.locales {
        args.push("--locale".to_string());
        args.push(locale);
    }
    if let Some(deploy) = request
        .deploy
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        args.push("--deploy".to_string());
        args.push(deploy.clone());
    }
    if request.dry_run {
        args.push("--dry-run".to_string());
    }
    if request.overwrite_remote {
        args.push("--overwrite-remote".to_string());
    }
    if request.json {
        args.push("--json".to_string());
    }
    args
}

fn run_current_fission_logged(args: Vec<String>, label: &str) -> Result<()> {
    let exe = env::current_exe().context("failed to resolve current fission executable")?;
    let mut command = Command::new(exe);
    command.args(args);
    run_logged_command(&mut command, label)
}

fn run_logged_command(command: &mut Command, label: &str) -> Result<()> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start {label}"))?;
    let stdout = child.stdout.take().context("failed to capture stdout")?;
    let stderr = child.stderr.take().context("failed to capture stderr")?;
    let (tx, rx) = mpsc::channel::<String>();
    spawn_log_reader(stdout, tx.clone());
    spawn_log_reader(stderr, tx);

    let interactive = io::stdout().is_terminal();
    let started = Instant::now();
    let mut tick = 0usize;
    let mut rendered = false;
    let mut tail = VecDeque::new();

    loop {
        while let Ok(line) = rx.try_recv() {
            record_log_line(&mut tail, line, interactive);
        }

        if let Some(status) = child.try_wait()? {
            while let Ok(line) = rx.try_recv() {
                record_log_line(&mut tail, line, interactive);
            }
            if interactive {
                render_log_window(
                    label,
                    &tail,
                    started,
                    tick,
                    Some(status.success()),
                    &mut rendered,
                )?;
            }
            if status.success() {
                if !interactive {
                    println!("{label}: done");
                }
                return Ok(());
            }
            let detail = tail.iter().cloned().collect::<Vec<_>>().join("\n");
            if detail.trim().is_empty() {
                bail!("{label} failed with {status}");
            }
            bail!("{label} failed with {status}\n{detail}");
        }

        match rx.recv_timeout(Duration::from_millis(120)) {
            Ok(line) => record_log_line(&mut tail, line, interactive),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }

        if interactive {
            render_log_window(label, &tail, started, tick, None, &mut rendered)?;
        }
        tick = tick.wrapping_add(1);
    }
}

fn spawn_log_reader<R>(reader: R, tx: mpsc::Sender<String>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let line = line.unwrap_or_else(|err| format!("failed to read process output: {err}"));
            let _ = tx.send(line.replace('\r', ""));
        }
    });
}

fn push_log_line(tail: &mut VecDeque<String>, line: String) {
    tail.push_back(line);
    while tail.len() > GUIDED_LOG_LINES - 1 {
        tail.pop_front();
    }
}

fn record_log_line(tail: &mut VecDeque<String>, line: String, interactive: bool) {
    let line = super::redact_sensitive_text(&line);
    if !interactive {
        println!("{line}");
    }
    push_log_line(tail, line);
}

fn render_log_window(
    label: &str,
    tail: &VecDeque<String>,
    started: Instant,
    tick: usize,
    done: Option<bool>,
    rendered: &mut bool,
) -> Result<()> {
    let mut stdout = io::stdout();
    if *rendered {
        write!(stdout, "\x1b[{GUIDED_LOG_LINES}F")?;
    }
    let status = match done {
        Some(true) => "done",
        Some(false) => "failed",
        None => "running",
    };
    let spinner = match done {
        Some(true) => "OK",
        Some(false) => "XX",
        None => ["|", "/", "-", "\\"][tick % 4],
    };
    let header = format!(
        "{spinner} {label} {} {status} {}s",
        progress_bar(tick, done),
        started.elapsed().as_secs()
    );
    let mut lines = Vec::with_capacity(GUIDED_LOG_LINES);
    lines.push(header);
    lines.extend(tail.iter().cloned());
    while lines.len() < GUIDED_LOG_LINES {
        lines.push(String::new());
    }
    let width = terminal_width();
    for line in lines.into_iter().take(GUIDED_LOG_LINES) {
        writeln!(stdout, "\x1b[K{}", fit_terminal_line(&line, width))?;
    }
    stdout.flush()?;
    *rendered = true;
    Ok(())
}

fn progress_bar(tick: usize, done: Option<bool>) -> String {
    let width = 18usize;
    if let Some(true) = done {
        return format!("[{}]", "=".repeat(width));
    }
    if let Some(false) = done {
        return format!("[{}]", "!".repeat(width));
    }
    let mut cells = vec![' '; width];
    let start = tick % (width - 3);
    for cell in cells.iter_mut().skip(start).take(3) {
        *cell = '=';
    }
    format!("[{}]", cells.into_iter().collect::<String>())
}

fn terminal_width() -> usize {
    env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 24)
        .unwrap_or(120)
}

fn fit_terminal_line(line: &str, width: usize) -> String {
    let max = width.saturating_sub(1).max(20);
    let mut out = String::new();
    for (idx, ch) in line.chars().enumerate() {
        if idx + 3 >= max {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

fn export_ci_checklist(cx: &PublishContext) {
    println!(
        "\nCI checklist for {} / {}",
        cx.target.as_str(),
        cx.provider.as_str()
    );
    println!("Set non-secret config in fission.toml. Store these as CI secrets or provider variables; do not commit values.");
    match cx.provider {
        DistributionProvider::PlayStore => {
            println!(
                "ANDROID_KEYSTORE_BASE64=$(base64 -i ~/.fission/{}/upload-key.jks)",
                sanitize_workspace_name(&cx.app_name)
            );
            println!("ANDROID_KEYSTORE_PASSWORD=<secret>");
            println!("ANDROID_KEYSTORE_ALIAS={}", default_android_alias(cx));
            println!("ANDROID_KEY_PASSWORD=<secret>");
            println!(
                "PLAY_STORE_SERVICE_ACCOUNT_JSON_BASE64=$(base64 -i ~/.fission/{}/play-service-account.json)",
                sanitize_workspace_name(&cx.app_name)
            );
        }
        DistributionProvider::AppStore => {
            println!("APP_STORE_CONNECT_API_KEY_BASE64=<base64 .p8 key>");
            println!("APP_STORE_CONNECT_KEY_ID=<key id>");
            println!("APP_STORE_CONNECT_ISSUER_ID=<issuer id>");
        }
        DistributionProvider::MicrosoftStore => {
            println!("WINDOWS_CERTIFICATE_BASE64=<base64 pfx>");
            println!("WINDOWS_CERTIFICATE_PASSWORD=<secret>");
            println!("AZURE_TENANT_ID=<tenant id>");
            println!("AZURE_CLIENT_ID=<client id>");
            println!("MICROSOFT_STORE_CLIENT_SECRET=<secret>");
        }
        DistributionProvider::S3 => {
            println!("AWS_PROFILE=<local only, or omit in CI>");
            println!("AWS_REGION=<region>");
            println!("AWS_WEB_IDENTITY_TOKEN_FILE=<oidc token file> # preferred CI mode");
            println!("AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY=<fallback secrets>");
        }
        _ => println!(
            "Use provider-specific env vars shown by fission auth setup --provider {}",
            cx.provider.as_str()
        ),
    }
    println!(
        "Track/channel: {}",
        cx.track.as_deref().unwrap_or("not set")
    );
    println!(
        "Locales: {}",
        if cx.locales.is_empty() {
            "from fission.toml".to_string()
        } else {
            cx.locales.join(",")
        }
    );
    println!("\nRecommended non-interactive CI commands:");
    println!(
        "fission readiness release --project-dir . --target {} --format {} --provider {}{} --json",
        cx.target.as_str(),
        cx.format.as_str(),
        cx.provider.as_str(),
        ci_track_locale_args(cx)
    );
    println!(
        "fission package --project-dir . --target {} --format {} --release --json",
        cx.target.as_str(),
        cx.format.as_str()
    );
    println!(
        "fission publish --project-dir . --provider {} --artifact target/fission/release/{}/{}/artifact-manifest.json{} --yes --json",
        cx.provider.as_str(),
        cx.target.as_str(),
        cx.format.as_str(),
        ci_track_locale_args(cx)
    );
    pause("Press Enter to continue.");
}

fn ci_track_locale_args(cx: &PublishContext) -> String {
    let mut args = String::new();
    if let Some(track) = cx.track.as_deref().filter(|track| !track.trim().is_empty()) {
        args.push_str(" --track ");
        args.push_str(track);
    }
    for locale in &cx.locales {
        args.push_str(" --locale ");
        args.push_str(locale);
    }
    args
}

fn select_file(start: &Path, extension: Option<&str>) -> Result<PathBuf> {
    let base = if start.exists() {
        if start.is_file() {
            start.parent().unwrap_or(Path::new("/")).to_path_buf()
        } else {
            start.to_path_buf()
        }
    } else {
        env::current_dir()?
    };
    loop {
        let expected = extension.map(|ext| format!(".{ext} ")).unwrap_or_default();
        println!(
            "\nEnter {}file path. Relative paths are resolved from the current directory first, then {}.",
            expected,
            base.display()
        );
        let choice = prompt("Path (q to cancel)")?;
        let choice = trim_path_input(&choice);
        if choice.eq_ignore_ascii_case("q") {
            bail!("file selection cancelled");
        }
        if choice.trim().is_empty() {
            println!("Path is required.");
            continue;
        }
        let path = resolve_input_file_path(choice, &base);
        if path.is_file() {
            validate_extension(&path, extension)?;
            return Ok(path);
        }
        println!("Not a file: {}", path.display());
    }
}

fn trim_path_input(input: &str) -> &str {
    let input = input.trim();
    if input.len() >= 2 {
        let bytes = input.as_bytes();
        if (bytes[0] == b'"' && bytes[input.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[input.len() - 1] == b'\'')
        {
            return &input[1..input.len() - 1];
        }
    }
    input
}

fn resolve_input_file_path(input: &str, base: &Path) -> PathBuf {
    let path = expand_home_path(input);
    if path.is_absolute() {
        return path;
    }
    let cwd_path = env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(&path);
    if cwd_path.is_file() {
        return cwd_path;
    }
    let base_path = base.join(path);
    if base_path.is_file() {
        return base_path;
    }
    cwd_path
}

fn expand_home_path(input: &str) -> PathBuf {
    if input == "~" {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(input));
    }
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(input)
}

fn validate_extension(path: &Path, extension: Option<&str>) -> Result<()> {
    if let Some(extension) = extension {
        let actual = path.extension().and_then(OsStr::to_str).unwrap_or_default();
        if !actual.eq_ignore_ascii_case(extension) {
            println!(
                "Warning: expected a .{extension} file, selected {}",
                path.display()
            );
            if !confirm("Use it anyway?", false)? {
                bail!("file selection cancelled");
            }
        }
    }
    Ok(())
}

fn handle_selected_file(
    selected: &Path,
    workspace: &Path,
    default_name: &str,
    project_dir: &Path,
) -> Result<PathBuf> {
    println!("Selected: {}", selected.display());
    println!("  1  Copy to {} (recommended)", workspace.display());
    println!("  2  Move to {}", workspace.display());
    println!("  3  Reference original path");
    match prompt_default("Action", "1")?.trim() {
        "2" => {
            fs::create_dir_all(workspace)?;
            set_private_dir_permissions(workspace)?;
            let dest = workspace.join(default_name);
            fs::rename(selected, &dest).with_context(|| {
                format!(
                    "failed to move {} to {}",
                    selected.display(),
                    dest.display()
                )
            })?;
            set_private_file_permissions(&dest)?;
            Ok(dest)
        }
        "3" => {
            if path_is_inside_project(selected, project_dir) {
                bail!(
                    "refusing to reference a secret file inside the project tree: {}; copy or move it to {} instead",
                    selected.display(),
                    workspace.display()
                );
            }
            Ok(selected.to_path_buf())
        }
        _ => {
            fs::create_dir_all(workspace)?;
            set_private_dir_permissions(workspace)?;
            let dest = workspace.join(default_name);
            fs::copy(selected, &dest).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    selected.display(),
                    dest.display()
                )
            })?;
            set_private_file_permissions(&dest)?;
            Ok(dest)
        }
    }
}

fn path_is_inside_project(path: &Path, project_dir: &Path) -> bool {
    let Ok(path) = fs::canonicalize(path) else {
        return false;
    };
    let Ok(project_dir) = fs::canonicalize(project_dir) else {
        return false;
    };
    path.starts_with(project_dir)
}

fn load_local_env(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some((key, value)) = line.split_once('=') {
            let value = unquote_env_value(value.trim());
            env::set_var(key.trim(), value);
        }
    }
    Ok(())
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim_end().to_string())
}

fn prompt_default(label: &str, default: &str) -> Result<String> {
    print!("{label} [{default}]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim_end();
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input.to_string())
    }
}

fn prompt_optional(label: &str) -> Result<Option<String>> {
    let value = prompt(&format!("{label} (leave blank to skip)"))?;
    Ok((!value.trim().is_empty()).then_some(value))
}

fn confirm(label: &str, default: bool) -> Result<bool> {
    let suffix = if default { "Y/n" } else { "y/N" };
    let value = prompt(&format!("{label} [{suffix}]"))?;
    if value.trim().is_empty() {
        return Ok(default);
    }
    Ok(matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn prompt_password_confirmed(label: &str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;
    let first = read_password()?;
    print!("Confirm {label}: ");
    io::stdout().flush()?;
    let second = read_password()?;
    if first != second {
        bail!("passwords did not match");
    }
    if first.is_empty() {
        bail!("password cannot be empty");
    }
    Ok(first)
}

fn pause(message: &str) {
    if io::stdin().is_terminal() {
        let _ = prompt(message);
    }
}

#[cfg(test)]
#[path = "publish_shell_tests.rs"]
mod tests;
