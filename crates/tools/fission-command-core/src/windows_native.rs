use crate::FissionProject;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeWindowsModuleConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msbuild_project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_target: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_binaries: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub products: Vec<NativeWindowsProductConfig>,
}

impl NativeWindowsModuleConfig {
    pub fn is_empty(&self) -> bool {
        self.msbuild_project.is_none()
            && self.platform.is_none()
            && self.build_target.is_none()
            && self.test_binaries.is_empty()
            && self.products.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeWindowsProductKind {
    Runtime,
    DriverPackage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeWindowsProductConfig {
    pub name: String,
    pub path: String,
    pub kind: NativeWindowsProductKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BuiltWindowsNativeProduct {
    pub module: String,
    pub name: String,
    pub kind: NativeWindowsProductKind,
    pub source: PathBuf,
    pub destination: PathBuf,
}

pub fn build_windows_native_modules(
    project_dir: &Path,
    project: &FissionProject,
    release: bool,
) -> Result<Vec<BuiltWindowsNativeProduct>> {
    let project_dir = canonical_project_dir(project_dir)?;
    let configuration = if release { "Release" } else { "Debug" };
    let mut products = Vec::new();

    for module in &project.native.modules {
        if module.windows.is_empty() {
            continue;
        }
        let build_project = required_config_path(
            &project_dir,
            module.windows.msbuild_project.as_deref(),
            &module.name,
        )?;
        let platform = optional_value(module.windows.platform.as_deref()).unwrap_or("x64");
        let target = optional_value(module.windows.build_target.as_deref()).unwrap_or("Build");
        let mut command = Command::new("msbuild");
        command
            .arg(&build_project)
            .arg("/m")
            .arg(format!("/t:{target}"))
            .arg(format!("/p:Configuration={configuration}"))
            .arg(format!("/p:Platform={platform}"));
        run_status(
            &mut command,
            &format!("Windows native module `{}`", module.name),
        )?;

        for product in &module.windows.products {
            products.push(resolve_product(
                &project_dir,
                &module.name,
                product,
                configuration,
                platform,
            )?);
        }
    }

    Ok(products)
}

pub fn test_windows_native_modules(project_dir: &Path, project: &FissionProject) -> Result<()> {
    let products = build_windows_native_modules(project_dir, project, false)?;
    drop(products);
    let project_dir = canonical_project_dir(project_dir)?;

    for module in &project.native.modules {
        if module.windows.is_empty() {
            continue;
        }
        let platform = optional_value(module.windows.platform.as_deref()).unwrap_or("x64");
        for test_binary in &module.windows.test_binaries {
            let test_binary = expand_path(test_binary, "Debug", platform);
            let test_binary = resolve_project_path(&project_dir, &test_binary);
            if !test_binary.is_file() {
                bail!(
                    "Windows native test binary for module `{}` does not exist: {}",
                    module.name,
                    test_binary.display()
                );
            }
            let mut command = Command::new("vstest.console.exe");
            command.arg(&test_binary);
            run_status(
                &mut command,
                &format!("Windows native test binary `{}`", test_binary.display()),
            )?;
        }
    }

    Ok(())
}

pub fn stage_windows_runtime_products(
    destination_root: &Path,
    products: &[BuiltWindowsNativeProduct],
) -> Result<()> {
    for product in products {
        if product.kind != NativeWindowsProductKind::Runtime {
            continue;
        }
        let destination = destination_root.join(&product.destination);
        if destination.exists() {
            bail!(
                "Windows native runtime product `{}` would overwrite {}",
                product.name,
                destination.display()
            );
        }
        copy_product(&product.source, &destination)?;
    }
    Ok(())
}

fn required_config_path(
    project_dir: &Path,
    configured: Option<&str>,
    module_name: &str,
) -> Result<PathBuf> {
    let configured = optional_value(configured).with_context(|| {
        format!("Windows native module `{module_name}` requires `msbuild_project`")
    })?;
    let path = resolve_project_path(project_dir, configured);
    if !path.is_file() {
        bail!(
            "Windows native module `{module_name}` MSBuild project does not exist: {}",
            path.display()
        );
    }
    Ok(path)
}

fn resolve_product(
    project_dir: &Path,
    module_name: &str,
    product: &NativeWindowsProductConfig,
    configuration: &str,
    platform: &str,
) -> Result<BuiltWindowsNativeProduct> {
    let name = required_value(&product.name, "Windows native product name")?;
    let path = required_value(&product.path, "Windows native product path")?;
    let source = resolve_project_path(project_dir, &expand_path(path, configuration, platform));
    if !source.exists() {
        bail!(
            "Windows native product `{name}` from module `{module_name}` does not exist: {}",
            source.display()
        );
    }
    let default_destination = source
        .file_name()
        .map(PathBuf::from)
        .context("Windows native product source has no file name")?;
    let destination = product
        .destination
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or(default_destination);
    validate_relative_destination(&destination)?;

    Ok(BuiltWindowsNativeProduct {
        module: module_name.to_string(),
        name: name.to_string(),
        kind: product.kind,
        source,
        destination,
    })
}

fn validate_relative_destination(destination: &Path) -> Result<()> {
    if destination.as_os_str().is_empty() || destination.is_absolute() {
        bail!("Windows native product destination must be a non-empty relative path");
    }
    if destination.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        bail!(
            "Windows native product destination cannot escape the application or installer root: {}",
            destination.display()
        );
    }
    Ok(())
}

fn expand_path(value: &str, configuration: &str, platform: &str) -> String {
    value
        .replace("{configuration}", configuration)
        .replace("{profile}", &configuration.to_ascii_lowercase())
        .replace("{platform}", platform)
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
        .context("Windows native product destination has no parent")?;
    fs::create_dir_all(parent)?;
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy Windows native product {} to {}",
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
    use crate::FissionProject;

    #[test]
    fn parses_windows_native_products() {
        let project: FissionProject = toml::from_str(
            r#"
targets = ["windows"]

[app]
name = "demo"
app_id = "com.example.demo"

[[native.modules]]
name = "demo-native"

[native.modules.windows]
msbuild_project = "platforms/windows/native/Demo.sln"
platform = "x64"
test_binaries = ["platforms/windows/native/{platform}/{configuration}/DemoTests.exe"]

[[native.modules.windows.products]]
name = "provider"
path = "platforms/windows/native/{platform}/{configuration}/Provider.dll"
kind = "runtime"
destination = "native/Provider.dll"

[[native.modules.windows.products]]
name = "minifilter"
path = "platforms/windows/native/{platform}/{configuration}/DriverPackage"
kind = "driver-package"
"#,
        )
        .unwrap();

        let module = &project.native.modules[0].windows;
        assert_eq!(module.platform.as_deref(), Some("x64"));
        assert_eq!(module.products.len(), 2);
        assert_eq!(
            module.products[1].kind,
            NativeWindowsProductKind::DriverPackage
        );
    }

    #[test]
    fn expands_configuration_and_platform_tokens() {
        assert_eq!(
            expand_path(
                "out/{platform}/{configuration}/{profile}",
                "Release",
                "ARM64"
            ),
            "out/ARM64/Release/release"
        );
    }

    #[test]
    fn rejects_destination_traversal() {
        let error = validate_relative_destination(Path::new("../driver.sys")).unwrap_err();
        assert!(error.to_string().contains("cannot escape"));
    }

    #[test]
    fn stages_runtime_products_without_driver_packages() {
        let root = unique_dir("windows-native-stage");
        let source = root.join("source");
        let destination = root.join("destination");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("provider.dll"), b"runtime").unwrap();
        fs::write(source.join("driver.sys"), b"driver").unwrap();
        let products = vec![
            BuiltWindowsNativeProduct {
                module: "demo".into(),
                name: "provider".into(),
                kind: NativeWindowsProductKind::Runtime,
                source: source.join("provider.dll"),
                destination: PathBuf::from("native/provider.dll"),
            },
            BuiltWindowsNativeProduct {
                module: "demo".into(),
                name: "driver".into(),
                kind: NativeWindowsProductKind::DriverPackage,
                source: source.join("driver.sys"),
                destination: PathBuf::from("driver/driver.sys"),
            },
        ];

        stage_windows_runtime_products(&destination, &products).unwrap();

        assert_eq!(
            fs::read(destination.join("native/provider.dll")).unwrap(),
            b"runtime"
        );
        assert!(!destination.join("driver/driver.sys").exists());
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
