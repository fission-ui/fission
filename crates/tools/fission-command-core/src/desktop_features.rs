use crate::Target;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
struct PackageManifest {
    package: Option<PackageRoot>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageRoot {
    linux: Option<DesktopPackageConfig>,
    macos: Option<DesktopPackageConfig>,
    windows: Option<DesktopPackageConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct DesktopPackageConfig {
    #[serde(default)]
    cargo_features: Vec<String>,
    #[serde(default)]
    cargo_no_default_features: bool,
    #[serde(default)]
    variants: BTreeMap<String, DesktopPackageVariant>,
}

#[derive(Debug, Default, Deserialize)]
struct DesktopPackageVariant {
    cargo_features: Option<Vec<String>>,
    cargo_no_default_features: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DesktopCargoOptions {
    pub features: Vec<String>,
    pub no_default_features: bool,
}

pub fn read_desktop_cargo_options(
    project_dir: &Path,
    target: Target,
    variant: Option<&str>,
) -> Result<DesktopCargoOptions> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: PackageManifest =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    let config = match target {
        Target::Linux => manifest.package.and_then(|package| package.linux),
        Target::Macos => manifest.package.and_then(|package| package.macos),
        Target::Windows => manifest.package.and_then(|package| package.windows),
        _ => None,
    };
    let Some(config) = config else {
        return Ok(DesktopCargoOptions::default());
    };

    let selected_variant = variant.and_then(|variant| config.variants.get(variant));
    let configured_features = selected_variant
        .and_then(|variant| variant.cargo_features.as_ref())
        .unwrap_or(&config.cargo_features);
    let mut features = BTreeSet::new();
    for feature in configured_features {
        validate_cargo_feature(feature)?;
        features.insert(feature.clone());
    }
    Ok(DesktopCargoOptions {
        features: features.into_iter().collect(),
        no_default_features: selected_variant
            .and_then(|variant| variant.cargo_no_default_features)
            .unwrap_or(config.cargo_no_default_features),
    })
}

fn validate_cargo_feature(feature: &str) -> Result<()> {
    let valid = !feature.is_empty()
        && feature.len() <= 128
        && !feature.starts_with('-')
        && !feature.contains(',')
        && feature
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b'=' | b'\\'));
    if !valid {
        bail!("desktop cargo feature `{feature}` is not a safe Cargo feature selector");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_variant_replaces_base_features_and_default_policy() {
        let root =
            std::env::temp_dir().join(format!("fission-desktop-features-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("fission.toml"),
            r#"
[package.macos]
cargo_features = ["base-only"]

[package.macos.variants.app-store]
cargo_features = ["macos-app-store", "shared", "shared"]
cargo_no_default_features = true

[package.windows.variants.store]
cargo_features = ["windows-store"]
"#,
        )
        .unwrap();

        assert_eq!(
            read_desktop_cargo_options(&root, Target::Macos, Some("app-store")).unwrap(),
            DesktopCargoOptions {
                features: vec!["macos-app-store".to_string(), "shared".to_string()],
                no_default_features: true,
            }
        );
        assert_eq!(
            read_desktop_cargo_options(&root, Target::Macos, None).unwrap(),
            DesktopCargoOptions {
                features: vec!["base-only".to_string()],
                no_default_features: false,
            }
        );
        assert_eq!(
            read_desktop_cargo_options(&root, Target::Windows, Some("store")).unwrap(),
            DesktopCargoOptions {
                features: vec!["windows-store".to_string()],
                no_default_features: false,
            }
        );
        assert!(
            read_desktop_cargo_options(&root, Target::Linux, Some("app-store"))
                .unwrap()
                .features
                .is_empty()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unsafe_feature_selectors_are_rejected() {
        let root = std::env::temp_dir().join(format!(
            "fission-desktop-features-invalid-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("fission.toml"),
            r#"
[package.macos.variants.store]
cargo_features = ["safe,unsafe"]
"#,
        )
        .unwrap();

        assert!(
            read_desktop_cargo_options(&root, Target::Macos, Some("store"))
                .unwrap_err()
                .to_string()
                .contains("safe Cargo feature selector")
        );
        let _ = fs::remove_dir_all(&root);
    }
}
