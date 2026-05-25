use super::*;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
struct ReleaseToml {
    app: Option<toml::Value>,
    release: Option<ReleaseRoot>,
    #[serde(default)]
    releases: Vec<ReleaseEntry>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseRoot {
    active_release: Option<String>,
    metadata_root: Option<String>,
    content_output_dir: Option<String>,
    #[serde(default)]
    default_locales: Vec<String>,
    #[serde(default)]
    store_listing: BTreeMap<String, BTreeMap<String, toml::Value>>,
}

#[derive(Debug, Deserialize, Default)]
struct ReleaseEntry {
    id: Option<String>,
    version: Option<String>,
    build: Option<u64>,
    status: Option<String>,
    #[serde(default)]
    tracks: Vec<String>,
    #[serde(default)]
    locales: Vec<String>,
    metadata: Option<String>,
    release_notes: Option<String>,
    review: Option<String>,
    privacy: Option<String>,
}

pub(super) fn validate_release_config_model(
    project_dir: &Path,
    provider: Option<DistributionProvider>,
) -> Result<LifecycleReport> {
    let mut report = base_report("release-config.validate", provider, None);
    let path = project_dir.join("fission.toml");
    report.checks.push(path_check(
        "release_config.fission_toml_exists",
        path.clone(),
        "fission.toml exists",
    ));
    if !path.exists() {
        finalize_status(&mut report);
        return Ok(report);
    }

    let data = fs::read_to_string(&path)?;
    let value = match toml::from_str::<toml::Value>(&data) {
        Ok(value) => {
            report.checks.push(ok_check(
                "release_config.toml_parses",
                "fission.toml parses",
            ));
            value
        }
        Err(error) => {
            report.checks.push(failed_check(
                "release_config.toml_parses",
                error.to_string(),
            ));
            finalize_status(&mut report);
            return Ok(report);
        }
    };
    let manifest: ReleaseToml = toml::from_str(&data).context("failed to parse release schema")?;

    report.checks.push(value_path_check(
        &value,
        "app",
        "release_config.app_table",
        "[app] table exists",
    ));
    report.checks.push(value_path_check(
        &value,
        "release",
        "release_config.release_table",
        "[release] table exists",
    ));
    report.checks.push(value_path_check(
        &value,
        "releases",
        "release_config.releases",
        "[[releases]] entries exist",
    ));

    if manifest.app.is_none() {
        report.checks.push(failed_check(
            "release_config.app_required",
            "[app] metadata is required for post-build lifecycle commands".to_string(),
        ));
    }
    let Some(root) = manifest.release.as_ref() else {
        finalize_status(&mut report);
        return Ok(report);
    };

    report.checks.push(required_scalar_check(
        "release_config.active_release",
        root.active_release.as_deref(),
        "release.active_release is set",
    ));
    report.checks.push(required_scalar_check(
        "release_config.metadata_root",
        root.metadata_root.as_deref(),
        "release.metadata_root is set",
    ));
    report.checks.push(required_scalar_check(
        "release_config.content_output_dir",
        root.content_output_dir.as_deref(),
        "release.content_output_dir is set",
    ));
    report.checks.push(list_check(
        "release_config.default_locales",
        &root.default_locales,
        "release.default_locales contains at least one locale",
    ));

    let active = root.active_release.as_deref();
    if let Some(active) = active {
        let exists = manifest
            .releases
            .iter()
            .any(|release| release.id.as_deref() == Some(active));
        report.checks.push(LifecycleCheck {
            id: "release_config.active_release_exists".to_string(),
            status: if exists { "passed" } else { "missing" }.to_string(),
            summary: "release.active_release points at a [[releases]] entry".to_string(),
            details: Some(active.to_string()),
            remediation: vec![
                "Add a matching [[releases]] entry or update release.active_release.".to_string(),
            ],
        });
    }

    for (index, release) in manifest.releases.iter().enumerate() {
        validate_release_entry(
            project_dir,
            root,
            release,
            index,
            provider,
            &mut report.checks,
        );
    }

    if let Some(provider) = provider {
        validate_provider_listing(provider, root, &mut report.checks);
    }

    finalize_status(&mut report);
    Ok(report)
}

fn validate_release_entry(
    project_dir: &Path,
    root: &ReleaseRoot,
    release: &ReleaseEntry,
    index: usize,
    provider: Option<DistributionProvider>,
    checks: &mut Vec<LifecycleCheck>,
) {
    let id = release
        .id
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| format!("entry-{index}"));
    checks.push(required_scalar_check(
        &format!("release_config.release.{id}.version"),
        release.version.as_deref(),
        "release version is set",
    ));
    checks.push(LifecycleCheck {
        id: format!("release_config.release.{id}.build"),
        status: if release.build.is_some() {
            "passed"
        } else {
            "missing"
        }
        .to_string(),
        summary: "release build number is set".to_string(),
        details: release.build.map(|build| build.to_string()),
        remediation: vec![
            "Set build to the monotonically increasing platform build number.".to_string(),
        ],
    });
    checks.push(required_scalar_check(
        &format!("release_config.release.{id}.status"),
        release.status.as_deref(),
        "release status is set",
    ));
    checks.push(list_check(
        &format!("release_config.release.{id}.tracks"),
        &release.tracks,
        "release has at least one target track/channel",
    ));

    let locales = if release.locales.is_empty() {
        &root.default_locales
    } else {
        &release.locales
    };
    checks.push(list_check(
        &format!("release_config.release.{id}.locales"),
        locales,
        "release resolves at least one locale",
    ));

    validate_release_path(
        project_dir,
        &id,
        "metadata",
        release.metadata.as_deref(),
        true,
        checks,
    );
    validate_release_path(
        project_dir,
        &id,
        "release_notes",
        release.release_notes.as_deref(),
        true,
        checks,
    );
    validate_release_path(
        project_dir,
        &id,
        "review",
        release.review.as_deref(),
        false,
        checks,
    );
    validate_release_path(
        project_dir,
        &id,
        "privacy",
        release.privacy.as_deref(),
        false,
        checks,
    );

    if let Some(notes_dir) = release.release_notes.as_deref() {
        for locale in locales {
            let path = project_dir.join(notes_dir).join(format!("{locale}.md"));
            checks.push(path_check(
                &format!("release_config.release.{id}.notes.{locale}"),
                path.clone(),
                "localized release notes exist",
            ));
            if path.exists() {
                validate_no_placeholder_text(
                    &format!("release_config.release.{id}.notes.{locale}.content"),
                    &path,
                    checks,
                );
            }
        }
    }

    if provider.is_some() {
        validate_tracks_match_provider(&id, provider, &release.tracks, checks);
    }
}

fn validate_release_path(
    project_dir: &Path,
    release_id: &str,
    kind: &str,
    relative: Option<&str>,
    required: bool,
    checks: &mut Vec<LifecycleCheck>,
) {
    let Some(relative) = relative.filter(|value| !value.trim().is_empty()) else {
        checks.push(LifecycleCheck {
            id: format!("release_config.release.{release_id}.{kind}"),
            status: if required { "missing" } else { "warning" }.to_string(),
            summary: format!("release {kind} path is configured"),
            details: None,
            remediation: vec![format!(
                "Set [[releases]].{kind} to a project-relative path."
            )],
        });
        return;
    };
    let path = project_dir.join(relative);
    checks.push(path_check(
        &format!("release_config.release.{release_id}.{kind}_exists"),
        path.clone(),
        &format!("release {kind} path exists"),
    ));
    checks.push(project_relative_path_check(
        &format!("release_config.release.{release_id}.{kind}_inside_project"),
        project_dir,
        &path,
    ));
    if path.is_file() {
        validate_no_placeholder_text(
            &format!("release_config.release.{release_id}.{kind}.content"),
            &path,
            checks,
        );
    }
}

fn validate_provider_listing(
    provider: DistributionProvider,
    root: &ReleaseRoot,
    checks: &mut Vec<LifecycleCheck>,
) {
    let key = match provider {
        DistributionProvider::PlayStore => "play_store",
        DistributionProvider::AppStore => "app_store",
        DistributionProvider::MicrosoftStore => "microsoft_store",
        _ => return,
    };
    let listings = root.store_listing.get(key);
    checks.push(LifecycleCheck {
        id: format!("release_config.{key}.store_listing_exists"),
        status: if listings.is_some() {
            "passed"
        } else {
            "missing"
        }
        .to_string(),
        summary: format!("release.store_listing.{key} has localized metadata"),
        details: listings.map(|listings| format!("{} locales", listings.len())),
        remediation: vec![format!(
            "Add [release.store_listing.{key}.<locale>] entries for every release locale."
        )],
    });
    if let Some(listings) = listings {
        for (locale, value) in listings {
            validate_listing_value(key, locale, value, checks);
        }
    }
}

fn validate_listing_value(
    provider_key: &str,
    locale: &str,
    value: &toml::Value,
    checks: &mut Vec<LifecycleCheck>,
) {
    let table = value.as_table();
    for field in [
        "title",
        "name",
        "short_description",
        "privacy_url",
        "support_url",
    ] {
        if table.and_then(|table| table.get(field)).is_some() {
            checks.push(ok_check(
                &format!("release_config.{provider_key}.{locale}.{field}"),
                format!("{field} configured"),
            ));
        }
    }
}

fn validate_tracks_match_provider(
    release_id: &str,
    provider: Option<DistributionProvider>,
    tracks: &[String],
    checks: &mut Vec<LifecycleCheck>,
) {
    let Some(provider) = provider else {
        return;
    };
    let prefix = provider.as_str();
    let matches = tracks.iter().any(|track| track.starts_with(prefix));
    checks.push(LifecycleCheck {
        id: format!("release_config.release.{release_id}.tracks_include_provider"),
        status: if matches { "passed" } else { "warning" }.to_string(),
        summary: "release tracks include the selected provider".to_string(),
        details: Some(format!(
            "provider = {prefix}, tracks = {}",
            tracks.join(",")
        )),
        remediation: vec![format!(
            "Add a track entry such as {prefix}:internal/testflight/public."
        )],
    });
}

fn validate_no_placeholder_text(id: &str, path: &Path, checks: &mut Vec<LifecycleCheck>) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let lowered = text.to_ascii_lowercase();
    let bad = ["todo", "tbd", "lorem ipsum", "placeholder"]
        .iter()
        .find(|needle| lowered.contains(**needle));
    checks.push(LifecycleCheck {
        id: id.to_string(),
        status: if bad.is_some() { "failed" } else { "passed" }.to_string(),
        summary: "release content has no placeholder text".to_string(),
        details: bad.map(|needle| format!("found {needle} in {}", path.display())),
        remediation: vec![
            "Replace placeholder release text with final store-ready content.".to_string(),
        ],
    });
}

fn project_relative_path_check(id: &str, project_dir: &Path, path: &Path) -> LifecycleCheck {
    let normalized = normalize_path(path);
    let normalized_project = normalize_path(project_dir);
    let inside = normalized.starts_with(&normalized_project);
    LifecycleCheck {
        id: id.to_string(),
        status: if inside { "passed" } else { "failed" }.to_string(),
        summary: "release content path stays inside the project".to_string(),
        details: Some(path.display().to_string()),
        remediation: vec!["Move release content under the project or explicitly configure an allowed workspace path once remote-builder support lands.".to_string()],
    }
}

fn required_scalar_check(id: &str, value: Option<&str>, summary: &str) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: if value.is_some_and(|value| !value.trim().is_empty()) {
            "passed"
        } else {
            "missing"
        }
        .to_string(),
        summary: summary.to_string(),
        details: value.map(str::to_string),
        remediation: vec!["Set the missing scalar field in fission.toml.".to_string()],
    }
}

fn list_check(id: &str, values: &[String], summary: &str) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: if values.is_empty() {
            "missing"
        } else {
            "passed"
        }
        .to_string(),
        summary: summary.to_string(),
        details: Some(values.join(",")),
        remediation: vec![
            "Add at least one value to the corresponding list in fission.toml.".to_string(),
        ],
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    path.components().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fission-release-model-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_release_project(dir: &Path, notes: &str) {
        fs::create_dir_all(dir.join("release-content/metadata/1.0.0+1/notes")).unwrap();
        fs::write(
            dir.join("release-content/metadata/1.0.0+1/release.toml"),
            "[play_store.en-US]\nfull_description = \"A focused release.\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("release-content/metadata/1.0.0+1/notes/en-US.md"),
            notes,
        )
        .unwrap();
        fs::write(
            dir.join("release-content/metadata/1.0.0+1/review.toml"),
            "notes = \"No login is required.\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("release-content/metadata/1.0.0+1/privacy.toml"),
            "privacy_url = \"https://example.com/privacy\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("fission.toml"),
            r#"[app]
name = "release-demo"
app_id = "com.example.release_demo"

[release]
active_release = "1.0.0+1"
metadata_root = "release-content/metadata"
content_output_dir = "release-content"
default_locales = ["en-US"]

[[releases]]
id = "1.0.0+1"
version = "1.0.0"
build = 1
status = "candidate"
tracks = ["play-store:internal"]
locales = ["en-US"]
metadata = "release-content/metadata/1.0.0+1/release.toml"
release_notes = "release-content/metadata/1.0.0+1/notes"
review = "release-content/metadata/1.0.0+1/review.toml"
privacy = "release-content/metadata/1.0.0+1/privacy.toml"

[release.store_listing.play_store.en-US]
title = "Release Demo"
short_description = "A focused release build."
privacy_url = "https://example.com/privacy"
"#,
        )
        .unwrap();
    }

    #[test]
    fn release_config_validation_accepts_complete_release_files() {
        let dir = unique_dir("valid");
        write_release_project(&dir, "A precise release note.\n");
        let report =
            validate_release_config_model(&dir, Some(DistributionProvider::PlayStore)).unwrap();
        assert_ne!(report.status, "blocked");
    }

    #[test]
    fn release_config_validation_rejects_placeholder_release_notes() {
        let dir = unique_dir("placeholder");
        write_release_project(&dir, "TODO fill this in.\n");
        let report =
            validate_release_config_model(&dir, Some(DistributionProvider::PlayStore)).unwrap();
        assert_eq!(report.status, "blocked");
        assert!(report
            .checks
            .iter()
            .any(|check| check.id.ends_with("notes.en-US.content") && check.status == "failed"));
    }
}
