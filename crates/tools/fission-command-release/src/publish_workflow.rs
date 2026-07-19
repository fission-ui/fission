use super::*;
use fission_command_core::{resolve_release_version_config, sync_release_platform_config};
use fission_command_package::{
    self as package_cmd, CheckSeverity, CheckStatus, DistributeAction, DistributeOptions,
    PackageFormat, PackageOptions, ReadinessCheck,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;

#[path = "publish_workflow_capabilities.rs"]
mod publish_workflow_capabilities;
use publish_workflow_capabilities::*;
#[path = "publish_workflow_steps.rs"]
mod publish_workflow_steps;
use publish_workflow_steps::*;

#[derive(Clone, Debug)]
pub struct PublishWorkflowOptions {
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReleasePlanSnapshot {
    pub context: ReleaseContextSnapshot,
    pub project_dir: String,
    pub provider: String,
    pub target: Option<String>,
    pub format: Option<String>,
    pub artifact: Option<String>,
    pub track: Option<String>,
    pub locales: Vec<String>,
    pub status: String,
    pub steps: Vec<ReleaseStepSnapshot>,
    pub capabilities: Vec<ProviderCapabilitySnapshot>,
    pub requirements: Vec<ReleaseRequirementSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReleaseContextSnapshot {
    pub project_dir: String,
    pub app_name: Option<String>,
    pub target: Option<String>,
    pub format: Option<String>,
    pub provider: String,
    pub release_id: Option<String>,
    pub track: Option<String>,
    pub locales: Vec<String>,
    pub interactive: bool,
    pub ci: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ProviderCapabilitySnapshot {
    pub id: String,
    pub status: String,
    pub summary: String,
    pub details: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReleaseRequirementSnapshot {
    pub id: String,
    pub level: String,
    pub status: String,
    pub summary: String,
    pub details: Option<String>,
    pub remediation: Vec<String>,
    pub can_fix_interactively: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReleaseStepSnapshot {
    pub id: String,
    pub title: String,
    pub status: String,
    pub jobs: Vec<ReleaseJobSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReleaseJobSnapshot {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
struct ReleasePlanReport {
    context: ReleaseContextReport,
    project_dir: String,
    provider: String,
    target: Option<String>,
    format: Option<String>,
    artifact: Option<String>,
    track: Option<String>,
    locales: Vec<String>,
    status: String,
    steps: Vec<ReleaseStep>,
    capabilities: Vec<ProviderCapability>,
    requirements: Vec<ReleaseRequirement>,
}

#[derive(Debug, Serialize)]
struct ReleaseContextReport {
    project_dir: String,
    app_name: Option<String>,
    target: Option<String>,
    format: Option<String>,
    provider: String,
    release_id: Option<String>,
    track: Option<String>,
    locales: Vec<String>,
    interactive: bool,
    ci: bool,
}

#[derive(Debug, Serialize)]
struct ReleaseRequirement {
    id: String,
    level: RequirementLevel,
    status: RequirementStatus,
    summary: String,
    details: Option<String>,
    remediation: Vec<String>,
    can_fix_interactively: bool,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseSkipToml {
    release: Option<ReleaseSkipRoot>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseSkipRoot {
    #[serde(default)]
    skip_requirements: BTreeSet<String>,
}

#[derive(Debug, Serialize)]
struct ReleaseStep {
    id: String,
    title: String,
    status: StepStatus,
    jobs: Vec<ReleaseJob>,
}

#[derive(Debug, Serialize)]
struct ReleaseJob {
    id: String,
    title: String,
    status: StepStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum StepStatus {
    Pending,
    Blocked,
    Warning,
    Ready,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RequirementLevel {
    ProviderRequired,
    FissionRecommended,
    Optional,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RequirementStatus {
    Passed,
    Missing,
    Failed,
    Warning,
    Skipped,
}

#[derive(Debug, Serialize)]
struct PublishWorkflowEvent {
    at_unix_seconds: u64,
    id: String,
    status: String,
    details: Option<String>,
}

#[derive(Debug, Serialize)]
struct PublishWorkflowReceipt<'a> {
    schema_version: u32,
    created_at_unix_seconds: u64,
    status: String,
    dry_run: bool,
    provider: String,
    target: Option<String>,
    format: Option<String>,
    artifact: Option<String>,
    track: Option<String>,
    locales: Vec<String>,
    release_id: Option<String>,
    version: Option<String>,
    build: Option<u64>,
    artifact_hash: Option<String>,
    artifact_manifest_sha256: Option<String>,
    release_content_assets: Vec<Value>,
    uploaded_bytes: u64,
    uploaded_assets: Vec<Value>,
    provider_deployment_id: Option<String>,
    canonical_url: Option<String>,
    preview_url: Option<String>,
    manual_follow_up: Vec<String>,
    omitted_requirements: Vec<ReceiptRequirement>,
    release_plan: &'a ReleasePlanReport,
    release_metadata: Option<Value>,
    release_content: Option<Value>,
    distribution: Option<Value>,
    provider_status: Option<Value>,
    events: &'a [PublishWorkflowEvent],
}

#[derive(Debug, Serialize)]
struct ReceiptRequirement {
    id: String,
    level: RequirementLevel,
    status: RequirementStatus,
    summary: String,
}

#[derive(Debug, Default)]
struct ReceiptDetails {
    release_id: Option<String>,
    version: Option<String>,
    build: Option<u64>,
    artifact_hash: Option<String>,
    artifact_manifest_sha256: Option<String>,
    release_content_assets: Vec<Value>,
    uploaded_bytes: u64,
    uploaded_assets: Vec<Value>,
    provider_deployment_id: Option<String>,
    canonical_url: Option<String>,
    preview_url: Option<String>,
    manual_follow_up: Vec<String>,
    omitted_requirements: Vec<ReceiptRequirement>,
}

pub fn readiness_release(options: PublishWorkflowOptions) -> Result<()> {
    let options = publish_options_with_defaults(options)?;
    let report = build_release_plan(&options, options.artifact.as_deref())?;
    print_release_plan(&report, options.json)?;
    if report.status == "blocked" {
        bail!("release readiness failed");
    }
    Ok(())
}

pub fn release_readiness_checks(options: PublishWorkflowOptions) -> Result<Vec<ReadinessCheck>> {
    let options = publish_options_with_defaults(options)?;
    let report = build_release_plan(&options, options.artifact.as_deref())?;
    let mut checks = report
        .requirements
        .into_iter()
        .map(release_requirement_readiness_check)
        .collect::<Vec<_>>();
    checks.extend(
        report
            .capabilities
            .into_iter()
            .map(provider_capability_readiness_check),
    );
    Ok(checks)
}

pub fn release_plan_snapshot(options: PublishWorkflowOptions) -> Result<ReleasePlanSnapshot> {
    let options = publish_options_with_defaults(options)?;
    let report = build_release_plan(&options, options.artifact.as_deref())?;
    Ok(release_plan_snapshot_from_report(report))
}

pub fn publish_workflow(mut options: PublishWorkflowOptions) -> Result<()> {
    options = publish_options_with_defaults(options)?;
    let mut events = Vec::new();
    record_event(&mut events, "workflow.started", "running", None);
    if !options.dry_run && !options.yes {
        record_event(
            &mut events,
            "workflow.confirmation",
            "blocked",
            Some("--yes is required before mutating provider state".to_string()),
        );
        let report = build_release_plan(&options, options.artifact.as_deref())?;
        let receipt_path = write_publish_workflow_receipt(
            &options, &report, "blocked", None, None, None, None, &events,
        )?;
        if options.json {
            print_publish_workflow_json(
                &report,
                None,
                None,
                None,
                None,
                &events,
                Some(&receipt_path),
            )?;
        } else {
            print_release_plan(&report, options.json)?;
        }
        bail!("publishing changes provider state; pass --yes after reviewing the release plan or use --dry-run");
    }

    record_event(&mut events, "release.version.resolve", "started", None);
    let resolved = resolve_release_version_config(&options.project_dir, options.target)?;
    if let Some(target) = options.target {
        if options.artifact.is_none() {
            sync_release_platform_config(&options.project_dir, target, &resolved)?;
            record_event(
                &mut events,
                "release.platform_config.sync",
                "completed",
                Some(target.as_str().to_string()),
            );
        } else {
            record_event(
                &mut events,
                "release.platform_config.sync",
                "skipped",
                Some("existing artifact supplied; native files are not rewritten".to_string()),
            );
        }
    }
    record_event(&mut events, "release.version.resolve", "completed", None);

    record_event(&mut events, "release.preflight", "started", None);
    let preflight = build_release_plan(&options, options.artifact.as_deref())?;
    record_event(
        &mut events,
        "release.preflight",
        preflight.status.as_str(),
        Some(format!("{} requirement(s)", preflight.requirements.len())),
    );
    if preflight.status == "blocked" {
        record_event(&mut events, "workflow.blocked", "blocked", None);
        let receipt_path = write_publish_workflow_receipt(
            &options, &preflight, "blocked", None, None, None, None, &events,
        )?;
        if options.json {
            print_publish_workflow_json(
                &preflight,
                None,
                None,
                None,
                None,
                &events,
                Some(&receipt_path),
            )?;
        } else {
            print_release_plan(&preflight, options.json)?;
        }
        bail!("release readiness failed before packaging");
    }

    let artifact_path = if let Some(path) = options.artifact.clone() {
        record_event(
            &mut events,
            "package.artifact",
            "reused",
            Some(path.display().to_string()),
        );
        path
    } else {
        let target = options
            .target
            .context("publish without --artifact requires --target")?;
        let format = options
            .format
            .context("publish without --artifact requires --format")?;
        record_event(
            &mut events,
            "package.artifact",
            "started",
            Some(format!("{} {}", target.as_str(), format.as_str())),
        );
        let path = match package_cmd::package_silent(PackageOptions {
            project_dir: options.project_dir.clone(),
            target,
            format,
            release: true,
            variant: None,
            json: options.json,
        }) {
            Ok(path) => path,
            Err(error) => {
                return fail_publish_workflow(
                    &options,
                    &preflight,
                    &mut events,
                    "package.artifact",
                    error.to_string(),
                    None,
                    None,
                    None,
                    None,
                );
            }
        };
        record_event(
            &mut events,
            "package.artifact",
            "completed",
            Some(path.display().to_string()),
        );
        path
    };
    options.artifact = Some(artifact_path.clone());

    record_event(&mut events, "release.plan", "started", None);
    let report = build_release_plan(&options, Some(&artifact_path))?;
    record_event(
        &mut events,
        "release.plan",
        report.status.as_str(),
        Some(format!("{} requirement(s)", report.requirements.len())),
    );
    if options.dry_run && !options.json {
        print_release_plan(&report, options.json)?;
    }
    if report.status == "blocked" {
        record_event(&mut events, "workflow.blocked", "blocked", None);
        let receipt_path = write_publish_workflow_receipt(
            &options, &report, "blocked", None, None, None, None, &events,
        )?;
        if options.json {
            print_publish_workflow_json(
                &report,
                None,
                None,
                None,
                None,
                &events,
                Some(&receipt_path),
            )?;
        }
        bail!("release readiness failed");
    }
    record_event(&mut events, "release.content_manifest", "started", None);
    let content_manifest =
        match content::materialize_release_content_manifest(&options.project_dir, options.provider)
        {
            Ok(manifest) => manifest,
            Err(error) => {
                let message = error.to_string();
                let release_content = json!({
                    "provider": options.provider.as_str(),
                    "status": "failed",
                    "stage": "release.content_manifest",
                    "error": message,
                });
                return fail_publish_workflow(
                    &options,
                    &report,
                    &mut events,
                    "release.content_manifest",
                    error.to_string(),
                    None,
                    Some(release_content),
                    None,
                    None,
                );
            }
        };
    record_event(
        &mut events,
        "release.content_manifest",
        if content_manifest.is_some() {
            "completed"
        } else {
            "skipped"
        },
        content_manifest
            .as_ref()
            .map(|path| path.display().to_string()),
    );
    record_event(&mut events, "release.metadata", "started", None);
    let metadata = match maybe_sync_release_metadata(&options, &report, options.dry_run) {
        Ok(metadata) => metadata,
        Err(error) => {
            let message = error.to_string();
            let metadata = json!({
                "provider": options.provider.as_str(),
                "status": "failed",
                "stage": "release.metadata",
                "error": message,
            });
            return fail_publish_workflow(
                &options,
                &report,
                &mut events,
                "release.metadata",
                error.to_string(),
                Some(metadata),
                None,
                None,
                None,
            );
        }
    };
    record_event(
        &mut events,
        "release.metadata",
        if metadata.is_some() {
            if options.dry_run {
                "dry-run"
            } else {
                "completed"
            }
        } else {
            "skipped"
        },
        metadata.as_ref().map(release_metadata_message_detail),
    );
    if !options.json {
        if let Some(metadata) = &metadata {
            println!("{}", release_metadata_message(metadata, options.dry_run));
        }
    }
    record_event(&mut events, "release.content", "started", None);
    let release_content = match maybe_sync_release_content(&options, &report, options.dry_run) {
        Ok(release_content) => release_content,
        Err(error) => {
            let message = error.to_string();
            let release_content = json!({
                "provider": options.provider.as_str(),
                "status": "failed",
                "stage": "release.content",
                "error": message,
            });
            return fail_publish_workflow(
                &options,
                &report,
                &mut events,
                "release.content",
                error.to_string(),
                metadata.clone(),
                Some(release_content),
                None,
                None,
            );
        }
    };
    let release_content_event_status = release_content
        .as_ref()
        .map(|value| release_content_status(value, options.dry_run))
        .unwrap_or_else(|| "skipped".to_string());
    record_event(
        &mut events,
        "release.content",
        &release_content_event_status,
        release_content.as_ref().map(release_content_message_detail),
    );
    if !options.json {
        if let Some(release_content) = &release_content {
            println!(
                "{}",
                release_content_message(release_content, options.dry_run)
            );
        }
    }
    record_event(
        &mut events,
        "distribution.publish",
        "started",
        Some(options.provider.as_str().to_string()),
    );
    let distribution_outcome = match package_cmd::distribute_publish_outcome(distribute_options(
        &options,
        options.dry_run,
    )) {
        Ok(outcome) => outcome,
        Err(error) => {
            let message = error.to_string();
            let distribution = json!({
                "provider": options.provider.as_str(),
                "status": "failed",
                "error": message,
            });
            return fail_publish_workflow(
                &options,
                &report,
                &mut events,
                "distribution.publish",
                error.to_string(),
                metadata.clone(),
                release_content.clone(),
                Some(distribution.clone()),
                None,
            );
        }
    };
    append_distribution_events(&mut events, &distribution_outcome.events);
    let distribution = distribution_outcome.receipt;
    let distribution_status = distribution
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or(if options.dry_run {
            "dry-run"
        } else {
            "completed"
        })
        .to_string();
    record_event(
        &mut events,
        "distribution.publish",
        &distribution_status,
        distribution
            .get("deployment_id")
            .and_then(Value::as_str)
            .map(str::to_string),
    );
    let provider_status = observe_provider_status(&options, &mut events);
    record_event(
        &mut events,
        "workflow.completed",
        &distribution_status,
        None,
    );
    let receipt_path = write_publish_workflow_receipt(
        &options,
        &report,
        &distribution_status,
        metadata.clone(),
        release_content.clone(),
        Some(distribution.clone()),
        provider_status.clone(),
        &events,
    )?;
    if options.json {
        print_publish_workflow_json(
            &report,
            metadata,
            release_content,
            Some(distribution),
            provider_status,
            &events,
            Some(&receipt_path),
        )?;
    } else {
        print_distribution_summary(&options, &distribution);
    }
    Ok(())
}

fn publish_options_with_defaults(
    mut options: PublishWorkflowOptions,
) -> Result<PublishWorkflowOptions> {
    let artifact_target_format = options
        .artifact
        .as_deref()
        .filter(|path| path.exists())
        .map(artifact_target_format)
        .transpose()?;

    if options.target.is_none() {
        options.target = artifact_target_format
            .as_ref()
            .and_then(|(target, _)| *target)
            .or_else(|| Some(package_cmd::default_target_for_provider(options.provider)));
    }
    if options.format.is_none() {
        options.format = artifact_target_format
            .as_ref()
            .and_then(|(_, format)| *format)
            .or_else(|| {
                options.target.map(|target| {
                    package_cmd::default_format_for_target_provider(target, options.provider)
                })
            });
    }
    if options.track.is_none() {
        options.track =
            package_cmd::default_track_for_provider(options.provider).map(str::to_string);
    }
    Ok(options)
}

fn maybe_sync_release_metadata(
    options: &PublishWorkflowOptions,
    report: &ReleasePlanReport,
    dry_run: bool,
) -> Result<Option<Value>> {
    if !release_metadata_provider(options.provider) || !release_config_ready(report) {
        return Ok(None);
    }
    let locales = (!options.locales.is_empty()).then(|| options.locales.join(","));
    store_ops::release_config_push_value(
        options.provider,
        locales.as_deref(),
        options.overwrite_remote,
        dry_run,
        true,
        &options.project_dir,
    )
    .map(Some)
}

fn maybe_sync_release_content(
    options: &PublishWorkflowOptions,
    report: &ReleasePlanReport,
    dry_run: bool,
) -> Result<Option<Value>> {
    if !release_content_provider(options.provider) || !release_content_ready(report) {
        return Ok(None);
    }
    let locales = (!options.locales.is_empty()).then(|| options.locales.join(","));
    store_ops::release_content_push_value(
        options.provider,
        locales.as_deref(),
        dry_run,
        true,
        &options.project_dir,
    )
    .map(Some)
}

fn print_publish_workflow_json(
    report: &ReleasePlanReport,
    metadata: Option<Value>,
    release_content: Option<Value>,
    distribution: Option<Value>,
    provider_status: Option<Value>,
    events: &[PublishWorkflowEvent],
    receipt_path: Option<&Path>,
) -> Result<()> {
    let status = distribution
        .as_ref()
        .and_then(|distribution| distribution.get("status"))
        .and_then(Value::as_str)
        .unwrap_or(report.status.as_str());
    let mut value = json!({
        "schema_version": 1,
        "status": status,
        "release_plan": report,
        "release_metadata": metadata,
        "release_content": release_content,
        "distribution": distribution,
        "provider_status": provider_status,
        "events": events,
        "workflow_receipt": receipt_path.map(|path| path.display().to_string()),
    });
    redact_json_value(&mut value);
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn fail_publish_workflow(
    options: &PublishWorkflowOptions,
    report: &ReleasePlanReport,
    events: &mut Vec<PublishWorkflowEvent>,
    stage_id: &str,
    message: String,
    metadata: Option<Value>,
    release_content: Option<Value>,
    distribution: Option<Value>,
    provider_status: Option<Value>,
) -> Result<()> {
    record_event(events, stage_id, "failed", Some(message.clone()));
    record_event(events, "workflow.failed", "failed", Some(message.clone()));
    let receipt_path = write_publish_workflow_receipt(
        options,
        report,
        "failed",
        metadata.clone(),
        release_content.clone(),
        distribution.clone(),
        provider_status.clone(),
        events,
    )?;
    if options.json {
        print_publish_workflow_json(
            report,
            metadata,
            release_content,
            distribution,
            provider_status,
            events,
            Some(&receipt_path),
        )?;
    }
    bail!(
        "{stage_id} failed: {}; workflow receipt: {}",
        redact_sensitive_text(&message),
        receipt_path.display()
    )
}

fn write_publish_workflow_receipt(
    options: &PublishWorkflowOptions,
    report: &ReleasePlanReport,
    status: &str,
    metadata: Option<Value>,
    release_content: Option<Value>,
    distribution: Option<Value>,
    provider_status: Option<Value>,
    events: &[PublishWorkflowEvent],
) -> Result<PathBuf> {
    let created_at = now_unix_seconds();
    let details = publish_receipt_details(
        options,
        report,
        release_content.as_ref(),
        distribution.as_ref(),
    )?;
    let receipt = PublishWorkflowReceipt {
        schema_version: 1,
        created_at_unix_seconds: created_at,
        status: status.to_string(),
        dry_run: options.dry_run,
        provider: options.provider.as_str().to_string(),
        target: options.target.map(|target| target.as_str().to_string()),
        format: options.format.map(|format| format.as_str().to_string()),
        artifact: options
            .artifact
            .as_ref()
            .map(|path| path.display().to_string()),
        track: options.track.clone(),
        locales: options.locales.clone(),
        release_id: details.release_id,
        version: details.version,
        build: details.build,
        artifact_hash: details.artifact_hash,
        artifact_manifest_sha256: details.artifact_manifest_sha256,
        release_content_assets: details.release_content_assets,
        uploaded_bytes: details.uploaded_bytes,
        uploaded_assets: details.uploaded_assets,
        provider_deployment_id: details.provider_deployment_id,
        canonical_url: details.canonical_url,
        preview_url: details.preview_url,
        manual_follow_up: details.manual_follow_up,
        omitted_requirements: details.omitted_requirements,
        release_plan: report,
        release_metadata: metadata,
        release_content,
        distribution,
        provider_status,
        events,
    };
    let dir = options
        .project_dir
        .join("target/fission/release-workflows/publish")
        .join(options.provider.as_str());
    fs::create_dir_all(&dir)?;
    let path = unique_receipt_path(dir.join(format!("publish-{created_at}.json")));
    let mut value =
        serde_json::to_value(&receipt).context("failed to serialize publish workflow receipt")?;
    redact_json_value(&mut value);
    fs::write(&path, serde_json::to_vec_pretty(&value)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn unique_receipt_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("receipt");
    let extension = path.extension().and_then(OsStr::to_str);
    for index in 1u32.. {
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

fn publish_receipt_details(
    options: &PublishWorkflowOptions,
    report: &ReleasePlanReport,
    release_content: Option<&Value>,
    distribution: Option<&Value>,
) -> Result<ReceiptDetails> {
    let artifact_manifest = options
        .artifact
        .as_deref()
        .filter(|path| path.exists())
        .map(read_json_file)
        .transpose()?;
    let artifact_manifest_sha256 = options
        .artifact
        .as_deref()
        .filter(|path| path.exists())
        .map(sha256_file)
        .transpose()?;

    let version = distribution
        .and_then(|value| string_field(value, "version"))
        .or_else(|| {
            artifact_manifest
                .as_ref()
                .and_then(|value| value.pointer("/project/version"))
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let build = distribution
        .and_then(|value| u64_field(value, "build"))
        .or_else(|| {
            artifact_manifest
                .as_ref()
                .and_then(|value| value.pointer("/project/build"))
                .and_then(value_as_u64)
        });
    let release_id = distribution
        .and_then(|value| string_field(value, "release_id"))
        .or_else(|| {
            version
                .as_ref()
                .zip(build)
                .map(|(version, build)| format!("{version}+{build}"))
        });
    let artifact_hash = distribution
        .and_then(|value| string_field(value, "artifact_hash"))
        .or_else(|| {
            artifact_manifest
                .as_ref()
                .and_then(|value| value.pointer("/artifacts/0/sha256"))
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let release_content_assets = release_content_asset_values(release_content);
    let mut uploaded_assets = distribution
        .and_then(|value| value.get("uploaded_assets"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if release_content
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "pushed" | "uploaded"))
    {
        uploaded_assets.extend(release_content_assets.clone());
    }
    let uploaded_bytes = distribution
        .and_then(|value| value.get("uploaded_bytes"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            uploaded_assets
                .iter()
                .filter_map(|asset| asset.get("size_bytes").and_then(Value::as_u64))
                .sum()
        });
    let mut manual_follow_up = distribution
        .and_then(|value| value.get("manual_follow_up"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    manual_follow_up.extend(
        release_content
            .and_then(|value| value.get("manual_follow_up"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string),
    );

    Ok(ReceiptDetails {
        release_id,
        version,
        build,
        artifact_hash,
        artifact_manifest_sha256: distribution
            .and_then(|value| string_field(value, "artifact_manifest_sha256"))
            .or(artifact_manifest_sha256),
        release_content_assets,
        uploaded_bytes,
        uploaded_assets,
        provider_deployment_id: distribution.and_then(|value| string_field(value, "deployment_id")),
        canonical_url: distribution.and_then(|value| string_field(value, "canonical_url")),
        preview_url: distribution.and_then(|value| string_field(value, "preview_url")),
        manual_follow_up,
        omitted_requirements: omitted_requirements(report),
    })
}

fn release_content_asset_values(release_content: Option<&Value>) -> Vec<Value> {
    let mut assets = Vec::new();
    for key in ["assets", "handoff_assets"] {
        assets.extend(
            release_content
                .and_then(|value| value.get(key))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .cloned(),
        );
    }
    assets
}

fn omitted_requirements(report: &ReleasePlanReport) -> Vec<ReceiptRequirement> {
    report
        .requirements
        .iter()
        .filter(|requirement| requirement.status != RequirementStatus::Passed)
        .map(|requirement| ReceiptRequirement {
            id: requirement.id.clone(),
            level: requirement.level,
            status: requirement.status,
            summary: requirement.summary.clone(),
        })
        .collect()
}

fn read_json_file(path: &Path) -> Result<Value> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
}

fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(value_as_u64)
}

fn value_as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn record_event(
    events: &mut Vec<PublishWorkflowEvent>,
    id: &str,
    status: &str,
    details: Option<String>,
) {
    events.push(PublishWorkflowEvent {
        at_unix_seconds: now_unix_seconds(),
        id: id.to_string(),
        status: status.to_string(),
        details: details.map(|details| redact_sensitive_text(&details)),
    });
}

fn append_distribution_events(
    events: &mut Vec<PublishWorkflowEvent>,
    distribution_events: &[package_cmd::DistributionEvent],
) {
    events.extend(distribution_events.iter().map(|event| {
        PublishWorkflowEvent {
            at_unix_seconds: event.at_unix_seconds,
            id: event.id.clone(),
            status: event.status.clone(),
            details: event
                .details
                .as_ref()
                .map(|details| redact_sensitive_text(details)),
        }
    }));
}

fn redact_sensitive_text(text: &str) -> String {
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

fn observe_provider_status(
    options: &PublishWorkflowOptions,
    events: &mut Vec<PublishWorkflowEvent>,
) -> Option<Value> {
    if options.dry_run {
        record_event(
            events,
            "provider.observe",
            "skipped",
            Some("dry run does not query provider state".to_string()),
        );
        return None;
    }

    record_event(
        events,
        "provider.observe",
        "started",
        Some(options.provider.as_str().to_string()),
    );
    match package_cmd::distribute_status_outcome(status_distribute_options(options)) {
        Ok(outcome) => {
            append_distribution_events(events, &outcome.events);
            let status = outcome.receipt;
            let observed_status = status
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("completed")
                .to_string();
            record_event(
                events,
                "provider.observe",
                &observed_status,
                status
                    .get("deployment_id")
                    .or_else(|| status.get("canonical_url"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            );
            Some(status)
        }
        Err(error) => {
            record_event(
                events,
                "provider.observe",
                "warning",
                Some(error.to_string()),
            );
            None
        }
    }
}

fn release_metadata_message_detail(value: &Value) -> String {
    let provider = value
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("provider");
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("complete");
    format!("{provider} {status}")
}

fn print_distribution_summary(options: &PublishWorkflowOptions, value: &Value) {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("complete");
    println!("{} publish status: {status}", options.provider.as_str());
    if let Some(url) = value.get("canonical_url").and_then(Value::as_str) {
        println!("URL: {url}");
    }
    if let Some(assets) = value.get("uploaded_assets").and_then(Value::as_array) {
        if !assets.is_empty() {
            println!("Uploaded/planned assets: {}", assets.len());
            for asset in assets.iter().take(10) {
                let relative = asset
                    .get("relative_path")
                    .and_then(Value::as_str)
                    .unwrap_or("<asset>");
                let destination = asset
                    .get("url")
                    .or_else(|| asset.get("provider_id"))
                    .and_then(Value::as_str)
                    .unwrap_or("<provider destination>");
                println!("  {relative} -> {destination}");
            }
            if assets.len() > 10 {
                println!("  ... {} more", assets.len() - 10);
            }
        }
    }
    if let Some(follow_up) = value.get("manual_follow_up").and_then(Value::as_array) {
        for item in follow_up.iter().filter_map(Value::as_str) {
            println!("Follow-up: {item}");
        }
    }
}

fn release_metadata_provider(provider: DistributionProvider) -> bool {
    matches!(
        provider,
        DistributionProvider::PlayStore
            | DistributionProvider::AppStore
            | DistributionProvider::MicrosoftStore
    )
}

fn release_content_provider(provider: DistributionProvider) -> bool {
    matches!(
        provider,
        DistributionProvider::PlayStore
            | DistributionProvider::AppStore
            | DistributionProvider::MicrosoftStore
    )
}

fn release_config_ready(report: &ReleasePlanReport) -> bool {
    let mut saw_release_config = false;
    for requirement in &report.requirements {
        if !requirement.id.starts_with("release_config.") {
            continue;
        }
        saw_release_config = true;
        if !matches!(
            requirement.status,
            RequirementStatus::Passed | RequirementStatus::Skipped
        ) {
            return false;
        }
    }
    saw_release_config
}

fn release_content_ready(report: &ReleasePlanReport) -> bool {
    let mut saw_release_content = false;
    let mut saw_asset = false;
    for requirement in &report.requirements {
        if !requirement.id.starts_with("release_content.") {
            continue;
        }
        saw_release_content = true;
        if requirement.id.contains(".rendered_assets")
            || requirement.id.contains(".required_assets")
            || requirement.id == "release_content.play_store.feature_graphic"
            || requirement.id == "release_content.play_store.screenshot_sets_dir"
        {
            saw_asset |= matches!(
                requirement.status,
                RequirementStatus::Passed | RequirementStatus::Skipped
            );
        }
        if requirement.level == RequirementLevel::ProviderRequired
            && matches!(
                requirement.status,
                RequirementStatus::Missing | RequirementStatus::Failed
            )
        {
            return false;
        }
    }
    saw_release_content && saw_asset
}

fn release_content_message_detail(value: &Value) -> String {
    let provider = value
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("provider");
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("complete");
    format!("{provider} {status}")
}

fn release_content_status(value: &Value, dry_run: bool) -> String {
    if dry_run {
        return "dry-run".to_string();
    }
    value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("completed")
        .to_string()
}

fn release_metadata_message(value: &Value, dry_run: bool) -> String {
    let provider = value
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("provider");
    let count = value
        .get("updated_locales")
        .or_else(|| value.get("pushed_locales"))
        .or_else(|| value.get("locales"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    if dry_run {
        format!("release metadata: would sync {count} {provider} locale(s)")
    } else {
        format!("release metadata: synced {count} {provider} locale(s)")
    }
}

fn release_content_message(value: &Value, dry_run: bool) -> String {
    let provider = value
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("provider");
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("completed");
    let count = value
        .get("assets")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    if dry_run {
        format!("release content: would sync {count} {provider} asset(s)")
    } else if status == "staged" {
        format!("release content: staged {count} {provider} asset(s) for provider handoff")
    } else {
        format!("release content: synced {count} {provider} asset(s)")
    }
}

fn distribute_options(options: &PublishWorkflowOptions, dry_run: bool) -> DistributeOptions {
    DistributeOptions {
        project_dir: options.project_dir.clone(),
        provider: options.provider,
        action: DistributeAction::Publish,
        target: options.target,
        format: options.format,
        artifact: options.artifact.clone(),
        site: options.site.clone(),
        deploy: options.deploy.clone(),
        track: options.track.clone(),
        locales: options.locales.clone(),
        dry_run,
        yes: options.yes,
        json: options.json,
    }
}

fn status_distribute_options(options: &PublishWorkflowOptions) -> DistributeOptions {
    DistributeOptions {
        project_dir: options.project_dir.clone(),
        provider: options.provider,
        action: DistributeAction::Status,
        target: options.target,
        format: options.format,
        artifact: options.artifact.clone(),
        site: options.site.clone(),
        deploy: options.deploy.clone(),
        track: options.track.clone(),
        locales: options.locales.clone(),
        dry_run: false,
        yes: true,
        json: false,
    }
}

fn build_release_plan(
    options: &PublishWorkflowOptions,
    artifact: Option<&Path>,
) -> Result<ReleasePlanReport> {
    let mut requirements = Vec::new();
    if let Some(target) = options.target {
        requirements.extend(
            package_cmd::package_readiness_checks_for_profile(
                &options.project_dir,
                Some(target),
                options.format,
                true,
            )?
            .into_iter()
            .map(package_requirement),
        );
        if release_target_requires_signing(target) {
            requirements.extend(
                signing_ops::status_checks(&options.project_dir, target)
                    .into_iter()
                    .map(|check| lifecycle_requirement(check, RequirementLevel::ProviderRequired)),
            );
        }
    }
    requirements.extend(
        package_cmd::distribution_readiness_checks(
            &options.project_dir,
            options.provider,
            &options.site,
            options.track.as_deref(),
            options.format,
            artifact,
        )?
        .into_iter()
        .map(package_requirement),
    );

    let release_config =
        model::validate_release_config_model(&options.project_dir, Some(options.provider))?;
    requirements.extend(release_config.checks.into_iter().map(|check| {
        let level = if check.id.starts_with("release_config.secret_field.") {
            RequirementLevel::ProviderRequired
        } else {
            RequirementLevel::FissionRecommended
        };
        lifecycle_requirement(check, level)
    }));
    if release_metadata_provider(options.provider) {
        requirements.push(lifecycle_requirement(
            store_ops::release_config_lock_check(
                &options.project_dir,
                options.provider,
                options.overwrite_remote,
            ),
            RequirementLevel::ProviderRequired,
        ));
    }

    let release_content =
        content::validate_release_content_model(&options.project_dir, Some(options.provider));
    requirements.extend(release_content.checks.into_iter().map(|check| {
        let level = release_content_requirement_level(options.provider, &check.id);
        lifecycle_requirement(check, level)
    }));

    requirements.extend(version_requirements(options, artifact)?);
    apply_skipped_requirements(&options.project_dir, &mut requirements)?;

    let status = if requirements.iter().any(|req| {
        req.level == RequirementLevel::ProviderRequired
            && matches!(
                req.status,
                RequirementStatus::Missing | RequirementStatus::Failed
            )
    }) {
        "blocked"
    } else if requirements.iter().any(|req| {
        !matches!(
            req.status,
            RequirementStatus::Passed | RequirementStatus::Skipped
        )
    }) {
        "warning"
    } else {
        "ready"
    }
    .to_string();

    let steps = release_steps(&requirements, artifact.is_some(), &status);
    let capabilities = provider_capabilities(options.provider);

    Ok(ReleasePlanReport {
        context: release_context_report(options),
        project_dir: options.project_dir.display().to_string(),
        provider: options.provider.as_str().to_string(),
        target: options.target.map(|target| target.as_str().to_string()),
        format: options.format.map(|format| format.as_str().to_string()),
        artifact: artifact.map(|path| path.display().to_string()),
        track: options.track.clone(),
        locales: options.locales.clone(),
        status,
        steps,
        capabilities,
        requirements,
    })
}

fn release_plan_snapshot_from_report(report: ReleasePlanReport) -> ReleasePlanSnapshot {
    ReleasePlanSnapshot {
        context: ReleaseContextSnapshot {
            project_dir: report.context.project_dir,
            app_name: report.context.app_name,
            target: report.context.target,
            format: report.context.format,
            provider: report.context.provider,
            release_id: report.context.release_id,
            track: report.context.track,
            locales: report.context.locales,
            interactive: report.context.interactive,
            ci: report.context.ci,
        },
        project_dir: report.project_dir,
        provider: report.provider,
        target: report.target,
        format: report.format,
        artifact: report.artifact,
        track: report.track,
        locales: report.locales,
        status: report.status,
        steps: report
            .steps
            .into_iter()
            .map(|step| ReleaseStepSnapshot {
                id: step.id,
                title: step.title,
                status: step_status_label(step.status).to_string(),
                jobs: step
                    .jobs
                    .into_iter()
                    .map(|job| ReleaseJobSnapshot {
                        id: job.id,
                        title: job.title,
                        status: step_status_label(job.status).to_string(),
                    })
                    .collect(),
            })
            .collect(),
        capabilities: report
            .capabilities
            .into_iter()
            .map(|capability| ProviderCapabilitySnapshot {
                id: capability.id,
                status: provider_capability_status_label(capability.status).to_string(),
                summary: capability.summary,
                details: capability.details,
            })
            .collect(),
        requirements: report
            .requirements
            .into_iter()
            .map(|requirement| ReleaseRequirementSnapshot {
                id: requirement.id,
                level: requirement_level_label(requirement.level).to_string(),
                status: requirement_status_label(requirement.status).to_string(),
                summary: requirement.summary,
                details: requirement.details,
                remediation: requirement.remediation,
                can_fix_interactively: requirement.can_fix_interactively,
            })
            .collect(),
    }
}

fn release_context_report(options: &PublishWorkflowOptions) -> ReleaseContextReport {
    let config = read_fission_toml_value(&options.project_dir).ok();
    ReleaseContextReport {
        project_dir: options.project_dir.display().to_string(),
        app_name: config
            .as_ref()
            .and_then(|value| value.pointer("/app/name"))
            .and_then(Value::as_str)
            .map(str::to_string),
        target: options.target.map(|target| target.as_str().to_string()),
        format: options.format.map(|format| format.as_str().to_string()),
        provider: options.provider.as_str().to_string(),
        release_id: config.as_ref().and_then(active_release_id_from_toml),
        track: options.track.clone(),
        locales: options.locales.clone(),
        interactive: !options.yes && !options.json,
        ci: options.yes && options.json,
    }
}

fn release_target_requires_signing(target: Target) -> bool {
    matches!(
        target,
        Target::Android | Target::Ios | Target::Macos | Target::Windows
    )
}

fn read_fission_toml_value(project_dir: &Path) -> Result<Value> {
    let path = project_dir.join("fission.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))?;
    serde_json::to_value(value).context("failed to convert fission.toml to JSON value")
}

fn active_release_id_from_toml(value: &Value) -> Option<String> {
    value
        .pointer("/release/active_release")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
}

fn package_requirement(check: ReadinessCheck) -> ReleaseRequirement {
    ReleaseRequirement {
        id: check.id,
        level: match check.severity {
            CheckSeverity::Error => RequirementLevel::ProviderRequired,
            CheckSeverity::Warning => RequirementLevel::FissionRecommended,
            CheckSeverity::Info => RequirementLevel::Optional,
        },
        status: match check.status {
            CheckStatus::Passed => RequirementStatus::Passed,
            CheckStatus::Missing => RequirementStatus::Missing,
            CheckStatus::Failed => RequirementStatus::Failed,
            CheckStatus::Warning => RequirementStatus::Warning,
            CheckStatus::Skipped => RequirementStatus::Skipped,
        },
        summary: check.summary,
        details: check.details,
        remediation: check.remediation,
        can_fix_interactively: !matches!(check.status, CheckStatus::Passed | CheckStatus::Skipped)
            && check.severity != CheckSeverity::Info,
    }
}

fn release_requirement_readiness_check(requirement: ReleaseRequirement) -> ReadinessCheck {
    ReadinessCheck {
        id: requirement.id,
        severity: match requirement.level {
            RequirementLevel::ProviderRequired => CheckSeverity::Error,
            RequirementLevel::FissionRecommended => CheckSeverity::Warning,
            RequirementLevel::Optional => CheckSeverity::Info,
        },
        status: match requirement.status {
            RequirementStatus::Passed => CheckStatus::Passed,
            RequirementStatus::Missing => CheckStatus::Missing,
            RequirementStatus::Failed => CheckStatus::Failed,
            RequirementStatus::Warning => CheckStatus::Warning,
            RequirementStatus::Skipped => CheckStatus::Skipped,
        },
        summary: requirement.summary,
        details: requirement.details,
        remediation: requirement.remediation,
    }
}

fn release_content_requirement_level(provider: DistributionProvider, id: &str) -> RequirementLevel {
    let provider_prefix = format!("release_content.{}.", provider.as_str());
    if id == format!("{provider_prefix}required_assets")
        || id == "release_content.microsoft_store.required_logos"
        || (id.starts_with(&provider_prefix) && (id.contains(".image.") || id.contains(".video.")))
    {
        RequirementLevel::ProviderRequired
    } else {
        RequirementLevel::FissionRecommended
    }
}

fn lifecycle_requirement(check: LifecycleCheck, level: RequirementLevel) -> ReleaseRequirement {
    ReleaseRequirement {
        id: check.id,
        level,
        status: match check.status.as_str() {
            "passed" | "ready" | "ok" => RequirementStatus::Passed,
            "missing" => RequirementStatus::Missing,
            "failed" | "blocked" => RequirementStatus::Failed,
            "warning" => RequirementStatus::Warning,
            "skipped" => RequirementStatus::Skipped,
            _ => RequirementStatus::Warning,
        },
        summary: check.summary,
        details: check.details,
        remediation: check.remediation,
        can_fix_interactively: !matches!(
            check.status.as_str(),
            "passed" | "ready" | "ok" | "skipped"
        ),
    }
}

fn version_requirements(
    options: &PublishWorkflowOptions,
    artifact: Option<&Path>,
) -> Result<Vec<ReleaseRequirement>> {
    let mut requirements = Vec::new();
    let resolved = resolve_release_version_config(&options.project_dir, options.target)?;
    let artifact_release = artifact
        .filter(|path| path.exists())
        .map(artifact_release_version_config)
        .transpose()?
        .unwrap_or_default();
    let version = resolved
        .version
        .clone()
        .or(artifact_release.version.clone());
    let build = resolved.build.or(artifact_release.build);
    requirements.push(ReleaseRequirement {
        id: "release.version.resolved".to_string(),
        level: RequirementLevel::ProviderRequired,
        status: if version
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            RequirementStatus::Passed
        } else {
            RequirementStatus::Missing
        },
        summary: "release version is resolved".to_string(),
        details: version.clone(),
        remediation: vec![
            "Set app.version, package target version fields, or add an active [[releases]] entry."
                .to_string(),
        ],
        can_fix_interactively: version.is_none(),
    });
    requirements.push(ReleaseRequirement {
        id: "release.build.resolved".to_string(),
        level: RequirementLevel::ProviderRequired,
        status: if build.is_some() {
            RequirementStatus::Passed
        } else {
            RequirementStatus::Missing
        },
        summary: "release build number is resolved".to_string(),
        details: build.map(|value| value.to_string()),
        remediation: vec![
            "Set app.build, package target build fields, or add an active [[releases]] entry."
                .to_string(),
        ],
        can_fix_interactively: build.is_none(),
    });
    if let (Some(config_version), Some(artifact_version)) = (
        resolved.version.as_deref(),
        artifact_release.version.as_deref(),
    ) {
        requirements.push(version_match_requirement(
            "release.version.artifact_matches_config",
            "artifact release version matches fission.toml",
            config_version == artifact_version,
            format!("config={config_version}, artifact={artifact_version}"),
            "Rebuild the artifact after changing app.version/release.version, or publish the artifact from the matching release record.",
        ));
    }
    if let (Some(config_build), Some(artifact_build)) = (resolved.build, artifact_release.build) {
        requirements.push(version_match_requirement(
            "release.build.artifact_matches_config",
            "artifact build number matches fission.toml",
            config_build == artifact_build,
            format!("config={config_build}, artifact={artifact_build}"),
            "Rebuild the artifact after changing app.build/release.build, or publish the artifact from the matching release record.",
        ));
    }
    Ok(requirements)
}

fn artifact_release_version_config(
    artifact: &Path,
) -> Result<fission_command_core::ReleaseVersionConfig> {
    let value: Value = serde_json::from_slice(
        &std::fs::read(artifact)
            .with_context(|| format!("failed to read artifact manifest {}", artifact.display()))?,
    )
    .with_context(|| format!("failed to parse artifact manifest {}", artifact.display()))?;
    let project = value.get("project").unwrap_or(&Value::Null);
    let version = project
        .get("version")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let build = project.get("build").and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
    });
    Ok(fission_command_core::ReleaseVersionConfig { version, build })
}

fn artifact_target_format(artifact: &Path) -> Result<(Option<Target>, Option<PackageFormat>)> {
    let value = read_json_file(artifact)?;
    Ok((
        value
            .get("target")
            .and_then(Value::as_str)
            .and_then(target_from_manifest),
        value
            .get("format")
            .and_then(Value::as_str)
            .and_then(format_from_manifest),
    ))
}

fn target_from_manifest(value: &str) -> Option<Target> {
    match value {
        "android" => Some(Target::Android),
        "ios" => Some(Target::Ios),
        "linux" => Some(Target::Linux),
        "macos" => Some(Target::Macos),
        "ssr" | "server" => Some(Target::Server),
        "static-site" | "site" => Some(Target::Site),
        "terminal" => Some(Target::Terminal),
        "web" => Some(Target::Web),
        "windows" => Some(Target::Windows),
        _ => None,
    }
}

fn format_from_manifest(value: &str) -> Option<PackageFormat> {
    match value {
        "aab" => Some(PackageFormat::Aab),
        "apk" => Some(PackageFormat::Apk),
        "app" => Some(PackageFormat::App),
        "docker-image" => Some(PackageFormat::DockerImage),
        "exe" => Some(PackageFormat::Exe),
        "ipa" => Some(PackageFormat::Ipa),
        "msi" => Some(PackageFormat::Msi),
        "msix" => Some(PackageFormat::Msix),
        "pkg" => Some(PackageFormat::Pkg),
        "run" => Some(PackageFormat::Run),
        "static" => Some(PackageFormat::Static),
        _ => None,
    }
}

fn apply_skipped_requirements(
    project_dir: &Path,
    requirements: &mut Vec<ReleaseRequirement>,
) -> Result<()> {
    let skipped = release_skipped_requirements(project_dir)?;
    if skipped.is_empty() {
        return Ok(());
    }
    let mut ignored_required = Vec::new();
    for requirement in requirements.iter_mut() {
        if !skipped.contains(&requirement.id) {
            continue;
        }
        if requirement.level == RequirementLevel::ProviderRequired {
            ignored_required.push(requirement.id.clone());
            continue;
        }
        requirement.status = RequirementStatus::Skipped;
        requirement.can_fix_interactively = false;
        requirement.details = Some(match requirement.details.take() {
            Some(details) => format!("{details}; explicitly skipped in release.skip_requirements"),
            None => "explicitly skipped in release.skip_requirements".to_string(),
        });
    }
    for id in ignored_required {
        requirements.push(ReleaseRequirement {
            id: format!("release.skip.provider_required.{}", check_id_fragment(&id)),
            level: RequirementLevel::FissionRecommended,
            status: RequirementStatus::Warning,
            summary: "provider-required requirement cannot be skipped".to_string(),
            details: Some(id),
            remediation: vec![
                "Remove this id from release.skip_requirements or satisfy the provider-required requirement."
                    .to_string(),
            ],
            can_fix_interactively: false,
        });
    }
    for id in skipped {
        if requirements.iter().any(|requirement| requirement.id == id) {
            continue;
        }
        requirements.push(ReleaseRequirement {
            id: format!("release.skip.{}", check_id_fragment(&id)),
            level: RequirementLevel::Optional,
            status: RequirementStatus::Warning,
            summary: "release.skip_requirements entry did not match a current requirement"
                .to_string(),
            details: Some(id),
            remediation: vec![
                "Remove stale skip entries or update them to match current release-plan requirement ids."
                    .to_string(),
            ],
            can_fix_interactively: false,
        });
    }
    Ok(())
}

fn release_skipped_requirements(project_dir: &Path) -> Result<BTreeSet<String>> {
    let path = project_dir.join("fission.toml");
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let config: ReleaseSkipToml = toml::from_str(
        &fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(config
        .release
        .map(|release| release.skip_requirements)
        .unwrap_or_default()
        .into_iter()
        .filter(|id| !id.trim().is_empty())
        .collect())
}

fn check_id_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn version_match_requirement(
    id: &str,
    summary: &str,
    matched: bool,
    details: String,
    remediation: &str,
) -> ReleaseRequirement {
    ReleaseRequirement {
        id: id.to_string(),
        level: RequirementLevel::FissionRecommended,
        status: if matched {
            RequirementStatus::Passed
        } else {
            RequirementStatus::Warning
        },
        summary: summary.to_string(),
        details: Some(details),
        remediation: vec![remediation.to_string()],
        can_fix_interactively: !matched,
    }
}

#[cfg(test)]
#[path = "publish_workflow_tests.rs"]
mod publish_workflow_tests;
