use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use fission_command_core::{DistributionProvider, FissionProject, Target};
use fission_credentials as credentials;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

mod files;
mod github_releases;
mod package;
mod static_hosts;
mod stores;

const ARTIFACT_MANIFEST: &str = "artifact-manifest.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum PackageFormat {
    Aab,
    Apk,
    App,
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
    pub json: bool,
}

#[derive(Clone, Debug)]
pub struct DistributeOptions {
    pub project_dir: PathBuf,
    pub provider: DistributionProvider,
    pub action: DistributeAction,
    pub artifact: Option<PathBuf>,
    pub site: String,
    pub deploy: Option<String>,
    pub track: Option<String>,
    pub dry_run: bool,
    pub yes: bool,
    pub json: bool,
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
    pub json: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactManifest {
    schema_version: u32,
    created_at_unix_seconds: u64,
    project: ArtifactProject,
    target: String,
    format: String,
    profile: String,
    root_dir: String,
    artifacts: Vec<ArtifactFile>,
    validation: ArtifactValidation,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactProject {
    app_id: String,
    name: String,
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
struct ArtifactValidation {
    state: String,
    checks: Vec<ReadinessCheck>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ReadinessCheck {
    id: String,
    severity: CheckSeverity,
    status: CheckStatus,
    summary: String,
    details: Option<String>,
    remediation: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CheckSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CheckStatus {
    Passed,
    Missing,
    Failed,
    Warning,
    Skipped,
}

#[derive(Debug, Serialize)]
struct ReadinessReport {
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
    service_account: Option<String>,
    release_status: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct AppStoreConfig {
    app_id: Option<String>,
    bundle_id: Option<String>,
    issuer_id: Option<String>,
    key_id: Option<String>,
    api_key_path: Option<String>,
    default_track: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct MicrosoftStoreConfig {
    product_id: Option<String>,
    package_identity_name: Option<String>,
    tenant_id: Option<String>,
    client_id: Option<String>,
    seller_id: Option<String>,
    package_url: Option<String>,
    package_type: Option<String>,
    flight_id: Option<String>,
    package_rollout_percentage: Option<u8>,
    msstore_project: Option<String>,
    msstore_reconfigure: Option<bool>,
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

pub fn package(options: PackageOptions) -> Result<()> {
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

pub fn distribute(options: DistributeOptions) -> Result<()> {
    let config = load_publish_manifest(&options.project_dir)?;
    match options.action {
        DistributeAction::Setup => setup_provider(&options, &config),
        DistributeAction::Status => provider_status(&options, &config),
        DistributeAction::Promote | DistributeAction::Rollback => {
            provider_lifecycle(&options, &config)
        }
        DistributeAction::Publish => publish_artifact(&options, &config),
    }
}

pub fn readiness(options: ReadinessOptions) -> Result<()> {
    let checks = match options.kind {
        ReadinessKind::Package => {
            readiness_package(&options.project_dir, options.target, options.format)
        }
        ReadinessKind::Release => {
            let config = load_publish_manifest(&options.project_dir)?;
            let mut checks =
                readiness_package(&options.project_dir, options.target, options.format)?;
            let provider = options
                .provider
                .context("readiness release requires --provider")?;
            checks.extend(readiness_distribute(
                &options.project_dir,
                provider,
                &options.site,
                options.track.as_deref(),
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
        if report.status == "blocked" {
            bail!("readiness checks failed");
        }
    }
    Ok(())
}

fn setup_provider(options: &DistributeOptions, config: &PublishManifest) -> Result<()> {
    match options.provider {
        DistributionProvider::GithubPages => setup_github_pages(options, config),
        DistributionProvider::GithubReleases => github_releases::setup(options, config),
        DistributionProvider::CloudflarePages => {
            let cfg = cloudflare_config(config, &options.site)?;
            println!("Cloudflare Pages setup checks for `{}`", options.site);
            println!(
                "account_id: {}",
                cfg.account_id.as_deref().unwrap_or("<missing>")
            );
            println!(
                "project_name: {}",
                cfg.project_name.as_deref().unwrap_or("<missing>")
            );
            println!("Run `fission readiness distribute --provider cloudflare-pages --site {} --project-dir {}` before publishing.", options.site, options.project_dir.display());
            Ok(())
        }
        DistributionProvider::Netlify => {
            let cfg = netlify_config(config, &options.site)?;
            println!("Netlify setup checks for `{}`", options.site);
            println!("site_id: {}", cfg.site_id.as_deref().unwrap_or("<missing>"));
            println!(
                "team_slug: {}",
                cfg.team_slug.as_deref().unwrap_or("<missing>")
            );
            println!("Run `fission readiness distribute --provider netlify --site {} --project-dir {}` before publishing.", options.site, options.project_dir.display());
            Ok(())
        }
        DistributionProvider::S3
        | DistributionProvider::GoogleDrive
        | DistributionProvider::OneDrive
        | DistributionProvider::Dropbox
        | DistributionProvider::PlayStore
        | DistributionProvider::AppStore
        | DistributionProvider::MicrosoftStore => setup_non_static_provider(options, config),
    }
}

fn publish_artifact(options: &DistributeOptions, config: &PublishManifest) -> Result<()> {
    let artifact_path = options
        .artifact
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            default_artifact_manifest_path(&options.project_dir, Target::Site, true)
        });
    let manifest = read_artifact_manifest(&artifact_path)?;
    let checks = readiness_distribute(
        &options.project_dir,
        options.provider,
        &options.site,
        options.track.as_deref(),
        Some(&artifact_path),
        config,
    )?;
    let errors = checks
        .iter()
        .filter(|check| {
            check.severity == CheckSeverity::Error && check.status != CheckStatus::Passed
        })
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        print_checks(&checks);
        bail!("distribution readiness failed");
    }

    let receipt = match options.provider {
        DistributionProvider::GithubPages => {
            publish_github_pages(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::GithubReleases => {
            github_releases::publish(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::CloudflarePages => {
            publish_cloudflare_pages(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::Netlify => {
            static_hosts::publish_netlify(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::S3 => files::publish_s3(options, config, &artifact_path, &manifest)?,
        DistributionProvider::GoogleDrive => {
            files::publish_google_drive(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::OneDrive => {
            files::publish_onedrive(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::Dropbox => {
            files::publish_dropbox(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::PlayStore => {
            stores::publish_play_store(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::AppStore => {
            stores::publish_app_store(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::MicrosoftStore => {
            stores::publish_microsoft_store(options, config, &artifact_path, &manifest)?
        }
    };
    write_receipt(&options.project_dir, &receipt)?;
    print_distribution_receipt(options, &receipt)
}

fn print_distribution_receipt(
    options: &DistributeOptions,
    receipt: &DistributionReceipt,
) -> Result<()> {
    if options.json {
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else {
        println!(
            "{} {} status: {}",
            receipt.provider, receipt.action, receipt.status
        );
        if let Some(url) = &receipt.canonical_url {
            println!("URL: {url}");
        }
        for item in &receipt.manual_follow_up {
            println!("Follow-up: {item}");
        }
    }
    Ok(())
}

fn provider_status(options: &DistributeOptions, config: &PublishManifest) -> Result<()> {
    let receipt = match options.provider {
        DistributionProvider::GithubPages => github_pages_status(options, config)?,
        DistributionProvider::GithubReleases => github_releases::status(options, config)?,
        DistributionProvider::CloudflarePages => cloudflare_pages_status(options, config)?,
        DistributionProvider::Netlify => static_hosts::netlify_status(options, config)?,
        DistributionProvider::PlayStore => stores::play_store_status(options, config)?,
        DistributionProvider::S3 => files::s3_status(options, config)?,
        DistributionProvider::GoogleDrive => files::google_drive_status(options, config)?,
        DistributionProvider::OneDrive => files::onedrive_status(options, config)?,
        DistributionProvider::Dropbox => files::dropbox_status(options, config)?,
        DistributionProvider::AppStore => stores::app_store_status(options, config)?,
        DistributionProvider::MicrosoftStore => stores::microsoft_store_status(options, config)?,
    };
    if options.json {
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else {
        println!("{} status: {}", receipt.provider, receipt.status);
        if let Some(stdout) = &receipt.stdout {
            print!("{stdout}");
        }
    }
    Ok(())
}

fn provider_lifecycle(options: &DistributeOptions, config: &PublishManifest) -> Result<()> {
    let receipt = match options.provider {
        DistributionProvider::Netlify => static_hosts::netlify_lifecycle(options, config)?,
        DistributionProvider::CloudflarePages => cloudflare_pages_lifecycle(options, config)?,
        _ => bail!(
            "{} currently supports setup, publish, and status; {} is not exposed by this provider backend",
            options.provider.as_str(),
            match options.action {
                DistributeAction::Promote => "promote",
                DistributeAction::Rollback => "rollback",
                _ => "this operation",
            }
        ),
    };
    write_receipt(&options.project_dir, &receipt)?;
    print_distribution_receipt(options, &receipt)
}

fn cloudflare_pages_lifecycle(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = cloudflare_config(config, &options.site)?;
    let account_id = cfg
        .account_id
        .clone()
        .or_else(|| env::var("CLOUDFLARE_ACCOUNT_ID").ok())
        .context(
            "distribution.cloudflare_pages.<site>.account_id or CLOUDFLARE_ACCOUNT_ID is required",
        )?;
    let project_name = cfg
        .project_name
        .as_deref()
        .context("distribution.cloudflare_pages.<site>.project_name is required")?;
    let deploy = options
        .deploy
        .as_deref()
        .context("cloudflare-pages promote/rollback requires --deploy <deployment-id>")?;
    if options.dry_run {
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: "cloudflare-pages".to_string(),
            site: options.site.clone(),
            action: match options.action {
                DistributeAction::Promote => "promote",
                DistributeAction::Rollback => "rollback",
                _ => "lifecycle",
            }
            .to_string(),
            artifact_manifest: None,
            deployment_id: Some(deploy.to_string()),
            canonical_url: cloudflare_url(&cfg),
            preview_url: None,
            custom_domain: non_empty(cfg.custom_domain.clone()),
            status: "dry-run".to_string(),
            stdout: None,
            stderr: None,
            manual_follow_up: vec![format!(
                "Would make Cloudflare Pages deployment {deploy} live by calling the provider rollback endpoint."
            )],
        });
    }
    let token = credentials::provider_secret(
        DistributionProvider::CloudflarePages,
        &["CLOUDFLARE_API_TOKEN"],
    )?
    .context("CLOUDFLARE_API_TOKEN or Fission vault credentials are required")?;
    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/pages/projects/{project_name}/deployments/{deploy}/rollback"
    );
    let response = reqwest::blocking::Client::builder()
        .user_agent("cargo-fission-publish/0.1")
        .build()?
        .post(url)
        .bearer_auth(token)
        .send()
        .context("failed to rollback Cloudflare Pages deployment")?;
    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        bail!("Cloudflare Pages rollback failed with {status}: {text}");
    }
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse Cloudflare Pages rollback response: {text}"))?;
    let result = value.get("result").unwrap_or(&value);
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "cloudflare-pages".to_string(),
        site: options.site.clone(),
        action: match options.action {
            DistributeAction::Promote => "promote",
            DistributeAction::Rollback => "rollback",
            _ => "lifecycle",
        }
        .to_string(),
        artifact_manifest: None,
        deployment_id: result
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .or_else(|| Some(deploy.to_string())),
        canonical_url: cloudflare_url(&cfg),
        preview_url: result
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        custom_domain: non_empty(cfg.custom_domain.clone()),
        status: result
            .pointer("/latest_stage/status")
            .or_else(|| result.get("status"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("rollback-requested")
            .to_string(),
        stdout: Some(serde_json::to_string_pretty(&value)?),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
}

fn setup_non_static_provider(options: &DistributeOptions, config: &PublishManifest) -> Result<()> {
    let checks = readiness_distribute(
        &options.project_dir,
        options.provider,
        &options.site,
        options.track.as_deref(),
        options.artifact.as_deref(),
        config,
    )?;
    if options.json {
        let report = ReadinessReport {
            project_dir: options.project_dir.display().to_string(),
            target: None,
            format: None,
            provider: Some(options.provider.as_str().to_string()),
            site: Some(options.site.clone()),
            status: report_status(&checks).to_string(),
            checks,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "{} setup checks for `{}`",
            options.provider.as_str(),
            options.site
        );
        print_checks(&checks);
    }
    Ok(())
}

fn setup_github_pages(options: &DistributeOptions, config: &PublishManifest) -> Result<()> {
    let cfg = github_config(config, &options.site)?;
    let workflow = cfg
        .workflow
        .clone()
        .unwrap_or_else(|| "fission-pages.yml".to_string());
    let workflow_path = github_workflow_path(&options.project_dir, &cfg, &workflow);
    let content = render_github_pages_workflow(&options.project_dir, &cfg);
    if options.dry_run {
        println!("Would write {}:\n{}", workflow_path.display(), content);
        return Ok(());
    }
    if workflow_path.exists() && !options.yes {
        bail!(
            "{} already exists; pass --yes to overwrite or edit it manually",
            workflow_path.display()
        );
    }
    if let Some(parent) = workflow_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&workflow_path, content)
        .with_context(|| format!("failed to write {}", workflow_path.display()))?;
    println!("Wrote {}", workflow_path.display());
    if cfg
        .custom_domain
        .as_deref()
        .filter(|s| !s.is_empty())
        .is_some()
    {
        println!("Custom domains for GitHub Actions Pages must be configured in repository Pages settings or via the GitHub Pages API.");
    }
    Ok(())
}

fn publish_github_pages(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = github_config(config, &options.site)?;
    let mode = cfg.mode.as_deref().unwrap_or("actions");
    match mode {
        "branch" => publish_github_pages_branch(options, &cfg, artifact_path, manifest),
        "actions" => {
            let owner = cfg
                .owner
                .clone()
                .or_else(|| infer_github_owner(&options.project_dir));
            let repo = cfg
                .repo
                .clone()
                .or_else(|| infer_github_repo(&options.project_dir));
            let workflow = cfg
                .workflow
                .clone()
                .unwrap_or_else(|| "fission-pages.yml".to_string());
            let mut follow_up = vec![format!(
                "Commit the generated workflow and push the configured production branch so GitHub Actions deploys the exact static site build."
            )];
            if let (Some(owner), Some(repo)) = (&owner, &repo) {
                follow_up.push(format!(
                    "Repository Pages URL should be https://{owner}.github.io/{repo}/ unless a custom domain is configured."
                ));
            }
            let workflow_path = github_workflow_path(&options.project_dir, &cfg, &workflow);
            if !workflow_path.exists() {
                follow_up.push(format!(
                    "Run `fission distribute setup --provider github-pages --site {} --project-dir {}` to generate {}.",
                    options.site,
                    options.project_dir.display(),
                    workflow_path.display()
                ));
            }
            Ok(DistributionReceipt {
                schema_version: 1,
                created_at_unix_seconds: now_unix_seconds(),
                provider: "github-pages".to_string(),
                site: options.site.clone(),
                action: "publish".to_string(),
                artifact_manifest: Some(artifact_path.display().to_string()),
                deployment_id: None,
                canonical_url: github_pages_url(&cfg, owner.as_deref(), repo.as_deref()),
                preview_url: None,
                custom_domain: non_empty(cfg.custom_domain.clone()),
                status: "workflow-required".to_string(),
                stdout: None,
                stderr: None,
                manual_follow_up: follow_up,
            })
        }
        other => bail!("unsupported github-pages mode `{other}`; expected actions or branch"),
    }
}

fn publish_github_pages_branch(
    options: &DistributeOptions,
    cfg: &GithubPagesConfig,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let remote = cfg.remote.as_deref().unwrap_or("origin");
    let branch = cfg.source_branch.as_deref().unwrap_or("gh-pages");
    let source_path = cfg.source_path.as_deref().unwrap_or("/");
    let repo_root = git_output(&options.project_dir, ["rev-parse", "--show-toplevel"])?;
    let repo_root = PathBuf::from(repo_root.trim());
    let remote_url = git_output(&repo_root, ["remote", "get-url", remote])?;
    let worktree = options
        .project_dir
        .join("target/fission/publish/github-pages")
        .join(&options.site);
    if options.dry_run {
        println!(
            "Would publish {} to {remote}:{branch} at {}",
            manifest.root_dir, source_path
        );
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: "github-pages".to_string(),
            site: options.site.clone(),
            action: "publish".to_string(),
            artifact_manifest: Some(artifact_path.display().to_string()),
            deployment_id: Some(format!("{remote}:{branch}")),
            canonical_url: github_pages_url(cfg, cfg.owner.as_deref(), cfg.repo.as_deref()),
            preview_url: None,
            custom_domain: non_empty(cfg.custom_domain.clone()),
            status: "dry-run".to_string(),
            stdout: None,
            stderr: None,
            manual_follow_up: Vec::new(),
        });
    }
    if worktree.exists() {
        fs::remove_dir_all(&worktree)
            .with_context(|| format!("failed to clean {}", worktree.display()))?;
    }
    if let Some(parent) = worktree.parent() {
        fs::create_dir_all(parent)?;
    }

    let clone_status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            branch,
            remote_url.trim(),
        ])
        .arg(&worktree)
        .status()
        .context("failed to run git clone for GitHub Pages branch")?;
    if !clone_status.success() {
        let status = Command::new("git")
            .args(["clone", "--depth", "1", remote_url.trim()])
            .arg(&worktree)
            .status()
            .context("failed to run git clone for GitHub Pages repository")?;
        if !status.success() {
            bail!("failed to clone {remote} for GitHub Pages publishing");
        }
        run_git(&worktree, ["checkout", "--orphan", branch])?;
    }

    let publish_root = if source_path == "/" || source_path == "." {
        worktree.clone()
    } else {
        worktree.join(source_path.trim_start_matches('/'))
    };
    clean_publish_root(&publish_root)?;
    copy_dir_contents(Path::new(&manifest.root_dir), &publish_root)?;
    fs::write(publish_root.join(".nojekyll"), "")?;
    if let Some(domain) = non_empty(cfg.custom_domain.clone()) {
        fs::write(publish_root.join("CNAME"), format!("{}\n", domain.trim()))?;
    }

    run_git(&worktree, ["add", "--all"])?;
    let commit = Command::new("git")
        .args(["commit", "-m", "Publish Fission static site"])
        .current_dir(&worktree)
        .output()
        .context("failed to run git commit for GitHub Pages")?;
    if !commit.status.success() {
        let stderr = String::from_utf8_lossy(&commit.stderr);
        if !stderr.contains("nothing to commit") && !stderr.contains("no changes added") {
            io::stderr().write_all(&commit.stderr).ok();
            bail!("git commit failed for GitHub Pages publish");
        }
    }
    run_git(&worktree, ["push", remote, branch])?;
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "github-pages".to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: Some(format!("{remote}:{branch}")),
        canonical_url: github_pages_url(cfg, cfg.owner.as_deref(), cfg.repo.as_deref()),
        preview_url: None,
        custom_domain: non_empty(cfg.custom_domain.clone()),
        status: "published".to_string(),
        stdout: None,
        stderr: None,
        manual_follow_up: github_pages_follow_up(cfg),
    })
}

fn publish_cloudflare_pages(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = cloudflare_config(config, &options.site)?;
    let project_name = cfg
        .project_name
        .as_deref()
        .context("distribution.cloudflare_pages.<site>.project_name is required")?;
    let mut args = vec![
        "pages".to_string(),
        "deploy".to_string(),
        manifest.root_dir.clone(),
        "--project-name".to_string(),
        project_name.to_string(),
    ];
    if let Some(environment) = cfg
        .environment
        .as_deref()
        .filter(|value| *value != "production")
    {
        args.push("--branch".to_string());
        args.push(environment.to_string());
    }
    run_publish_command(
        options,
        "cloudflare-pages",
        "wrangler",
        args,
        artifact_path,
        || cloudflare_url(&cfg),
    )
}

fn run_publish_command<F>(
    options: &DistributeOptions,
    provider: &str,
    program: &str,
    args: Vec<String>,
    artifact_path: &Path,
    canonical_url: F,
) -> Result<DistributionReceipt>
where
    F: FnOnce() -> Option<String>,
{
    if options.dry_run {
        println!("Would run: {} {}", program, args.join(" "));
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: provider.to_string(),
            site: options.site.clone(),
            action: "publish".to_string(),
            artifact_manifest: Some(artifact_path.display().to_string()),
            deployment_id: None,
            canonical_url: canonical_url(),
            preview_url: None,
            custom_domain: None,
            status: "dry-run".to_string(),
            stdout: None,
            stderr: None,
            manual_follow_up: Vec::new(),
        });
    }
    let output = Command::new(program)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!("failed to run {program}; install it or run readiness for remediation")
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        eprint!("{stderr}");
        bail!("{provider} publish failed with {}", output.status);
    }
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: provider.to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: None,
        canonical_url: canonical_url(),
        preview_url: first_url(&stdout),
        custom_domain: None,
        status: "published".to_string(),
        stdout: Some(stdout),
        stderr: (!stderr.trim().is_empty()).then_some(stderr),
        manual_follow_up: Vec::new(),
    })
}

fn github_pages_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = github_config(config, &options.site)?;
    let owner = cfg
        .owner
        .clone()
        .or_else(|| infer_github_owner(&options.project_dir));
    let repo = cfg
        .repo
        .clone()
        .or_else(|| infer_github_repo(&options.project_dir));
    let (Some(owner), Some(repo)) = (owner, repo) else {
        bail!("github-pages status requires owner and repo in fission.toml or a GitHub remote");
    };
    command_status_receipt(
        options,
        "github-pages",
        "gh",
        vec!["api".to_string(), format!("repos/{owner}/{repo}/pages")],
    )
}

fn cloudflare_pages_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = cloudflare_config(config, &options.site)?;
    let account_id = cfg
        .account_id
        .clone()
        .or_else(|| env::var("CLOUDFLARE_ACCOUNT_ID").ok())
        .context(
            "distribution.cloudflare_pages.<site>.account_id or CLOUDFLARE_ACCOUNT_ID is required",
        )?;
    let project_name = cfg
        .project_name
        .as_deref()
        .context("distribution.cloudflare_pages.<site>.project_name is required")?;
    let token = credentials::provider_secret(
        DistributionProvider::CloudflarePages,
        &["CLOUDFLARE_API_TOKEN"],
    )?
    .context("CLOUDFLARE_API_TOKEN or Fission vault credentials are required")?;
    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/pages/projects/{project_name}/deployments"
    );
    let response = reqwest::blocking::Client::builder()
        .user_agent("cargo-fission-publish/0.1")
        .build()?
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to query Cloudflare Pages deployments")?;
    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        bail!("Cloudflare Pages status failed with {status}: {text}");
    }
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse Cloudflare Pages status response: {text}"))?;
    let latest = value
        .get("result")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first());
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "cloudflare-pages".to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: latest
            .and_then(|item| item.get("id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        canonical_url: cloudflare_url(&cfg),
        preview_url: latest
            .and_then(|item| item.get("url"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        custom_domain: non_empty(cfg.custom_domain.clone()),
        status: latest
            .and_then(|item| item.pointer("/latest_stage/status"))
            .or_else(|| latest.and_then(|item| item.get("deployment_trigger")))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("ok")
            .to_string(),
        stdout: Some(serde_json::to_string_pretty(&value)?),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
}

fn command_status_receipt(
    options: &DistributeOptions,
    provider: &str,
    program: &str,
    args: Vec<String>,
) -> Result<DistributionReceipt> {
    let output = Command::new(program)
        .args(&args)
        .output()
        .with_context(|| format!("failed to run {program}; install it or authenticate first"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: provider.to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: options.deploy.clone(),
        canonical_url: first_url(&stdout),
        preview_url: None,
        custom_domain: None,
        status: if output.status.success() {
            "ok"
        } else {
            "failed"
        }
        .to_string(),
        stdout: Some(stdout),
        stderr: (!stderr.trim().is_empty()).then_some(stderr),
        manual_follow_up: Vec::new(),
    })
}

fn readiness_package(
    project_dir: &Path,
    target: Option<Target>,
    format: Option<PackageFormat>,
) -> Result<Vec<ReadinessCheck>> {
    let target = target.unwrap_or(Target::Site);
    let format = format.unwrap_or(PackageFormat::Static);
    let mut checks = Vec::new();
    let format_supported = package_format_supported(target, format);
    checks.push(check(
        "release.package.format_supported",
        CheckSeverity::Error,
        if format_supported {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "package format is supported for the selected target",
        Some(format!(
            "--target {} --format {}",
            target.as_str(),
            format.as_str()
        )),
        vec!["Use a valid target/format pair, such as site/static, linux/run, macos/app, macos/pkg, windows/exe, windows/msi, windows/msix, android/apk, android/aab, or ios/ipa."],
    ));
    checks.push(check_path(
        "release.package.fission_toml_exists",
        project_dir.join("fission.toml"),
        "fission.toml exists",
        "Run `fission init .` or point --project-dir at a Fission project.",
    ));
    if let Ok(project) = fission_command_core::read_project_config(project_dir) {
        checks.push(check(
            "release.package.target_configured",
            CheckSeverity::Error,
            if project.targets.contains(&target) {
                CheckStatus::Passed
            } else {
                CheckStatus::Missing
            },
            "target is configured in fission.toml",
            Some(format!("target = {}", target.as_str())),
            vec!["Run `fission add-target <target> --project-dir .` before packaging."],
        ));
    }
    if matches!(target, Target::Site) {
        let has_content = project_dir.join("content").exists();
        let has_entry = load_publish_manifest(project_dir)
            .ok()
            .and_then(|manifest| manifest.site.and_then(|site| site.entry))
            .is_some_and(|entry| !entry.trim().is_empty());
        checks.push(check(
            "release.package.site_content_or_entry",
            CheckSeverity::Error,
            if has_content || has_entry {
                CheckStatus::Passed
            } else {
                CheckStatus::Missing
            },
            "default content directory exists or custom site entry handles routing",
            Some(format!(
                "content: {}, site.entry: {}",
                project_dir.join("content").display(),
                has_entry
            )),
            vec!["Add content/ or configure [site].entry for a custom static site."],
        ));
    }
    readiness_package_tools(project_dir, target, format, &mut checks);
    package::readiness_secondary_artifacts(project_dir, &mut checks);
    Ok(checks)
}

fn package_format_supported(target: Target, format: PackageFormat) -> bool {
    matches!(
        (target, format),
        (Target::Site, PackageFormat::Static)
            | (Target::Web, PackageFormat::Static)
            | (Target::Linux, PackageFormat::Run)
            | (Target::Macos, PackageFormat::App)
            | (Target::Macos, PackageFormat::Pkg)
            | (Target::Windows, PackageFormat::Exe)
            | (Target::Windows, PackageFormat::Msi)
            | (Target::Windows, PackageFormat::Msix)
            | (Target::Android, PackageFormat::Apk)
            | (Target::Android, PackageFormat::Aab)
            | (Target::Ios, PackageFormat::Ipa)
    )
}

fn readiness_package_tools(
    project_dir: &Path,
    target: Target,
    format: PackageFormat,
    checks: &mut Vec<ReadinessCheck>,
) {
    match (target, format) {
        (Target::Site, PackageFormat::Static) | (Target::Web, PackageFormat::Static) => {
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
        }
        (Target::Linux, PackageFormat::Run) => {
            checks.push(host_os_check("release.package.host_is_linux", "linux"));
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
        }
        (Target::Macos, PackageFormat::App) => {
            checks.push(host_os_check("release.package.host_is_macos", "macos"));
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
            checks.push(check_tool(
                "release.package.codesign_available",
                "codesign",
                "Install Xcode command line tools so Fission can verify signed .app bundles.",
            ));
        }
        (Target::Macos, PackageFormat::Pkg) => {
            checks.push(host_os_check("release.package.host_is_macos", "macos"));
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
            checks.push(check_tool(
                "release.package.pkgbuild_available",
                "pkgbuild",
                "Install Xcode command line tools.",
            ));
            checks.push(check_tool(
                "release.package.productbuild_available",
                "productbuild",
                "Install Xcode command line tools.",
            ));
            checks.push(check_tool(
                "release.package.pkgutil_available",
                "pkgutil",
                "Install macOS package tools so Fission can inspect produced .pkg files.",
            ));
        }
        (Target::Windows, PackageFormat::Exe) => {
            checks.push(host_os_check("release.package.host_is_windows", "windows"));
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
        }
        (Target::Windows, PackageFormat::Msi) => {
            checks.push(host_os_check("release.package.host_is_windows", "windows"));
            checks.push(check_path(
                "release.package.windows_msi_script_exists",
                project_dir.join("platforms/windows/package-msi.ps1"),
                "Windows MSI packaging script exists",
                "Configure platforms/windows/package-msi.ps1 or install the Windows packaging target template.",
            ));
            checks.push(check_any_tool(
                "release.package.windows_msi_builder_available",
                &["wix", "candle"],
                "WiX MSI packaging tooling is available",
                "Install WiX Toolset or configure platforms/windows/package-msi.ps1 to call the approved MSI packager.",
            ));
            checks.push(check_tool(
                "release.package.signtool_available",
                "signtool",
                "Install Windows SDK signing tools and ensure signtool is on PATH.",
            ));
        }
        (Target::Windows, PackageFormat::Msix) => {
            checks.push(host_os_check("release.package.host_is_windows", "windows"));
            checks.push(check_path(
                "release.package.windows_msix_script_exists",
                project_dir.join("platforms/windows/package-msix.ps1"),
                "Windows MSIX packaging script exists",
                "Configure platforms/windows/package-msix.ps1 or install the Windows packaging target template.",
            ));
            checks.push(check_tool(
                "release.package.makeappx_available",
                "makeappx",
                "Install Windows SDK MSIX packaging tools and ensure makeappx is on PATH.",
            ));
            checks.push(check_tool(
                "release.package.signtool_available",
                "signtool",
                "Install Windows SDK signing tools and ensure signtool is on PATH.",
            ));
        }
        (Target::Android, PackageFormat::Apk) => {
            checks.push(check_path(
                "release.package.android_apk_script_exists",
                project_dir.join("platforms/android/package-apk.sh"),
                "Android APK packaging script exists",
                "Run `fission add-target android --project-dir .` or restore platforms/android/package-apk.sh.",
            ));
            android_packaging_checks(checks);
        }
        (Target::Android, PackageFormat::Aab) => {
            checks.push(check_path(
                "release.package.android_aab_script_exists",
                project_dir.join("platforms/android/package-aab.sh"),
                "Android AAB packaging script exists",
                "Add platforms/android/package-aab.sh once release AAB packaging is configured.",
            ));
            android_packaging_checks(checks);
            checks.push(check_env_or_tool(
                "release.package.bundletool_available",
                &["BUNDLETOOL"],
                &["bundletool"],
                "Android bundletool is available for AAB validation",
                "Install bundletool or set BUNDLETOOL to the bundletool jar/path used by the project packaging script.",
            ));
        }
        (Target::Ios, PackageFormat::Ipa) => {
            checks.push(host_os_check("release.package.host_is_macos", "macos"));
            checks.push(check_path(
                "release.package.ios_ipa_script_exists",
                project_dir.join("platforms/ios/package-ipa.sh"),
                "iOS IPA packaging script exists",
                "Add platforms/ios/package-ipa.sh once release IPA export is configured.",
            ));
            checks.push(check_tool(
                "release.package.xcrun_available",
                "xcrun",
                "Install Xcode command line tools and select an Xcode installation.",
            ));
            checks.push(check_tool(
                "release.package.xcodebuild_available",
                "xcodebuild",
                "Install Xcode so Fission can archive and export iOS IPA files.",
            ));
            checks.push(check_tool(
                "release.package.codesign_available",
                "codesign",
                "Install Xcode command line tools so Fission can verify iOS signing.",
            ));
        }
        _ => {}
    }
}

fn android_packaging_checks(checks: &mut Vec<ReadinessCheck>) {
    checks.push(check_tool(
        "release.package.cargo_available",
        "cargo",
        "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
    ));
    checks.push(check_any_env(
        "release.package.android_sdk_configured",
        &["ANDROID_HOME", "ANDROID_SDK_ROOT"],
        "Android SDK path is configured",
        "Set ANDROID_HOME or ANDROID_SDK_ROOT to the installed Android SDK.",
    ));
    checks.push(check_any_env(
        "release.package.android_ndk_configured",
        &["ANDROID_NDK_HOME", "ANDROID_NDK_ROOT"],
        "Android NDK path is configured",
        "Set ANDROID_NDK_HOME or ANDROID_NDK_ROOT to the installed Android NDK used by Rust cross-compilation.",
    ));
    checks.push(check_tool(
        "release.package.aapt2_available",
        "aapt2",
        "Install Android SDK build-tools and ensure aapt2 is on PATH.",
    ));
    checks.push(check_tool(
        "release.package.zipalign_available",
        "zipalign",
        "Install Android SDK build-tools and ensure zipalign is on PATH.",
    ));
    checks.push(check_tool(
        "release.package.apksigner_available",
        "apksigner",
        "Install Android SDK build-tools and ensure apksigner is on PATH.",
    ));
}

fn readiness_distribute(
    project_dir: &Path,
    provider: DistributionProvider,
    site: &str,
    track: Option<&str>,
    artifact: Option<&Path>,
    config: &PublishManifest,
) -> Result<Vec<ReadinessCheck>> {
    let mut checks = Vec::new();
    if let Some(path) = artifact {
        checks.push(check_path(
            "release.distribution.artifact_manifest_exists",
            path.to_path_buf(),
            "artifact manifest exists",
            "Run `fission package --target site --format static --release` first.",
        ));
        if path.exists() {
            let manifest = read_artifact_manifest(path)?;
            if provider_requires_static_root(provider) {
                checks.push(check(
                    "release.distribution.static_root_exists",
                    CheckSeverity::Error,
                    if Path::new(&manifest.root_dir).join("index.html").exists() {
                        CheckStatus::Passed
                    } else {
                        CheckStatus::Missing
                    },
                    "static artifact root contains index.html",
                    Some(manifest.root_dir),
                    vec!["Rebuild the static package and ensure the output includes index.html."],
                ));
            }
        }
    }

    match provider {
        DistributionProvider::GithubPages => {
            readiness_github_pages(project_dir, site, config, &mut checks)?
        }
        DistributionProvider::GithubReleases => {
            github_releases::readiness(project_dir, site, artifact, config, &mut checks)?
        }
        DistributionProvider::CloudflarePages => {
            readiness_cloudflare_pages(site, config, &mut checks)?
        }
        DistributionProvider::Netlify => readiness_netlify(site, config, &mut checks)?,
        DistributionProvider::S3 => files::readiness_s3(site, config, &mut checks)?,
        DistributionProvider::GoogleDrive => {
            files::readiness_google_drive(site, config, &mut checks)?
        }
        DistributionProvider::OneDrive => files::readiness_onedrive(site, config, &mut checks)?,
        DistributionProvider::Dropbox => files::readiness_dropbox(site, config, &mut checks)?,
        DistributionProvider::PlayStore => {
            stores::readiness_play_store(track, artifact, config, &mut checks)?
        }
        DistributionProvider::AppStore => {
            stores::readiness_app_store(track, artifact, config, &mut checks)?
        }
        DistributionProvider::MicrosoftStore => {
            stores::readiness_microsoft_store(track, artifact, config, &mut checks)?
        }
    }
    Ok(checks)
}

fn provider_requires_static_root(provider: DistributionProvider) -> bool {
    matches!(
        provider,
        DistributionProvider::GithubPages
            | DistributionProvider::CloudflarePages
            | DistributionProvider::Netlify
    )
}

fn readiness_github_pages(
    project_dir: &Path,
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = github_config(config, site)?;
    let owner = cfg
        .owner
        .clone()
        .or_else(|| infer_github_owner(project_dir));
    let repo = cfg.repo.clone().or_else(|| infer_github_repo(project_dir));
    checks.push(required_value(
        "release.github_pages.owner_configured",
        owner.as_deref(),
        "GitHub owner is configured or inferable from git remote",
        "Set distribution.github_pages.<site>.owner or configure an origin GitHub remote.",
    ));
    checks.push(required_value(
        "release.github_pages.repo_configured",
        repo.as_deref(),
        "GitHub repository is configured or inferable from git remote",
        "Set distribution.github_pages.<site>.repo or configure an origin GitHub remote.",
    ));
    let mode = cfg.mode.as_deref().unwrap_or("actions");
    checks.push(check(
        "release.github_pages.mode_supported",
        CheckSeverity::Error,
        if matches!(mode, "actions" | "branch" | "manual") {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "GitHub Pages mode is supported",
        Some(mode.to_string()),
        vec!["Use mode = \"actions\", \"branch\", or \"manual\"."],
    ));
    if mode == "branch" {
        checks.push(check_tool(
            "release.github_pages.git_available",
            "git",
            "Install Git and authenticate to the repository remote.",
        ));
    } else {
        checks.push(check(
            "release.github_pages.source_is_actions",
            CheckSeverity::Warning,
            if cfg.source.as_deref().unwrap_or("github-actions") == "github-actions" {
                CheckStatus::Passed
            } else {
                CheckStatus::Warning
            },
            "GitHub Pages source is configured for Actions publishing",
            cfg.source.clone(),
            vec!["Set distribution.github_pages.<site>.source = \"github-actions\" for Actions-based Pages publishing."],
        ));
        let workflow = cfg.workflow.as_deref().unwrap_or("fission-pages.yml");
        checks.push(check_path(
            "release.github_pages.workflow_exists",
            github_workflow_path(project_dir, &cfg, workflow),
            "GitHub Pages workflow exists",
            "Run `fission distribute setup --provider github-pages --site production` to generate it.",
        ));
        checks.push(check(
            "release.github_pages.local_api_token_optional",
            CheckSeverity::Info,
            if env::var_os("GH_TOKEN").is_some()
                || env::var_os("GITHUB_TOKEN").is_some()
                || credentials::provider_secret(DistributionProvider::GithubPages, &[])
                    .ok()
                    .flatten()
                    .is_some()
            {
                CheckStatus::Passed
            } else {
                CheckStatus::Skipped
            },
            "GitHub API token is available for local status/domain setup",
            None,
            vec!["For local Pages status or future domain setup automation, set GH_TOKEN/GITHUB_TOKEN or import a GitHub credential into the Fission vault."],
        ));
    }
    let base = cfg.base_path.as_deref().unwrap_or("/");
    let expected = expected_github_base_path(&cfg, repo.as_deref());
    checks.push(check(
        "release.github_pages.base_path_matches_domain_mode",
        CheckSeverity::Warning,
        if base == expected { CheckStatus::Passed } else { CheckStatus::Warning },
        "GitHub Pages base path matches custom-domain/project-site mode",
        Some(format!("configured {base}, expected {expected}")),
        vec!["Set distribution.github_pages.<site>.base_path to the expected value or adjust the site renderer base URL."],
    ));
    checks.push(check(
        "release.github_pages.https_policy_set",
        CheckSeverity::Info,
        if cfg.enforce_https.unwrap_or(true) {
            CheckStatus::Passed
        } else {
            CheckStatus::Warning
        },
        "GitHub Pages HTTPS policy is explicit",
        Some(format!("enforce_https = {}", cfg.enforce_https.unwrap_or(true))),
        vec!["Keep enforce_https = true for public production sites unless there is a provider limitation."],
    ));
    Ok(())
}

fn readiness_cloudflare_pages(
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = cloudflare_config(config, site)?;
    let env_account_id = env::var("CLOUDFLARE_ACCOUNT_ID").ok();
    checks.push(required_value(
        "release.cloudflare_pages.account_id_configured",
        cfg.account_id.as_deref().or(env_account_id.as_deref()),
        "Cloudflare account id is configured",
        "Set distribution.cloudflare_pages.<site>.account_id or CLOUDFLARE_ACCOUNT_ID.",
    ));
    checks.push(required_value(
        "release.cloudflare_pages.project_name_configured",
        cfg.project_name.as_deref(),
        "Cloudflare Pages project name is configured",
        "Set distribution.cloudflare_pages.<site>.project_name.",
    ));
    checks.push(required_provider_secret(
        "release.cloudflare_pages.token_available",
        DistributionProvider::CloudflarePages,
        &["CLOUDFLARE_API_TOKEN"],
        "Create a Cloudflare API token with Pages Edit permission and store it in CI secrets or the Fission release vault.",
    ));
    checks.push(check_tool(
        "release.cloudflare_pages.wrangler_available",
        "wrangler",
        "Install Wrangler and authenticate it; Cloudflare Pages upload intentionally uses the provider CLI backend.",
    ));
    checks.push(base_path_check(
        "release.cloudflare_pages.base_path_root",
        cfg.base_path.as_deref(),
    ));
    Ok(())
}

fn readiness_netlify(
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = netlify_config(config, site)?;
    checks.push(required_value(
        "release.netlify.site_configured",
        cfg.site_id.as_deref(),
        "Netlify site id is configured",
        "Set distribution.netlify.<site>.site_id or run provider setup after creating a Netlify site.",
    ));
    checks.push(required_provider_secret(
        "release.netlify.token_available",
        DistributionProvider::Netlify,
        &["NETLIFY_AUTH_TOKEN"],
        "Create a Netlify access token and store it in CI secrets, your shell environment, or the Fission release vault.",
    ));
    checks.push(base_path_check(
        "release.netlify.base_path_root",
        cfg.base_path.as_deref(),
    ));
    Ok(())
}

fn build_artifact_manifest(
    project: &FissionProject,
    options: &PackageOptions,
    root: &Path,
    profile: &str,
) -> Result<ArtifactManifest> {
    let mut files = Vec::new();
    collect_artifacts(root, root, &mut files)?;
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(ArtifactManifest {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        project: ArtifactProject {
            app_id: project.app.app_id.clone(),
            name: project.app.name.clone(),
            version: cargo_package_version(&options.project_dir),
        },
        target: options.target.as_str().to_string(),
        format: options.format.as_str().to_string(),
        profile: profile.to_string(),
        root_dir: root.display().to_string(),
        artifacts: files,
        validation: ArtifactValidation {
            state: "passed".to_string(),
            checks: Vec::new(),
        },
    })
}

fn collect_artifacts(root: &Path, current: &Path, files: &mut Vec<ArtifactFile>) -> Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("failed to read {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            collect_artifacts(root, &path, files)?;
        } else if file_type.is_file() {
            if path.file_name().and_then(OsStr::to_str) == Some(ARTIFACT_MANIFEST) {
                continue;
            }
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            let (sha256, size_bytes) = hash_file(&path)?;
            files.push(ArtifactFile {
                kind: if relative == "index.html" {
                    "entry"
                } else {
                    "asset"
                }
                .to_string(),
                purpose: None,
                platform: None,
                upload_provider: None,
                path: path.display().to_string(),
                relative_path: relative,
                sha256,
                size_bytes,
                mime_type: content_type(&path).to_string(),
            });
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buf = [0u8; 8192];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&buf[..read]);
    }
    Ok((hex_lower(&hasher.finalize()), size))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(OsStr::to_str).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" | "webmanifest" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn copy_dir_contents(source: &Path, dest: &Path) -> Result<()> {
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&dest_path)?;
            copy_dir_contents(&source_path, &dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &dest_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn clean_publish_root(root: &Path) -> Result<()> {
    fs::create_dir_all(root)?;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
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
    project_dir
        .join("target/fission")
        .join(if release { "release" } else { "debug" })
        .join(target.as_str())
        .join("static")
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
        "target/fission/release/site/static".to_string()
    } else {
        format!("{package_project_dir}/target/fission/release/site/static")
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
        run: fission package --project-dir {package_project_dir} --target site --format static --release

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

fn write_receipt(project_dir: &Path, receipt: &DistributionReceipt) -> Result<()> {
    let dir = project_dir
        .join("target/fission/distribution")
        .join(&receipt.provider)
        .join(&receipt.site);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "{}-{}.json",
        receipt.action, receipt.created_at_unix_seconds
    ));
    fs::write(&path, serde_json::to_vec_pretty(receipt)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn print_readiness_report(report: &ReadinessReport) {
    println!("Readiness: {}", report.status);
    print_checks(&report.checks);
}

fn print_checks(checks: &[ReadinessCheck]) {
    for check in checks {
        println!(
            "[{:?}/{:?}] {} - {}",
            check.severity, check.status, check.id, check.summary
        );
        if let Some(details) = &check.details {
            println!("  {details}");
        }
        for remediation in &check.remediation {
            println!("  fix: {remediation}");
        }
    }
}

fn report_status(checks: &[ReadinessCheck]) -> &'static str {
    if checks
        .iter()
        .any(|check| check.severity == CheckSeverity::Error && check.status != CheckStatus::Passed)
    {
        "blocked"
    } else if checks
        .iter()
        .any(|check| check.status == CheckStatus::Warning)
    {
        "warning"
    } else {
        "ready"
    }
}

fn check(
    id: impl Into<String>,
    severity: CheckSeverity,
    status: CheckStatus,
    summary: impl Into<String>,
    details: Option<String>,
    remediation: Vec<&str>,
) -> ReadinessCheck {
    ReadinessCheck {
        id: id.into(),
        severity,
        status,
        summary: summary.into(),
        details,
        remediation: remediation.into_iter().map(str::to_string).collect(),
    }
}

fn check_path(id: &str, path: PathBuf, summary: &str, remediation: &str) -> ReadinessCheck {
    check(
        id,
        CheckSeverity::Error,
        if path.exists() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        Some(path.display().to_string()),
        vec![remediation],
    )
}

fn required_value(
    id: &str,
    value: Option<&str>,
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    check(
        id,
        CheckSeverity::Error,
        if value.is_some_and(|value| !value.trim().is_empty()) {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        value.map(str::to_string),
        vec![remediation],
    )
}

fn required_provider_secret(
    id: &str,
    provider: DistributionProvider,
    env_names: &[&str],
    remediation: &str,
) -> ReadinessCheck {
    let env_name = env_names.iter().find(|name| env::var_os(name).is_some());
    let vault_present = credentials::provider_secret(provider, &[])
        .ok()
        .flatten()
        .is_some();
    check(
        id,
        CheckSeverity::Error,
        if env_name.is_some() || vault_present {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "provider credentials are available",
        env_name
            .map(|name| format!("environment variable {name}"))
            .or_else(|| vault_present.then(|| "Fission release vault".to_string())),
        vec![remediation],
    )
}

fn base_path_check(id: &str, base_path: Option<&str>) -> ReadinessCheck {
    let value = base_path.unwrap_or("/");
    check(
        id,
        CheckSeverity::Warning,
        if value == "/" {
            CheckStatus::Passed
        } else {
            CheckStatus::Warning
        },
        "static hosting provider base path is root",
        Some(format!("base_path = {value}")),
        vec!["Dedicated static hosting providers usually serve production sites from `/`; use a non-root base path only when deliberately hosting below a subpath."],
    )
}

fn host_os_check(id: &str, expected: &str) -> ReadinessCheck {
    let current = env::consts::OS;
    check(
        id,
        CheckSeverity::Error,
        if current == expected {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        format!("host operating system is {expected}"),
        Some(format!("current host: {current}")),
        vec!["Run this package format on the platform that owns the native packaging/signing toolchain."],
    )
}

fn check_tool(id: &str, tool: &str, remediation: &str) -> ReadinessCheck {
    check(
        id,
        CheckSeverity::Error,
        if find_in_path(tool).is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        format!("{tool} is available on PATH"),
        find_in_path(tool).map(|path| path.display().to_string()),
        vec![remediation],
    )
}

fn check_any_tool(id: &str, tools: &[&str], summary: &str, remediation: &str) -> ReadinessCheck {
    let found = tools
        .iter()
        .find_map(|tool| find_in_path(tool).map(|path| (*tool, path)));
    check(
        id,
        CheckSeverity::Error,
        if found.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        found
            .map(|(tool, path)| format!("{tool}: {}", path.display()))
            .or_else(|| Some(format!("checked: {}", tools.join(", ")))),
        vec![remediation],
    )
}

fn check_any_env(id: &str, names: &[&str], summary: &str, remediation: &str) -> ReadinessCheck {
    let found = names.iter().find(|name| env::var_os(name).is_some());
    check(
        id,
        CheckSeverity::Error,
        if found.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        found.map(|name| format!("{name}={}", env::var(name).unwrap_or_default())),
        vec![remediation],
    )
}

fn check_env_or_tool(
    id: &str,
    env_names: &[&str],
    tools: &[&str],
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    let found_env = env_names.iter().find(|name| env::var_os(name).is_some());
    let found_tool = tools
        .iter()
        .find_map(|tool| find_in_path(tool).map(|path| (*tool, path)));
    check(
        id,
        CheckSeverity::Error,
        if found_env.is_some() || found_tool.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        found_env
            .map(|name| format!("{name}={}", env::var(name).unwrap_or_default()))
            .or_else(|| found_tool.map(|(tool, path)| format!("{tool}: {}", path.display()))),
        vec![remediation],
    )
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
        if cfg!(windows) {
            for extension in ["exe", "cmd", "bat", "ps1"] {
                let candidate = dir.join(format!("{name}.{extension}"));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cargo_package_version(project_dir: &Path) -> Option<String> {
    let data = fs::read_to_string(project_dir.join("Cargo.toml")).ok()?;
    let value: toml::Value = toml::from_str(&data).ok()?;
    value
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(|version| version.as_str())
        .map(str::to_string)
}

fn resolve_project_path(project_dir: &Path, path: String) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn expected_github_base_path(cfg: &GithubPagesConfig, repo: Option<&str>) -> String {
    if cfg
        .custom_domain
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        "/".to_string()
    } else if cfg.site_kind.as_deref() == Some("user")
        || cfg.site_kind.as_deref() == Some("organization")
    {
        "/".to_string()
    } else {
        repo.map(|repo| format!("/{repo}/"))
            .unwrap_or_else(|| "/".to_string())
    }
}

fn github_pages_url(
    cfg: &GithubPagesConfig,
    owner: Option<&str>,
    repo: Option<&str>,
) -> Option<String> {
    if let Some(domain) = cfg
        .custom_domain
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(format!("https://{}", domain.trim()));
    }
    let owner = owner?;
    if cfg.site_kind.as_deref() == Some("user") || cfg.site_kind.as_deref() == Some("organization")
    {
        Some(format!("https://{owner}.github.io/"))
    } else {
        repo.map(|repo| format!("https://{owner}.github.io/{repo}/"))
    }
}

fn cloudflare_url(cfg: &CloudflarePagesConfig) -> Option<String> {
    if let Some(domain) = cfg
        .custom_domain
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        Some(format!("https://{}", domain.trim()))
    } else {
        cfg.project_name
            .as_ref()
            .map(|name| format!("https://{name}.pages.dev"))
    }
}

fn github_pages_follow_up(cfg: &GithubPagesConfig) -> Vec<String> {
    let mut follow_up = Vec::new();
    if cfg
        .custom_domain
        .as_deref()
        .filter(|value| !value.is_empty())
        .is_some()
    {
        follow_up.push(
            "Verify the GitHub Pages custom domain and HTTPS state in repository settings."
                .to_string(),
        );
    }
    follow_up
}

fn infer_github_owner(project_dir: &Path) -> Option<String> {
    parse_github_remote(project_dir).map(|(owner, _)| owner)
}

fn infer_github_repo(project_dir: &Path) -> Option<String> {
    parse_github_remote(project_dir).map(|(_, repo)| repo)
}

fn parse_github_remote(project_dir: &Path) -> Option<(String, String)> {
    let remote = git_output(project_dir, ["remote", "get-url", "origin"]).ok()?;
    let remote = remote.trim().trim_end_matches(".git");
    if let Some(rest) = remote.strip_prefix("git@github.com:") {
        let (owner, repo) = rest.split_once('/')?;
        return Some((owner.to_string(), repo.to_string()));
    }
    if let Some(rest) = remote.strip_prefix("https://github.com/") {
        let (owner, repo) = rest.split_once('/')?;
        return Some((owner.to_string(), repo.to_string()));
    }
    None
}

fn git_repo_root(project_dir: &Path) -> Option<PathBuf> {
    git_output(project_dir, ["rev-parse", "--show-toplevel"])
        .ok()
        .map(|value| PathBuf::from(value.trim()))
}

fn git_output<'a, I>(dir: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        bail!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git<'a, I>(dir: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = &'a str>,
{
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .context("failed to run git")?;
    if !status.success() {
        bail!("git command failed with {status}");
    }
    Ok(())
}

fn first_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|part| part.starts_with("https://") || part.starts_with("http://"))
        .map(|value| {
            value
                .trim_matches(|c| c == ',' || c == ')' || c == '(')
                .to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fission-publish-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn write_minimal_site(dir: &Path) {
        fs::create_dir_all(dir.join("content")).unwrap();
        fs::write(
            dir.join("fission.toml"),
            r#"targets = ["site"]

[app]
name = "site-demo"
app_id = "com.example.site_demo"

[site]
title = "Site Demo"
out_dir = "dist/site"
generate_sitemap = false
generate_robots = false

[distribution.github_pages.production]
owner = "example"
repo = "site-demo"
mode = "actions"
site_kind = "project"
base_path = "/site-demo/"

[distribution.github_releases.production]
owner = "example"
repo = "site-demo"
tag = "v1.2.3"
name = "Site Demo 1.2.3"
draft = true
prerelease = false
replace_assets = true
upload_artifact_manifest = true
"#,
        )
        .unwrap();
        fs::write(
            dir.join("content/index.md"),
            "---\ntitle: Home\n---\n# Home\n",
        )
        .unwrap();
    }

    #[test]
    fn static_package_builds_artifact_manifest() {
        let dir = unique_dir("package");
        write_minimal_site(&dir);
        let manifest = package::package_static(&PackageOptions {
            project_dir: dir.clone(),
            target: Target::Site,
            format: PackageFormat::Static,
            release: true,
            json: false,
        })
        .unwrap();
        assert_eq!(manifest.target, "site");
        assert!(dir
            .join("target/fission/release/site/static/artifact-manifest.json")
            .exists());
        assert!(manifest
            .artifacts
            .iter()
            .any(|file| file.relative_path == "index.html"));
        assert!(dir
            .join("target/fission/release/site/static/fission-route-manifest.json")
            .exists());
        assert!(dir
            .join("target/fission/release/site/static/fission-mime-map.json")
            .exists());
        assert!(manifest
            .validation
            .checks
            .iter()
            .any(|check| check.id == "release.package.artifact.primary_present"));
    }

    #[test]
    fn static_package_includes_configured_secondary_artifacts() {
        let dir = unique_dir("secondary-artifacts");
        write_minimal_site(&dir);
        fs::create_dir_all(dir.join("release-content/symbols")).unwrap();
        fs::write(dir.join("release-content/symbols/app.dSYM.zip"), b"symbols").unwrap();
        let mut toml = fs::read_to_string(dir.join("fission.toml")).unwrap();
        toml.push_str(
            r#"
[[package.symbols]]
path = "release-content/symbols/app.dSYM.zip"
platform = "ios"
upload_provider = "crash-service"
"#,
        );
        fs::write(dir.join("fission.toml"), toml).unwrap();

        let manifest = package::package_static(&PackageOptions {
            project_dir: dir.clone(),
            target: Target::Site,
            format: PackageFormat::Static,
            release: true,
            json: false,
        })
        .unwrap();

        let symbols = manifest
            .artifacts
            .iter()
            .find(|file| file.kind == "debug_symbols")
            .expect("debug symbols should be present");
        assert_eq!(symbols.platform.as_deref(), Some("ios"));
        assert_eq!(symbols.upload_provider.as_deref(), Some("crash-service"));
    }

    #[test]
    fn github_pages_setup_writes_workflow() {
        let dir = unique_dir("github-setup");
        write_minimal_site(&dir);
        let config = load_publish_manifest(&dir).unwrap();
        setup_github_pages(
            &DistributeOptions {
                project_dir: dir.clone(),
                provider: DistributionProvider::GithubPages,
                action: DistributeAction::Setup,
                artifact: None,
                site: "production".to_string(),
                deploy: None,
                track: None,
                dry_run: false,
                yes: true,
                json: false,
            },
            &config,
        )
        .unwrap();
        let workflow = fs::read_to_string(dir.join(".github/workflows/fission-pages.yml")).unwrap();
        assert!(workflow.contains("actions/upload-pages-artifact"));
        assert!(workflow.contains("actions/deploy-pages"));
        assert!(workflow.contains("fission package"));
    }

    #[test]
    fn github_base_path_accounts_for_custom_domain() {
        let cfg = GithubPagesConfig {
            custom_domain: Some("docs.example.com".to_string()),
            repo: Some("repo".to_string()),
            ..Default::default()
        };
        assert_eq!(expected_github_base_path(&cfg, Some("repo")), "/");
        let cfg = GithubPagesConfig {
            repo: Some("repo".to_string()),
            ..Default::default()
        };
        assert_eq!(expected_github_base_path(&cfg, Some("repo")), "/repo/");
    }

    #[test]
    fn android_aab_readiness_checks_official_toolchain() {
        let dir = unique_dir("android-aab-readiness");
        write_minimal_site(&dir);
        let checks = readiness_package(&dir, Some(Target::Android), Some(PackageFormat::Aab))
            .expect("readiness should produce checks even when blocked");
        for id in [
            "release.package.android_aab_script_exists",
            "release.package.android_sdk_configured",
            "release.package.android_ndk_configured",
            "release.package.aapt2_available",
            "release.package.zipalign_available",
            "release.package.apksigner_available",
            "release.package.bundletool_available",
        ] {
            assert!(checks.iter().any(|check| check.id == id), "missing {id}");
        }
    }

    #[test]
    fn cloudflare_readiness_requires_wrangler_backend() {
        let dir = unique_dir("cloudflare-readiness");
        write_minimal_site(&dir);
        let mut toml = fs::read_to_string(dir.join("fission.toml")).unwrap();
        toml.push_str(
            r#"
[distribution.cloudflare_pages.production]
account_id = "account"
project_name = "site-demo"
"#,
        );
        fs::write(dir.join("fission.toml"), toml).unwrap();
        let config = load_publish_manifest(&dir).unwrap();
        let checks = readiness_distribute(
            &dir,
            DistributionProvider::CloudflarePages,
            "production",
            None,
            None,
            &config,
        )
        .unwrap();
        assert!(checks
            .iter()
            .any(|check| check.id == "release.cloudflare_pages.wrangler_available"));
    }

    #[test]
    fn github_releases_readiness_is_not_static_site_specific() {
        let dir = unique_dir("github-releases-readiness");
        write_minimal_site(&dir);
        let artifact_root = dir.join("target/fission/release/linux/run");
        fs::create_dir_all(&artifact_root).unwrap();
        let binary = artifact_root.join("site-demo.run");
        fs::write(&binary, b"run").unwrap();
        let manifest_path = artifact_root.join(ARTIFACT_MANIFEST);
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&ArtifactManifest {
                schema_version: 1,
                created_at_unix_seconds: 0,
                project: ArtifactProject {
                    app_id: "com.example.site_demo".to_string(),
                    name: "site-demo".to_string(),
                    version: Some("1.2.3".to_string()),
                },
                target: "linux".to_string(),
                format: "run".to_string(),
                profile: "release".to_string(),
                root_dir: artifact_root.display().to_string(),
                artifacts: vec![ArtifactFile {
                    kind: "asset".to_string(),
                    purpose: None,
                    platform: None,
                    upload_provider: None,
                    path: binary.display().to_string(),
                    relative_path: "site-demo.run".to_string(),
                    sha256: "abc".to_string(),
                    size_bytes: 3,
                    mime_type: "application/octet-stream".to_string(),
                }],
                validation: ArtifactValidation {
                    state: "passed".to_string(),
                    checks: Vec::new(),
                },
            })
            .unwrap(),
        )
        .unwrap();
        let config = load_publish_manifest(&dir).unwrap();
        let checks = readiness_distribute(
            &dir,
            DistributionProvider::GithubReleases,
            "production",
            None,
            Some(&manifest_path),
            &config,
        )
        .unwrap();
        assert!(checks.iter().any(|check| {
            check.id == "release.github_releases.assets_available"
                && check.status == CheckStatus::Passed
        }));
        assert!(!checks
            .iter()
            .any(|check| check.id == "release.distribution.static_root_exists"));
    }

    #[test]
    fn microsoft_store_msix_readiness_uses_msstore_not_package_url() {
        let dir = unique_dir("microsoft-msix-readiness");
        write_minimal_site(&dir);
        let mut toml = fs::read_to_string(dir.join("fission.toml")).unwrap();
        toml.push_str(
            r#"
[distribution.microsoft_store]
product_id = "9N1234567890"
package_identity_name = "Example.SiteDemo"
package_type = "msix"
"#,
        );
        fs::write(dir.join("fission.toml"), toml).unwrap();

        let artifact_root = dir.join("target/fission/release/windows/msix");
        fs::create_dir_all(&artifact_root).unwrap();
        let package = artifact_root.join("site-demo.msixupload");
        fs::write(&package, b"msix").unwrap();
        let manifest_path = artifact_root.join(ARTIFACT_MANIFEST);
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&ArtifactManifest {
                schema_version: 1,
                created_at_unix_seconds: 0,
                project: ArtifactProject {
                    app_id: "com.example.site_demo".to_string(),
                    name: "site-demo".to_string(),
                    version: Some("1.2.3".to_string()),
                },
                target: "windows".to_string(),
                format: "msix".to_string(),
                profile: "release".to_string(),
                root_dir: artifact_root.display().to_string(),
                artifacts: vec![ArtifactFile {
                    kind: "installer".to_string(),
                    purpose: Some("store-upload".to_string()),
                    platform: Some("windows".to_string()),
                    upload_provider: Some("microsoft-store".to_string()),
                    path: package.display().to_string(),
                    relative_path: "site-demo.msixupload".to_string(),
                    sha256: "abc".to_string(),
                    size_bytes: 4,
                    mime_type: "application/vnd.ms-appx".to_string(),
                }],
                validation: ArtifactValidation {
                    state: "passed".to_string(),
                    checks: Vec::new(),
                },
            })
            .unwrap(),
        )
        .unwrap();

        let config = load_publish_manifest(&dir).unwrap();
        let checks = readiness_distribute(
            &dir,
            DistributionProvider::MicrosoftStore,
            "production",
            None,
            Some(&manifest_path),
            &config,
        )
        .unwrap();

        assert!(checks
            .iter()
            .any(|check| check.id == "release.microsoft_store.msstore_available"));
        assert!(checks.iter().any(|check| {
            check.id == "release.microsoft_store.msix_upload_artifact_present"
                && check.status == CheckStatus::Passed
        }));
        assert!(!checks
            .iter()
            .any(|check| check.id == "release.microsoft_store.package_url_configured"));
    }
}
