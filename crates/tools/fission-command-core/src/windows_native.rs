use crate::FissionProject;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeWindowsModuleConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nuget_packages_config: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nuget_packages_directory: Option<String>,
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
        self.nuget_packages_config.is_none()
            && self.nuget_packages_directory.is_none()
            && self.msbuild_project.is_none()
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
        let native_tool_paths =
            restore_windows_native_packages(&project_dir, &module.name, &module.windows)?;
        let build_project = required_config_path(
            &project_dir,
            module.windows.msbuild_project.as_deref(),
            &module.name,
        )?;
        let platform = optional_value(module.windows.platform.as_deref()).unwrap_or("x64");
        let target = optional_value(module.windows.build_target.as_deref()).unwrap_or("Build");
        let mut command = Command::new("msbuild");
        command
            .arg(windows_external_tool_path(&build_project))
            .arg("/m")
            .arg(format!("/t:{target}"))
            .arg(format!("/p:Configuration={configuration}"))
            .arg(format!("/p:Platform={platform}"));
        prepend_windows_native_tool_paths(&mut command, &native_tool_paths)?;
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

fn restore_windows_native_packages(
    project_dir: &Path,
    module_name: &str,
    config: &NativeWindowsModuleConfig,
) -> Result<Vec<PathBuf>> {
    let Some(packages_config) = optional_value(config.nuget_packages_config.as_deref()) else {
        if config.nuget_packages_directory.is_some() {
            bail!(
                "Windows native module `{module_name}` sets `nuget_packages_directory` without `nuget_packages_config`"
            );
        }
        return Ok(Vec::new());
    };
    let packages_config = resolve_project_path(project_dir, packages_config);
    if !packages_config.is_file() {
        bail!(
            "Windows native module `{module_name}` NuGet packages config does not exist: {}",
            packages_config.display()
        );
    }
    let packages_directory = optional_value(config.nuget_packages_directory.as_deref())
        .map(|path| resolve_project_path(project_dir, path))
        .unwrap_or_else(|| {
            packages_config
                .parent()
                .unwrap_or(project_dir)
                .join("packages")
        });
    let mut command = Command::new("nuget");
    command
        .arg("restore")
        .arg(windows_external_tool_path(&packages_config))
        .arg("-PackagesDirectory")
        .arg(windows_external_tool_path(&packages_directory))
        .arg("-NonInteractive");
    run_status(
        &mut command,
        &format!("Windows native module `{module_name}` NuGet restore"),
    )?;

    discover_windows_native_tool_paths(&packages_directory, env::consts::ARCH)
}

fn discover_windows_native_tool_paths(
    packages_directory: &Path,
    host_architecture: &str,
) -> Result<Vec<PathBuf>> {
    let host_architecture = match host_architecture {
        "x86_64" => "x64",
        "aarch64" => "ARM64",
        _ => return Ok(Vec::new()),
    };
    let mut paths = Vec::new();

    for package in read_directories(packages_directory)? {
        let bin_root = package.join("c").join("bin");
        if !bin_root.is_dir() {
            continue;
        }
        for version in read_directories(&bin_root)? {
            let candidate = version.join(host_architecture);
            if candidate.is_dir() {
                paths.push(candidate);
            }
        }
    }

    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn read_directories(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root).with_context(|| {
        format!(
            "failed to inspect restored NuGet directory {}",
            root.display()
        )
    })? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn prepend_windows_native_tool_paths(command: &mut Command, paths: &[PathBuf]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut combined = paths
        .iter()
        .map(|path| windows_external_tool_path(path))
        .collect::<Vec<_>>();
    if let Some(current) = env::var_os("PATH") {
        combined.extend(env::split_paths(&current));
    }
    let joined = env::join_paths(combined)
        .context("failed to construct PATH for Windows native module tools")?;
    command.env("PATH", joined);
    Ok(())
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
            command.arg(windows_external_tool_path(&test_binary));
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

fn windows_external_tool_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        let encoded = path.as_os_str().encode_wide().collect::<Vec<_>>();
        return normalize_windows_verbatim_units(&encoded)
            .map(|units| PathBuf::from(OsString::from_wide(&units)))
            .unwrap_or_else(|| path.to_path_buf());
    }

    #[cfg(not(windows))]
    path.to_path_buf()
}

#[cfg(any(windows, test))]
fn normalize_windows_verbatim_units(path: &[u16]) -> Option<Vec<u16>> {
    const VERBATIM_PREFIX: &[u16] = &[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
    const VERBATIM_UNC_PREFIX: &[u16] = &[
        b'\\' as u16,
        b'\\' as u16,
        b'?' as u16,
        b'\\' as u16,
        b'U' as u16,
        b'N' as u16,
        b'C' as u16,
        b'\\' as u16,
    ];

    if let Some(remainder) = path.strip_prefix(VERBATIM_UNC_PREFIX) {
        let mut normalized = vec![b'\\' as u16, b'\\' as u16];
        normalized.extend_from_slice(remainder);
        return Some(normalized);
    }
    path.strip_prefix(VERBATIM_PREFIX).map(Vec::from)
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
nuget_packages_config = "platforms/windows/native/packages.config"
nuget_packages_directory = "platforms/windows/native/packages"
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
        assert_eq!(
            module.nuget_packages_config.as_deref(),
            Some("platforms/windows/native/packages.config")
        );
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

    #[test]
    fn discovers_restored_wdk_tools_for_host_architecture() {
        let root = unique_dir("windows-native-tools");
        let x64 = root
            .join("Microsoft.Windows.WDK.x64.10.0.1")
            .join("c/bin/10.0.1/x64");
        let arm64 = root
            .join("Microsoft.Windows.WDK.x64.10.0.1")
            .join("c/bin/10.0.1/ARM64");
        fs::create_dir_all(&x64).unwrap();
        fs::create_dir_all(&arm64).unwrap();
        fs::write(x64.join("stampinf.exe"), b"fixture").unwrap();

        let paths = discover_windows_native_tool_paths(&root, "x86_64").unwrap();

        assert_eq!(paths, vec![x64]);
    }

    #[test]
    fn ignores_restored_packages_without_native_host_tools() {
        let root = unique_dir("windows-native-no-tools");
        fs::create_dir_all(root.join("Example.Package.1.0.0/lib/net8.0")).unwrap();

        let paths = discover_windows_native_tool_paths(&root, "x86_64").unwrap();

        assert!(paths.is_empty());
    }

    #[test]
    fn normalizes_verbatim_drive_path_for_windows_tools() {
        let path = r"\\?\D:\a\demo\packages".encode_utf16().collect::<Vec<_>>();
        let normalized = normalize_windows_verbatim_units(&path).unwrap();

        assert_eq!(
            String::from_utf16(normalized.as_slice()).unwrap(),
            r"D:\a\demo\packages"
        );
    }

    #[test]
    fn normalizes_verbatim_unc_path_for_windows_tools() {
        let path = r"\\?\UNC\server\share\packages"
            .encode_utf16()
            .collect::<Vec<_>>();
        let normalized = normalize_windows_verbatim_units(&path).unwrap();

        assert_eq!(
            String::from_utf16(normalized.as_slice()).unwrap(),
            r"\\server\share\packages"
        );
    }

    #[test]
    fn leaves_non_verbatim_windows_path_unchanged() {
        let path = r"D:\a\demo\packages".encode_utf16().collect::<Vec<_>>();

        assert!(normalize_windows_verbatim_units(&path).is_none());
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
