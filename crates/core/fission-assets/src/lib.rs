//! Asset provenance contracts shared by Fission importers and release tooling.
//!
//! This crate deliberately contains no media and performs no network access.
//! Applications retain the original licence evidence and describe it here so
//! build tooling can reject unattributed or unlicensed assets before release.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Stable, application-owned identity for one source asset.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetId(pub String);

impl AssetId {
    /// Creates an asset identity. IDs should describe content responsibility,
    /// not a mutable display name or its position in a directory.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// Broad media category used by diagnostics and release inventories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetKind {
    Image,
    Sprite,
    Model,
    Material,
    Texture,
    Animation,
    Font,
    Music,
    Sound,
    Other,
}

/// Whether the original media may be distributed independently of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedistributionPolicy {
    /// The licence permits redistribution of the original asset.
    Permitted,
    /// The asset may be shipped as part of a game but not as a raw asset pack.
    IntegratedProductOnly,
    /// The evidence does not establish redistribution rights.
    Unknown,
}

/// Licence evidence captured when an asset is acquired.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetLicense {
    /// SPDX expression when one exists, otherwise a stable provider licence ID.
    pub id: String,
    /// URL or repository path containing the complete applicable licence text.
    pub evidence: String,
    /// Whether a credit is required when the asset is distributed.
    #[serde(default)]
    pub attribution_required: bool,
    /// Rules for redistribution of the original media.
    pub redistribution: RedistributionPolicy,
}

impl AssetLicense {
    /// Creates a CC0 licence record suitable for public-domain asset libraries.
    pub fn cc0(evidence: impl Into<String>) -> Self {
        Self {
            id: "CC0-1.0".into(),
            evidence: evidence.into(),
            attribution_required: false,
            redistribution: RedistributionPolicy::Permitted,
        }
    }

    /// Creates a Creative Commons Attribution licence record.
    pub fn cc_by(version: &str, evidence: impl Into<String>) -> Self {
        Self {
            id: format!("CC-BY-{version}"),
            evidence: evidence.into(),
            attribution_required: true,
            redistribution: RedistributionPolicy::Permitted,
        }
    }
}

/// Auditable origin and transformation history for one asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetProvenance {
    pub id: AssetId,
    pub kind: AssetKind,
    /// Original download page or source repository, not a search-result URL.
    pub source: String,
    pub creator: String,
    /// ISO 8601 calendar date on which the asset and licence were obtained.
    pub acquired_on: String,
    /// Lowercase SHA-256 of the original downloaded file or archive.
    pub source_sha256: String,
    pub license: AssetLicense,
    /// Complete credit line when attribution is required.
    #[serde(default)]
    pub attribution: Option<String>,
    /// Ordered descriptions of material modifications made after acquisition.
    #[serde(default)]
    pub modifications: Vec<String>,
}

/// Complete provenance inventory for a compiled asset collection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetManifest {
    #[serde(default)]
    pub assets: Vec<AssetProvenance>,
}

/// A release-blocking defect in asset provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetManifestError {
    pub asset: Option<AssetId>,
    pub message: String,
}

impl AssetManifest {
    /// Validates evidence required for commercial packaging.
    ///
    /// Validation accumulates all defects so import tooling can show one useful
    /// report rather than requiring repeated build attempts.
    pub fn validate(&self) -> Result<(), Vec<AssetManifestError>> {
        let mut errors = Vec::new();
        let mut ids = BTreeSet::new();

        for asset in &self.assets {
            let id = Some(asset.id.clone());
            if asset.id.0.trim().is_empty() {
                errors.push(error(id.clone(), "asset id is empty"));
            } else if !ids.insert(asset.id.clone()) {
                errors.push(error(id.clone(), "asset id is duplicated"));
            }
            if asset.source.trim().is_empty() {
                errors.push(error(id.clone(), "source is empty"));
            }
            if asset.creator.trim().is_empty() {
                errors.push(error(id.clone(), "creator is empty"));
            }
            if asset.acquired_on.trim().is_empty() {
                errors.push(error(id.clone(), "acquisition date is empty"));
            }
            if !is_sha256(&asset.source_sha256) {
                errors.push(error(
                    id.clone(),
                    "source_sha256 must be 64 lowercase hex digits",
                ));
            }
            if asset.license.id.trim().is_empty() || asset.license.evidence.trim().is_empty() {
                errors.push(error(id.clone(), "licence id and evidence are required"));
            }
            if asset.license.attribution_required
                && asset.attribution.as_deref().is_none_or(str::is_empty)
            {
                errors.push(error(id, "required attribution is missing"));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Returns stable, deduplicated credit lines for an in-game credits screen.
    pub fn attribution_lines(&self) -> Vec<&str> {
        let mut lines = BTreeSet::new();
        for asset in &self.assets {
            if let Some(line) = asset.attribution.as_deref().filter(|line| !line.is_empty()) {
                lines.insert(line);
            }
        }
        lines.into_iter().collect()
    }
}

fn error(asset: Option<AssetId>, message: &str) -> AssetManifestError {
    AssetManifestError {
        asset,
        message: message.into(),
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(id: &str) -> AssetProvenance {
        AssetProvenance {
            id: AssetId::new(id),
            kind: AssetKind::Sprite,
            source: "https://example.invalid/source".into(),
            creator: "Example Artist".into(),
            acquired_on: "2026-09-04".into(),
            source_sha256: "a".repeat(64),
            license: AssetLicense::cc0("https://example.invalid/license"),
            attribution: None,
            modifications: Vec::new(),
        }
    }

    #[test]
    fn valid_manifest_generates_deduplicated_sorted_credits() {
        let mut first = asset("player");
        first.license = AssetLicense::cc_by("4.0", "licence.txt");
        first.attribution = Some("Art by Example".into());
        let mut second = asset("boat");
        second.attribution = Some("Art by Example".into());
        let manifest = AssetManifest {
            assets: vec![first, second],
        };

        assert_eq!(manifest.validate(), Ok(()));
        assert_eq!(manifest.attribution_lines(), vec!["Art by Example"]);
    }

    #[test]
    fn validation_reports_all_release_blocking_evidence_gaps() {
        let mut invalid = asset("duplicate");
        invalid.source_sha256 = "not-a-hash".into();
        invalid.license = AssetLicense::cc_by("3.0", "");
        let manifest = AssetManifest {
            assets: vec![invalid, asset("duplicate")],
        };

        let errors = manifest.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("source_sha256")));
        assert!(errors.iter().any(|error| error.message.contains("licence")));
        assert!(errors
            .iter()
            .any(|error| error.message.contains("attribution")));
        assert!(errors
            .iter()
            .any(|error| error.message.contains("duplicated")));
    }
}
