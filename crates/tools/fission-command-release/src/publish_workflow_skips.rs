use super::*;

pub(super) fn apply_skipped_requirements(
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

pub(super) fn release_skipped_requirements(project_dir: &Path) -> Result<BTreeSet<String>> {
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

pub(super) fn check_id_fragment(value: &str) -> String {
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

pub(super) fn version_match_requirement(
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
