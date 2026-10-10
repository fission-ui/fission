use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    ResolvedVersionMatch,
    VersionMismatch,
    Unresolved,
    LocalPathUnverified,
    GitUnverified,
    Unparseable,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkDependency {
    pub compatibility: Compatibility,
    pub source: String,
    pub requested_version: Option<String>,
    pub resolved_version: Option<String>,
    pub message: String,
}

fn result(
    compatibility: Compatibility,
    source: &str,
    requested: Option<String>,
    resolved: Option<String>,
    message: &str,
) -> FrameworkDependency {
    FrameworkDependency {
        compatibility,
        source: source.into(),
        requested_version: requested,
        resolved_version: resolved,
        message: message.into(),
    }
}

pub(super) fn inspect(project: &Path) -> FrameworkDependency {
    let doc = match fs::read_to_string(project.join("Cargo.toml"))
        .ok()
        .and_then(|s| s.parse::<toml::Value>().ok())
    {
        Some(doc) => doc,
        None => {
            return result(
                Compatibility::Unparseable,
                "unknown",
                None,
                None,
                "Cannot read/parse project Cargo.toml; no compatibility claim.",
            )
        }
    };
    let dependency = doc
        .get("dependencies")
        .and_then(|d| d.as_table())
        .and_then(|d| {
            d.get("fission").or_else(|| {
                d.values()
                    .find(|v| v.get("package").and_then(|p| p.as_str()) == Some("fission"))
            })
        });
    let Some(dep) = dependency else {
        return result(Compatibility::Missing, "unknown", None, None, "No direct Fission dependency in [dependencies]; target-specific/indirect dependencies require manual inspection.");
    };
    let requested = dep
        .as_str()
        .or_else(|| dep.get("version").and_then(|v| v.as_str()))
        .map(str::to_owned);
    if dep.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
        return result(
            Compatibility::Unresolved,
            "workspace",
            requested,
            None,
            "Workspace-inherited dependency requires manual source/lockfile inspection.",
        );
    }
    if let Some(path) = dep.get("path").and_then(|v| v.as_str()) {
        let declared = fs::read_to_string(project.join(path).join("Cargo.toml"))
            .ok()
            .and_then(|s| s.parse::<toml::Value>().ok())
            .and_then(|d| {
                d.get("package")?
                    .get("version")?
                    .as_str()
                    .map(str::to_owned)
            });
        return result(Compatibility::LocalPathUnverified, "path", requested, declared, "Local package version is descriptive, not proof of matching APIs/revision; inspect checkout source. No network lookup.");
    }
    if dep.get("git").is_some() {
        return result(
            Compatibility::GitUnverified,
            "git",
            requested,
            None,
            "Git revisions are not verified against CLI assets; inspect the pinned source.",
        );
    }
    let requirement = match requested
        .as_deref()
        .and_then(|s| semver::VersionReq::parse(s).ok())
    {
        Some(req) => req,
        None => {
            return result(
                Compatibility::Unparseable,
                "registry",
                requested,
                None,
                "Missing/unparseable version requirement; no compatibility claim.",
            )
        }
    };
    // Overrides can use the same version while containing different APIs.
    let workspace = project.ancestors().find(|p| {
        fs::read_to_string(p.join("Cargo.toml"))
            .ok()
            .and_then(|s| s.parse::<toml::Value>().ok())
            .is_some_and(|d| d.get("workspace").is_some())
    });
    if doc.get("patch").is_some()
        || doc.get("replace").is_some()
        || workspace.is_some_and(|p| {
            fs::read_to_string(p.join("Cargo.toml"))
                .ok()
                .and_then(|s| s.parse::<toml::Value>().ok())
                .is_some_and(|d| d.get("patch").is_some() || d.get("replace").is_some())
        })
    {
        return result(Compatibility::Unresolved, "overridden", requested, None, "Cargo patch/replace may change dependency source; inspect source and lockfile explicitly.");
    }
    let api = semver::Version::parse(super::FRAMEWORK_API_VERSION).expect("bundled API version");
    if !requirement.matches(&api) {
        return result(Compatibility::VersionMismatch, "registry", requested, None,
            "The app's declared requirement excludes the bundled framework API. Use a matching CLI; updating guidance cannot change the app dependency.");
    }
    let local_lock = project.join("Cargo.lock");
    let lock = if local_lock.is_file() {
        Some(local_lock)
    } else {
        workspace
            .map(|p| p.join("Cargo.lock"))
            .filter(|p| p.is_file())
    };
    let packages = lock
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| s.parse::<toml::Value>().ok())
        .and_then(|d| d.get("package").and_then(|p| p.as_array()).cloned())
        .unwrap_or_default();
    let named = packages
        .iter()
        .filter(|p| p.get("name").and_then(|v| v.as_str()) == Some("fission"))
        .collect::<Vec<_>>();
    let mut matches = named
        .iter()
        .copied()
        .filter(|p| {
            p.get("version")
                .and_then(|v| v.as_str())
                .and_then(|v| semver::Version::parse(v).ok())
                .is_some_and(|version| requirement.matches(&version))
        })
        .collect::<Vec<_>>();
    // Preserve useful mismatch evidence for a sole locked package. A monorepo's
    // unrelated Fission versions must not make this app's exact pin ambiguous.
    if matches.is_empty() && named.len() == 1 {
        matches = named;
    }
    if matches.len() != 1 {
        return result(Compatibility::Unresolved, "registry", requested, None, "No unambiguous locked Fission package; requested version alone is not resolved compatibility evidence.");
    }
    let locked = matches[0];
    let resolved = locked
        .get("version")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let Some(version) = resolved
        .as_deref()
        .and_then(|s| semver::Version::parse(s).ok())
    else {
        return result(
            Compatibility::Unparseable,
            "registry",
            requested,
            resolved,
            "Unparseable locked version.",
        );
    };
    // Do not call arbitrary alternate registries equivalent to the published framework.
    let source = locked.get("source").and_then(|v| v.as_str());
    if source != Some("registry+https://github.com/rust-lang/crates.io-index") {
        return result(
            Compatibility::Unresolved,
            source.unwrap_or("path/unknown"),
            requested,
            resolved,
            "Locked source is not the default published registry; inspect its APIs manually.",
        );
    }
    let compatible = requirement.matches(&version) && version == api;
    result(
        if compatible {
            Compatibility::ResolvedVersionMatch
        } else {
            Compatibility::VersionMismatch
        },
        "registry",
        requested,
        resolved,
        if compatible {
            "Locked registry version matches the CLI's framework API version; this is version evidence, not a build/runtime guarantee."
        } else {
            "Locked version or requirement differs from bundled framework APIs. Use a matching CLI/framework revision; updating assets alone does not fix this."
        },
    )
}
