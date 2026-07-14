use super::*;

pub(super) fn release_steps(
    requirements: &[ReleaseRequirement],
    has_artifact: bool,
    status: &str,
) -> Vec<ReleaseStep> {
    vec![
        ReleaseStep {
            id: "inspect".to_string(),
            title: "Inspect project and release configuration".to_string(),
            status: step_status_for(
                requirements,
                &["release_config.", "release.version.", "release.build."],
            ),
            jobs: vec![ReleaseJob {
                id: "inspect.fission_toml".to_string(),
                title: "Read fission.toml, package config, and active release".to_string(),
                status: step_status_for(
                    requirements,
                    &["release_config.", "release.version.", "release.build."],
                ),
            }],
        },
        ReleaseStep {
            id: "preflight".to_string(),
            title: "Preflight target, package, provider, and secrets".to_string(),
            status: step_status_for(requirements, PREFLIGHT_PREFIXES),
            jobs: vec![ReleaseJob {
                id: "preflight.requirements".to_string(),
                title: "Evaluate provider-required and Fission-recommended requirements"
                    .to_string(),
                status: step_status_for(requirements, PREFLIGHT_PREFIXES),
            }],
        },
        ReleaseStep {
            id: "configure".to_string(),
            title: "Configure app identity, signing, track, locales, and provider account"
                .to_string(),
            status: step_status_for(
                requirements,
                &[
                    "release_config.",
                    "signing.",
                    "release.play_store.track",
                    "release.app_store.track",
                    "release.microsoft_store.track",
                    "distribution.",
                ],
            ),
            jobs: vec![ReleaseJob {
                id: "configure.release_inputs".to_string(),
                title: "Resolve release inputs and interactive fixes".to_string(),
                status: step_status_for(
                    requirements,
                    &[
                        "release_config.",
                        "signing.",
                        "release.play_store.track",
                        "release.app_store.track",
                        "release.microsoft_store.track",
                        "distribution.",
                    ],
                ),
            }],
        },
        ReleaseStep {
            id: "release_content".to_string(),
            title: "Validate release notes, screenshots, metadata, and review content".to_string(),
            status: step_status_for(requirements, &["release_content."]),
            jobs: vec![ReleaseJob {
                id: "release_content.validate".to_string(),
                title: "Validate release-content references and provider assets".to_string(),
                status: step_status_for(requirements, &["release_content."]),
            }],
        },
        ReleaseStep {
            id: "package".to_string(),
            title: "Build and package artifact".to_string(),
            status: if has_artifact {
                StepStatus::Completed
            } else if status == "blocked" {
                StepStatus::Pending
            } else {
                StepStatus::Ready
            },
            jobs: vec![ReleaseJob {
                id: "package.artifact".to_string(),
                title: "Create artifact manifest with target, format, version, build, and hashes"
                    .to_string(),
                status: if has_artifact {
                    StepStatus::Completed
                } else {
                    StepStatus::Pending
                },
            }],
        },
        ReleaseStep {
            id: "dry_run".to_string(),
            title: "Preview artifact, provider destination, metadata, assets, and mutations"
                .to_string(),
            status: if status == "blocked" {
                StepStatus::Pending
            } else {
                StepStatus::Ready
            },
            jobs: vec![ReleaseJob {
                id: "dry_run.plan".to_string(),
                title: "Show exact artifact, version/build, track, locales, and provider action"
                    .to_string(),
                status: if status == "blocked" {
                    StepStatus::Pending
                } else {
                    StepStatus::Ready
                },
            }],
        },
        ReleaseStep {
            id: "publish".to_string(),
            title: "Dry-run, publish, and write receipt".to_string(),
            status: match status {
                "blocked" => StepStatus::Blocked,
                "warning" => StepStatus::Warning,
                _ => StepStatus::Ready,
            },
            jobs: vec![ReleaseJob {
                id: "publish.provider".to_string(),
                title: "Upload/update provider state and write a distribution receipt".to_string(),
                status: match status {
                    "blocked" => StepStatus::Blocked,
                    _ => StepStatus::Ready,
                },
            }],
        },
        ReleaseStep {
            id: "observe".to_string(),
            title: "Observe provider processing state and follow-up".to_string(),
            status: if status == "blocked" {
                StepStatus::Pending
            } else {
                StepStatus::Ready
            },
            jobs: vec![ReleaseJob {
                id: "observe.provider_status".to_string(),
                title: "Fetch provider status after upload/update when supported".to_string(),
                status: if status == "blocked" {
                    StepStatus::Pending
                } else {
                    StepStatus::Ready
                },
            }],
        },
    ]
}

pub(super) fn step_status_label(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Pending => "pending",
        StepStatus::Blocked => "blocked",
        StepStatus::Warning => "warning",
        StepStatus::Ready => "ready",
        StepStatus::Completed => "completed",
    }
}

pub(super) fn requirement_level_label(level: RequirementLevel) -> &'static str {
    match level {
        RequirementLevel::ProviderRequired => "provider-required",
        RequirementLevel::FissionRecommended => "fission-recommended",
        RequirementLevel::Optional => "optional",
    }
}

pub(super) fn requirement_status_label(status: RequirementStatus) -> &'static str {
    match status {
        RequirementStatus::Passed => "passed",
        RequirementStatus::Missing => "missing",
        RequirementStatus::Failed => "failed",
        RequirementStatus::Warning => "warning",
        RequirementStatus::Skipped => "skipped",
    }
}

const PREFLIGHT_PREFIXES: &[&str] = &[
    "release.package.",
    "release.distribution.",
    "release.play_store.",
    "release.app_store.",
    "release.microsoft_store.",
    "release.github_releases.",
    "release.github_pages.",
    "release.cloudflare_pages.",
    "release.netlify.",
    "release.s3.",
    "release.google_drive.",
    "release.onedrive.",
    "release.dropbox.",
    "release.docker_registry.",
];

fn step_status_for(requirements: &[ReleaseRequirement], prefixes: &[&str]) -> StepStatus {
    let matching = requirements
        .iter()
        .filter(|req| prefixes.iter().any(|prefix| req.id.starts_with(prefix)))
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return StepStatus::Ready;
    }
    if matching.iter().any(|req| {
        req.level == RequirementLevel::ProviderRequired
            && matches!(
                req.status,
                RequirementStatus::Missing | RequirementStatus::Failed
            )
    }) {
        StepStatus::Blocked
    } else if matching.iter().any(|req| {
        !matches!(
            req.status,
            RequirementStatus::Passed | RequirementStatus::Skipped
        )
    }) {
        StepStatus::Warning
    } else {
        StepStatus::Ready
    }
}

pub(super) fn print_release_plan(report: &ReleasePlanReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!("release plan: {}", report.status);
    println!("provider: {}", report.provider);
    if let Some(target) = &report.target {
        println!("target: {target}");
    }
    if let Some(format) = &report.format {
        println!("format: {format}");
    }
    if let Some(artifact) = &report.artifact {
        println!("artifact: {artifact}");
    }
    println!();
    println!("capabilities:");
    for capability in &report.capabilities {
        println!(
            "[{:?}] {} - {}",
            capability.status, capability.id, capability.summary
        );
        if let Some(details) = &capability.details {
            println!("  {details}");
        }
    }
    println!();
    for req in &report.requirements {
        let marker = match req.level {
            RequirementLevel::ProviderRequired => "required",
            RequirementLevel::FissionRecommended => "recommended",
            RequirementLevel::Optional => "optional",
        };
        println!("[{:?}] {marker}: {}", req.status, req.id);
        if let Some(details) = &req.details {
            println!("  {details}");
        }
        for remediation in &req.remediation {
            println!("  fix: {remediation}");
        }
    }
    Ok(())
}
