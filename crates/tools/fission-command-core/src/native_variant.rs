use crate::{FissionProject, NativeModuleConfig, Target};
use anyhow::{bail, Result};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct NativeVariant(String);

impl NativeVariant {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for NativeVariant {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for NativeVariant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NativeVariant {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let valid = !value.is_empty()
            && value.len() <= 64
            && value.split('-').all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            });
        if !valid {
            return Err(
                "variant must be 1-64 lowercase ASCII letters or digits separated by single hyphens"
                    .to_string(),
            );
        }
        Ok(Self(value.to_string()))
    }
}

impl<'de> Deserialize<'de> for NativeVariant {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl NativeModuleConfig {
    pub fn is_selected_for_variant(&self, variant: Option<&NativeVariant>) -> bool {
        self.variants.is_empty() || variant.is_some_and(|variant| self.variants.contains(variant))
    }
}

impl FissionProject {
    pub fn native_modules_for_variant<'a>(
        &'a self,
        variant: Option<&'a NativeVariant>,
    ) -> impl Iterator<Item = &'a NativeModuleConfig> + 'a {
        self.native
            .modules
            .iter()
            .filter(move |module| module.is_selected_for_variant(variant))
    }
}

pub fn ensure_native_variant_target(target: Target, variant: Option<&NativeVariant>) -> Result<()> {
    if variant.is_some() && !matches!(target, Target::Linux | Target::Macos | Target::Windows) {
        bail!(
            "--variant is supported only for linux, macos, and windows desktop targets, not `{}`",
            target.as_str()
        );
    }
    Ok(())
}

pub fn variant_output_path(base: impl Into<PathBuf>, variant: Option<&NativeVariant>) -> PathBuf {
    let base = base.into();
    match variant {
        Some(variant) => base.join("variants").join(variant.as_str()),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_variant_membership_selects_restricted_and_universal_modules() {
        let project: FissionProject = toml::from_str(
            r#"
targets = ["macos"]

[app]
name = "Variant Demo"
app_id = "com.example.variant-demo"

[[native.modules]]
name = "shared"

[[native.modules]]
name = "scanner-engine"
variants = ["scanner"]

[[native.modules]]
name = "protection-engine"
variants = ["full"]
"#,
        )
        .unwrap();
        let scanner: NativeVariant = "scanner".parse().unwrap();
        let full: NativeVariant = "full".parse().unwrap();

        let names = |variant| {
            project
                .native_modules_for_variant(variant)
                .map(|module| module.name.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(None), vec!["shared"]);
        assert_eq!(names(Some(&scanner)), vec!["shared", "scanner-engine"]);
        assert_eq!(names(Some(&full)), vec!["shared", "protection-engine"]);
    }

    #[test]
    fn variant_names_are_path_stable_and_lowercase() {
        assert!("scanner-v2".parse::<NativeVariant>().is_ok());
        assert!("Scanner".parse::<NativeVariant>().is_err());
        assert!("scanner--v2".parse::<NativeVariant>().is_err());
        assert!("scanner_v2".parse::<NativeVariant>().is_err());
    }

    #[test]
    fn selected_variants_get_distinct_output_paths() {
        let scanner: NativeVariant = "scanner".parse().unwrap();
        assert_eq!(
            variant_output_path("target/fission/release/macos/pkg", Some(&scanner)),
            PathBuf::from("target/fission/release/macos/pkg/variants/scanner")
        );
        assert_eq!(
            variant_output_path("target/fission/release/macos/pkg", None),
            PathBuf::from("target/fission/release/macos/pkg")
        );
    }
}
