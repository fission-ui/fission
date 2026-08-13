use super::TaskStatus;
use anyhow::Result;
use fission_command_package::{
    publish_flow_snapshot, CheckSeverity, CheckStatus, PublishFlowSnapshot, PublishShellOptions,
    ReadinessCheck,
};
use fission_command_release::{
    release_plan_snapshot, release_readiness_checks, PublishWorkflowOptions, ReleasePlanSnapshot,
};
use std::env;
use std::sync::{Arc, Mutex};

pub(super) fn redact_output_lines(output: &str) -> Vec<String> {
    output.lines().map(redact_sensitive_text).collect()
}

fn redact_sensitive_text(text: &str) -> String {
    let mut redacted = text.to_string();
    for (key, value) in secret_env_values() {
        redacted = redacted.replace(&value, &format!("<redacted:{key}>"));
    }
    redacted
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

#[derive(Clone, Debug)]
pub(super) struct SnapshotRefreshResult {
    pub(super) snapshot: PublishFlowSnapshot,
    pub(super) release_plan: Option<ReleasePlanSnapshot>,
    pub(super) release_checks: Vec<ReadinessCheck>,
}

pub(super) fn collect_refresh_snapshot(
    options: PublishShellOptions,
) -> Result<SnapshotRefreshResult> {
    let snapshot = publish_flow_snapshot(&options)?;
    let workflow_options = publish_workflow_options_for_snapshot(&options, &snapshot);
    let release_plan = release_plan_snapshot(workflow_options.clone()).ok();
    let release_checks = release_readiness_checks(workflow_options)
        .unwrap_or_else(|err| readiness_error("release.plan.readiness_failed", err));
    Ok(SnapshotRefreshResult {
        snapshot,
        release_plan,
        release_checks,
    })
}

fn publish_workflow_options_for_snapshot(
    options: &PublishShellOptions,
    snapshot: &PublishFlowSnapshot,
) -> PublishWorkflowOptions {
    PublishWorkflowOptions {
        project_dir: snapshot.project_dir.clone(),
        provider: snapshot.provider,
        target: Some(snapshot.target),
        format: Some(snapshot.format),
        artifact: if snapshot.artifact_manifest.as_os_str().is_empty() {
            None
        } else {
            Some(snapshot.artifact_manifest.clone())
        },
        site: snapshot.site.clone(),
        deploy: options.deploy.clone(),
        track: snapshot.track.clone().or_else(|| options.track.clone()),
        locales: if snapshot.locales.is_empty() {
            options.locales.clone()
        } else {
            snapshot.locales.clone()
        },
        overwrite_remote: false,
        dry_run: true,
        yes: true,
        json: true,
    }
}

fn readiness_error(id: &str, err: anyhow::Error) -> Vec<ReadinessCheck> {
    vec![ReadinessCheck {
        id: id.to_string(),
        severity: CheckSeverity::Error,
        status: CheckStatus::Failed,
        summary: "release plan could not be loaded".to_string(),
        details: Some(err.to_string()),
        remediation: vec![
            "Run `fission readiness release --json` for full release-plan diagnostics.".to_string(),
        ],
    }]
}

#[derive(Clone, Debug)]
pub(crate) struct SnapshotRefreshState {
    pub(super) shared: Arc<Mutex<SnapshotRefreshData>>,
}

impl SnapshotRefreshState {
    pub(super) fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(SnapshotRefreshData {
                status: TaskStatus::Running,
                revision: 1,
                message: "Refreshing preflight...".to_string(),
                result: None,
            })),
        }
    }

    pub(super) fn status(&self) -> TaskStatus {
        self.shared
            .lock()
            .expect("snapshot refresh lock poisoned")
            .status
    }

    pub(super) fn revision(&self) -> u64 {
        self.shared
            .lock()
            .expect("snapshot refresh lock poisoned")
            .revision
    }

    pub(super) fn message(&self) -> String {
        self.shared
            .lock()
            .expect("snapshot refresh lock poisoned")
            .message
            .clone()
    }

    pub(super) fn result(&self) -> Option<Result<SnapshotRefreshResult, String>> {
        self.shared
            .lock()
            .expect("snapshot refresh lock poisoned")
            .result
            .clone()
    }
}

impl PartialEq for SnapshotRefreshState {
    fn eq(&self, other: &Self) -> bool {
        self.revision() == other.revision()
    }
}

#[derive(Clone, Debug)]
pub(super) struct SnapshotRefreshData {
    pub(super) status: TaskStatus,
    pub(super) revision: u64,
    pub(super) message: String,
    pub(super) result: Option<Result<SnapshotRefreshResult, String>>,
}
