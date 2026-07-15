use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct MacosPackageConfig {
    pub bundle_id: Option<String>,
    pub minimum_os: Option<String>,
    pub entitlements: Option<String>,
    pub signing_identity: Option<String>,
    pub installer_identity: Option<String>,
    pub notarize: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageManifest {
    package: Option<PackageRoot>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageRoot {
    macos: Option<MacosPackageConfig>,
}

pub fn read_macos_package_config(project_dir: &Path) -> Result<MacosPackageConfig> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: PackageManifest =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(manifest
        .package
        .and_then(|package| package.macos)
        .unwrap_or_default())
}

pub fn sign_macos_app_if_configured(
    project_dir: &Path,
    app_bundle: &Path,
    macos: &MacosPackageConfig,
) -> Result<()> {
    let Some(identity) = macos
        .signing_identity
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(());
    };

    let status = Command::new("codesign")
        .args(codesign_arguments(project_dir, identity, macos))
        .arg(app_bundle)
        .status()
        .context("failed to run codesign")?;
    if !status.success() {
        bail!("codesign failed with {status}");
    }

    let verify = Command::new("codesign")
        .args(["--verify", "--deep", "--strict", "--verbose=2"])
        .arg(app_bundle)
        .status()
        .context("failed to verify macOS code signature")?;
    if !verify.success() {
        bail!("codesign verification failed with {verify}");
    }
    Ok(())
}

fn codesign_arguments(
    project_dir: &Path,
    identity: &str,
    macos: &MacosPackageConfig,
) -> Vec<OsString> {
    let mut arguments = vec![
        "--force".into(),
        "--timestamp".into(),
        "--options".into(),
        "runtime".into(),
        "--sign".into(),
        identity.into(),
    ];
    if let Some(entitlements) = macos
        .entitlements
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        arguments.push("--entitlements".into());
        arguments.push(resolve_project_path(project_dir, entitlements).into_os_string());
    }
    arguments
}

fn resolve_project_path(project_dir: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_macos_package_signing_configuration() {
        let manifest: PackageManifest = toml::from_str(
            r#"
[package.macos]
bundle_id = "com.example.app"
minimum_os = "14.0"
entitlements = "platforms/macos/App.entitlements"
signing_identity = "Apple Development"
installer_identity = "Developer ID Installer"
notarize = true
"#,
        )
        .unwrap();

        assert_eq!(
            manifest.package.unwrap().macos.unwrap(),
            MacosPackageConfig {
                bundle_id: Some("com.example.app".into()),
                minimum_os: Some("14.0".into()),
                entitlements: Some("platforms/macos/App.entitlements".into()),
                signing_identity: Some("Apple Development".into()),
                installer_identity: Some("Developer ID Installer".into()),
                notarize: Some(true),
            }
        );
    }

    #[test]
    fn codesign_arguments_resolve_relative_entitlements() {
        let config = MacosPackageConfig {
            entitlements: Some("platforms/macos/App.entitlements".into()),
            ..Default::default()
        };

        assert_eq!(
            codesign_arguments(Path::new("/project"), "-", &config),
            vec![
                OsString::from("--force"),
                OsString::from("--timestamp"),
                OsString::from("--options"),
                OsString::from("runtime"),
                OsString::from("--sign"),
                OsString::from("-"),
                OsString::from("--entitlements"),
                OsString::from("/project/platforms/macos/App.entitlements"),
            ]
        );
    }
}
