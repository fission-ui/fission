//! Derive local framework inputs from Cargo's resolved graph, not a crate-name list.
use super::{hash_directory, PreviewCacheHasher};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn contexts(manifest: &Path) -> Result<BTreeMap<String, String>> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--filter-platform",
            "wasm32-unknown-unknown",
            "--manifest-path",
        ])
        .arg(manifest)
        .stderr(Stdio::inherit())
        .output()
        .context("resolve tutorial dependencies for cache validation")?;
    if !output.status.success() {
        bail!("Cargo metadata failed for {}", manifest.display());
    }
    fingerprints(&serde_json::from_slice(&output.stdout)?)
}

fn fingerprints(metadata: &Value) -> Result<BTreeMap<String, String>> {
    let members = metadata["workspace_members"]
        .as_array()
        .context("missing workspace members")?
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let mut inputs = BTreeMap::new();
    // Versions, sources and resolved features of registry/git crates are inputs,
    // as are every file of local dependencies (including build scripts/assets).
    let packages = metadata["packages"]
        .as_array()
        .context("missing dependency packages")?;
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .context("missing resolved graph")?;
    for node in nodes {
        let mut hasher = PreviewCacheHasher::default();
        let id = node["id"].as_str().context("missing dependency identity")?;
        if members.contains(id) {
            continue;
        }
        hasher.field(id.as_bytes());
        hasher.field(&serde_json::to_vec(&node["features"])?);
        let package = packages
            .iter()
            .find(|package| package["id"].as_str() == Some(id))
            .context("resolved dependency is missing its package")?;
        hasher.field(&serde_json::to_vec(&package["source"])?);
        if package["source"].is_null() {
            let manifest = Path::new(
                package["manifest_path"]
                    .as_str()
                    .context("missing dependency manifest")?,
            );
            let directory = manifest
                .parent()
                .context("dependency manifest has no parent")?;
            hash_directory(&mut hasher, directory, directory)?;
        }
        inputs.insert(id, format!("{:016x}", hasher.finish()));
    }
    let mut result = BTreeMap::new();
    for member in members {
        let mut reached = BTreeSet::new();
        let mut pending = vec![member];
        while let Some(id) = pending.pop() {
            if !reached.insert(id) {
                continue;
            }
            let node = nodes
                .iter()
                .find(|node| node["id"].as_str() == Some(id))
                .context("missing dependency node")?;
            if let Some(deps) = node["deps"].as_array() {
                for dependency in deps {
                    pending.push(
                        dependency["pkg"]
                            .as_str()
                            .context("missing dependency edge")?,
                    );
                }
            }
        }
        let mut hasher = PreviewCacheHasher::default();
        for id in reached {
            if let Some(input) = inputs.get(id) {
                hasher.field(input.as_bytes());
            }
        }
        let package = packages
            .iter()
            .find(|package| package["id"].as_str() == Some(member))
            .context("missing tutorial package")?;
        result.insert(
            package["name"]
                .as_str()
                .context("missing tutorial name")?
                .to_owned(),
            format!("{:016x}", hasher.finish()),
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn member_identity_does_not_invalidate_dependency_context_but_features_do() {
        let mut metadata = serde_json::json!({
            "workspace_members": ["lesson"],
            "packages": [{"id":"serde", "source":"registry+test"}, {"id":"lesson", "name":"example"}],
            "resolve":{"nodes":[{"id":"lesson", "features":[], "deps":[{"pkg":"serde"}]}, {"id":"serde", "features":["derive"]}]}
        });
        let original = fingerprints(&metadata).unwrap();
        metadata["workspace_members"][0] = "another-lesson".into();
        metadata["resolve"]["nodes"][0]["id"] = "another-lesson".into();
        metadata["packages"][1]["id"] = "another-lesson".into();
        assert_eq!(original, fingerprints(&metadata).unwrap());
        metadata["resolve"]["nodes"][1]["features"] = serde_json::json!(["derive", "rc"]);
        assert_ne!(original, fingerprints(&metadata).unwrap());
        let before = fingerprints(&metadata).unwrap()["example"].clone();
        metadata["workspace_members"]
            .as_array_mut()
            .unwrap()
            .push("unrelated".into());
        metadata["packages"].as_array_mut().unwrap().extend([
            serde_json::json!({"id":"unrelated", "name":"unrelated"}),
            serde_json::json!({"id":"other-dep", "source":"registry+test"}),
        ]);
        metadata["resolve"]["nodes"]
            .as_array_mut()
            .unwrap()
            .extend([
                serde_json::json!({"id":"unrelated", "deps":[{"pkg":"other-dep"}]}),
                serde_json::json!({"id":"other-dep", "features":[]}),
            ]);
        assert_eq!(before, fingerprints(&metadata).unwrap()["example"]);
    }
}
