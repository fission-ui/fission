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
    pub provisioning_profile: Option<String>,
    pub signing_identity: Option<String>,
    pub installer_identity: Option<String>,
    pub notarize: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageManifest {
    package: Option<PackageRoot>,
    run: Option<RunRoot>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageRoot {
    macos: Option<MacosPackageConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct RunRoot {
    macos: Option<MacosRunConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct MacosRunConfig {
    entitlements: Option<String>,
    provisioning_profile: Option<String>,
    signing_identity: Option<String>,
}

pub fn read_macos_package_config(project_dir: &Path) -> Result<MacosPackageConfig> {
    let manifest = read_manifest(project_dir)?;
    Ok(package_config(manifest.package))
}

pub fn read_macos_run_config(project_dir: &Path) -> Result<MacosPackageConfig> {
    Ok(run_config(read_manifest(project_dir)?))
}

fn run_config(manifest: PackageManifest) -> MacosPackageConfig {
    let run = manifest.run.and_then(|run| run.macos);
    let mut config = package_config(manifest.package);
    if let Some(run) = run {
        if run.entitlements.is_some() {
            config.entitlements = run.entitlements;
        }
        if run.provisioning_profile.is_some() {
            config.provisioning_profile = run.provisioning_profile;
        }
        if run.signing_identity.is_some() {
            config.signing_identity = run.signing_identity;
        }
    }
    config
}

fn read_manifest(project_dir: &Path) -> Result<PackageManifest> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn package_config(package: Option<PackageRoot>) -> MacosPackageConfig {
    package
        .and_then(|package| package.macos)
        .unwrap_or_default()
}

pub fn sign_macos_app_if_configured(
    project_dir: &Path,
    app_bundle: &Path,
    macos: &MacosPackageConfig,
) -> Result<()> {
    let identity = macos
        .signing_identity
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let profile = macos
        .provisioning_profile
        .as_deref()
        .filter(|value| !value.trim().is_empty());

    if profile.is_some() && identity.is_none() {
        bail!(
            "macOS provisioning_profile requires package.macos.signing_identity or run.macos.signing_identity"
        );
    }
    if let Some(profile) = profile {
        embed_macos_provisioning_profile(project_dir, app_bundle, profile)?;
    }

    let Some(identity) = identity else {
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

fn embed_macos_provisioning_profile(
    project_dir: &Path,
    app_bundle: &Path,
    profile: &str,
) -> Result<()> {
    let source = resolve_project_path(project_dir, profile);
    if !source.is_file() {
        bail!(
            "macOS provisioning profile does not exist or is not a file: {}",
            source.display()
        );
    }

    let destination = app_bundle.join("Contents/embedded.provisionprofile");
    fs::copy(&source, &destination).with_context(|| {
        format!(
            "failed to embed macOS provisioning profile {} at {}",
            source.display(),
            destination.display()
        )
    })?;
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
provisioning_profile = "profiles/Developer.provisionprofile"
signing_identity = "Apple Development"
installer_identity = "Developer ID Installer"
notarize = true

[run.macos]
entitlements = "platforms/macos/Development.entitlements"
provisioning_profile = "profiles/Developer-Local.provisionprofile"
signing_identity = "-"
"#,
        )
        .unwrap();

        assert_eq!(
            manifest.package.unwrap().macos.unwrap(),
            MacosPackageConfig {
                bundle_id: Some("com.example.app".into()),
                minimum_os: Some("14.0".into()),
                entitlements: Some("platforms/macos/App.entitlements".into()),
                provisioning_profile: Some("profiles/Developer.provisionprofile".into()),
                signing_identity: Some("Apple Development".into()),
                installer_identity: Some("Developer ID Installer".into()),
                notarize: Some(true),
            }
        );
        let run = manifest.run.unwrap().macos.unwrap();
        assert_eq!(
            run.entitlements.as_deref(),
            Some("platforms/macos/Development.entitlements")
        );
        assert_eq!(
            run.provisioning_profile.as_deref(),
            Some("profiles/Developer-Local.provisionprofile")
        );
        assert_eq!(run.signing_identity.as_deref(), Some("-"));
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

    #[test]
    fn run_signing_overrides_package_signing_only() {
        let manifest: PackageManifest = toml::from_str(
            r#"
[package.macos]
bundle_id = "com.example.app"
minimum_os = "14.0"
entitlements = "platforms/macos/Release.entitlements"
provisioning_profile = "profiles/Release.provisionprofile"
signing_identity = "Apple Development"

[run.macos]
entitlements = "platforms/macos/Development.entitlements"
provisioning_profile = "profiles/Development.provisionprofile"
signing_identity = "-"
"#,
        )
        .unwrap();
        let config = run_config(manifest);

        assert_eq!(config.bundle_id.as_deref(), Some("com.example.app"));
        assert_eq!(config.minimum_os.as_deref(), Some("14.0"));
        assert_eq!(
            config.entitlements.as_deref(),
            Some("platforms/macos/Development.entitlements")
        );
        assert_eq!(
            config.provisioning_profile.as_deref(),
            Some("profiles/Development.provisionprofile")
        );
        assert_eq!(config.signing_identity.as_deref(), Some("-"));
    }

    #[test]
    fn embeds_relative_macos_provisioning_profile() {
        let root =
            std::env::temp_dir().join(format!("fission-macos-profile-{}", std::process::id()));
        let project = root.join("project");
        let app = root.join("Demo.app");
        fs::remove_dir_all(&root).ok();
        fs::create_dir_all(project.join("profiles")).unwrap();
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(
            project.join("profiles/Development.provisionprofile"),
            b"profile-data",
        )
        .unwrap();

        embed_macos_provisioning_profile(&project, &app, "profiles/Development.provisionprofile")
            .unwrap();

        assert_eq!(
            fs::read(app.join("Contents/embedded.provisionprofile")).unwrap(),
            b"profile-data"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn provisioning_profile_requires_signing_identity() {
        let config = MacosPackageConfig {
            provisioning_profile: Some("profiles/Development.provisionprofile".into()),
            ..Default::default()
        };

        let error = sign_macos_app_if_configured(
            Path::new("/project"),
            Path::new("/project/Demo.app"),
            &config,
        )
        .unwrap_err();

        assert!(error.to_string().contains("requires"));
    }
}
