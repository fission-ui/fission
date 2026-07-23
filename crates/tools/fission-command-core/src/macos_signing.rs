use crate::NativeVariant;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
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
    pub pkg_builder: Option<String>,
    #[serde(default)]
    pub cargo_features: Vec<String>,
    #[serde(default)]
    pub cargo_no_default_features: bool,
}

#[derive(Debug, Default, Deserialize)]
struct PackageManifest {
    package: Option<PackageRoot>,
    run: Option<RunRoot>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageRoot {
    macos: Option<MacosPackageManifest>,
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

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
struct MacosPackageManifest {
    #[serde(flatten)]
    base: MacosPackageConfig,
    release: Option<MacosPackageOverlay>,
    #[serde(default)]
    variants: BTreeMap<NativeVariant, MacosPackageOverlay>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
struct MacosPackageOverlay {
    entitlements: Option<String>,
    provisioning_profile: Option<String>,
    signing_identity: Option<String>,
    installer_identity: Option<String>,
    notarize: Option<bool>,
    pkg_builder: Option<String>,
    cargo_features: Option<Vec<String>>,
    cargo_no_default_features: Option<bool>,
}

impl MacosPackageManifest {
    fn effective(&self, release: bool, variant: Option<&NativeVariant>) -> MacosPackageConfig {
        let mut config = self.base.clone();
        if release {
            if let Some(overlay) = &self.release {
                overlay.apply_to(&mut config);
            }
        }
        if let Some(overlay) = variant.and_then(|variant| self.variants.get(variant)) {
            overlay.apply_to(&mut config);
        }
        config
    }
}

impl MacosPackageOverlay {
    fn apply_to(&self, config: &mut MacosPackageConfig) {
        if self.entitlements.is_some() {
            config.entitlements.clone_from(&self.entitlements);
        }
        if self.provisioning_profile.is_some() {
            config
                .provisioning_profile
                .clone_from(&self.provisioning_profile);
        }
        if self.signing_identity.is_some() {
            config.signing_identity.clone_from(&self.signing_identity);
        }
        if self.installer_identity.is_some() {
            config
                .installer_identity
                .clone_from(&self.installer_identity);
        }
        if self.notarize.is_some() {
            config.notarize = self.notarize;
        }
        if self.pkg_builder.is_some() {
            config.pkg_builder.clone_from(&self.pkg_builder);
        }
        if let Some(features) = &self.cargo_features {
            config.cargo_features.clone_from(features);
        }
        if let Some(no_default_features) = self.cargo_no_default_features {
            config.cargo_no_default_features = no_default_features;
        }
    }
}

pub fn read_macos_package_config(project_dir: &Path) -> Result<MacosPackageConfig> {
    read_macos_package_config_for_profile(project_dir, false)
}

pub fn read_macos_package_config_for_profile(
    project_dir: &Path,
    release: bool,
) -> Result<MacosPackageConfig> {
    read_macos_package_config_for_profile_and_variant(project_dir, release, None)
}

pub fn read_macos_package_config_for_profile_and_variant(
    project_dir: &Path,
    release: bool,
    variant: Option<&NativeVariant>,
) -> Result<MacosPackageConfig> {
    let manifest = read_manifest(project_dir)?;
    Ok(package_config(manifest.package.as_ref(), release, variant))
}

pub fn read_macos_run_config(project_dir: &Path) -> Result<MacosPackageConfig> {
    read_macos_run_config_for_profile(project_dir, false)
}

pub fn read_macos_run_config_for_profile(
    project_dir: &Path,
    release: bool,
) -> Result<MacosPackageConfig> {
    Ok(run_config(&read_manifest(project_dir)?, release))
}

fn run_config(manifest: &PackageManifest, release: bool) -> MacosPackageConfig {
    let run = manifest.run.as_ref().and_then(|run| run.macos.as_ref());
    let mut config = package_config(manifest.package.as_ref(), release, None);
    if let Some(run) = run {
        if run.entitlements.is_some() {
            config.entitlements.clone_from(&run.entitlements);
        }
        if run.provisioning_profile.is_some() {
            config
                .provisioning_profile
                .clone_from(&run.provisioning_profile);
        }
        if run.signing_identity.is_some() {
            config.signing_identity.clone_from(&run.signing_identity);
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

fn package_config(
    package: Option<&PackageRoot>,
    release: bool,
    variant: Option<&NativeVariant>,
) -> MacosPackageConfig {
    package
        .and_then(|package| package.macos.as_ref())
        .map(|macos| macos.effective(release, variant))
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

    if profile.is_some() && identity.is_none_or(|value| value == "-") {
        bail!(
            "macOS provisioning_profile requires a real effective package.macos signing identity or run.macos.signing_identity; ad-hoc signing with `-` cannot embed a provisioning profile"
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
            package_config(manifest.package.as_ref(), false, None),
            MacosPackageConfig {
                bundle_id: Some("com.example.app".into()),
                minimum_os: Some("14.0".into()),
                entitlements: Some("platforms/macos/App.entitlements".into()),
                provisioning_profile: Some("profiles/Developer.provisionprofile".into()),
                signing_identity: Some("Apple Development".into()),
                installer_identity: Some("Developer ID Installer".into()),
                notarize: Some(true),
                ..Default::default()
            }
        );
        let run = manifest.run.as_ref().unwrap().macos.as_ref().unwrap();
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
    fn release_signing_overlay_is_ignored_for_debug_packages() {
        let manifest: PackageManifest = toml::from_str(
            r#"
[package.macos]
bundle_id = "com.example.app"
minimum_os = "14.0"
entitlements = "platforms/macos/Development.entitlements"
signing_identity = "-"

[package.macos.release]
entitlements = "platforms/macos/Release.entitlements"
provisioning_profile = "profiles/Distribution.provisionprofile"
signing_identity = "Developer ID Application: Example Ltd"
installer_identity = "Developer ID Installer: Example Ltd"
notarize = true
"#,
        )
        .unwrap();

        let debug = package_config(manifest.package.as_ref(), false, None);
        assert_eq!(
            debug.entitlements.as_deref(),
            Some("platforms/macos/Development.entitlements")
        );
        assert_eq!(debug.signing_identity.as_deref(), Some("-"));
        assert_eq!(debug.provisioning_profile, None);
        assert_eq!(debug.installer_identity, None);
        assert_eq!(debug.notarize, None);

        let release = package_config(manifest.package.as_ref(), true, None);
        assert_eq!(
            release.entitlements.as_deref(),
            Some("platforms/macos/Release.entitlements")
        );
        assert_eq!(
            release.provisioning_profile.as_deref(),
            Some("profiles/Distribution.provisionprofile")
        );
        assert_eq!(
            release.signing_identity.as_deref(),
            Some("Developer ID Application: Example Ltd")
        );
        assert_eq!(
            release.installer_identity.as_deref(),
            Some("Developer ID Installer: Example Ltd")
        );
        assert_eq!(release.notarize, Some(true));

        let release_run = run_config(&manifest, true);
        assert_eq!(
            release_run.signing_identity.as_deref(),
            Some("Developer ID Application: Example Ltd")
        );
    }

    #[test]
    fn selected_variant_overrides_effective_release_signing() {
        let manifest: PackageManifest = toml::from_str(
            r#"
[package.macos]
bundle_id = "com.example.app"
signing_identity = "-"

[package.macos.release]
entitlements = "platforms/macos/DeveloperId.entitlements"
provisioning_profile = "profiles/DeveloperId.provisionprofile"
signing_identity = "Developer ID Application: Example Ltd"
installer_identity = "Developer ID Installer: Example Ltd"
notarize = true

[package.macos.variants.app-store]
entitlements = "platforms/macos/AppStore.entitlements"
provisioning_profile = "profiles/AppStore.provisionprofile"
signing_identity = "Apple Distribution: Example Ltd"
installer_identity = "3rd Party Mac Developer Installer: Example Ltd"
notarize = false
pkg_builder = "productbuild"
cargo_features = ["macos-app-store"]
cargo_no_default_features = true
"#,
        )
        .unwrap();
        let variant: NativeVariant = "app-store".parse().unwrap();

        let config = package_config(manifest.package.as_ref(), true, Some(&variant));

        assert_eq!(
            config.entitlements.as_deref(),
            Some("platforms/macos/AppStore.entitlements")
        );
        assert_eq!(
            config.provisioning_profile.as_deref(),
            Some("profiles/AppStore.provisionprofile")
        );
        assert_eq!(
            config.signing_identity.as_deref(),
            Some("Apple Distribution: Example Ltd")
        );
        assert_eq!(
            config.installer_identity.as_deref(),
            Some("3rd Party Mac Developer Installer: Example Ltd")
        );
        assert_eq!(config.notarize, Some(false));
        assert_eq!(config.pkg_builder.as_deref(), Some("productbuild"));
        assert_eq!(config.cargo_features, ["macos-app-store"]);
        assert!(config.cargo_no_default_features);
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
        let config = run_config(&manifest, false);

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
    fn provisioning_profile_rejects_ad_hoc_signing_identity() {
        let config = MacosPackageConfig {
            provisioning_profile: Some("profiles/Development.provisionprofile".into()),
            signing_identity: Some("-".into()),
            ..Default::default()
        };

        let error = sign_macos_app_if_configured(
            Path::new("/project"),
            Path::new("/project/Demo.app"),
            &config,
        )
        .unwrap_err();

        assert!(error.to_string().contains("ad-hoc signing"));
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
