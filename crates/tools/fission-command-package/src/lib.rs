use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use fission_command_core::{
    normalize_windows_package_version, resolve_release_version_config,
    sync_resolved_release_platform_config, DistributionProvider, FissionProject, NativeVariant,
    Target,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod artifact;
mod distribution;
mod docker_registry;
mod files;
mod github_releases;
mod macos_notarization;
mod package;
mod publish_shell;
mod readiness;
mod static_hosts;
mod stores;
mod support;

use support::*;

use artifact::*;
use distribution::*;
use readiness::*;

const ARTIFACT_MANIFEST: &str = "artifact-manifest.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum PackageFormat {
    Aab,
    Apk,
    App,
    #[value(name = "docker-image")]
    DockerImage,
    Exe,
    Ipa,
    Msi,
    Msix,
    Pkg,
    Run,
    Static,
}

impl PackageFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aab => "aab",
            Self::Apk => "apk",
            Self::App => "app",
            Self::DockerImage => "docker-image",
            Self::Exe => "exe",
            Self::Ipa => "ipa",
            Self::Msi => "msi",
            Self::Msix => "msix",
            Self::Pkg => "pkg",
            Self::Run => "run",
            Self::Static => "static",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum DistributeAction {
    Setup,
    Publish,
    Status,
    Promote,
    Rollback,
}

impl DistributeAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Publish => "publish",
            Self::Status => "status",
            Self::Promote => "promote",
            Self::Rollback => "rollback",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ReadinessKind {
    Package,
    Distribute,
    Release,
}

#[derive(Clone, Debug)]
pub struct PackageOptions {
    pub project_dir: PathBuf,
    pub target: Target,
    pub format: PackageFormat,
    pub release: bool,
    pub variant: Option<NativeVariant>,
    pub json: bool,
}

#[derive(Clone, Debug)]
pub struct DistributeOptions {
    pub project_dir: PathBuf,
    pub provider: DistributionProvider,
    pub action: DistributeAction,
    pub target: Option<Target>,
    pub format: Option<PackageFormat>,
    pub artifact: Option<PathBuf>,
    pub site: String,
    pub deploy: Option<String>,
    pub track: Option<String>,
    pub locales: Vec<String>,
    pub dry_run: bool,
    pub yes: bool,
    pub json: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DistributionEvent {
    pub at_unix_seconds: u64,
    pub id: String,
    pub status: String,
    pub details: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DistributionPublishOutcome {
    pub receipt: Value,
    pub events: Vec<DistributionEvent>,
}

#[derive(Clone, Debug)]
pub struct ReadinessOptions {
    pub project_dir: PathBuf,
    pub kind: ReadinessKind,
    pub target: Option<Target>,
    pub format: Option<PackageFormat>,
    pub provider: Option<DistributionProvider>,
    pub artifact: Option<PathBuf>,
    pub site: String,
    pub track: Option<String>,
    pub release: bool,
    pub json: bool,
}

pub use publish_shell::{
    default_format_for_target_provider, default_target_for_provider, default_track_for_provider,
    ensure_publish_workspace, publish_flow_snapshot, run_publish_shell,
    run_publish_shell_with_hooks, PublishFlowSnapshot, PublishShellHooks, PublishShellOptions,
    PublishShellProviderCapability, PublishShellReleasePlan, PublishShellReleaseStep,
    PublishShellWorkflowRequest,
};

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactManifest {
    schema_version: u32,
    created_at_unix_seconds: u64,
    project: ArtifactProject,
    target: String,
    format: String,
    profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    variant: Option<String>,
    root_dir: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    source_config: Vec<ArtifactSourceConfig>,
    artifacts: Vec<ArtifactFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon_manifest: Option<ArtifactIconManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    signing: Option<ArtifactSigning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notarization: Option<Value>,
    validation: ArtifactValidation,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactProject {
    app_id: String,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    build: Option<u64>,
    version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactFile {
    kind: String,
    purpose: Option<String>,
    platform: Option<String>,
    upload_provider: Option<String>,
    path: String,
    relative_path: String,
    sha256: String,
    size_bytes: u64,
    mime_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactSourceConfig {
    kind: String,
    path: String,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactIconManifest {
    path: String,
    sha256: String,
    outputs: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactSigning {
    state: String,
    identity: Option<String>,
    certificate_sha256: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactValidation {
    state: String,
    checks: Vec<ReadinessCheck>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadinessCheck {
    pub id: String,
    pub severity: CheckSeverity,
    pub status: CheckStatus,
    pub summary: String,
    pub details: Option<String>,
    pub remediation: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Passed,
    Missing,
    Failed,
    Warning,
    Skipped,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReadinessReport {
    project_dir: String,
    target: Option<String>,
    format: Option<String>,
    provider: Option<String>,
    site: Option<String>,
    status: String,
    checks: Vec<ReadinessCheck>,
}

#[derive(Debug, Serialize)]
struct DistributionReceipt {
    schema_version: u32,
    created_at_unix_seconds: u64,
    provider: String,
    site: String,
    action: String,
    artifact_manifest: Option<String>,
    deployment_id: Option<String>,
    canonical_url: Option<String>,
    preview_url: Option<String>,
    custom_domain: Option<String>,
    status: String,
    stdout: Option<String>,
    stderr: Option<String>,
    manual_follow_up: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DistributionReceiptView<'a> {
    #[serde(flatten)]
    receipt: &'a DistributionReceipt,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(rename = "track_channel", skip_serializing_if = "Option::is_none")]
    track: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    locales: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact_manifest_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_content_manifest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_content_manifest_sha256: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    release_content_assets: Vec<Value>,
    uploaded_bytes: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    uploaded_assets: Vec<Value>,
}

#[derive(Debug, Deserialize, Default)]
struct PublishManifest {
    site: Option<SiteManifest>,
    distribution: Option<DistributionManifest>,
}

#[derive(Debug, Deserialize, Default)]
struct SiteManifest {
    entry: Option<String>,
    out_dir: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DistributionManifest {
    #[serde(default)]
    s3: BTreeMap<String, S3Config>,
    #[serde(default)]
    google_drive: BTreeMap<String, GoogleDriveConfig>,
    #[serde(default)]
    onedrive: BTreeMap<String, OneDriveConfig>,
    #[serde(default)]
    dropbox: BTreeMap<String, DropboxConfig>,
    play_store: Option<PlayStoreConfig>,
    app_store: Option<AppStoreConfig>,
    microsoft_store: Option<MicrosoftStoreConfig>,
    #[serde(default)]
    github_pages: BTreeMap<String, GithubPagesConfig>,
    #[serde(default)]
    github_releases: BTreeMap<String, GithubReleasesConfig>,
    #[serde(default)]
    cloudflare_pages: BTreeMap<String, CloudflarePagesConfig>,
    #[serde(default)]
    docker_registry: BTreeMap<String, DockerRegistryConfig>,
    #[serde(default)]
    netlify: BTreeMap<String, NetlifyConfig>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct S3Config {
    endpoint: Option<String>,
    region: Option<String>,
    bucket: Option<String>,
    prefix: Option<String>,
    profile: Option<String>,
    path_style: Option<bool>,
    visibility: Option<String>,
    presign_ttl_seconds: Option<u64>,
    overwrite: Option<bool>,
    cache_control: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct GoogleDriveConfig {
    folder_id: Option<String>,
    name_prefix: Option<String>,
    share: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct OneDriveConfig {
    root: Option<String>,
    path_prefix: Option<String>,
    conflict_behavior: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct DropboxConfig {
    path_prefix: Option<String>,
    mode: Option<String>,
    autorename: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct PlayStoreConfig {
    package_name: Option<String>,
    default_track: Option<String>,
    release_status: Option<String>,
    access_token_env: Option<String>,
    service_account_json_env: Option<String>,
    service_account_json_base64_env: Option<String>,
    google_application_credentials_env: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct AppStoreConfig {
    app_id: Option<String>,
    bundle_id: Option<String>,
    platform: Option<String>,
    issuer_id: Option<String>,
    key_id: Option<String>,
    default_track: Option<String>,
    access_token_env: Option<String>,
    issuer_id_env: Option<String>,
    key_id_env: Option<String>,
    api_key_env: Option<String>,
    api_key_base64_env: Option<String>,
    api_key_path_env: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct MicrosoftStoreConfig {
    product_id: Option<String>,
    package_identity_name: Option<String>,
    tenant_id: Option<String>,
    client_id: Option<String>,
    seller_id: Option<String>,
    token_env: Option<String>,
    tenant_id_env: Option<String>,
    client_id_env: Option<String>,
    client_secret_env: Option<String>,
    seller_id_env: Option<String>,
    package_url: Option<String>,
    package_type: Option<String>,
    flight_id: Option<String>,
    package_rollout_percentage: Option<u8>,
    msstore_project: Option<String>,
    languages: Option<Vec<String>>,
    architectures: Option<Vec<String>>,
    is_silent_install: Option<bool>,
    installer_parameters: Option<String>,
    generic_doc_url: Option<String>,
    submit: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct GithubPagesConfig {
    owner: Option<String>,
    repo: Option<String>,
    mode: Option<String>,
    source: Option<String>,
    source_branch: Option<String>,
    source_path: Option<String>,
    site_kind: Option<String>,
    base_path: Option<String>,
    custom_domain: Option<String>,
    enforce_https: Option<bool>,
    remote: Option<String>,
    production_branch: Option<String>,
    workflow: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct GithubReleasesConfig {
    owner: Option<String>,
    repo: Option<String>,
    tag: Option<String>,
    name: Option<String>,
    target_commitish: Option<String>,
    notes: Option<String>,
    notes_file: Option<String>,
    draft: Option<bool>,
    prerelease: Option<bool>,
    make_latest: Option<String>,
    replace_assets: Option<bool>,
    upload_artifact_manifest: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct CloudflarePagesConfig {
    account_id: Option<String>,
    project_name: Option<String>,
    environment: Option<String>,
    custom_domain: Option<String>,
    base_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct NetlifyConfig {
    site_id: Option<String>,
    team_slug: Option<String>,
    production: Option<bool>,
    custom_domain: Option<String>,
    base_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct DockerRegistryConfig {
    tags: Option<Vec<String>>,
}

pub fn package(options: PackageOptions) -> Result<()> {
    if options.release {
        sync_resolved_release_platform_config(&options.project_dir, options.target)?;
    }
    let manifest = package::package_artifact(&options)?;
    if options.json {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
    } else {
        println!(
            "Packaged {} {} artifact into {}",
            manifest.target, manifest.format, manifest.root_dir
        );
        println!("{} files", manifest.artifacts.len());
        println!(
            "{}",
            Path::new(&manifest.root_dir)
                .join(ARTIFACT_MANIFEST)
                .display()
        );
    }
    Ok(())
}

pub fn package_silent(options: PackageOptions) -> Result<PathBuf> {
    if options.release {
        sync_resolved_release_platform_config(&options.project_dir, options.target)?;
    }
    package::package_artifact(&options)?;
    Ok(package_artifact_manifest_path(
        &options.project_dir,
        options.target,
        options.format,
        options.release,
    ))
}

pub fn package_artifact_manifest_path(
    project_dir: &Path,
    target: Target,
    format: PackageFormat,
    release: bool,
) -> PathBuf {
    default_artifact_manifest_path_for_format(project_dir, target, format, release)
}

pub fn package_readiness_checks(
    project_dir: &Path,
    target: Option<Target>,
    format: Option<PackageFormat>,
) -> Result<Vec<ReadinessCheck>> {
    readiness_package(project_dir, target, format, false)
}

pub fn package_readiness_checks_for_profile(
    project_dir: &Path,
    target: Option<Target>,
    format: Option<PackageFormat>,
    release: bool,
) -> Result<Vec<ReadinessCheck>> {
    readiness_package(project_dir, target, format, release)
}

pub fn distribution_readiness_checks(
    project_dir: &Path,
    provider: DistributionProvider,
    site: &str,
    track: Option<&str>,
    format: Option<PackageFormat>,
    artifact: Option<&Path>,
) -> Result<Vec<ReadinessCheck>> {
    let config = load_publish_manifest(project_dir)?;
    readiness_distribute(
        project_dir,
        provider,
        site,
        track,
        format,
        artifact,
        &config,
    )
}

pub fn distribute(options: DistributeOptions) -> Result<()> {
    let mut events = Vec::new();
    push_distribution_event(
        &mut events,
        "distribution.config",
        "started",
        Some(options.provider.as_str().to_string()),
    );
    let config = match load_publish_manifest(&options.project_dir) {
        Ok(config) => config,
        Err(error) => {
            let message = error.to_string();
            let receipt_path = write_failed_distribution_receipt(
                &options,
                &mut events,
                "distribution.config",
                &message,
            )?;
            bail!(
                "distribution config failed: {}; distribution receipt: {}",
                redact_sensitive_text(&message),
                receipt_path.display()
            );
        }
    };
    push_distribution_event(&mut events, "distribution.config", "completed", None);
    let result = match options.action {
        DistributeAction::Setup => setup_provider(&options, &config),
        DistributeAction::Status => provider_status(&options, &config),
        DistributeAction::Promote | DistributeAction::Rollback => {
            provider_lifecycle(&options, &config)
        }
        DistributeAction::Publish => publish_artifact(&options, &config),
    };
    if let Err(error) = result {
        let message = error.to_string();
        if message.contains("distribution receipt:") {
            bail!("{message}");
        }
        let receipt_path = write_failed_distribution_receipt(
            &options,
            &mut events,
            distribute_action_stage(options.action),
            &message,
        )?;
        bail!(
            "{} failed: {}; distribution receipt: {}",
            options.action.as_str(),
            redact_sensitive_text(&message),
            receipt_path.display()
        );
    }
    if options.action == DistributeAction::Setup {
        let receipt_path = write_setup_distribution_receipt(&options, &mut events)?;
        if !options.json {
            println!("Distribution receipt: {}", receipt_path.display());
        }
    }
    Ok(())
}

pub fn distribute_publish_value(options: DistributeOptions) -> Result<Value> {
    distribute_publish_outcome(options).map(|outcome| outcome.receipt)
}

pub fn distribute_status_value(options: DistributeOptions) -> Result<Value> {
    distribute_status_outcome(options).map(|outcome| outcome.receipt)
}

pub fn distribute_publish_outcome(
    options: DistributeOptions,
) -> Result<DistributionPublishOutcome> {
    if options.action != DistributeAction::Publish {
        bail!("distribute_publish_value only supports publish actions");
    }
    let mut events = Vec::new();
    push_distribution_event(
        &mut events,
        "distribution.config",
        "started",
        Some(options.provider.as_str().to_string()),
    );
    let config = match load_publish_manifest(&options.project_dir) {
        Ok(config) => config,
        Err(error) => {
            let message = error.to_string();
            let receipt_path = write_failed_distribution_receipt(
                &options,
                &mut events,
                "distribution.config",
                &message,
            )?;
            bail!(
                "distribution config failed: {}; distribution receipt: {}",
                redact_sensitive_text(&message),
                receipt_path.display()
            );
        }
    };
    push_distribution_event(&mut events, "distribution.config", "completed", None);
    let (_, value) = match publish_artifact_receipt_with_events(&options, &config, &mut events) {
        Ok(value) => value,
        Err(error) => {
            let message = error.to_string();
            let receipt_path = write_failed_distribution_receipt(
                &options,
                &mut events,
                "distribution.publish",
                &message,
            )?;
            bail!(
                "distribution publish failed: {}; distribution receipt: {}",
                redact_sensitive_text(&message),
                receipt_path.display()
            );
        }
    };
    Ok(DistributionPublishOutcome {
        receipt: value,
        events,
    })
}

pub fn distribute_status_outcome(options: DistributeOptions) -> Result<DistributionPublishOutcome> {
    if options.action != DistributeAction::Status {
        bail!("distribute_status_value only supports status actions");
    }
    let mut events = Vec::new();
    push_distribution_event(
        &mut events,
        "distribution.config",
        "started",
        Some(options.provider.as_str().to_string()),
    );
    let config = match load_publish_manifest(&options.project_dir) {
        Ok(config) => config,
        Err(error) => {
            let message = error.to_string();
            let receipt_path = write_failed_distribution_receipt(
                &options,
                &mut events,
                "distribution.config",
                &message,
            )?;
            bail!(
                "distribution config failed: {}; distribution receipt: {}",
                redact_sensitive_text(&message),
                receipt_path.display()
            );
        }
    };
    push_distribution_event(&mut events, "distribution.config", "completed", None);
    push_distribution_event(
        &mut events,
        "provider.status",
        "started",
        Some(options.provider.as_str().to_string()),
    );
    push_provider_request_event(&mut events, &options, "status");
    let receipt = match provider_status_receipt(&options, &config) {
        Ok(receipt) => receipt,
        Err(error) => {
            let message = error.to_string();
            let receipt_path = write_failed_distribution_receipt(
                &options,
                &mut events,
                "provider.status",
                &message,
            )?;
            bail!(
                "distribution status failed: {}; distribution receipt: {}",
                redact_sensitive_text(&message),
                receipt_path.display()
            );
        }
    };
    push_provider_response_event(&mut events, &receipt);
    push_distribution_event(
        &mut events,
        "provider.status",
        receipt.status.as_str(),
        Some(provider_event_detail(&receipt)),
    );
    let receipt_path = receipt_output_path(&options.project_dir, &receipt);
    push_distribution_event(
        &mut events,
        "distribution.receipt",
        "written",
        Some(receipt_path.display().to_string()),
    );
    let value = distribution_receipt_value_with_events(&options, &receipt, None, None, &events)?;
    write_receipt(&options.project_dir, &receipt, &value)?;
    Ok(DistributionPublishOutcome {
        receipt: value,
        events,
    })
}

pub fn readiness(options: ReadinessOptions) -> Result<()> {
    let checks = match options.kind {
        ReadinessKind::Package => readiness_package(
            &options.project_dir,
            options.target,
            options.format,
            options.release,
        ),
        ReadinessKind::Release => {
            let config = load_publish_manifest(&options.project_dir)?;
            let mut checks = readiness_package(
                &options.project_dir,
                options.target,
                options.format,
                options.release,
            )?;
            let provider = options
                .provider
                .context("readiness release requires --provider")?;
            checks.extend(readiness_distribute(
                &options.project_dir,
                provider,
                &options.site,
                options.track.as_deref(),
                options.format,
                options.artifact.as_deref(),
                &config,
            )?);
            Ok(checks)
        }
        ReadinessKind::Distribute => {
            let config = load_publish_manifest(&options.project_dir)?;
            let provider = options
                .provider
                .context("readiness distribute requires --provider")?;
            let artifact = options.artifact.as_deref();
            readiness_distribute(
                &options.project_dir,
                provider,
                &options.site,
                options.track.as_deref(),
                options.format,
                artifact,
                &config,
            )
        }
    }?;
    let report = ReadinessReport {
        project_dir: options.project_dir.display().to_string(),
        target: options.target.map(|target| target.as_str().to_string()),
        format: options.format.map(|format| format.as_str().to_string()),
        provider: options
            .provider
            .map(|provider| provider.as_str().to_string()),
        site: matches!(
            options.kind,
            ReadinessKind::Distribute | ReadinessKind::Release
        )
        .then(|| options.site.clone()),
        status: report_status(&checks).to_string(),
        checks,
    };
    if options.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_readiness_report(&report);
    }
    if report.status == "blocked" {
        bail!("readiness checks failed");
    }
    Ok(())
}

fn load_publish_manifest(project_dir: &Path) -> Result<PublishManifest> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn site_output_dir(project_dir: &Path) -> Result<PathBuf> {
    let manifest = load_publish_manifest(project_dir)?;
    Ok(manifest
        .site
        .and_then(|site| site.out_dir)
        .map(|path| resolve_project_path(project_dir, path))
        .unwrap_or_else(|| project_dir.join("target/fission/site")))
}

fn read_artifact_manifest(path: &Path) -> Result<ArtifactManifest> {
    let data = fs::read_to_string(path)
        .with_context(|| format!("failed to read artifact manifest {}", path.display()))?;
    serde_json::from_str(&data)
        .with_context(|| format!("failed to parse artifact manifest {}", path.display()))
}

fn default_artifact_manifest_path(project_dir: &Path, target: Target, release: bool) -> PathBuf {
    default_artifact_manifest_path_for_format(project_dir, target, PackageFormat::Static, release)
}

fn default_artifact_manifest_path_for_format(
    project_dir: &Path,
    target: Target,
    format: PackageFormat,
    release: bool,
) -> PathBuf {
    project_dir
        .join("target/fission")
        .join(if release { "release" } else { "debug" })
        .join(target.as_str())
        .join(format.as_str())
        .join(ARTIFACT_MANIFEST)
}

fn github_config(config: &PublishManifest, site: &str) -> Result<GithubPagesConfig> {
    Ok(config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.github_pages.get(site))
        .cloned()
        .unwrap_or_default())
}

fn github_releases_config(config: &PublishManifest, site: &str) -> Result<GithubReleasesConfig> {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.github_releases.get(site))
        .cloned()
        .with_context(|| format!("missing [distribution.github_releases.{site}] in fission.toml"))
}

fn docker_registry_config(config: &PublishManifest, site: &str) -> Result<DockerRegistryConfig> {
    Ok(config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.docker_registry.get(site))
        .cloned()
        .unwrap_or_default())
}

fn s3_config(config: &PublishManifest, site: &str) -> Result<S3Config> {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.s3.get(site))
        .cloned()
        .with_context(|| format!("missing [distribution.s3.{site}] in fission.toml"))
}

fn google_drive_config(config: &PublishManifest, site: &str) -> Result<GoogleDriveConfig> {
    Ok(config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.google_drive.get(site))
        .cloned()
        .unwrap_or_default())
}

fn onedrive_config(config: &PublishManifest, site: &str) -> Result<OneDriveConfig> {
    Ok(config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.onedrive.get(site))
        .cloned()
        .unwrap_or_default())
}

fn dropbox_config(config: &PublishManifest, site: &str) -> Result<DropboxConfig> {
    Ok(config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.dropbox.get(site))
        .cloned()
        .unwrap_or_default())
}

fn cloudflare_config(config: &PublishManifest, site: &str) -> Result<CloudflarePagesConfig> {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.cloudflare_pages.get(site))
        .cloned()
        .with_context(|| format!("missing [distribution.cloudflare_pages.{site}] in fission.toml"))
}

fn netlify_config(config: &PublishManifest, site: &str) -> Result<NetlifyConfig> {
    config
        .distribution
        .as_ref()
        .and_then(|distribution| distribution.netlify.get(site))
        .cloned()
        .with_context(|| format!("missing [distribution.netlify.{site}] in fission.toml"))
}

fn github_workflow_path(project_dir: &Path, _cfg: &GithubPagesConfig, workflow: &str) -> PathBuf {
    git_repo_root(project_dir)
        .unwrap_or_else(|| project_dir.to_path_buf())
        .join(".github/workflows")
        .join(workflow)
}

fn project_dir_argument_for_workflow(project_dir: &Path) -> String {
    let Some(repo_root) = git_repo_root(project_dir) else {
        return ".".to_string();
    };
    let Ok(project_dir) = fs::canonicalize(project_dir) else {
        return ".".to_string();
    };
    let Ok(repo_root) = fs::canonicalize(repo_root) else {
        return ".".to_string();
    };
    if project_dir == repo_root {
        ".".to_string()
    } else {
        project_dir
            .strip_prefix(&repo_root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| ".".to_string())
    }
}

fn render_github_pages_workflow(project_dir: &Path, cfg: &GithubPagesConfig) -> String {
    let branch = cfg.production_branch.as_deref().unwrap_or("main");
    let package_project_dir = project_dir_argument_for_workflow(project_dir);
    let artifact_path = if package_project_dir == "." {
        "target/fission/release/static-site/static".to_string()
    } else {
        format!("{package_project_dir}/target/fission/release/static-site/static")
    };
    format!(
        r#"name: Publish Fission site

on:
  push:
    branches:
      - {branch}
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: github-pages
  cancel-in-progress: true

jobs:
  build:
    runs-on: ubuntu-latest
    environment:
      name: github-pages
    steps:
      - name: Check out repository
        uses: actions/checkout@v4
        with:
          submodules: recursive

      - name: Set up Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Build Fission static package
        run: fission package --project-dir {package_project_dir} --target static-site --format static --release

      - name: Upload GitHub Pages artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: {artifact_path}

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{{{ steps.deployment.outputs.page_url }}}}
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
"#,
        branch = branch,
        package_project_dir = package_project_dir,
        artifact_path = artifact_path,
    )
}

fn distribution_receipt_value(
    options: &DistributeOptions,
    receipt: &DistributionReceipt,
    artifact_path: Option<&Path>,
    manifest: Option<&ArtifactManifest>,
) -> Result<Value> {
    let artifact_manifest_sha256 = artifact_path
        .filter(|path| path.exists())
        .map(|path| hash_file(path).map(|(sha256, _)| sha256))
        .transpose()?;
    let release_content_manifest =
        release_content_manifest_path(&options.project_dir, options.provider);
    let release_content_manifest_sha256 = release_content_manifest
        .as_deref()
        .map(|path| hash_file(path).map(|(sha256, _)| sha256))
        .transpose()?;
    let provider_uploaded = provider_uploaded_assets_by_relative(receipt);
    let uploaded_assets = manifest
        .map(|manifest| {
            manifest
                .artifacts
                .iter()
                .map(|artifact| {
                    let mut value = json!({
                        "kind": artifact.kind,
                        "purpose": artifact.purpose,
                        "platform": artifact.platform,
                        "upload_provider": artifact.upload_provider,
                        "path": artifact.path,
                        "relative_path": artifact.relative_path,
                        "sha256": artifact.sha256,
                        "size_bytes": artifact.size_bytes,
                        "mime_type": artifact.mime_type,
                    });
                    if let Some(provider) = provider_uploaded.get(&artifact.relative_path) {
                        if let Some(provider_id) = provider.get("provider_id").cloned() {
                            value["provider_id"] = provider_id;
                        }
                        if let Some(url) = provider.get("url").cloned() {
                            value["url"] = url;
                        }
                    }
                    value
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let uploaded_bytes = uploaded_assets
        .iter()
        .filter_map(|asset| asset.get("size_bytes").and_then(Value::as_u64))
        .sum();
    let view = DistributionReceiptView {
        receipt,
        release_id: active_release_id(&options.project_dir)
            .or_else(|| release_id_from_manifest(manifest))
            .or_else(|| release_id_from_version_build(manifest, &options.project_dir)),
        target: options
            .target
            .map(|target| target.as_str().to_string())
            .or_else(|| manifest.map(|manifest| manifest.target.clone())),
        format: options
            .format
            .map(|format| format.as_str().to_string())
            .or_else(|| manifest.map(|manifest| manifest.format.clone())),
        track: options.track.clone(),
        locales: release_locales(options),
        version: manifest
            .and_then(|manifest| manifest.project.version.clone())
            .or_else(|| release_version(&options.project_dir)),
        build: manifest
            .and_then(|manifest| manifest.project.build)
            .or_else(|| release_build_number(&options.project_dir)),
        artifact_hash: manifest
            .and_then(|manifest| manifest.artifacts.first())
            .map(|artifact| artifact.sha256.clone()),
        artifact_manifest_sha256,
        release_content_manifest: release_content_manifest
            .as_ref()
            .map(|path| path.display().to_string()),
        release_content_manifest_sha256,
        release_content_assets: release_content_manifest
            .as_deref()
            .map(release_content_assets_from_manifest)
            .transpose()?
            .unwrap_or_default(),
        uploaded_bytes,
        uploaded_assets,
    };
    let mut value =
        serde_json::to_value(view).context("failed to serialize distribution receipt")?;
    redact_json_value(&mut value);
    Ok(value)
}

fn distribution_receipt_value_with_events(
    options: &DistributeOptions,
    receipt: &DistributionReceipt,
    artifact_path: Option<&Path>,
    manifest: Option<&ArtifactManifest>,
    events: &[DistributionEvent],
) -> Result<Value> {
    let mut value = distribution_receipt_value(options, receipt, artifact_path, manifest)?;
    if !events.is_empty() {
        let events_value =
            serde_json::to_value(events).context("failed to serialize distribution events")?;
        if let Value::Object(object) = &mut value {
            object.insert("events".to_string(), events_value);
        }
    }
    Ok(value)
}

fn receipt_artifact_context(
    receipt: &DistributionReceipt,
) -> Result<(Option<PathBuf>, Option<ArtifactManifest>)> {
    let Some(path) = receipt
        .artifact_manifest
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
    else {
        return Ok((None, None));
    };
    let manifest = path
        .exists()
        .then(|| read_artifact_manifest(&path))
        .transpose()?;
    Ok((Some(path), manifest))
}

fn provider_uploaded_assets_by_relative(receipt: &DistributionReceipt) -> BTreeMap<String, Value> {
    receipt
        .stdout
        .as_deref()
        .and_then(|stdout| serde_json::from_str::<Value>(stdout).ok())
        .and_then(|value| value.get("uploaded").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|asset| {
            let relative = asset
                .get("relative_path")
                .and_then(Value::as_str)
                .map(str::to_string)?;
            Some((relative, asset))
        })
        .collect()
}

fn release_content_assets_from_manifest(path: &Path) -> Result<Vec<Value>> {
    let value: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    if let Some(assets) = value
        .pointer("/rendered_screenshots/manifest/assets")
        .and_then(Value::as_array)
    {
        return Ok(assets.clone());
    }
    if let Some(assets) = value.get("assets").and_then(Value::as_array) {
        return Ok(assets.clone());
    }
    Ok(Vec::new())
}

fn release_content_manifest_path(
    project_dir: &Path,
    provider: DistributionProvider,
) -> Option<PathBuf> {
    let data = fs::read_to_string(project_dir.join("fission.toml")).ok();
    let rendered_dir = data
        .as_deref()
        .and_then(|data| toml::from_str::<toml::Value>(data).ok())
        .and_then(|value| {
            value
                .get("release")
                .and_then(|release| release.get("screenshots"))
                .and_then(|screenshots| screenshots.get("rendered_dir"))
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "release-content/screenshots/rendered".to_string());
    [
        project_dir.join("release-content/content-manifest.json"),
        project_dir
            .join(&rendered_dir)
            .join(provider.as_str())
            .join("release-content-manifest.json"),
    ]
    .into_iter()
    .find(|path| path.exists())
}

fn active_release_id(project_dir: &Path) -> Option<String> {
    let data = fs::read_to_string(project_dir.join("fission.toml")).ok()?;
    let value: toml::Value = toml::from_str(&data).ok()?;
    value
        .get("release")
        .and_then(|release| release.get("active_release"))
        .and_then(toml::Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
}

fn release_id_from_manifest(manifest: Option<&ArtifactManifest>) -> Option<String> {
    let manifest = manifest?;
    let version = manifest.project.version.as_deref()?;
    let build = manifest.project.build?;
    Some(format!("{version}+{build}"))
}

fn release_id_from_version_build(
    manifest: Option<&ArtifactManifest>,
    project_dir: &Path,
) -> Option<String> {
    if manifest.is_some() {
        return None;
    }
    Some(format!(
        "{}+{}",
        release_version(project_dir)?,
        release_build_number(project_dir)?
    ))
}

fn release_locales(options: &DistributeOptions) -> Vec<String> {
    if !options.locales.is_empty() {
        return options.locales.clone();
    }
    let data = match fs::read_to_string(options.project_dir.join("fission.toml")) {
        Ok(data) => data,
        Err(_) => return Vec::new(),
    };
    let Ok(value) = toml::from_str::<toml::Value>(&data) else {
        return Vec::new();
    };
    value
        .get("release")
        .and_then(|release| release.get("default_locales"))
        .and_then(toml::Value::as_array)
        .map(|locales| {
            locales
                .iter()
                .filter_map(toml::Value::as_str)
                .filter(|locale| !locale.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn write_receipt(
    project_dir: &Path,
    receipt: &DistributionReceipt,
    value: &Value,
) -> Result<PathBuf> {
    let path = receipt_output_path(project_dir, receipt);
    let dir = path
        .parent()
        .context("distribution receipt path has no parent directory")?;
    fs::create_dir_all(&dir)?;
    fs::write(&path, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn receipt_output_path(project_dir: &Path, receipt: &DistributionReceipt) -> PathBuf {
    let base = project_dir
        .join("target/fission/distribution")
        .join(&receipt.provider)
        .join(&receipt.site)
        .join(format!(
            "{}-{}.json",
            receipt.action, receipt.created_at_unix_seconds
        ));
    unique_receipt_path(base)
}

fn unique_receipt_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("receipt");
    let extension = path.extension().and_then(|value| value.to_str());
    for index in 2.. {
        let file_name = match extension {
            Some(extension) => format!("{stem}-{index}.{extension}"),
            None => format!("{stem}-{index}"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("unbounded receipt path search should always return")
}

fn write_failed_distribution_receipt(
    options: &DistributeOptions,
    events: &mut Vec<DistributionEvent>,
    stage_id: &str,
    message: &str,
) -> Result<PathBuf> {
    push_distribution_event(events, stage_id, "failed", Some(message.to_string()));
    push_distribution_event(
        events,
        "distribution.failed",
        "failed",
        Some(message.to_string()),
    );
    let artifact_path = options.artifact.as_deref().filter(|path| path.exists());
    let manifest = artifact_path
        .map(read_artifact_manifest)
        .transpose()
        .unwrap_or(None);
    let receipt = DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: options.provider.as_str().to_string(),
        site: options.site.clone(),
        action: options.action.as_str().to_string(),
        artifact_manifest: options
            .artifact
            .as_ref()
            .map(|path| path.display().to_string()),
        deployment_id: None,
        canonical_url: None,
        preview_url: None,
        custom_domain: None,
        status: "failed".to_string(),
        stdout: None,
        stderr: Some(redact_sensitive_text(message)),
        manual_follow_up: vec![
            "Fix the failed distribution stage, then rerun readiness or publish.".to_string(),
        ],
    };
    let receipt_path = receipt_output_path(&options.project_dir, &receipt);
    push_distribution_event(
        events,
        "distribution.receipt",
        "written",
        Some(receipt_path.display().to_string()),
    );
    let value = distribution_receipt_value_with_events(
        options,
        &receipt,
        artifact_path,
        manifest.as_ref(),
        events,
    )?;
    write_receipt(&options.project_dir, &receipt, &value)
}

fn write_setup_distribution_receipt(
    options: &DistributeOptions,
    events: &mut Vec<DistributionEvent>,
) -> Result<PathBuf> {
    push_distribution_event(
        events,
        "distribution.setup",
        if options.dry_run {
            "dry-run"
        } else {
            "completed"
        },
        Some(options.provider.as_str().to_string()),
    );
    let receipt = DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: options.provider.as_str().to_string(),
        site: options.site.clone(),
        action: options.action.as_str().to_string(),
        artifact_manifest: options
            .artifact
            .as_ref()
            .map(|path| path.display().to_string()),
        deployment_id: options.deploy.clone(),
        canonical_url: None,
        preview_url: None,
        custom_domain: None,
        status: if options.dry_run {
            "dry-run".to_string()
        } else {
            "completed".to_string()
        },
        stdout: None,
        stderr: None,
        manual_follow_up: vec![
            "Run readiness before publishing, then use fission publish or fission distribute publish.".to_string(),
        ],
    };
    let receipt_path = receipt_output_path(&options.project_dir, &receipt);
    push_distribution_event(
        events,
        "distribution.receipt",
        "written",
        Some(receipt_path.display().to_string()),
    );
    let value = distribution_receipt_value_with_events(options, &receipt, None, None, events)?;
    write_receipt(&options.project_dir, &receipt, &value)
}

fn distribute_action_stage(action: DistributeAction) -> &'static str {
    match action {
        DistributeAction::Setup => "distribution.setup",
        DistributeAction::Publish => "distribution.publish",
        DistributeAction::Status => "distribution.status",
        DistributeAction::Promote | DistributeAction::Rollback => "provider.lifecycle",
    }
}

fn push_distribution_event(
    events: &mut Vec<DistributionEvent>,
    id: &str,
    status: &str,
    details: Option<String>,
) {
    events.push(DistributionEvent {
        at_unix_seconds: now_unix_seconds(),
        id: id.to_string(),
        status: status.to_string(),
        details: details.map(|details| redact_sensitive_text(&details)),
    });
}

fn push_provider_stdio_line_events(events: &mut Vec<DistributionEvent>, id: &str, text: &str) {
    const MAX_STDIO_LINE_EVENTS: usize = 200;
    let mut count = 0usize;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if count == MAX_STDIO_LINE_EVENTS {
            push_distribution_event(
                events,
                id,
                "truncated",
                Some(
                    "additional provider output is retained in the distribution receipt"
                        .to_string(),
                ),
            );
            break;
        }
        push_distribution_event(events, id, "captured", Some(truncate_event_detail(line)));
        count += 1;
    }
}

fn push_provider_request_event(
    events: &mut Vec<DistributionEvent>,
    options: &DistributeOptions,
    operation: &str,
) {
    let details = json!({
        "provider": options.provider.as_str(),
        "operation": operation,
        "site": &options.site,
        "track": &options.track,
        "artifact": options.artifact.as_ref().map(|path| path.display().to_string()),
        "dry_run": options.dry_run,
    });
    push_distribution_event(
        events,
        "provider.request",
        "started",
        Some(details.to_string()),
    );
}

fn push_provider_response_event(
    events: &mut Vec<DistributionEvent>,
    receipt: &DistributionReceipt,
) {
    let details = json!({
        "provider": &receipt.provider,
        "action": &receipt.action,
        "status": &receipt.status,
        "deployment_id": &receipt.deployment_id,
        "canonical_url": &receipt.canonical_url,
        "preview_url": &receipt.preview_url,
    });
    push_distribution_event(
        events,
        "provider.response",
        receipt.status.as_str(),
        Some(details.to_string()),
    );
}

fn push_provider_uploaded_asset_events(
    events: &mut Vec<DistributionEvent>,
    options: &DistributeOptions,
    manifest: &ArtifactManifest,
    provider_uploaded: &BTreeMap<String, Value>,
) {
    const MAX_UPLOAD_ASSET_EVENTS: usize = 200;
    for (index, artifact) in manifest.artifacts.iter().enumerate() {
        if index == MAX_UPLOAD_ASSET_EVENTS {
            push_distribution_event(
                events,
                "provider.uploaded_asset",
                "truncated",
                Some(
                    "additional uploaded/planned assets are retained in the distribution receipt"
                        .to_string(),
                ),
            );
            break;
        }
        let uploaded = provider_uploaded.get(&artifact.relative_path);
        let detail = json!({
            "relative_path": artifact.relative_path,
            "path": artifact.path,
            "kind": artifact.kind,
            "purpose": artifact.purpose,
            "size_bytes": artifact.size_bytes,
            "sha256": artifact.sha256,
            "mime_type": artifact.mime_type,
            "provider_id": uploaded.and_then(|value| value.get("provider_id")).cloned(),
            "url": uploaded.and_then(|value| value.get("url")).cloned(),
        });
        push_distribution_event(
            events,
            "provider.uploaded_asset",
            if options.dry_run {
                "planned"
            } else {
                "uploaded"
            },
            Some(detail.to_string()),
        );
    }
}

fn provider_event_detail(receipt: &DistributionReceipt) -> String {
    [
        receipt.deployment_id.as_deref(),
        receipt.canonical_url.as_deref(),
        receipt.preview_url.as_deref(),
    ]
    .into_iter()
    .flatten()
    .next()
    .unwrap_or(&receipt.provider)
    .to_string()
}

pub(crate) fn redact_sensitive_text(text: &str) -> String {
    let mut redacted = text.to_string();
    for (key, value) in secret_env_values() {
        redacted = redacted.replace(&value, &format!("<redacted:{key}>"));
    }
    redacted
}

fn redact_json_value(value: &mut Value) {
    match value {
        Value::String(text) => {
            *text = redact_sensitive_text(text);
        }
        Value::Array(items) => {
            for item in items {
                redact_json_value(item);
            }
        }
        Value::Object(object) => {
            for item in object.values_mut() {
                redact_json_value(item);
            }
        }
        _ => {}
    }
}

fn secret_env_values() -> Vec<(String, String)> {
    let mut values = env::vars()
        .filter(|(key, value)| secretish_env_key(key) && value.len() >= 8)
        .map(|(key, value)| (key.to_ascii_uppercase(), value))
        .collect::<Vec<_>>();
    values.sort_by(|(_, left), (_, right)| right.len().cmp(&left.len()));
    values.dedup_by(|(_, left), (_, right)| left == right);
    values
}

fn secretish_env_key(key: &str) -> bool {
    let key = key.to_ascii_uppercase();
    [
        "PASSWORD",
        "TOKEN",
        "SECRET",
        "PRIVATE",
        "CREDENTIAL",
        "KEYSTORE",
        "SERVICE_ACCOUNT",
        "API_KEY",
        "ACCESS_KEY",
        "CLIENT_SECRET",
        "CERTIFICATE",
        "P8",
        "P12",
        "PFX",
        "JKS",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

fn truncate_event_detail(value: &str) -> String {
    const MAX_EVENT_DETAIL_CHARS: usize = 2_000;
    let mut detail = value
        .trim()
        .chars()
        .take(MAX_EVENT_DETAIL_CHARS)
        .collect::<String>();
    if value.trim().chars().count() > MAX_EVENT_DETAIL_CHARS {
        detail.push_str("...");
    }
    detail
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
