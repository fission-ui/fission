use crate::{FissionProject, MacosPackageConfig};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeMacosModuleConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xcodegen_spec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xcode_project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_data: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_schemes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub products: Vec<NativeMacosProductConfig>,
}

impl NativeMacosModuleConfig {
    pub fn is_empty(&self) -> bool {
        self.xcodegen_spec.is_none()
            && self.xcode_project.is_none()
            && self.derived_data.is_none()
            && self.test_schemes.is_empty()
            && self.products.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeMacosProductKind {
    AppExtension,
    SystemExtension,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeMacosProductSigningConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entitlements: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provisioning_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_identity: Option<String>,
}

impl NativeMacosProductSigningConfig {
    fn is_empty(&self) -> bool {
        self.entitlements.is_none()
            && self.provisioning_profile.is_none()
            && self.signing_identity.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeMacosProductConfig {
    pub scheme: String,
    pub bundle: String,
    pub kind: NativeMacosProductKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entitlements: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provisioning_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_identity: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "NativeMacosProductSigningConfig::is_empty"
    )]
    pub run: NativeMacosProductSigningConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacosNativeBundleMode {
    Package,
    Run,
}

#[derive(Clone, Debug)]
struct BuiltProduct {
    config: NativeMacosProductConfig,
    path: PathBuf,
}

#[derive(Clone, Copy)]
struct EffectiveSigning<'a> {
    entitlements: Option<&'a str>,
    provisioning_profile: Option<&'a str>,
    signing_identity: Option<&'a str>,
}

pub fn build_macos_native_modules(
    project_dir: &Path,
    project: &FissionProject,
    release: bool,
) -> Result<()> {
    let _ = build_products(project_dir, project, release)?;
    Ok(())
}

pub fn test_macos_native_modules(project_dir: &Path, project: &FissionProject) -> Result<()> {
    let project_dir = canonical_project_dir(project_dir)?;
    for module in &project.native.modules {
        if module.macos.is_empty() || module.macos.test_schemes.is_empty() {
            continue;
        }
        let xcode_project = prepare_xcode_project(&project_dir, &module.name, &module.macos)?;
        let derived_data = derived_data_path(&project_dir, &module.name, &module.macos, "test");
        for scheme in &module.macos.test_schemes {
            let scheme = required_value(scheme, "macOS native test scheme")?;
            let mut command = Command::new("xcodebuild");
            command
                .arg("-quiet")
                .arg("-project")
                .arg(&xcode_project)
                .arg("-scheme")
                .arg(scheme)
                .arg("-destination")
                .arg("platform=macOS")
                .arg("-derivedDataPath")
                .arg(&derived_data)
                .arg("CODE_SIGNING_ALLOWED=NO")
                .arg("CODE_SIGNING_REQUIRED=NO")
                .arg("test");
            run_status(
                &mut command,
                &format!("macOS native test scheme `{scheme}`"),
            )?;
        }
    }
    Ok(())
}

pub fn embed_and_sign_macos_native_modules(
    project_dir: &Path,
    app_bundle: &Path,
    project: &FissionProject,
    host_signing: &MacosPackageConfig,
    mode: MacosNativeBundleMode,
    release: bool,
) -> Result<()> {
    let project_dir = canonical_project_dir(project_dir)?;
    let app_bundle = fs::canonicalize(app_bundle).with_context(|| {
        format!(
            "failed to resolve macOS app bundle {}",
            app_bundle.display()
        )
    })?;
    for built in build_products(&project_dir, project, release)? {
        let destination = native_product_destination(&app_bundle, &built.config)?;
        if destination.exists() {
            fs::remove_dir_all(&destination).with_context(|| {
                format!(
                    "failed to remove previous macOS native product {}",
                    destination.display()
                )
            })?;
        }
        let parent = destination
            .parent()
            .context("macOS native product destination has no parent")?;
        fs::create_dir_all(parent)?;
        copy_bundle(&built.path, &destination)?;
        sign_native_product(
            &project_dir,
            &destination,
            &built.config,
            host_signing,
            mode,
        )?;
    }
    Ok(())
}

fn build_products(
    project_dir: &Path,
    project: &FissionProject,
    release: bool,
) -> Result<Vec<BuiltProduct>> {
    let project_dir = canonical_project_dir(project_dir)?;
    let profile = if release { "Release" } else { "Debug" };
    let profile_dir = profile.to_ascii_lowercase();
    let mut built = Vec::new();
    for module in &project.native.modules {
        if module.macos.is_empty() || module.macos.products.is_empty() {
            continue;
        }
        let xcode_project = prepare_xcode_project(&project_dir, &module.name, &module.macos)?;
        let derived_data =
            derived_data_path(&project_dir, &module.name, &module.macos, &profile_dir);
        let product_dir = native_output_root(&project_dir, &module.name, &profile_dir);
        if product_dir.exists() {
            fs::remove_dir_all(&product_dir).with_context(|| {
                format!(
                    "failed to clear macOS native product directory {}",
                    product_dir.display()
                )
            })?;
        }
        fs::create_dir_all(&product_dir)?;

        for product in &module.macos.products {
            validate_product(product)?;
            let mut command = Command::new("xcodebuild");
            command
                .arg("-quiet")
                .arg("-project")
                .arg(&xcode_project)
                .arg("-scheme")
                .arg(product.scheme.trim())
                .arg("-configuration")
                .arg(profile)
                .arg("-derivedDataPath")
                .arg(&derived_data)
                .arg(format!("CONFIGURATION_BUILD_DIR={}", product_dir.display()))
                .arg("CODE_SIGNING_ALLOWED=NO")
                .arg("CODE_SIGNING_REQUIRED=NO")
                .arg("build");
            run_status(
                &mut command,
                &format!("macOS native scheme `{}`", product.scheme.trim()),
            )?;

            let path = product_dir.join(product.bundle.trim());
            if !path.is_dir() {
                bail!(
                    "macOS native scheme `{}` completed but expected bundle is missing at {}",
                    product.scheme.trim(),
                    path.display()
                );
            }
            built.push(BuiltProduct {
                config: product.clone(),
                path,
            });
        }
    }
    Ok(built)
}

fn prepare_xcode_project(
    project_dir: &Path,
    module_name: &str,
    config: &NativeMacosModuleConfig,
) -> Result<PathBuf> {
    let project = config
        .xcode_project
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .context("macOS native module requires `xcode_project`")?;
    let project = resolve_project_path(project_dir, project);
    if let Some(spec) = config
        .xcodegen_spec
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let spec = resolve_project_path(project_dir, spec);
        if !spec.is_file() {
            bail!(
                "macOS native module `{module_name}` XcodeGen spec does not exist: {}",
                spec.display()
            );
        }
        let output_dir = project
            .parent()
            .context("macOS native Xcode project has no parent directory")?;
        let status = Command::new("xcodegen")
            .arg("--spec")
            .arg(&spec)
            .arg("--project")
            .arg(output_dir)
            .status()
            .context("failed to run xcodegen; install XcodeGen or remove `xcodegen_spec`")?;
        if !status.success() {
            bail!("xcodegen failed for macOS native module `{module_name}` with {status}");
        }
    }
    if !project.is_dir() {
        bail!(
            "macOS native module `{module_name}` Xcode project does not exist: {}",
            project.display()
        );
    }
    Ok(project)
}

fn native_product_destination(
    app_bundle: &Path,
    product: &NativeMacosProductConfig,
) -> Result<PathBuf> {
    validate_product(product)?;
    let relative = match product.kind {
        NativeMacosProductKind::AppExtension => "Contents/PlugIns",
        NativeMacosProductKind::SystemExtension => "Contents/Library/SystemExtensions",
    };
    Ok(app_bundle.join(relative).join(product.bundle.trim()))
}

fn validate_product(product: &NativeMacosProductConfig) -> Result<()> {
    required_value(&product.scheme, "macOS native product scheme")?;
    let bundle = required_value(&product.bundle, "macOS native product bundle")?;
    let expected_extension = match product.kind {
        NativeMacosProductKind::AppExtension => "appex",
        NativeMacosProductKind::SystemExtension => "systemextension",
    };
    if Path::new(bundle)
        .extension()
        .and_then(|value| value.to_str())
        != Some(expected_extension)
    {
        bail!(
            "macOS native product bundle `{bundle}` must use the .{expected_extension} extension"
        );
    }
    if Path::new(bundle)
        .file_name()
        .and_then(|value| value.to_str())
        != Some(bundle)
    {
        bail!("macOS native product bundle `{bundle}` must be a file name, not a path");
    }
    Ok(())
}

fn sign_native_product(
    project_dir: &Path,
    bundle: &Path,
    product: &NativeMacosProductConfig,
    host_signing: &MacosPackageConfig,
    mode: MacosNativeBundleMode,
) -> Result<()> {
    let signing = effective_signing(product, host_signing, mode);
    if signing.provisioning_profile.is_some() && signing.signing_identity.is_none() {
        bail!(
            "macOS native product `{}` has a provisioning profile but no signing identity",
            product.bundle
        );
    }
    if let Some(profile) = signing.provisioning_profile {
        embed_provisioning_profile(project_dir, bundle, profile)?;
    }
    let Some(identity) = signing.signing_identity else {
        return Ok(());
    };

    let status = Command::new("codesign")
        .args(native_codesign_arguments(
            project_dir,
            identity,
            signing.entitlements,
        ))
        .arg(bundle)
        .status()
        .with_context(|| format!("failed to sign macOS native product {}", bundle.display()))?;
    if !status.success() {
        bail!(
            "codesign failed for macOS native product {} with {status}",
            bundle.display()
        );
    }
    let verify = Command::new("codesign")
        .args(["--verify", "--strict", "--verbose=2"])
        .arg(bundle)
        .status()
        .with_context(|| format!("failed to verify macOS native product {}", bundle.display()))?;
    if !verify.success() {
        bail!(
            "codesign verification failed for macOS native product {} with {verify}",
            bundle.display()
        );
    }
    Ok(())
}

fn effective_signing<'a>(
    product: &'a NativeMacosProductConfig,
    host: &'a MacosPackageConfig,
    mode: MacosNativeBundleMode,
) -> EffectiveSigning<'a> {
    let run = matches!(mode, MacosNativeBundleMode::Run);
    EffectiveSigning {
        entitlements: optional_value(if run {
            product
                .run
                .entitlements
                .as_deref()
                .or(product.entitlements.as_deref())
        } else {
            product.entitlements.as_deref()
        }),
        provisioning_profile: optional_value(if run {
            product
                .run
                .provisioning_profile
                .as_deref()
                .or(product.provisioning_profile.as_deref())
        } else {
            product.provisioning_profile.as_deref()
        }),
        signing_identity: optional_value(if run {
            product
                .run
                .signing_identity
                .as_deref()
                .or(product.signing_identity.as_deref())
                .or(host.signing_identity.as_deref())
        } else {
            product
                .signing_identity
                .as_deref()
                .or(host.signing_identity.as_deref())
        }),
    }
}

fn native_codesign_arguments(
    project_dir: &Path,
    identity: &str,
    entitlements: Option<&str>,
) -> Vec<OsString> {
    let mut args = vec![
        "--force".into(),
        "--timestamp".into(),
        "--options".into(),
        "runtime".into(),
        "--sign".into(),
        identity.into(),
    ];
    if let Some(entitlements) = entitlements {
        args.push("--entitlements".into());
        args.push(resolve_project_path(project_dir, entitlements).into_os_string());
    }
    args
}

fn embed_provisioning_profile(project_dir: &Path, bundle: &Path, profile: &str) -> Result<()> {
    let source = resolve_project_path(project_dir, profile);
    if !source.is_file() {
        bail!(
            "macOS native product provisioning profile does not exist: {}",
            source.display()
        );
    }
    let contents = bundle.join("Contents");
    fs::create_dir_all(&contents)?;
    let destination = contents.join("embedded.provisionprofile");
    fs::copy(&source, &destination).with_context(|| {
        format!(
            "failed to embed macOS native product profile {} at {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn copy_bundle(source: &Path, destination: &Path) -> Result<()> {
    let status = Command::new("ditto")
        .arg(source)
        .arg(destination)
        .status()
        .context("failed to run ditto while embedding a macOS native product")?;
    if !status.success() {
        bail!(
            "ditto failed while embedding {} at {} with {status}",
            source.display(),
            destination.display()
        );
    }
    Ok(())
}

fn derived_data_path(
    project_dir: &Path,
    module_name: &str,
    config: &NativeMacosModuleConfig,
    profile: &str,
) -> PathBuf {
    config
        .derived_data
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| resolve_project_path(project_dir, value))
        .unwrap_or_else(|| {
            project_dir
                .join(".fission/native/macos")
                .join(sanitize_component(module_name))
                .join(profile)
                .join("DerivedData")
        })
}

fn native_output_root(project_dir: &Path, module_name: &str, profile: &str) -> PathBuf {
    project_dir
        .join(".fission/native/macos")
        .join(sanitize_component(module_name))
        .join(profile)
        .join("Products")
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

fn sanitize_component(value: &str) -> String {
    let value = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value.trim_matches('-');
    if value.is_empty() {
        "module".to_string()
    } else {
        value.to_string()
    }
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
    fn parses_macos_native_module_products() {
        let project: FissionProject = toml::from_str(
            r#"
targets = ["macos"]

[app]
name = "demo"
app_id = "com.example.demo"

[[native.modules]]
name = "demo-native"

[native.modules.macos]
xcodegen_spec = "platforms/macos/native/project.yml"
xcode_project = "platforms/macos/native/Demo.xcodeproj"
test_schemes = ["DemoNativeTests"]

[[native.modules.macos.products]]
scheme = "DemoFileProvider"
bundle = "DemoFileProvider.appex"
kind = "app-extension"
entitlements = "platforms/macos/native/FileProvider.entitlements"
provisioning_profile = "profiles/FileProvider.provisionprofile"

[native.modules.macos.products.run]
provisioning_profile = "profiles/FileProviderDevelopment.provisionprofile"
signing_identity = "Apple Development"
"#,
        )
        .unwrap();

        let module = &project.native.modules[0].macos;
        assert_eq!(module.test_schemes, ["DemoNativeTests"]);
        assert_eq!(
            module.products[0].kind,
            NativeMacosProductKind::AppExtension
        );
        assert_eq!(
            module.products[0].run.provisioning_profile.as_deref(),
            Some("profiles/FileProviderDevelopment.provisionprofile")
        );
    }

    #[test]
    fn resolves_native_bundle_destinations() {
        let app = Path::new("/tmp/Demo.app");
        let app_extension = product(NativeMacosProductKind::AppExtension, "Provider.appex");
        let system_extension = product(
            NativeMacosProductKind::SystemExtension,
            "Security.systemextension",
        );

        assert_eq!(
            native_product_destination(app, &app_extension).unwrap(),
            Path::new("/tmp/Demo.app/Contents/PlugIns/Provider.appex")
        );
        assert_eq!(
            native_product_destination(app, &system_extension).unwrap(),
            Path::new("/tmp/Demo.app/Contents/Library/SystemExtensions/Security.systemextension")
        );
    }

    #[test]
    fn run_signing_overrides_package_values_and_inherits_host_identity() {
        let mut product = product(NativeMacosProductKind::AppExtension, "Provider.appex");
        product.entitlements = Some("release.entitlements".into());
        product.provisioning_profile = Some("release.provisionprofile".into());
        product.run.entitlements = Some("development.entitlements".into());
        product.run.provisioning_profile = Some("development.provisionprofile".into());
        let host = MacosPackageConfig {
            signing_identity: Some("Apple Development".into()),
            ..Default::default()
        };

        let signing = effective_signing(&product, &host, MacosNativeBundleMode::Run);

        assert_eq!(signing.entitlements, Some("development.entitlements"));
        assert_eq!(
            signing.provisioning_profile,
            Some("development.provisionprofile")
        );
        assert_eq!(signing.signing_identity, Some("Apple Development"));
    }

    #[test]
    fn rejects_bundle_suffix_mismatches() {
        let product = product(NativeMacosProductKind::SystemExtension, "Provider.appex");

        let error = validate_product(&product).unwrap_err();

        assert!(error.to_string().contains(".systemextension"));
    }

    fn product(kind: NativeMacosProductKind, bundle: &str) -> NativeMacosProductConfig {
        NativeMacosProductConfig {
            scheme: "Demo".into(),
            bundle: bundle.into(),
            kind,
            entitlements: None,
            provisioning_profile: None,
            signing_identity: None,
            run: NativeMacosProductSigningConfig::default(),
        }
    }
}
