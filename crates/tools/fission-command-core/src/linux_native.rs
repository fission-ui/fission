use crate::FissionProject;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeLinuxModuleConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_manifest_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_package: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_default_features: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub products: Vec<NativeLinuxProductConfig>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl NativeLinuxModuleConfig {
    pub fn is_empty(&self) -> bool {
        self.cargo_manifest_path.is_none()
            && self.cargo_package.is_none()
            && self.features.is_empty()
            && !self.no_default_features
            && self.products.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeLinuxProductKind {
    Runtime,
    PrivilegedHelper,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeLinuxProductConfig {
    pub name: String,
    pub path: String,
    pub kind: NativeLinuxProductKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BuiltLinuxNativeProduct {
    pub module: String,
    pub name: String,
    pub kind: NativeLinuxProductKind,
    pub source: PathBuf,
    pub destination: PathBuf,
}

pub fn build_linux_native_modules(
    project_dir: &Path,
    project: &FissionProject,
    release: bool,
) -> Result<Vec<BuiltLinuxNativeProduct>> {
    let project_dir = canonical_project_dir(project_dir)?;
    let profile = if release { "release" } else { "debug" };
    let mut products = Vec::new();

    for module in &project.native.modules {
        if module.linux.is_empty() {
            continue;
        }
        run_cargo_module_command(
            &project_dir,
            module.path.as_deref(),
            &module.name,
            &module.linux,
            "build",
            release,
        )?;

        for product in &module.linux.products {
            products.push(resolve_product(
                &project_dir,
                &module.name,
                product,
                profile,
                env::consts::ARCH,
            )?);
        }
    }

    Ok(products)
}

pub fn test_linux_native_modules(project_dir: &Path, project: &FissionProject) -> Result<()> {
    let project_dir = canonical_project_dir(project_dir)?;
    for module in &project.native.modules {
        if module.linux.is_empty() {
            continue;
        }
        run_cargo_module_command(
            &project_dir,
            module.path.as_deref(),
            &module.name,
            &module.linux,
            "test",
            false,
        )?;
    }
    Ok(())
}

pub fn stage_linux_native_products(
    destination_root: &Path,
    products: &[BuiltLinuxNativeProduct],
) -> Result<()> {
    for product in products {
        let destination = destination_root.join(&product.destination);
        if destination.exists() {
            bail!(
                "Linux native product `{}` would overwrite {}",
                product.name,
                destination.display()
            );
        }
        copy_product(&product.source, &destination)?;
    }
    Ok(())
}

fn run_cargo_module_command(
    project_dir: &Path,
    module_path: Option<&str>,
    module_name: &str,
    config: &NativeLinuxModuleConfig,
    cargo_command: &str,
    release: bool,
) -> Result<()> {
    let package = required_optional_value(
        config.cargo_package.as_deref(),
        &format!("Linux native module `{module_name}` requires `cargo_package`"),
    )?;
    let manifest = resolve_manifest_path(
        project_dir,
        module_path,
        config.cargo_manifest_path.as_deref(),
        module_name,
    )?;
    let mut command = Command::new("cargo");
    command
        .arg(cargo_command)
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--package")
        .arg(package)
        .current_dir(project_dir);
    if release && cargo_command == "build" {
        command.arg("--release");
    }
    if config.no_default_features {
        command.arg("--no-default-features");
    }
    if !config.features.is_empty() {
        let features = config
            .features
            .iter()
            .map(|feature| required_value(feature, "Linux native Cargo feature"))
            .collect::<Result<Vec<_>>>()?
            .join(",");
        command.arg("--features").arg(features);
    }
    run_status(
        &mut command,
        &format!("Linux native module `{module_name}` Cargo {cargo_command}"),
    )
}

fn resolve_manifest_path(
    project_dir: &Path,
    module_path: Option<&str>,
    configured: Option<&str>,
    module_name: &str,
) -> Result<PathBuf> {
    let manifest = if let Some(configured) = optional_value(configured) {
        resolve_project_path(project_dir, configured)
    } else if let Some(module_path) = optional_value(module_path) {
        resolve_project_path(project_dir, module_path).join("Cargo.toml")
    } else {
        project_dir.join("Cargo.toml")
    };
    if !manifest.is_file() {
        bail!(
            "Linux native module `{module_name}` Cargo manifest does not exist: {}",
            manifest.display()
        );
    }
    fs::canonicalize(&manifest).with_context(|| {
        format!(
            "failed to resolve Linux native module `{module_name}` Cargo manifest {}",
            manifest.display()
        )
    })
}

fn resolve_product(
    project_dir: &Path,
    module_name: &str,
    product: &NativeLinuxProductConfig,
    profile: &str,
    architecture: &str,
) -> Result<BuiltLinuxNativeProduct> {
    let name = required_value(&product.name, "Linux native product name")?;
    let path = required_value(&product.path, "Linux native product path")?;
    let source = resolve_project_path(project_dir, &expand_path(path, profile, architecture));
    if !source.exists() {
        bail!(
            "Linux native product `{name}` from module `{module_name}` does not exist: {}",
            source.display()
        );
    }
    if product.kind == NativeLinuxProductKind::PrivilegedHelper && !source.is_file() {
        bail!("Linux privileged helper `{name}` must be a regular file");
    }
    let default_destination = source
        .file_name()
        .map(PathBuf::from)
        .context("Linux native product source has no file name")?;
    let destination = product
        .destination
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or(default_destination);
    validate_relative_destination(&destination)?;

    Ok(BuiltLinuxNativeProduct {
        module: module_name.to_string(),
        name: name.to_string(),
        kind: product.kind,
        source,
        destination,
    })
}

fn validate_relative_destination(destination: &Path) -> Result<()> {
    if destination.as_os_str().is_empty() || destination.is_absolute() {
        bail!("Linux native product destination must be a non-empty relative path");
    }
    if destination.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        bail!(
            "Linux native product destination cannot escape the application root: {}",
            destination.display()
        );
    }
    Ok(())
}

fn expand_path(value: &str, profile: &str, architecture: &str) -> String {
    let configuration = if profile == "release" {
        "Release"
    } else {
        "Debug"
    };
    value
        .replace("{profile}", profile)
        .replace("{configuration}", configuration)
        .replace("{architecture}", architecture)
}

fn copy_product(source: &Path, destination: &Path) -> Result<()> {
    if source.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_product(&entry.path(), &destination.join(entry.file_name()))?;
        }
        return Ok(());
    }
    let parent = destination
        .parent()
        .context("Linux native product destination has no parent")?;
    fs::create_dir_all(parent)?;
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy Linux native product {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn resolve_project_path(project_dir: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_dir.join(path)
    }
}

fn canonical_project_dir(project_dir: &Path) -> Result<PathBuf> {
    fs::canonicalize(project_dir).with_context(|| {
        format!(
            "failed to resolve project directory {}",
            project_dir.display()
        )
    })
}

fn required_optional_value<'a>(value: Option<&'a str>, message: &str) -> Result<&'a str> {
    optional_value(value).with_context(|| message.to_string())
}

fn required_value<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} cannot be empty");
    }
    Ok(value)
}

fn optional_value(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn run_status(command: &mut Command, label: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to run {label}"))?;
    if !status.success() {
        bail!("{label} failed with {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_native_products() {
        let project: FissionProject = toml::from_str(
            r#"
targets = ["linux"]

[app]
name = "demo"
app_id = "com.example.demo"

[[native.modules]]
name = "demo-native"
path = "platforms/linux/native"

[native.modules.linux]
cargo_package = "demo-mount-helper"
features = ["mount"]
no_default_features = true

[[native.modules.linux.products]]
name = "mount-helper"
path = "target/{profile}/demo-mount-helper"
kind = "privileged-helper"
destination = "libexec/demo-mount-helper"
"#,
        )
        .unwrap();

        let module = &project.native.modules[0].linux;
        assert_eq!(module.cargo_package.as_deref(), Some("demo-mount-helper"));
        assert_eq!(module.features, ["mount"]);
        assert!(module.no_default_features);
        assert_eq!(
            module.products[0].kind,
            NativeLinuxProductKind::PrivilegedHelper
        );
    }

    #[test]
    fn expands_profile_configuration_and_architecture_tokens() {
        assert_eq!(
            expand_path(
                "target/{architecture}/{configuration}/{profile}",
                "release",
                "x86_64"
            ),
            "target/x86_64/Release/release"
        );
    }

    #[test]
    fn rejects_destination_traversal() {
        let error = validate_relative_destination(Path::new("../helper")).unwrap_err();
        assert!(error.to_string().contains("cannot escape"));
    }

    #[test]
    fn stages_runtime_and_privileged_products() {
        let root = unique_dir("linux-native-stage");
        let source = root.join("source");
        let destination = root.join("destination");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("provider.so"), b"runtime").unwrap();
        fs::write(source.join("mount-helper"), b"helper").unwrap();
        let products = vec![
            BuiltLinuxNativeProduct {
                module: "demo".into(),
                name: "provider".into(),
                kind: NativeLinuxProductKind::Runtime,
                source: source.join("provider.so"),
                destination: PathBuf::from("lib/provider.so"),
            },
            BuiltLinuxNativeProduct {
                module: "demo".into(),
                name: "mount-helper".into(),
                kind: NativeLinuxProductKind::PrivilegedHelper,
                source: source.join("mount-helper"),
                destination: PathBuf::from("libexec/mount-helper"),
            },
        ];

        stage_linux_native_products(&destination, &products).unwrap();

        assert_eq!(
            fs::read(destination.join("lib/provider.so")).unwrap(),
            b"runtime"
        );
        assert_eq!(
            fs::read(destination.join("libexec/mount-helper")).unwrap(),
            b"helper"
        );
    }

    fn unique_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "fission-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
