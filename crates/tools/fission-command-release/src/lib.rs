use anyhow::{bail, Context, Result};
use clap::Subcommand;
use fission_command_core::{
    normalize_windows_package_version, resolve_release_version_config, DistributionProvider, Target,
};
use fission_command_package as publish;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{
    Array as TomlEditArray, DocumentMut, Item as TomlEditItem, Table as TomlEditTable,
    Value as TomlEditValue,
};

mod auth_ops;
mod commands;
mod content;
mod microsoft_store_ops;
mod model;
mod publish_workflow;
mod signing_ops;
mod store_ops;
mod workflow_ops;

pub use commands::{
    AuthCommand, BetaCommand, BetaGroupsCommand, BetaTestersCommand, ReleaseConfigCommand,
    ReleaseContentCommand, ReleaseWorkflowCommand, ReviewsCommand, SigningCommand,
};

pub use publish_workflow::{
    publish_workflow, readiness_release, release_plan_snapshot, release_readiness_checks,
    ProviderCapabilitySnapshot, PublishWorkflowOptions, ReleaseContextSnapshot, ReleaseJobSnapshot,
    ReleasePlanSnapshot, ReleaseRequirementSnapshot, ReleaseStepSnapshot,
};

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Serialize)]
struct LifecycleReport {
    area: String,
    status: String,
    provider: Option<String>,
    target: Option<String>,
    checks: Vec<LifecycleCheck>,
}

#[derive(Debug, Serialize)]
struct LifecycleCheck {
    id: String,
    status: String,
    summary: String,
    details: Option<String>,
    remediation: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ReleaseConfigMutationReceipt {
    schema_version: u32,
    action: String,
    status: String,
    project_dir: String,
    target: Option<String>,
    provider: Option<String>,
    field: Option<String>,
    kind: Option<String>,
    locale: Option<String>,
    path: Option<String>,
    old_value: Option<String>,
    new_value: Option<String>,
    release_id: Option<String>,
    requirement_id: Option<String>,
}

struct BumpBuildPlan {
    field: &'static str,
    old_value: String,
    new_value: String,
    next_build: u64,
}

#[derive(Debug, Serialize)]
struct VersionStateReport {
    schema_version: u32,
    created_at_unix_seconds: u64,
    action: String,
    status: String,
    project_dir: String,
    provider: String,
    target: String,
    format: String,
    track: Option<String>,
    local: VersionStateLocal,
    provider_state: VersionStateProviderState,
    provider_status: Option<JsonValue>,
    provider_error: Option<String>,
    monotonic: VersionStateMonotonic,
    next_action: String,
}

#[derive(Debug, Serialize)]
struct VersionStateLocal {
    version: Option<String>,
    build: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
struct VersionStateProviderState {
    latest_uploaded_build: Option<u64>,
    latest_released_build: Option<u64>,
    latest_status: Option<String>,
    review_status: Option<String>,
    observed_builds: Vec<VersionStateObservedBuild>,
}

#[derive(Debug, Clone, Serialize)]
struct VersionStateObservedBuild {
    build: u64,
    status: Option<String>,
    source: String,
}

#[derive(Debug, Clone, Serialize)]
struct VersionStateMonotonic {
    status: String,
    local_build: Option<u64>,
    latest_provider_build: Option<u64>,
    details: Option<String>,
}

pub fn release_config(command: ReleaseConfigCommand) -> Result<()> {
    match command {
        ReleaseConfigCommand::Edit {
            project_dir,
            tui,
            provider: _,
        } => edit_release_config(&project_dir, tui),
        ReleaseConfigCommand::Validate {
            provider,
            project_dir,
            json,
        } => print_report(
            model::validate_release_config_model(&project_dir, provider)?,
            json,
        ),
        ReleaseConfigCommand::Set {
            field,
            value,
            project_dir,
            dry_run,
            yes,
            json,
        } => set_release_field_command(&project_dir, &field, &value, dry_run, yes, json),
        ReleaseConfigCommand::AddRelease {
            version,
            build,
            from,
            project_dir,
            dry_run,
            yes,
            json,
        } => add_release_command(
            &project_dir,
            &version,
            build,
            from.as_deref(),
            dry_run,
            yes,
            json,
        ),
        ReleaseConfigCommand::SkipRequirement {
            id,
            project_dir,
            dry_run,
            yes,
            json,
        } => skip_release_requirement_command(&project_dir, &id, dry_run, yes, json),
        ReleaseConfigCommand::BumpBuild {
            target,
            by,
            project_dir,
            dry_run,
            yes,
            json,
        } => bump_release_build_command(&project_dir, target, by, dry_run, yes, json),
        ReleaseConfigCommand::VersionState {
            provider,
            target,
            format,
            track,
            site,
            artifact,
            project_dir,
            json,
        } => version_state_command(
            &project_dir,
            provider,
            target,
            format,
            track,
            &site,
            artifact.as_deref(),
            json,
        ),
        ReleaseConfigCommand::EditFile {
            release,
            kind,
            locale,
            project_dir,
        } => edit_release_file(&project_dir, &release, &kind, locale.as_deref()),
        ReleaseConfigCommand::WriteFile {
            release,
            kind,
            provider,
            locale,
            value,
            from_file,
            project_dir,
            dry_run,
            yes,
            json,
        } => write_release_file_command(
            &project_dir,
            &release,
            &kind,
            provider,
            locale.as_deref(),
            value.as_deref(),
            from_file.as_deref(),
            dry_run,
            yes,
            json,
        ),
        ReleaseConfigCommand::Import {
            provider,
            locales,
            dry_run,
            yes,
            project_dir,
            json,
        } => store_ops::release_config_import(provider, locales, dry_run, yes, &project_dir, json),
        ReleaseConfigCommand::Diff {
            provider,
            project_dir,
            json,
        } => store_ops::release_config_diff(provider, &project_dir, json),
        ReleaseConfigCommand::Push {
            provider,
            locales,
            overwrite_remote,
            dry_run,
            yes,
            project_dir,
            json,
        } => store_ops::release_config_push(
            provider,
            locales,
            overwrite_remote,
            dry_run,
            yes,
            &project_dir,
            json,
        ),
        ReleaseConfigCommand::Lock {
            provider,
            locales,
            dry_run,
            yes,
            project_dir,
            json,
        } => store_ops::release_config_lock(provider, locales, dry_run, yes, &project_dir, json),
    }
}

pub fn release_config_readiness_checks(
    project_dir: &Path,
    provider: Option<DistributionProvider>,
) -> Result<Vec<publish::ReadinessCheck>> {
    Ok(model::validate_release_config_model(project_dir, provider)?
        .checks
        .into_iter()
        .map(lifecycle_readiness_check)
        .collect())
}

pub fn release_content_readiness_checks(
    project_dir: &Path,
    provider: Option<DistributionProvider>,
) -> Result<Vec<publish::ReadinessCheck>> {
    Ok(
        content::validate_release_content_model(project_dir, provider)
            .checks
            .into_iter()
            .map(lifecycle_readiness_check)
            .collect(),
    )
}

fn lifecycle_readiness_check(check: LifecycleCheck) -> publish::ReadinessCheck {
    let status = match check.status.as_str() {
        "passed" | "ready" | "ok" => publish::CheckStatus::Passed,
        "missing" => publish::CheckStatus::Missing,
        "failed" | "blocked" => publish::CheckStatus::Failed,
        "warning" => publish::CheckStatus::Warning,
        "skipped" => publish::CheckStatus::Skipped,
        _ => publish::CheckStatus::Warning,
    };
    let severity = match status {
        publish::CheckStatus::Failed | publish::CheckStatus::Missing => {
            publish::CheckSeverity::Error
        }
        publish::CheckStatus::Warning => publish::CheckSeverity::Warning,
        publish::CheckStatus::Passed | publish::CheckStatus::Skipped => {
            publish::CheckSeverity::Info
        }
    };
    publish::ReadinessCheck {
        id: check.id,
        severity,
        status,
        summary: check.summary,
        details: check.details,
        remediation: check.remediation,
    }
}

pub fn release_content(command: ReleaseContentCommand) -> Result<()> {
    match command {
        ReleaseContentCommand::Validate {
            provider,
            project_dir,
            json,
        } => print_report(
            content::validate_release_content_model(&project_dir, provider),
            json,
        ),
        ReleaseContentCommand::Capture {
            target,
            set,
            project_dir,
            json,
        } => print_report(
            content::capture_release_content(&project_dir, target, &set)?,
            json,
        ),
        ReleaseContentCommand::Render {
            provider,
            project_dir,
            json,
        } => print_report(
            content::render_release_content(&project_dir, provider)?,
            json,
        ),
        ReleaseContentCommand::Push {
            provider,
            locales,
            dry_run,
            yes,
            project_dir,
            json,
        } => store_ops::release_content_push(provider, locales, dry_run, yes, &project_dir, json),
    }
}

pub fn beta(command: BetaCommand) -> Result<()> {
    match command {
        BetaCommand::Groups { command } => match command {
            BetaGroupsCommand::List {
                provider,
                project_dir,
                json,
            } => store_ops::beta_groups_list(provider, &project_dir, json),
            BetaGroupsCommand::Sync {
                provider,
                from,
                project_dir,
                dry_run,
                yes,
                json,
            } => store_ops::beta_groups_sync(provider, &from, &project_dir, dry_run, yes, json),
        },
        BetaCommand::Testers { command } => match command {
            BetaTestersCommand::Import {
                provider,
                group,
                track,
                csv,
                project_dir,
                dry_run,
                yes,
                json,
            } => store_ops::beta_testers_import(
                provider,
                group.as_deref(),
                track.as_deref(),
                &csv,
                &project_dir,
                dry_run,
                yes,
                json,
            ),
            BetaTestersCommand::Export {
                provider,
                group,
                track,
                output,
                project_dir,
                json,
            } => store_ops::beta_testers_export(
                provider,
                group.as_deref(),
                track.as_deref(),
                &output,
                &project_dir,
                json,
            ),
        },
        BetaCommand::Distribute {
            provider,
            artifact,
            group,
            track,
            project_dir,
            dry_run,
            yes,
            json,
        } => {
            if provider == DistributionProvider::AppStore {
                store_ops::app_store_beta_distribute(
                    &project_dir,
                    &artifact,
                    group.as_deref(),
                    dry_run,
                    yes,
                    json,
                )
            } else {
                publish::distribute(publish::DistributeOptions {
                    project_dir,
                    provider,
                    action: publish::DistributeAction::Publish,
                    target: None,
                    format: None,
                    artifact: Some(artifact),
                    site: group.unwrap_or_else(|| "beta".to_string()),
                    deploy: None,
                    track,
                    locales: Vec::new(),
                    dry_run,
                    yes,
                    json,
                })
            }
        }
    }
}

pub fn signing(command: SigningCommand) -> Result<()> {
    match command {
        SigningCommand::Status {
            target,
            project_dir,
            json,
        } => signing_ops::status(&project_dir, target, json),
        SigningCommand::Sync {
            target,
            readonly,
            project_dir,
            json,
        } => signing_ops::sync(&project_dir, target, readonly, json),
        SigningCommand::Import {
            target,
            keystore,
            alias,
            project_dir,
            dry_run,
            yes,
            json,
        } => signing_ops::import(&project_dir, target, keystore, alias, dry_run, yes, json),
    }
}

pub fn reviews(command: ReviewsCommand) -> Result<()> {
    match command {
        ReviewsCommand::List {
            provider,
            since,
            project_dir,
            json,
        } => store_ops::reviews_list(provider, since, &project_dir, json),
        ReviewsCommand::Reply {
            provider,
            review,
            message_file,
            project_dir,
            dry_run,
            yes,
            json,
        } => store_ops::reviews_reply(
            provider,
            &review,
            &message_file,
            &project_dir,
            dry_run,
            yes,
            json,
        ),
    }
}

pub fn release_workflow(command: ReleaseWorkflowCommand) -> Result<()> {
    match command {
        ReleaseWorkflowCommand::List { project_dir, json } => {
            workflow_ops::list(&project_dir, json)
        }
        ReleaseWorkflowCommand::Run {
            name,
            project_dir,
            dry_run,
            yes,
            json,
        } => workflow_ops::run(&project_dir, &name, dry_run, yes, json),
    }
}

pub fn auth(command: AuthCommand) -> Result<()> {
    auth_ops::auth(command)
}

fn edit_release_config(project_dir: &Path, tui: bool) -> Result<()> {
    let path = project_dir.join("fission.toml");
    fs::metadata(&path).with_context(|| format!("{} does not exist", path.display()))?;
    if tui {
        bail!("release-config edit --tui must be dispatched by the fission command entrypoint");
    }
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let status = Command::new(editor)
        .arg(&path)
        .status()
        .context("failed to launch editor")?;
    if !status.success() {
        bail!("editor exited with {status}");
    }
    Ok(())
}

fn set_release_field_command(
    project_dir: &Path,
    field: &str,
    value: &str,
    dry_run: bool,
    yes: bool,
    json: bool,
) -> Result<()> {
    if model::release_config_field_is_forbidden_secret(field, value) {
        bail!(
            "{field} looks like secret material or a machine-specific secret path; Fission refuses to write it to fission.toml. Store the value in an environment variable, CI secret, provider-owned auth state, or a local file outside the repository and write only the env var name when needed."
        );
    }
    validate_release_field_value(project_dir, field, value)?;
    let old_value = toml_field_value(project_dir, field)?;
    let mut receipt = mutation_receipt(project_dir, "release-config.set", dry_run);
    receipt.field = Some(field.to_string());
    receipt.old_value = old_value;
    receipt.new_value = Some(value.to_string());
    if !dry_run {
        set_release_field(project_dir, field, value, yes)?;
        receipt.status = "applied".to_string();
    }
    print_mutation_receipt(&receipt, json)
}

fn validate_release_field_value(project_dir: &Path, field: &str, value: &str) -> Result<()> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let doc = parse_toml_edit_document(&data, &path)?;
    toml_edit_value_for_field(&doc, field, value).map(|_| ())
}

fn set_release_field(project_dir: &Path, field: &str, value: &str, yes: bool) -> Result<()> {
    if !yes {
        bail!("set rewrites fission.toml; pass --yes after reviewing the field path");
    }
    if model::release_config_field_is_forbidden_secret(field, value) {
        bail!(
            "{field} looks like secret material or a machine-specific secret path; Fission refuses to write it to fission.toml. Store the value in an environment variable, CI secret, provider-owned auth state, or a local file outside the repository and write only the env var name when needed."
        );
    }
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut doc = parse_toml_edit_document(&data, &path)?;
    let value = toml_edit_value_for_field(&doc, field, value)?;
    set_toml_edit_path(&mut doc, field, value)?;
    write_toml_edit_document(&path, &doc)?;
    Ok(())
}

fn toml_edit_value_for_field(root: &DocumentMut, field: &str, value: &str) -> Result<TomlEditItem> {
    if let Some(existing) = toml_edit_path_item(root, field)? {
        if let Some(existing_value) = existing.as_value() {
            if existing_value.as_bool().is_some() {
                return parse_toml_bool_value(field, value);
            }
            if existing_value.as_integer().is_some() {
                return parse_toml_integer_value(field, value);
            }
            if existing_value.as_float().is_some() {
                return parse_toml_float_value(field, value);
            }
        }
    }

    let key = field.rsplit('.').next().unwrap_or(field);
    if release_config_integer_field(key) {
        return parse_toml_integer_value(field, value);
    }
    if release_config_bool_field(key) {
        return parse_toml_bool_value(field, value);
    }

    Ok(toml_edit::value(value.to_string()))
}

fn toml_edit_path_item<'a>(root: &'a DocumentMut, field: &str) -> Result<Option<&'a TomlEditItem>> {
    let parts = toml_path_segments(field)?;
    let mut current = root.as_table();
    for part in &parts[..parts.len() - 1] {
        let Some(item) = current.get(part.as_str()) else {
            return Ok(None);
        };
        let Some(table) = item.as_table() else {
            return Ok(None);
        };
        current = table;
    }
    Ok(current.get(parts[parts.len() - 1].as_str()))
}

fn release_config_integer_field(key: &str) -> bool {
    matches!(
        key,
        "build"
            | "version_code"
            | "min_sdk"
            | "target_sdk"
            | "presign_ttl_seconds"
            | "package_rollout_percentage"
    )
}

fn release_config_bool_field(key: &str) -> bool {
    matches!(
        key,
        "allow_upscale"
            | "include_desktop_entry"
            | "include_appstream_metadata"
            | "path_style"
            | "overwrite"
            | "share"
            | "autorename"
            | "draft"
            | "prerelease"
            | "replace_assets"
            | "upload_artifact_manifest"
            | "enforce_https"
            | "submit"
            | "is_silent_install"
            | "notarize"
    )
}

fn parse_toml_bool_value(field: &str, value: &str) -> Result<TomlEditItem> {
    match value.trim() {
        "true" => Ok(toml_edit::value(true)),
        "false" => Ok(toml_edit::value(false)),
        _ => bail!("{field} expects a boolean value: true or false"),
    }
}

fn parse_toml_integer_value(field: &str, value: &str) -> Result<TomlEditItem> {
    let parsed = value
        .trim()
        .parse::<i64>()
        .with_context(|| format!("{field} expects an integer value"))?;
    Ok(toml_edit::value(parsed))
}

fn parse_toml_float_value(field: &str, value: &str) -> Result<TomlEditItem> {
    let parsed = value
        .trim()
        .parse::<f64>()
        .with_context(|| format!("{field} expects a floating-point value"))?;
    Ok(toml_edit::value(parsed))
}

fn skip_release_requirement_command(
    project_dir: &Path,
    id: &str,
    dry_run: bool,
    yes: bool,
    json: bool,
) -> Result<()> {
    let id = id.trim();
    if id.is_empty() {
        bail!("skip requirement id cannot be empty");
    }
    let mut receipt = mutation_receipt(project_dir, "release-config.skip-requirement", dry_run);
    receipt.requirement_id = Some(id.to_string());
    if !dry_run {
        skip_release_requirement(project_dir, id, yes)?;
        receipt.status = "applied".to_string();
    }
    print_mutation_receipt(&receipt, json)
}

pub fn skip_release_requirement(project_dir: &Path, id: &str, yes: bool) -> Result<()> {
    if !yes {
        bail!(
            "skip-requirement rewrites fission.toml; pass --yes after reviewing the requirement id"
        );
    }
    let id = id.trim();
    if id.is_empty() {
        bail!("skip requirement id cannot be empty");
    }
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut doc = parse_toml_edit_document(&data, &path)?;
    append_release_skip_requirement(&mut doc, id)?;
    write_toml_edit_document(&path, &doc)?;
    Ok(())
}

pub fn bump_release_build(
    project_dir: &Path,
    target: Option<Target>,
    by: u64,
    yes: bool,
) -> Result<()> {
    bump_release_build_impl(project_dir, target, by, yes, true).map(|_| ())
}

fn bump_release_build_command(
    project_dir: &Path,
    target: Option<Target>,
    by: u64,
    dry_run: bool,
    yes: bool,
    json: bool,
) -> Result<()> {
    if by == 0 {
        bail!("--by must be greater than zero");
    }
    let plan = bump_build_plan(project_dir, target, by)?;
    let mut receipt = mutation_receipt(project_dir, "release-config.bump-build", dry_run);
    receipt.target = target.map(|target| target.as_str().to_string());
    receipt.field = Some(plan.field.to_string());
    receipt.old_value = Some(plan.old_value);
    receipt.new_value = Some(plan.new_value);
    if !dry_run {
        bump_release_build_impl(project_dir, target, by, yes, !json)?;
        receipt.status = "applied".to_string();
    }
    print_mutation_receipt(&receipt, json)
}

fn version_state_command(
    project_dir: &Path,
    provider: DistributionProvider,
    target: Option<Target>,
    format: Option<publish::PackageFormat>,
    track: Option<String>,
    site: &str,
    artifact: Option<&Path>,
    json_output: bool,
) -> Result<()> {
    let target = target.unwrap_or_else(|| publish::default_target_for_provider(provider));
    let format =
        format.unwrap_or_else(|| publish::default_format_for_target_provider(target, provider));
    let track = track.or_else(|| publish::default_track_for_provider(provider).map(str::to_string));
    let local = resolve_release_version_config(project_dir, Some(target))?;
    let local = VersionStateLocal {
        version: local.version,
        build: local.build,
    };
    let provider_status = publish::distribute_status_value(publish::DistributeOptions {
        project_dir: project_dir.to_path_buf(),
        provider,
        action: publish::DistributeAction::Status,
        target: Some(target),
        format: Some(format),
        artifact: artifact.map(Path::to_path_buf),
        site: site.to_string(),
        deploy: None,
        track: track.clone(),
        locales: Vec::new(),
        dry_run: false,
        yes: true,
        json: true,
    });
    let (provider_status, provider_error) = match provider_status {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let report = version_state_report(
        project_dir,
        provider,
        target,
        format,
        track,
        local,
        provider_status,
        provider_error,
    );
    print_version_state_report(&report, json_output)?;
    if report.status == "blocked" {
        bail!("release-config.version-state is blocked");
    }
    Ok(())
}

fn version_state_report(
    project_dir: &Path,
    provider: DistributionProvider,
    target: Target,
    format: publish::PackageFormat,
    track: Option<String>,
    local: VersionStateLocal,
    provider_status: Option<JsonValue>,
    provider_error: Option<String>,
) -> VersionStateReport {
    let stdout = provider_status
        .as_ref()
        .and_then(provider_status_stdout_json);
    let provider_state = provider_version_state(provider, stdout.as_ref());
    let monotonic = version_state_monotonic(provider, local.build, &provider_state);
    let next_action = version_state_next_action(&monotonic, provider_error.as_deref());
    let status = if monotonic.status == "failed" {
        "blocked"
    } else if provider_error.is_some() {
        "provider-unavailable"
    } else {
        "ok"
    }
    .to_string();
    VersionStateReport {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        action: "release-config.version-state".to_string(),
        status,
        project_dir: project_dir.display().to_string(),
        provider: provider.as_str().to_string(),
        target: target.as_str().to_string(),
        format: format.as_str().to_string(),
        track,
        local,
        provider_state,
        provider_status,
        provider_error,
        monotonic,
        next_action,
    }
}

fn provider_status_stdout_json(provider_status: &JsonValue) -> Option<JsonValue> {
    provider_status
        .get("stdout")
        .and_then(JsonValue::as_str)
        .and_then(|stdout| serde_json::from_str::<JsonValue>(stdout).ok())
}

fn provider_version_state(
    provider: DistributionProvider,
    stdout: Option<&JsonValue>,
) -> VersionStateProviderState {
    match provider {
        DistributionProvider::PlayStore => stdout.map(play_store_version_state).unwrap_or_default(),
        DistributionProvider::AppStore => stdout.map(app_store_version_state).unwrap_or_default(),
        DistributionProvider::MicrosoftStore => stdout
            .map(microsoft_store_version_state)
            .unwrap_or_default(),
        _ => VersionStateProviderState::default(),
    }
}

fn play_store_version_state(value: &JsonValue) -> VersionStateProviderState {
    let mut state = VersionStateProviderState::default();
    for release in value
        .get("releases")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        let status = release
            .get("status")
            .and_then(JsonValue::as_str)
            .map(str::to_string);
        for code in release
            .get("versionCodes")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(json_u64_or_string)
        {
            state.latest_released_build = max_optional_u64(state.latest_released_build, code);
            state.observed_builds.push(VersionStateObservedBuild {
                build: code,
                status: status.clone(),
                source: "play-store.track.release".to_string(),
            });
        }
        if state.latest_status.is_none() {
            state.latest_status = status;
        }
    }
    state
}

fn app_store_version_state(value: &JsonValue) -> VersionStateProviderState {
    let mut state = VersionStateProviderState::default();
    for build in value
        .get("builds")
        .and_then(|builds| builds.get("data"))
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        let Some(attributes) = build.get("attributes") else {
            continue;
        };
        let status = attributes
            .get("processingState")
            .and_then(JsonValue::as_str)
            .map(str::to_string);
        let Some(build_number) = attributes
            .get("version")
            .and_then(JsonValue::as_str)
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        state.latest_uploaded_build = max_optional_u64(state.latest_uploaded_build, build_number);
        state.observed_builds.push(VersionStateObservedBuild {
            build: build_number,
            status: status.clone(),
            source: "app-store.build".to_string(),
        });
        if state.latest_status.is_none() {
            state.latest_status = status;
        }
    }
    state.review_status = value
        .get("review_submissions")
        .and_then(|submissions| submissions.get("data"))
        .and_then(JsonValue::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("attributes"))
        .and_then(|attributes| attributes.get("state"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    state
}

fn microsoft_store_version_state(value: &JsonValue) -> VersionStateProviderState {
    let mut state = VersionStateProviderState::default();
    state.latest_status = value
        .get("status")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .or_else(|| {
            value
                .get("submissionStatus")
                .and_then(JsonValue::as_str)
                .map(str::to_string)
        });
    state
}

fn version_state_monotonic(
    provider: DistributionProvider,
    local_build: Option<u64>,
    provider_state: &VersionStateProviderState,
) -> VersionStateMonotonic {
    let latest_provider_build = max_optional_pair(
        provider_state.latest_uploaded_build,
        provider_state.latest_released_build,
    );
    if !matches!(
        provider,
        DistributionProvider::PlayStore
            | DistributionProvider::AppStore
            | DistributionProvider::MicrosoftStore
    ) {
        return VersionStateMonotonic {
            status: "not-applicable".to_string(),
            local_build,
            latest_provider_build,
            details: Some(
                "provider does not expose an app-store monotonic build constraint".to_string(),
            ),
        };
    }
    let Some(local_build) = local_build else {
        return VersionStateMonotonic {
            status: "unknown".to_string(),
            local_build,
            latest_provider_build,
            details: Some("local build number is not configured".to_string()),
        };
    };
    let Some(latest_provider_build) = latest_provider_build else {
        return VersionStateMonotonic {
            status: "unknown".to_string(),
            local_build: Some(local_build),
            latest_provider_build,
            details: Some("provider status did not include a comparable build number".to_string()),
        };
    };
    if local_build <= latest_provider_build {
        VersionStateMonotonic {
            status: "failed".to_string(),
            local_build: Some(local_build),
            latest_provider_build: Some(latest_provider_build),
            details: Some(format!(
                "local build {local_build} is not greater than provider build {latest_provider_build}"
            )),
        }
    } else {
        VersionStateMonotonic {
            status: "passed".to_string(),
            local_build: Some(local_build),
            latest_provider_build: Some(latest_provider_build),
            details: Some(format!(
                "local build {local_build} is greater than provider build {latest_provider_build}"
            )),
        }
    }
}

fn version_state_next_action(
    monotonic: &VersionStateMonotonic,
    provider_error: Option<&str>,
) -> String {
    if provider_error.is_some() {
        return "inspect-provider-credentials".to_string();
    }
    match monotonic.status.as_str() {
        "failed" => "bump-build".to_string(),
        "passed" => "upload-new-build".to_string(),
        "unknown" => "inspect-provider-state".to_string(),
        _ => "continue".to_string(),
    }
}

fn print_version_state_report(report: &VersionStateReport, json_output: bool) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!("{}: {}", report.action, report.status);
    println!("provider: {}", report.provider);
    println!("target: {}", report.target);
    println!("format: {}", report.format);
    if let Some(track) = &report.track {
        println!("track: {track}");
    }
    println!(
        "local: version={} build={}",
        report.local.version.as_deref().unwrap_or("unknown"),
        report
            .local
            .build
            .map(|build| build.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    if let Some(build) = report.provider_state.latest_uploaded_build {
        println!("provider latest uploaded build: {build}");
    }
    if let Some(build) = report.provider_state.latest_released_build {
        println!("provider latest released build: {build}");
    }
    if let Some(status) = &report.provider_state.latest_status {
        println!("provider latest status: {status}");
    }
    if let Some(status) = &report.provider_state.review_status {
        println!("provider review status: {status}");
    }
    println!("monotonic: {}", report.monotonic.status);
    if let Some(details) = &report.monotonic.details {
        println!("details: {details}");
    }
    if let Some(error) = &report.provider_error {
        println!("provider error: {error}");
    }
    println!("next action: {}", report.next_action);
    Ok(())
}

fn json_u64_or_string(value: &JsonValue) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
}

fn max_optional_u64(current: Option<u64>, candidate: u64) -> Option<u64> {
    Some(current.map_or(candidate, |current| current.max(candidate)))
}

fn max_optional_pair(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn bump_release_build_impl(
    project_dir: &Path,
    target: Option<Target>,
    by: u64,
    yes: bool,
    emit_human: bool,
) -> Result<u64> {
    if !yes {
        bail!("bump-build rewrites fission.toml; pass --yes after reviewing the target");
    }
    if by == 0 {
        bail!("--by must be greater than zero");
    }
    let plan = bump_build_plan(project_dir, target, by)?;
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut doc = parse_toml_edit_document(&data, &path)?;
    let value = if matches!(target, Some(Target::Windows | Target::Ios | Target::Macos)) {
        toml_edit::value(plan.new_value.clone())
    } else {
        toml_edit::value(
            i64::try_from(plan.next_build).context("build number is too large for TOML integer")?,
        )
    };
    set_toml_edit_path(&mut doc, plan.field, value)?;
    write_toml_edit_document(&path, &doc)?;
    if emit_human {
        println!(
            "Bumped {} from {} to {}",
            plan.field, plan.old_value, plan.new_value
        );
    }
    Ok(plan.next_build)
}

fn build_field_for_target(target: Option<Target>) -> &'static str {
    match target {
        Some(Target::Android) => "package.android.version_code",
        Some(Target::Ios) => "package.ios.build_number",
        Some(Target::Macos) => "package.macos.build_number",
        Some(Target::Windows) => "package.windows.version",
        _ => "app.build",
    }
}

fn bump_build_plan(project_dir: &Path, target: Option<Target>, by: u64) -> Result<BumpBuildPlan> {
    let resolved = resolve_release_version_config(project_dir, target)?;
    let current = if target == Some(Target::Windows) {
        windows_package_version_build(resolved.version.as_deref()).or(resolved.build)
    } else {
        resolved.build
    }
    .unwrap_or(0);
    let next = current
        .checked_add(by)
        .context("build number overflow while incrementing release build")?;
    if target == Some(Target::Windows) {
        let old_version =
            normalize_windows_package_version(resolved.version.as_deref(), Some(current))?;
        let new_version = windows_package_version_with_build(resolved.version.as_deref(), next)?;
        return Ok(BumpBuildPlan {
            field: build_field_for_target(target),
            old_value: old_version,
            new_value: new_version,
            next_build: next,
        });
    }
    Ok(BumpBuildPlan {
        field: build_field_for_target(target),
        old_value: current.to_string(),
        new_value: next.to_string(),
        next_build: next,
    })
}

fn windows_package_version_build(version: Option<&str>) -> Option<u64> {
    version?
        .split('.')
        .nth(3)
        .and_then(|value| value.parse::<u64>().ok())
}

fn windows_package_version_with_build(version: Option<&str>, build: u64) -> Result<String> {
    let version = version.unwrap_or("0.1.0");
    let mut parts = version.split('.').collect::<Vec<_>>();
    if parts.len() > 4 {
        bail!("Windows package version `{version}` must have one to four numeric components");
    }
    for part in &parts {
        part.parse::<u16>()
            .with_context(|| format!("Windows package version `{version}` must be numeric"))?;
    }
    if build > u16::MAX as u64 {
        bail!("Windows package build `{build}` must fit in a 16-bit version component");
    }
    while parts.len() < 3 {
        parts.push("0");
    }
    if parts.len() == 3 {
        parts.push("");
    }
    let mut owned = parts.into_iter().map(str::to_string).collect::<Vec<_>>();
    owned[3] = build.to_string();
    Ok(owned.join("."))
}

fn append_release_skip_requirement(doc: &mut DocumentMut, id: &str) -> Result<()> {
    let release = doc
        .as_table_mut()
        .entry("release")
        .or_insert(TomlEditItem::Table(TomlEditTable::new()))
        .as_table_mut()
        .context("release path traversed through a non-table value")?;
    let item = release
        .entry("skip_requirements")
        .or_insert_with(|| TomlEditItem::Value(TomlEditValue::Array(TomlEditArray::default())));
    let array = item
        .as_array_mut()
        .context("release.skip_requirements must be an array")?;
    if !array.iter().any(|value| value.as_str() == Some(id)) {
        array.push(id);
    }
    Ok(())
}

fn add_release(
    project_dir: &Path,
    version: &str,
    build: u64,
    from: Option<&str>,
    yes: bool,
) -> Result<()> {
    if !yes {
        bail!("add-release appends to fission.toml; pass --yes after reviewing the release id");
    }
    let path = project_dir.join("fission.toml");
    let mut text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let id = format!("{version}+{build}");
    text.push_str(&format!(
        "\n[[releases]]\nid = \"{id}\"\nversion = \"{version}\"\nbuild = {build}\nstatus = \"candidate\"\nmetadata = \"release-content/metadata/{id}/release.toml\"\nrelease_notes = \"release-content/metadata/{id}/notes\"\nreview = \"release-content/metadata/{id}/review.toml\"\nprivacy = \"release-content/metadata/{id}/privacy.toml\"\n"
    ));
    if let Some(source) = from {
        text.push_str(&format!("# copied_from = \"{source}\"\n"));
    }
    fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn parse_toml_edit_document(text: &str, path: &Path) -> Result<DocumentMut> {
    text.parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn toml_field_value(project_dir: &Path, field: &str) -> Result<Option<String>> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    let parts = toml_path_segments(field)?;
    Ok(parts
        .iter()
        .try_fold(&value, |current, part| current.get(part))
        .map(toml_value_summary))
}

fn toml_value_summary(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => value.clone(),
        toml::Value::Integer(value) => value.to_string(),
        toml::Value::Float(value) => value.to_string(),
        toml::Value::Boolean(value) => value.to_string(),
        toml::Value::Datetime(value) => value.to_string(),
        _ => value.to_string(),
    }
}

fn write_toml_edit_document(path: &Path, doc: &DocumentMut) -> Result<()> {
    fs::write(path, format!("{doc}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

fn set_toml_edit_path(root: &mut DocumentMut, path: &str, value: TomlEditItem) -> Result<()> {
    let parts = toml_path_segments(path)?;
    let mut current = root.as_table_mut();
    for part in &parts[..parts.len() - 1] {
        current = current
            .entry(part.as_str())
            .or_insert(TomlEditItem::Table(TomlEditTable::new()))
            .as_table_mut()
            .context("field path traversed through a non-table value")?;
    }
    current[parts[parts.len() - 1].as_str()] = value;
    Ok(())
}

fn toml_path_segments(path: &str) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = path.chars().peekable();
    let mut quote = None;
    let mut escaped = false;
    let mut closed_quote = false;

    while let Some(ch) = chars.next() {
        if escaped {
            current.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
            continue;
        }
        if let Some(q) = quote {
            match ch {
                '\\' if q == '"' => escaped = true,
                c if c == q => {
                    quote = None;
                    closed_quote = true;
                }
                c => current.push(c),
            }
            continue;
        }
        match ch {
            '"' | '\'' if current.trim().is_empty() => {
                current.clear();
                quote = Some(ch);
            }
            '.' => {
                push_toml_path_segment(path, &mut parts, &mut current)?;
                closed_quote = false;
            }
            c if c.is_whitespace() && closed_quote => {}
            _ if closed_quote => bail!(
                "invalid TOML field path `{path}`: quoted segment must be followed by `.` or end"
            ),
            c => current.push(c),
        }
    }
    if let Some(q) = quote {
        bail!("unterminated {q} quote in TOML field path `{path}`");
    }
    push_toml_path_segment(path, &mut parts, &mut current)?;
    Ok(parts)
}

fn push_toml_path_segment(
    original: &str,
    parts: &mut Vec<String>,
    current: &mut String,
) -> Result<()> {
    let part = current.trim();
    if part.is_empty() {
        bail!("field path must be dot-separated and non-empty: `{original}`");
    }
    parts.push(part.to_string());
    current.clear();
    Ok(())
}

fn toml_edit_string_array(values: impl IntoIterator<Item = String>) -> TomlEditItem {
    let mut array = TomlEditArray::default();
    for value in values {
        array.push(value);
    }
    TomlEditItem::Value(TomlEditValue::Array(array))
}

fn add_release_command(
    project_dir: &Path,
    version: &str,
    build: u64,
    from: Option<&str>,
    dry_run: bool,
    yes: bool,
    json: bool,
) -> Result<()> {
    let release_id = format!("{version}+{build}");
    let mut receipt = mutation_receipt(project_dir, "release-config.add-release", dry_run);
    receipt.release_id = Some(release_id);
    receipt.new_value = Some(format!("version={version}, build={build}"));
    if let Some(source) = from {
        receipt.old_value = Some(format!("copied_from={source}"));
    }
    if !dry_run {
        add_release(project_dir, version, build, from, yes)?;
        receipt.status = "applied".to_string();
    }
    print_mutation_receipt(&receipt, json)
}

fn mutation_receipt(
    project_dir: &Path,
    action: impl Into<String>,
    dry_run: bool,
) -> ReleaseConfigMutationReceipt {
    ReleaseConfigMutationReceipt {
        schema_version: 1,
        action: action.into(),
        status: if dry_run { "dry-run" } else { "pending" }.to_string(),
        project_dir: project_dir.display().to_string(),
        target: None,
        provider: None,
        field: None,
        kind: None,
        locale: None,
        path: None,
        old_value: None,
        new_value: None,
        release_id: None,
        requirement_id: None,
    }
}

fn print_mutation_receipt(receipt: &ReleaseConfigMutationReceipt, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(receipt)?);
    } else {
        println!("{}: {}", receipt.action, receipt.status);
        if let Some(field) = &receipt.field {
            println!("field: {field}");
        }
        if let Some(provider) = &receipt.provider {
            println!("provider: {provider}");
        }
        if let Some(kind) = &receipt.kind {
            println!("kind: {kind}");
        }
        if let Some(locale) = &receipt.locale {
            println!("locale: {locale}");
        }
        if let Some(path) = &receipt.path {
            println!("path: {path}");
        }
        if let Some(release_id) = &receipt.release_id {
            println!("release: {release_id}");
        }
        if let Some(requirement_id) = &receipt.requirement_id {
            println!("requirement: {requirement_id}");
        }
        if let Some(old_value) = &receipt.old_value {
            println!("old: {old_value}");
        }
        if let Some(new_value) = &receipt.new_value {
            println!("new: {new_value}");
        }
    }
    Ok(())
}

fn edit_release_file(
    project_dir: &Path,
    release: &str,
    kind: &str,
    locale: Option<&str>,
) -> Result<()> {
    let path = release_file_path(project_dir, release, kind, locale)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        fs::write(&path, "")?;
    }
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let status = Command::new(editor).arg(&path).status()?;
    if !status.success() {
        bail!("editor exited with {status}");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_release_file_command(
    project_dir: &Path,
    release: &str,
    kind: &str,
    provider: Option<DistributionProvider>,
    locale: Option<&str>,
    value: Option<&str>,
    from_file: Option<&Path>,
    dry_run: bool,
    yes: bool,
    json: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("write-file rewrites a release metadata file; pass --yes after reviewing the release, kind, locale, and source");
    }
    let content = release_file_content(project_dir, value, from_file)?;
    let path = release_file_path(project_dir, release, kind, locale)?;
    let mut receipt = mutation_receipt(project_dir, "release-config.write-file", dry_run);
    receipt.provider = provider.map(|provider| provider.as_str().to_string());
    receipt.kind = Some(kind.to_string());
    receipt.locale = locale.map(str::to_string);
    receipt.path = Some(path.display().to_string());
    receipt.release_id = Some(release.to_string());
    receipt.old_value = release_file_summary(&path)?;
    receipt.new_value = Some(format!(
        "{} byte(s) from {}",
        content.len(),
        from_file
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "--value".to_string())
    ));
    if !dry_run {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
        receipt.status = "applied".to_string();
    }
    print_mutation_receipt(&receipt, json)
}

fn release_file_content(
    project_dir: &Path,
    value: Option<&str>,
    from_file: Option<&Path>,
) -> Result<Vec<u8>> {
    match (value, from_file) {
        (Some(_), Some(_)) => bail!("write-file accepts either --value or --from-file, not both"),
        (Some(value), None) => Ok(value.as_bytes().to_vec()),
        (None, Some(path)) => {
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                project_dir.join(path)
            };
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))
        }
        (None, None) => bail!("write-file requires --value or --from-file"),
    }
}

fn release_file_summary(path: &Path) -> Result<Option<String>> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(format!("exists, {} byte(s)", metadata.len()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to stat {}", path.display())),
    }
}

fn release_file_path(
    project_dir: &Path,
    release: &str,
    kind: &str,
    locale: Option<&str>,
) -> Result<PathBuf> {
    let release = release_path_segment("release", release)?;
    let mut path = project_dir
        .join("release-content")
        .join("metadata")
        .join(release);
    match kind {
        "notes" => {
            path = path.join("notes");
            path.push(format!(
                "{}.md",
                release_path_segment("locale", locale.unwrap_or("en-US"))?
            ));
        }
        "review" => path.push("review.toml"),
        "privacy" => path.push("privacy.toml"),
        "metadata" | "release" => path.push("release.toml"),
        other => bail!("unsupported release file kind `{other}`"),
    }
    Ok(path)
}

fn release_path_segment<'a>(label: &str, value: &'a str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains('\0')
    {
        bail!("{label} must be a single path segment");
    }
    Ok(value)
}

fn set_toml_path(root: &mut toml::Value, path: &str, value: toml::Value) -> Result<()> {
    let mut current = root;
    let parts = path.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.trim().is_empty()) {
        bail!("field path must be dot-separated and non-empty");
    }
    for part in &parts[..parts.len() - 1] {
        let table = current
            .as_table_mut()
            .context("field path traversed through a non-table value")?;
        current = table
            .entry((*part).to_string())
            .or_insert_with(|| toml::Value::Table(Default::default()));
    }
    let table = current
        .as_table_mut()
        .context("field path parent is not a table")?;
    table.insert(parts[parts.len() - 1].to_string(), value);
    Ok(())
}

fn base_report(
    area: &str,
    provider: Option<DistributionProvider>,
    target: Option<Target>,
) -> LifecycleReport {
    LifecycleReport {
        area: area.to_string(),
        status: "ready".to_string(),
        provider: provider.map(|provider| provider.as_str().to_string()),
        target: target.map(|target| target.as_str().to_string()),
        checks: Vec::new(),
    }
}

fn path_check(id: &str, path: PathBuf, summary: &str) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: if path.exists() { "passed" } else { "missing" }.to_string(),
        summary: summary.to_string(),
        details: Some(path.display().to_string()),
        remediation: vec![
            "Create the file/directory or update fission.toml to point at the correct path."
                .to_string(),
        ],
    }
}

fn value_path_check(value: &toml::Value, path: &str, id: &str, summary: &str) -> LifecycleCheck {
    let exists = path
        .split('.')
        .try_fold(value, |current, segment| current.get(segment))
        .is_some();
    LifecycleCheck {
        id: id.to_string(),
        status: if exists { "passed" } else { "missing" }.to_string(),
        summary: summary.to_string(),
        details: Some(path.to_string()),
        remediation: vec![
            "Add the missing release configuration or use fission release-config add-release/set."
                .to_string(),
        ],
    }
}

fn ok_check(id: &str, details: impl Into<String>) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: "passed".to_string(),
        summary: id.replace('_', " "),
        details: Some(details.into()),
        remediation: Vec::new(),
    }
}

fn warning_check(id: &str, details: String) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: "warning".to_string(),
        summary: id.replace('_', " "),
        details: Some(details),
        remediation: vec![
            "Wire the provider backend before using this command to mutate remote state."
                .to_string(),
        ],
    }
}

fn failed_check(id: &str, details: String) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: "failed".to_string(),
        summary: id.replace('_', " "),
        details: Some(details),
        remediation: vec!["Fix the reported error and rerun the command.".to_string()],
    }
}

fn finalize_status(report: &mut LifecycleReport) {
    report.status = if report
        .checks
        .iter()
        .any(|check| check.status == "failed" || check.status == "missing")
    {
        "blocked"
    } else if report.checks.iter().any(|check| check.status == "warning") {
        "warning"
    } else {
        "ready"
    }
    .to_string();
}

fn print_report(mut report: LifecycleReport, json: bool) -> Result<()> {
    finalize_status(&mut report);
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}: {}", report.area, report.status);
        for check in &report.checks {
            println!("[{}] {} - {}", check.status, check.id, check.summary);
            if let Some(details) = &check.details {
                println!("  {details}");
            }
            for remediation in &check.remediation {
                println!("  fix: {remediation}");
            }
        }
    }
    if report.status == "blocked" {
        bail!("{} is blocked", report.area);
    }
    Ok(())
}

#[cfg(test)]
mod release_config_tests;
