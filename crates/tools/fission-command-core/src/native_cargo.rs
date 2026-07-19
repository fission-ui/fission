use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const CARGO_TARGET_DIRECTORY_TOKEN: &str = "{cargo_target_dir}";

#[derive(Deserialize)]
struct CargoMetadata {
    target_directory: PathBuf,
}

pub(crate) fn cargo_target_directory(
    project_dir: &Path,
    manifest: &Path,
    module_name: &str,
    platform: &str,
) -> Result<PathBuf> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .arg("--manifest-path")
        .arg(manifest)
        .current_dir(project_dir)
        .output()
        .with_context(|| {
            format!(
                "failed to resolve the Cargo target directory for {platform} native module `{module_name}`"
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        bail!(
            "Cargo metadata failed for {platform} native module `{module_name}` with {}{}",
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        );
    }
    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout).with_context(|| {
        format!("Cargo returned invalid metadata for {platform} native module `{module_name}`")
    })?;
    if !metadata.target_directory.is_absolute() {
        bail!(
            "Cargo returned a non-absolute target directory for {platform} native module `{module_name}`: {}",
            metadata.target_directory.display()
        );
    }
    Ok(metadata.target_directory)
}

pub(crate) fn expand_cargo_target_directory(
    value: &str,
    target_directory: Option<&Path>,
    module_name: &str,
    platform: &str,
) -> Result<String> {
    if !value.contains(CARGO_TARGET_DIRECTORY_TOKEN) {
        return Ok(value.to_owned());
    }
    let target_directory = target_directory.with_context(|| {
        format!(
            "{platform} native module `{module_name}` uses {CARGO_TARGET_DIRECTORY_TOKEN} for a product that is not built by Cargo"
        )
    })?;
    Ok(value.replace(
        CARGO_TARGET_DIRECTORY_TOKEN,
        &target_directory.to_string_lossy(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_the_configured_cargo_target_directory() {
        let expanded = expand_cargo_target_directory(
            "{cargo_target_dir}/release/demo-helper",
            Some(Path::new("/shared/cargo")),
            "demo",
            "Linux",
        )
        .unwrap();

        assert_eq!(expanded, "/shared/cargo/release/demo-helper");
    }

    #[test]
    fn rejects_the_token_for_non_cargo_products() {
        let error = expand_cargo_target_directory(
            "{cargo_target_dir}/release/demo-helper",
            None,
            "demo",
            "Windows",
        )
        .unwrap_err();

        assert!(error.to_string().contains("not built by Cargo"));
    }
}
