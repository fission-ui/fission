use super::macos_notarization::notarize_macos_artifact_if_configured;
use super::*;
use anyhow::{bail, Context, Result};
use fission_command_core::{
    build_linux_native_modules, build_windows_native_modules, cargo_package_name,
    embed_and_sign_macos_native_modules, ensure_native_variant_target, normalized_extension,
    read_desktop_cargo_options, read_macos_package_config_for_profile_and_variant,
    read_project_config, resolve_app_icon, sign_macos_app_if_configured,
    stage_linux_native_products, stage_project_assets, stage_windows_runtime_products,
    sync_platform_config, variant_output_path, BuiltLinuxNativeProduct, BuiltWindowsNativeProduct,
    DesktopCargoOptions, FissionProject, MacosNativeBundleMode, MacosPackageConfig,
    NativeLinuxProductKind, NativeWindowsProductKind, PlatformCapability, Target,
};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Builder as TarBuilder;

#[derive(Debug, Deserialize, Default)]
struct PackageManifest {
    package: Option<PackageRoot>,
}

#[derive(Debug, Deserialize, Default)]
struct PackageRoot {
    docker: Option<DockerPackageConfig>,
    linux: Option<LinuxPackageConfig>,
    windows: Option<WindowsPackageConfig>,
    #[serde(default)]
    secondary_artifacts: Vec<SecondaryArtifactConfig>,
    #[serde(default)]
    symbols: Vec<SecondaryArtifactConfig>,
    #[serde(default)]
    crash_assets: Vec<SecondaryArtifactConfig>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct LinuxPackageConfig {
    run: Option<LinuxRunPackageConfig>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct LinuxRunPackageConfig {
    installer_script: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct WindowsPackageConfig {
    exe_installer_script: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct SecondaryArtifactConfig {
    kind: Option<String>,
    purpose: Option<String>,
    platform: Option<String>,
    path: Option<String>,
    upload_provider: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct DockerPackageConfig {
    adapter: Option<DockerStaticAdapter>,
    port: Option<u16>,
    base_image: Option<String>,
    tags: Option<Vec<String>>,
    build: Option<bool>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum DockerStaticAdapter {
    Actix,
    Axum,
}

impl Default for DockerStaticAdapter {
    fn default() -> Self {
        Self::Axum
    }
}

impl DockerPackageConfig {
    fn adapter(&self) -> DockerStaticAdapter {
        self.adapter.unwrap_or_default()
    }

    fn port(&self) -> u16 {
        self.port.unwrap_or(8080)
    }

    fn base_image(&self) -> &str {
        self.base_image
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("debian:bookworm-slim")
    }

    fn build(&self) -> bool {
        self.build.unwrap_or(true)
    }
}

#[path = "package_validation.rs"]
mod package_validation;
use package_validation::{
    manifest_validation_state, package_artifact_checks, prepare_package_validation_inputs,
};

#[path = "package_docker.rs"]
mod package_docker;
use package_docker::*;

pub(super) fn package_artifact(options: &PackageOptions) -> Result<ArtifactManifest> {
    ensure_native_variant_target(options.target, options.variant.as_ref())?;
    match options.format {
        PackageFormat::Static => package_static(options),
        PackageFormat::Run => {
            if options.target == Target::Terminal {
                package_terminal_run(options)
            } else {
                package_linux_run(options)
            }
        }
        PackageFormat::DockerImage => package_docker_image(options),
        PackageFormat::App => package_macos_app(options),
        PackageFormat::Pkg => package_macos_pkg(options),
        PackageFormat::Exe => package_windows_exe(options),
        PackageFormat::Apk => package_android_apk(options),
        PackageFormat::Aab => package_with_project_script(
            options,
            Target::Android,
            "platforms/android/package-aab.sh",
            "aab",
        ),
        PackageFormat::Ipa => {
            package_with_project_script(options, Target::Ios, "platforms/ios/package-ipa.sh", "ipa")
        }
        PackageFormat::Msi => package_with_project_script(
            options,
            Target::Windows,
            "platforms/windows/package-msi.ps1",
            "msi",
        ),
        PackageFormat::Msix => package_with_project_script(
            options,
            Target::Windows,
            "platforms/windows/package-msix.ps1",
            "msix",
        ),
    }
}

pub(super) fn package_static(options: &PackageOptions) -> Result<ArtifactManifest> {
    if options.format != PackageFormat::Static {
        bail!("only --format static is currently supported");
    }
    let project = read_project_config(&options.project_dir)?;
    if !project.targets.contains(&options.target) {
        bail!(
            "target `{}` is not configured for this app; run `fission add-target {} --project-dir {}`",
            options.target.as_str(),
            options.target.as_str(),
            options.project_dir.display()
        );
    }

    let source_dir = match options.target {
        Target::Site => {
            fission_command_site::build(&options.project_dir, options.release)?;
            site_output_dir(&options.project_dir)?
        }
        Target::Web => {
            fission_command_run::build_app(fission_command_run::BuildOptions {
                project_dir: options.project_dir.clone(),
                target: Some(Target::Web),
                release: options.release,
                variant: None,
            })?;
            options.project_dir.join("platforms/web")
        }
        other => bail!(
            "static packaging currently supports static-site and web targets, not `{}`",
            other.as_str()
        ),
    };

    if !source_dir.join("index.html").exists() {
        bail!(
            "static package source {} does not contain index.html",
            source_dir.display()
        );
    }

    let profile = profile_name(options.release);
    let staging_dir = clean_package_dir(options)?;
    copy_dir_contents(&source_dir, &staging_dir)?;
    write_static_package_metadata(&options.project_dir, &staging_dir)?;

    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn package_docker_image(options: &PackageOptions) -> Result<ArtifactManifest> {
    if !matches!(options.target, Target::Server | Target::Site)
        || options.format != PackageFormat::DockerImage
    {
        bail!("docker-image packaging supports --target ssr or --target static-site");
    }
    let project = read_project_config(&options.project_dir)?;
    if !project.targets.contains(&options.target) {
        bail!(
            "target `{}` is not configured for this app; run `fission add-target {} --project-dir {}`",
            options.target.as_str(),
            options.target.as_str(),
            options.project_dir.display()
        );
    }

    let config = docker_package_config(&options.project_dir)?;
    let profile = profile_name(options.release);
    let staging_dir = clean_package_dir(options)?;
    let tags = docker_image_tags(options, &project, &config);
    match options.target {
        Target::Server => {
            write_server_docker_context(options, &project, &config, &staging_dir, &tags)?
        }
        Target::Site => {
            write_static_site_docker_context(options, &project, &config, &staging_dir, &tags)?
        }
        _ => unreachable!(),
    }

    let mut built = false;
    if config.build() {
        build_docker_image(&staging_dir, &tags)?;
        built = true;
    }
    write_docker_image_metadata(options, &project, &config, &staging_dir, &tags, built)?;
    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn package_linux_run(options: &PackageOptions) -> Result<ArtifactManifest> {
    ensure_package_target(options, Target::Linux, PackageFormat::Run)?;
    require_host_os(Target::Linux)?;
    let project = read_project_config(&options.project_dir)?;
    let profile = profile_name(options.release);
    let staging_dir = clean_package_dir(options)?;
    let payload_dir = staging_dir.join("payload");
    fs::create_dir_all(&payload_dir)?;
    let binary = build_desktop_binary(options)?;
    let executable_name = binary
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("app")
        .to_string();
    fs::copy(&binary, payload_dir.join(&executable_name)).with_context(|| {
        format!(
            "failed to copy {} to {}",
            binary.display(),
            payload_dir.display()
        )
    })?;
    let native_products = build_linux_native_modules(
        &options.project_dir,
        &project,
        options.variant.as_ref(),
        options.release,
    )?;
    stage_linux_native_products(&payload_dir, &native_products)?;
    let native_products_manifest =
        write_linux_native_products_manifest(&payload_dir, &native_products, profile, "run")?;
    stage_project_assets(&options.project_dir, &payload_dir)?;

    let package_name = sanitize_file_stem(&project.app.name);
    let run_path = staging_dir.join(format!(
        "{package_name}-{}-{}.run",
        cargo_package_version(&options.project_dir).unwrap_or_else(|| "0.0.0".to_string()),
        profile
    ));
    let linux = linux_package_config(&options.project_dir)?;
    if let Some(script) = linux
        .run
        .and_then(|run| run.installer_script)
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let script = resolve_project_path(&options.project_dir, script.to_string());
        let environment = linux_packaging_environment(
            &payload_dir,
            &payload_dir.join(&executable_name),
            &native_products_manifest,
        );
        let output_path = run_packaging_script_with_env(
            &options.project_dir,
            &script,
            options.release,
            options.variant.as_ref(),
            &environment,
        )?
        .with_context(|| format!("{} did not print a .run installer path", script.display()))?;
        if output_path.extension().and_then(OsStr::to_str) != Some("run") {
            bail!(
                "{} printed {}, expected a .run installer",
                script.display(),
                output_path.display()
            );
        }
        fs::copy(&output_path, &run_path).with_context(|| {
            format!(
                "failed to copy Linux installer {} to {}",
                output_path.display(),
                run_path.display()
            )
        })?;
        set_executable(&run_path)?;
    } else {
        write_linux_run(&payload_dir, &run_path, &project.app.name, &executable_name)?;
    }
    fs::remove_dir_all(&payload_dir).ok();
    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn package_terminal_run(options: &PackageOptions) -> Result<ArtifactManifest> {
    ensure_package_target(options, Target::Terminal, PackageFormat::Run)?;
    let project = read_project_config(&options.project_dir)?;
    let profile = profile_name(options.release);
    let staging_dir = clean_package_dir(options)?;
    let payload_dir = staging_dir.join("payload");
    fs::create_dir_all(&payload_dir)?;
    let binary = build_desktop_binary(options)?;
    let executable_name = binary
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("app")
        .to_string();
    fs::copy(&binary, payload_dir.join(&executable_name)).with_context(|| {
        format!(
            "failed to copy {} to {}",
            binary.display(),
            payload_dir.display()
        )
    })?;
    stage_project_assets(&options.project_dir, &payload_dir)?;

    let package_name = sanitize_file_stem(&project.app.name);
    let run_path = staging_dir.join(format!(
        "{package_name}-terminal-{}-{}.run",
        cargo_package_version(&options.project_dir).unwrap_or_else(|| "0.0.0".to_string()),
        profile
    ));
    write_linux_run(&payload_dir, &run_path, &project.app.name, &executable_name)?;
    fs::remove_dir_all(&payload_dir).ok();
    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn package_macos_app(options: &PackageOptions) -> Result<ArtifactManifest> {
    ensure_package_target(options, Target::Macos, PackageFormat::App)?;
    require_host_os(Target::Macos)?;
    let project = read_project_config(&options.project_dir)?;
    let profile = profile_name(options.release);
    let staging_dir = clean_package_dir(options)?;
    let macos = read_macos_package_config_for_profile_and_variant(
        &options.project_dir,
        options.release,
        options.variant.as_ref(),
    )?;
    let app_bundle = create_macos_app_bundle(options, &project, &staging_dir, &macos)?;
    embed_and_sign_macos_native_modules(
        &options.project_dir,
        &app_bundle,
        &project,
        options.variant.as_ref(),
        &macos,
        MacosNativeBundleMode::Package,
        options.release,
    )?;
    sign_macos_app_if_configured(&options.project_dir, &app_bundle, &macos)?;
    notarize_macos_artifact_if_configured(&app_bundle, &macos)?;
    println!("{}", app_bundle.display());
    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn package_macos_pkg(options: &PackageOptions) -> Result<ArtifactManifest> {
    ensure_package_target(options, Target::Macos, PackageFormat::Pkg)?;
    require_host_os(Target::Macos)?;
    let project = read_project_config(&options.project_dir)?;
    let profile = profile_name(options.release);
    let staging_dir = clean_package_dir(options)?;
    let app_staging = staging_dir.join("app-staging");
    let macos = read_macos_package_config_for_profile_and_variant(
        &options.project_dir,
        options.release,
        options.variant.as_ref(),
    )?;
    let app_bundle = create_macos_app_bundle(options, &project, &app_staging, &macos)?;
    embed_and_sign_macos_native_modules(
        &options.project_dir,
        &app_bundle,
        &project,
        options.variant.as_ref(),
        &macos,
        MacosNativeBundleMode::Package,
        options.release,
    )?;
    sign_macos_app_if_configured(&options.project_dir, &app_bundle, &macos)?;
    notarize_macos_artifact_if_configured(&app_bundle, &macos)?;
    let version = resolved_package_version(&options.project_dir, options.target)?;
    let pkg_path = staging_dir.join(format!(
        "{}-{}.pkg",
        sanitize_file_stem(&project.app.name),
        version
    ));
    let component_plist = if macos.pkg_builder.as_deref().unwrap_or("pkgbuild") == "pkgbuild" {
        Some(write_macos_component_plist(&staging_dir, &app_bundle)?)
    } else {
        None
    };
    let (pkg_builder, pkg_arguments) =
        macos_pkg_builder_command(&app_bundle, &pkg_path, component_plist.as_deref(), &macos)?;
    if find_in_path(pkg_builder).is_none() {
        bail!(
            "{pkg_builder} was not found; install Xcode command line tools to create macOS .pkg packages"
        );
    }
    let status = Command::new(pkg_builder)
        .args(pkg_arguments)
        .status()
        .with_context(|| format!("failed to run {pkg_builder}"))?;
    if !status.success() {
        bail!("{pkg_builder} failed with {status}");
    }
    if let Some(component_plist) = component_plist {
        fs::remove_file(&component_plist).with_context(|| {
            format!(
                "failed to remove macOS component property list {}",
                component_plist.display()
            )
        })?;
    }
    notarize_macos_artifact_if_configured(&pkg_path, &macos)?;
    fs::remove_dir_all(&app_staging).ok();
    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn package_windows_exe(options: &PackageOptions) -> Result<ArtifactManifest> {
    ensure_package_target(options, Target::Windows, PackageFormat::Exe)?;
    require_host_os(Target::Windows)?;
    let project = read_project_config(&options.project_dir)?;
    let profile = profile_name(options.release);
    let staging_dir = clean_package_dir(options)?;
    let binary = build_desktop_binary(options)?;
    let native_products = build_windows_native_modules(
        &options.project_dir,
        &project,
        options.variant.as_ref(),
        options.release,
    )?;
    let windows = windows_package_config(&options.project_dir)?;
    if let Some(script) = windows
        .exe_installer_script
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let script = resolve_project_path(&options.project_dir, script.to_string());
        let manifest = write_windows_native_products_manifest(
            &options.project_dir,
            &native_products,
            true,
            profile,
            "exe",
            options.variant.as_ref(),
        )?;
        let environment = windows_packaging_environment(&options.project_dir, &binary, &manifest)?;
        let output_path = run_packaging_script_with_env(
            &options.project_dir,
            &script,
            options.release,
            options.variant.as_ref(),
            &environment,
        )?
        .with_context(|| format!("{} did not print an .exe path", script.display()))?;
        if output_path.extension().and_then(OsStr::to_str) != Some("exe") {
            bail!(
                "{} printed {}, expected an .exe installer",
                script.display(),
                output_path.display()
            );
        }
        let destination = staging_dir.join(
            output_path
                .file_name()
                .unwrap_or_else(|| OsStr::new("installer.exe")),
        );
        fs::copy(&output_path, &destination).with_context(|| {
            format!(
                "failed to copy Windows installer {} to {}",
                output_path.display(),
                destination.display()
            )
        })?;
    } else {
        let dest = staging_dir.join(binary.file_name().unwrap_or_else(|| OsStr::new("app.exe")));
        fs::copy(&binary, &dest).with_context(|| {
            format!("failed to copy {} to {}", binary.display(), dest.display())
        })?;
        stage_windows_runtime_products(&staging_dir, &native_products)?;
        stage_project_assets(&options.project_dir, &staging_dir)?;
    }
    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn package_android_apk(options: &PackageOptions) -> Result<ArtifactManifest> {
    ensure_package_target(options, Target::Android, PackageFormat::Apk)?;
    let project = read_project_config(&options.project_dir)?;
    sync_platform_config(&options.project_dir, &project)?;
    let profile = profile_name(options.release);
    let staging_dir = clean_package_dir(options)?;
    let script = options.project_dir.join("platforms/android/package-apk.sh");
    let output_path = run_packaging_script(
        &options.project_dir,
        &script,
        options.release,
        options.variant.as_ref(),
    )?
    .with_context(|| format!("{} did not print an .apk path", script.display()))?;
    if output_path.extension().and_then(OsStr::to_str) != Some("apk") {
        bail!(
            "{} printed {}, expected an .apk artifact",
            script.display(),
            output_path.display()
        );
    }
    let dest = staging_dir.join(
        output_path
            .file_name()
            .unwrap_or_else(|| OsStr::new("app.apk")),
    );
    fs::copy(&output_path, &dest).with_context(|| {
        format!(
            "failed to copy Android APK {} to {}",
            output_path.display(),
            dest.display()
        )
    })?;
    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn package_with_project_script(
    options: &PackageOptions,
    target: Target,
    relative_script: &str,
    expected_extension: &str,
) -> Result<ArtifactManifest> {
    ensure_package_target(options, target, options.format)?;
    let project = read_project_config(&options.project_dir)?;
    if matches!(target, Target::Android | Target::Ios | Target::Windows) {
        sync_platform_config(&options.project_dir, &project)?;
    }
    let profile = profile_name(options.release);
    let staging_dir = clean_package_dir(options)?;
    let script = options.project_dir.join(relative_script);
    if !script.exists() {
        bail!(
            "{} packaging requires {}; this target packaging flow has not been configured for this project yet",
            options.format.as_str(),
            script.display()
        );
    }
    let mut environment = Vec::new();
    if target == Target::Windows {
        let binary = build_desktop_binary(options)?;
        let native_products = build_windows_native_modules(
            &options.project_dir,
            &project,
            options.variant.as_ref(),
            options.release,
        )?;
        let include_driver_packages = options.format != PackageFormat::Msix;
        let manifest = write_windows_native_products_manifest(
            &options.project_dir,
            &native_products,
            include_driver_packages,
            profile,
            options.format.as_str(),
            options.variant.as_ref(),
        )?;
        environment = windows_packaging_environment(&options.project_dir, &binary, &manifest)?;
    }
    let output_path = run_packaging_script_with_env(
        &options.project_dir,
        &script,
        options.release,
        options.variant.as_ref(),
        &environment,
    )?
    .with_context(|| format!("{} did not print a package path", script.display()))?;
    if output_path.extension().and_then(OsStr::to_str) != Some(expected_extension) {
        bail!(
            "{} printed {}, expected a .{} artifact",
            script.display(),
            output_path.display(),
            expected_extension
        );
    }
    let dest = staging_dir.join(
        output_path
            .file_name()
            .unwrap_or_else(|| OsStr::new("artifact")),
    );
    fs::copy(&output_path, &dest).with_context(|| {
        format!(
            "failed to copy package {} to {}",
            output_path.display(),
            dest.display()
        )
    })?;
    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn finish_artifact_manifest(
    project: &FissionProject,
    options: &PackageOptions,
    staging_dir: &Path,
    profile: &str,
) -> Result<ArtifactManifest> {
    prepare_package_validation_inputs(options, staging_dir)?;
    let mut manifest = build_artifact_manifest(project, options, staging_dir, profile)?;
    add_configured_secondary_artifacts(&options.project_dir, &mut manifest)?;
    manifest.validation.checks = package_artifact_checks(options, staging_dir, &manifest);
    manifest.validation.state = manifest_validation_state(&manifest.validation.checks).to_string();
    manifest.signing = package_signing_context(
        &options.project_dir,
        options.target,
        options.format,
        options.release,
        options.variant.as_ref(),
        &manifest.validation.checks,
    )?;
    manifest.notarization = package_notarization_context(
        &options.project_dir,
        options.target,
        options.release,
        options.variant.as_ref(),
    )?;
    let manifest_path = staging_dir.join(ARTIFACT_MANIFEST);
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).with_context(|| {
        format!(
            "failed to write artifact manifest {}",
            manifest_path.display()
        )
    })?;
    Ok(manifest)
}

fn add_configured_secondary_artifacts(
    project_dir: &Path,
    manifest: &mut ArtifactManifest,
) -> Result<()> {
    let config = package_manifest(project_dir)?;
    for artifact in configured_secondary_artifacts(&config) {
        let Some(relative_path) = artifact
            .path
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        let source = resolve_project_path(project_dir, relative_path.to_string());
        if !source.exists() {
            bail!(
                "configured secondary artifact {} does not exist",
                source.display()
            );
        }
        let kind = artifact
            .kind
            .clone()
            .unwrap_or_else(|| "secondary_artifact".to_string());
        let purpose = artifact.purpose.clone().or_else(|| Some(kind.clone()));
        collect_secondary_artifacts(
            project_dir,
            &source,
            &source,
            &kind,
            purpose.as_deref(),
            artifact.platform.as_deref(),
            artifact.upload_provider.as_deref(),
            manifest,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_secondary_artifacts(
    project_dir: &Path,
    root: &Path,
    current: &Path,
    kind: &str,
    purpose: Option<&str>,
    platform: Option<&str>,
    upload_provider: Option<&str>,
    manifest: &mut ArtifactManifest,
) -> Result<()> {
    let metadata = fs::metadata(current)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            collect_secondary_artifacts(
                project_dir,
                root,
                &entry.path(),
                kind,
                purpose,
                platform,
                upload_provider,
                manifest,
            )?;
        }
        return Ok(());
    }
    if !metadata.is_file() {
        return Ok(());
    }
    let relative_path = current
        .strip_prefix(project_dir)
        .unwrap_or_else(|_| current.strip_prefix(root).unwrap_or(current))
        .to_string_lossy()
        .replace('\\', "/");
    let (sha256, size_bytes) = hash_file(current)?;
    manifest.artifacts.push(ArtifactFile {
        kind: kind.to_string(),
        purpose: purpose.map(str::to_string),
        platform: platform.map(str::to_string),
        upload_provider: upload_provider.map(str::to_string),
        path: current.display().to_string(),
        relative_path,
        sha256,
        size_bytes,
        mime_type: content_type(current).to_string(),
    });
    Ok(())
}

fn configured_secondary_artifacts(config: &PackageManifest) -> Vec<SecondaryArtifactConfig> {
    let Some(package) = config.package.as_ref() else {
        return Vec::new();
    };
    let mut artifacts = Vec::new();
    artifacts.extend(package.secondary_artifacts.iter().cloned());
    artifacts.extend(package.symbols.iter().cloned().map(|mut item| {
        item.kind.get_or_insert_with(|| "debug_symbols".to_string());
        item
    }));
    artifacts.extend(package.crash_assets.iter().cloned().map(|mut item| {
        item.kind
            .get_or_insert_with(|| "crash_diagnostics".to_string());
        item
    }));
    artifacts
}

fn package_manifest(project_dir: &Path) -> Result<PackageManifest> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn docker_package_config(project_dir: &Path) -> Result<DockerPackageConfig> {
    Ok(package_manifest(project_dir)?
        .package
        .and_then(|package| package.docker)
        .unwrap_or(DockerPackageConfig {
            adapter: None,
            port: None,
            base_image: None,
            tags: None,
            build: None,
        }))
}

fn windows_package_config(project_dir: &Path) -> Result<WindowsPackageConfig> {
    Ok(package_manifest(project_dir)?
        .package
        .and_then(|package| package.windows)
        .unwrap_or_default())
}

fn linux_package_config(project_dir: &Path) -> Result<LinuxPackageConfig> {
    Ok(package_manifest(project_dir)?
        .package
        .and_then(|package| package.linux)
        .unwrap_or_default())
}

fn write_static_package_metadata(project_dir: &Path, staging_dir: &Path) -> Result<()> {
    let fission_toml = project_dir.join("fission.toml");
    let doc = fs::read_to_string(&fission_toml)
        .ok()
        .and_then(|data| toml::from_str::<toml::Value>(&data).ok());
    let site = doc.as_ref().and_then(|doc| doc.get("site"));
    let base_path = site
        .and_then(|site| site.get("base_path"))
        .and_then(toml::Value::as_str)
        .unwrap_or("/");
    let canonical_url = site
        .and_then(|site| site.get("canonical_url"))
        .and_then(toml::Value::as_str);
    let cache_control = site
        .and_then(|site| site.get("cache_control"))
        .and_then(toml::Value::as_str)
        .unwrap_or("public, max-age=31536000, immutable");

    let routes = collect_static_routes(staging_dir, staging_dir)?;
    let assets = collect_static_assets(staging_dir, staging_dir)?;
    let mime_map = assets
        .iter()
        .map(|asset| {
            json!({
                "path": asset,
                "mime_type": content_type(&staging_dir.join(asset))
            })
        })
        .collect::<Vec<_>>();

    fs::write(
        staging_dir.join("fission-route-manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "base_path": base_path,
            "canonical_url": canonical_url,
            "routes": routes
        }))?,
    )?;
    fs::write(
        staging_dir.join("fission-asset-manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "assets": assets
        }))?,
    )?;
    fs::write(
        staging_dir.join("fission-mime-map.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "files": mime_map
        }))?,
    )?;
    fs::write(
        staging_dir.join("fission-cache-policy.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "default": cache_control
        }))?,
    )?;
    write_static_headers(staging_dir, cache_control)?;
    Ok(())
}

fn collect_static_routes(root: &Path, current: &Path) -> Result<Vec<String>> {
    let mut routes = Vec::new();
    collect_static_routes_inner(root, current, &mut routes)?;
    routes.sort();
    routes.dedup();
    Ok(routes)
}

fn collect_static_routes_inner(
    root: &Path,
    current: &Path,
    routes: &mut Vec<String>,
) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_static_routes_inner(root, &path, routes)?;
            continue;
        }
        if path.extension().and_then(OsStr::to_str) != Some("html") {
            continue;
        }
        let relative = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        let route = if relative == "index.html" {
            "/".to_string()
        } else if let Some(prefix) = relative.strip_suffix("/index.html") {
            format!("/{prefix}/")
        } else {
            format!("/{}", relative.trim_end_matches(".html"))
        };
        routes.push(route);
    }
    Ok(())
}

fn collect_static_assets(root: &Path, current: &Path) -> Result<Vec<String>> {
    let mut assets = Vec::new();
    collect_static_assets_inner(root, current, &mut assets)?;
    assets.sort();
    Ok(assets)
}

fn collect_static_assets_inner(
    root: &Path,
    current: &Path,
    assets: &mut Vec<String>,
) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_static_assets_inner(root, &path, assets)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        if !matches!(
            relative.as_str(),
            "fission-route-manifest.json"
                | "fission-asset-manifest.json"
                | "fission-mime-map.json"
                | "fission-cache-policy.json"
        ) {
            assets.push(relative);
        }
    }
    Ok(())
}

fn write_static_headers(staging_dir: &Path, cache_control: &str) -> Result<()> {
    let body = format!(
        r#"/assets/*
  Cache-Control: {cache_control}

/*.wasm
  Content-Type: application/wasm
  Cache-Control: {cache_control}

/*.js
  Content-Type: text/javascript; charset=utf-8
  Cache-Control: {cache_control}

/*.css
  Content-Type: text/css; charset=utf-8
  Cache-Control: {cache_control}
"#
    );
    fs::write(staging_dir.join("_headers"), body)?;
    Ok(())
}

pub(super) fn readiness_secondary_artifacts(project_dir: &Path, checks: &mut Vec<ReadinessCheck>) {
    let Ok(config) = package_manifest(project_dir) else {
        return;
    };
    for artifact in configured_secondary_artifacts(&config) {
        let id = artifact
            .path
            .as_deref()
            .map(sanitize_file_stem)
            .unwrap_or_else(|| "unnamed".to_string());
        let path = artifact
            .path
            .as_ref()
            .map(|path| resolve_project_path(project_dir, path.to_string()));
        checks.push(check(
            format!("release.package.secondary_artifact.{id}.path"),
            CheckSeverity::Error,
            if path.as_ref().is_some_and(|path| path.exists()) {
                CheckStatus::Passed
            } else {
                CheckStatus::Missing
            },
            "configured secondary release artifact exists",
            path.map(|path| path.display().to_string()),
            vec!["Create the configured symbol/diagnostic artifact before packaging or remove the stale package artifact entry."],
        ));
        let kind = artifact.kind.as_deref().unwrap_or("secondary_artifact");
        if matches!(kind, "debug_symbols" | "crash_diagnostics" | "symbols")
            && artifact
                .upload_provider
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_none()
        {
            checks.push(check(
                format!("release.package.secondary_artifact.{id}.upload_provider"),
                CheckSeverity::Warning,
                CheckStatus::Warning,
                "debug/crash artifact has an upload provider",
                Some(kind.to_string()),
                vec!["Set upload_provider when symbols must be sent to a store or crash diagnostics backend."],
            ));
        }
    }
}

fn ensure_package_target(
    options: &PackageOptions,
    expected_target: Target,
    expected_format: PackageFormat,
) -> Result<()> {
    if options.target != expected_target || options.format != expected_format {
        bail!(
            "--target {} --format {} is required for this package path",
            expected_target.as_str(),
            expected_format.as_str()
        );
    }
    let project = read_project_config(&options.project_dir)?;
    if !project.targets.contains(&options.target) {
        bail!(
            "target `{}` is not configured for this app; run `fission add-target {} --project-dir {}`",
            options.target.as_str(),
            options.target.as_str(),
            options.project_dir.display()
        );
    }
    Ok(())
}

fn profile_name(release: bool) -> &'static str {
    if release {
        "release"
    } else {
        "debug"
    }
}

fn clean_package_dir(options: &PackageOptions) -> Result<PathBuf> {
    let staging_dir = variant_output_path(
        options
            .project_dir
            .join("target/fission")
            .join(profile_name(options.release))
            .join(options.target.as_str())
            .join(options.format.as_str()),
        options.variant.as_ref(),
    );
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)
            .with_context(|| format!("failed to clean {}", staging_dir.display()))?;
    }
    fs::create_dir_all(&staging_dir)
        .with_context(|| format!("failed to create {}", staging_dir.display()))?;
    Ok(staging_dir)
}

fn require_host_os(target: Target) -> Result<()> {
    let ok = match target {
        Target::Linux => cfg!(target_os = "linux"),
        Target::Macos => cfg!(target_os = "macos"),
        Target::Windows => cfg!(target_os = "windows"),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        bail!(
            "{} packaging must run on a {} host for now",
            target.as_str(),
            target.as_str()
        )
    }
}

fn desktop_package_cargo_options(options: &PackageOptions) -> Result<DesktopCargoOptions> {
    read_desktop_cargo_options(
        &options.project_dir,
        options.target,
        options.variant.as_ref().map(NativeVariant::as_str),
    )
}

fn build_desktop_binary(options: &PackageOptions) -> Result<PathBuf> {
    let cargo_options = desktop_package_cargo_options(options)?;
    build_desktop_binary_with_cargo_options(
        &options.project_dir,
        options.release,
        &cargo_options.features,
        cargo_options.no_default_features,
    )
}

fn build_desktop_binary_with_cargo_options(
    project_dir: &Path,
    release: bool,
    cargo_features: &[String],
    cargo_no_default_features: bool,
) -> Result<PathBuf> {
    let project_dir = fs::canonicalize(project_dir).with_context(|| {
        format!(
            "failed to resolve project directory {}",
            project_dir.display()
        )
    })?;
    let manifest_path = project_dir.join("Cargo.toml");
    let name = cargo_package_name(&project_dir).context("Cargo.toml package.name is required")?;
    let mut command = desktop_cargo_build_command(
        &project_dir,
        &manifest_path,
        &name,
        release,
        cargo_features,
        cargo_no_default_features,
    );
    let status = command.status().context("failed to run cargo build")?;
    if !status.success() {
        bail!("desktop build failed with {status}");
    }
    let target_directory = cargo_target_directory(&project_dir, &manifest_path)?;
    let executable = if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name
    };
    let cargo_build_target = env::var_os("CARGO_BUILD_TARGET").filter(|value| !value.is_empty());
    let path = desktop_binary_output_path(
        &target_directory,
        profile_name(release),
        &executable,
        cargo_build_target.as_deref(),
    );
    if !path.exists() {
        bail!("expected built binary at {}", path.display());
    }
    Ok(path)
}

fn desktop_cargo_build_command(
    project_dir: &Path,
    manifest_path: &Path,
    name: &str,
    release: bool,
    cargo_features: &[String],
    cargo_no_default_features: bool,
) -> Command {
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--package")
        .arg(&name)
        .current_dir(&project_dir);
    if release {
        command.arg("--release").arg("--locked");
    }
    if cargo_no_default_features {
        command.arg("--no-default-features");
    }
    if !cargo_features.is_empty() {
        command.arg("--features").arg(cargo_features.join(","));
    }
    command
}

fn desktop_binary_output_path(
    target_directory: &Path,
    profile: &str,
    executable: &str,
    cargo_build_target: Option<&OsStr>,
) -> PathBuf {
    let mut path = target_directory.to_path_buf();
    if let Some(target) = cargo_build_target {
        path.push(target);
    }
    path.join(profile).join(executable)
}

#[derive(Deserialize)]
struct CargoTargetMetadata {
    target_directory: PathBuf,
}

fn cargo_target_directory(project_dir: &Path, manifest_path: &Path) -> Result<PathBuf> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .arg("--manifest-path")
        .arg(manifest_path)
        .current_dir(project_dir)
        .output()
        .context("failed to run cargo metadata for desktop package")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let metadata: CargoTargetMetadata = serde_json::from_slice(&output.stdout)
        .context("failed to parse cargo metadata for desktop package")?;
    Ok(metadata.target_directory)
}

fn create_macos_app_bundle(
    options: &PackageOptions,
    project: &FissionProject,
    staging_dir: &Path,
    macos: &MacosPackageConfig,
) -> Result<PathBuf> {
    let binary = build_desktop_binary_with_cargo_options(
        &options.project_dir,
        options.release,
        &macos.cargo_features,
        macos.cargo_no_default_features,
    )?;
    let executable = binary
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("app")
        .to_string();
    let app_name = display_app_name(&project.app.name);
    let app_bundle = staging_dir.join(format!("{app_name}.app"));
    let contents = app_bundle.join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources)?;
    fs::copy(&binary, macos_dir.join(&executable)).with_context(|| {
        format!(
            "failed to copy {} into {}",
            binary.display(),
            app_bundle.display()
        )
    })?;
    if let Some(icon) = resolve_app_icon(&options.project_dir, Target::Macos)? {
        let extension = normalized_extension(&icon.path)?;
        let destination = resources.join(format!("AppIcon.{extension}"));
        fs::copy(&icon.path, &destination).with_context(|| {
            format!(
                "failed to copy macOS app icon {} to {}",
                icon.path.display(),
                destination.display()
            )
        })?;
    }
    stage_project_assets(&options.project_dir, &resources)?;
    let (version, build) = resolved_macos_bundle_version(&options.project_dir)?;
    let plist = render_info_plist(project, &app_name, &executable, macos, &version, &build);
    fs::write(contents.join("Info.plist"), plist)?;
    fs::write(contents.join("PkgInfo"), "APPL????")?;
    Ok(app_bundle)
}

fn resolved_package_version(project_dir: &Path, target: Target) -> Result<String> {
    let release = resolve_release_version_config(project_dir, Some(target))?;
    Ok(release
        .version
        .or_else(|| cargo_package_version(project_dir))
        .unwrap_or_else(|| "0.1.0".to_string()))
}

fn resolved_macos_bundle_version(project_dir: &Path) -> Result<(String, String)> {
    let release = resolve_release_version_config(project_dir, Some(Target::Macos))?;
    let version = release
        .version
        .or_else(|| cargo_package_version(project_dir))
        .unwrap_or_else(|| "0.1.0".to_string());
    let build = release
        .build
        .map(|value| value.to_string())
        .unwrap_or_else(|| "1".to_string());
    Ok((version, build))
}

fn render_info_plist(
    project: &FissionProject,
    app_name: &str,
    executable: &str,
    macos: &MacosPackageConfig,
    version: &str,
    build: &str,
) -> String {
    let bundle_id = macos
        .bundle_id
        .as_deref()
        .unwrap_or(project.app.app_id.as_str());
    let minimum_os = macos.minimum_os.as_deref().unwrap_or("13.0");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>{}</string>
  <key>CFBundleName</key>
  <string>{}</string>
  <key>CFBundleDisplayName</key>
  <string>{}</string>
  <key>CFBundleExecutable</key>
  <string>{}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleShortVersionString</key>
  <string>{}</string>
  <key>CFBundleVersion</key>
  <string>{}</string>
  <key>LSMinimumSystemVersion</key>
  <string>{}</string>
{}
{}
</dict>
</plist>
"#,
        escape_xml(bundle_id),
        escape_xml(app_name),
        escape_xml(app_name),
        escape_xml(executable),
        escape_xml(version),
        escape_xml(build),
        escape_xml(minimum_os),
        render_macos_application_category_entry(macos),
        render_macos_info_plist_capability_entries(project)
    )
}

pub(super) fn render_macos_application_category_entry(macos: &MacosPackageConfig) -> String {
    macos
        .application_category
        .as_deref()
        .filter(|category| !category.trim().is_empty())
        .map(|category| {
            format!(
                "  <key>LSApplicationCategoryType</key>\n  <string>{}</string>",
                escape_xml(category)
            )
        })
        .unwrap_or_default()
}

fn render_macos_info_plist_capability_entries(project: &FissionProject) -> String {
    let mut out = String::new();
    if project
        .capabilities
        .contains(&PlatformCapability::Bluetooth)
    {
        out.push_str(
            "  <key>NSBluetoothAlwaysUsageDescription</key>\n  <string>This app uses Bluetooth when you request nearby-device features.</string>\n",
        );
    }
    if project.capabilities.contains(&PlatformCapability::Camera)
        || project
            .capabilities
            .contains(&PlatformCapability::BarcodeScanner)
    {
        out.push_str(
            "  <key>NSCameraUsageDescription</key>\n  <string>This app uses the camera when you request camera or barcode features.</string>\n",
        );
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Geolocation)
        || project.capabilities.contains(&PlatformCapability::Wifi)
    {
        out.push_str(
            "  <key>NSLocationWhenInUseUsageDescription</key>\n  <string>This app uses location information when you request location-aware or Wi-Fi features.</string>\n",
        );
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Microphone)
    {
        out.push_str(
            "  <key>NSMicrophoneUsageDescription</key>\n  <string>This app uses the microphone when you request audio capture.</string>\n",
        );
    }
    out
}

fn pkgbuild_signing_args(macos: &MacosPackageConfig) -> Vec<String> {
    macos
        .installer_identity
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|identity| vec!["--sign".to_string(), identity.to_string()])
        .unwrap_or_default()
}

pub(super) fn macos_pkg_builder_command(
    app_bundle: &Path,
    pkg_path: &Path,
    component_plist: Option<&Path>,
    macos: &MacosPackageConfig,
) -> Result<(&'static str, Vec<OsString>)> {
    match macos.pkg_builder.as_deref().unwrap_or("pkgbuild") {
        "pkgbuild" => {
            let component_root = app_bundle
                .parent()
                .context("macOS app bundle must have a component root")?;
            let component_plist =
                component_plist.context("pkgbuild requires a macOS component property list")?;
            let mut args = vec![
                OsString::from("--root"),
                component_root.as_os_str().to_owned(),
                OsString::from("--install-location"),
                OsString::from("/Applications"),
                OsString::from("--component-plist"),
                component_plist.as_os_str().to_owned(),
            ];
            args.extend(pkgbuild_signing_args(macos).into_iter().map(OsString::from));
            args.push(pkg_path.as_os_str().to_owned());
            Ok(("pkgbuild", args))
        }
        "productbuild" => {
            let mut args = Vec::new();
            if let Some(identity) = macos.installer_identity.as_deref() {
                args.push(OsString::from("--sign"));
                args.push(OsString::from(identity));
            }
            args.extend([
                OsString::from("--component"),
                app_bundle.as_os_str().to_owned(),
                OsString::from("/Applications"),
                pkg_path.as_os_str().to_owned(),
            ]);
            Ok(("productbuild", args))
        }
        other => {
            bail!("package.macos pkg_builder must be `pkgbuild` or `productbuild`, got `{other}`")
        }
    }
}

pub(super) fn write_macos_component_plist(
    staging_dir: &Path,
    app_bundle: &Path,
) -> Result<PathBuf> {
    let bundle_name = app_bundle
        .file_name()
        .and_then(OsStr::to_str)
        .context("macOS app bundle name must be valid UTF-8")?;
    let component_plist = staging_dir.join("components.plist");
    let contents = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<array>
  <dict>
    <key>RootRelativeBundlePath</key>
    <string>{}</string>
    <key>BundleIsRelocatable</key>
    <false/>
    <key>BundleIsVersionChecked</key>
    <false/>
    <key>BundleHasStrictIdentifier</key>
    <true/>
    <key>BundleOverwriteAction</key>
    <string>upgrade</string>
  </dict>
</array>
</plist>
"#,
        escape_xml(bundle_name)
    );
    fs::write(&component_plist, contents).with_context(|| {
        format!(
            "failed to write macOS component property list {}",
            component_plist.display()
        )
    })?;
    Ok(component_plist)
}

fn write_linux_run(
    payload_dir: &Path,
    run_path: &Path,
    app_name: &str,
    executable_name: &str,
) -> Result<()> {
    let mut archive = Vec::new();
    {
        let encoder = GzEncoder::new(&mut archive, Compression::default());
        let mut tar = TarBuilder::new(encoder);
        tar.append_dir_all(".", payload_dir)?;
        let encoder = tar.into_inner()?;
        encoder.finish()?;
    }
    let mut file = fs::File::create(run_path)?;
    writeln!(
        file,
        r#"#!/bin/sh
set -eu
APP_NAME="{app_name}"
EXECUTABLE="{executable_name}"
DEST="${{FISSION_INSTALL_DIR:-$HOME/.local/opt/$APP_NAME}}"
if [ -z "$DEST" ] || [ "$DEST" = "/" ]; then
  echo "Refusing unsafe install destination: $DEST" >&2
  exit 64
fi
ARCHIVE_LINE=$(awk '/^__FISSION_ARCHIVE_BELOW__$/ {{ print NR + 1; exit 0; }}' "$0")
MODE="${{1:---install}}"
case "$MODE" in
  --verify)
    tail -n +"$ARCHIVE_LINE" "$0" | tar -tz >/dev/null
    echo "Verified $APP_NAME package archive"
    exit 0
    ;;
  --install|install)
    mkdir -p "$DEST"
    tail -n +"$ARCHIVE_LINE" "$0" | tar -xz -C "$DEST"
    chmod +x "$DEST/$EXECUTABLE" 2>/dev/null || true
    printf 'app=%s\nexecutable=%s\n' "$APP_NAME" "$EXECUTABLE" > "$DEST/.fission-install-receipt"
    echo "Installed $APP_NAME to $DEST"
    echo "Run: $DEST/$EXECUTABLE"
    exit 0
    ;;
  --uninstall|uninstall)
    if [ -d "$DEST" ]; then
      rm -rf "$DEST"
      echo "Uninstalled $APP_NAME from $DEST"
    else
      echo "Nothing to uninstall at $DEST"
    fi
    exit 0
    ;;
  --help|-h)
    echo "Usage: $0 [--install|--verify|--uninstall]"
    exit 0
    ;;
  *)
    echo "Unknown option: $MODE" >&2
    echo "Usage: $0 [--install|--verify|--uninstall]" >&2
    exit 64
    ;;
esac
__FISSION_ARCHIVE_BELOW__"#
    )?;
    file.write_all(&archive)?;
    set_executable(run_path)?;
    Ok(())
}

fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(perms.mode() | 0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[derive(Serialize)]
struct LinuxNativeProductsManifest<'a> {
    schema_version: u32,
    profile: &'a str,
    package_format: &'a str,
    products: Vec<LinuxNativeProductReceipt<'a>>,
}

#[derive(Serialize)]
struct LinuxNativeProductReceipt<'a> {
    module: &'a str,
    name: &'a str,
    kind: NativeLinuxProductKind,
    destination: String,
    sha256: String,
    size_bytes: u64,
}

fn write_linux_native_products_manifest(
    payload_dir: &Path,
    products: &[BuiltLinuxNativeProduct],
    profile: &str,
    package_format: &str,
) -> Result<PathBuf> {
    let products = products
        .iter()
        .map(|product| {
            let staged = payload_dir.join(&product.destination);
            let (sha256, size_bytes) = hash_native_product(&staged)?;
            Ok(LinuxNativeProductReceipt {
                module: &product.module,
                name: &product.name,
                kind: product.kind,
                destination: product.destination.to_string_lossy().replace('\\', "/"),
                sha256,
                size_bytes,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let path = payload_dir.join(".fission/native/linux-products.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let manifest = LinuxNativeProductsManifest {
        schema_version: 1,
        profile,
        package_format,
        products,
    };
    fs::write(&path, serde_json::to_vec_pretty(&manifest)?).with_context(|| {
        format!(
            "failed to write Linux native product manifest {}",
            path.display()
        )
    })?;
    Ok(path)
}

fn hash_native_product(path: &Path) -> Result<(String, u64)> {
    if path.is_file() {
        return hash_file(path);
    }
    if !path.is_dir() {
        bail!(
            "Linux native product is not a file or directory: {}",
            path.display()
        );
    }
    let mut files = Vec::new();
    collect_native_product_files(path, path, &mut files)?;
    files.sort();
    let mut digest = Sha256::new();
    let mut total_size = 0_u64;
    for file in files {
        let relative = file
            .strip_prefix(path)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let (file_sha256, size_bytes) = hash_file(&file)?;
        digest.update((relative.len() as u64).to_le_bytes());
        digest.update(relative.as_bytes());
        digest.update(file_sha256.as_bytes());
        digest.update(size_bytes.to_le_bytes());
        total_size = total_size.saturating_add(size_bytes);
    }
    Ok((format!("{:x}", digest.finalize()), total_size))
}

fn collect_native_product_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to inspect Linux native product {}", root.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_native_product_files(root, &entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        } else {
            bail!(
                "Linux native product contains an unsupported filesystem entry: {}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct WindowsNativeProductsManifest<'a> {
    schema_version: u32,
    profile: &'a str,
    package_format: &'a str,
    products: Vec<&'a BuiltWindowsNativeProduct>,
}

fn write_windows_native_products_manifest(
    project_dir: &Path,
    products: &[BuiltWindowsNativeProduct],
    include_driver_packages: bool,
    profile: &str,
    package_format: &str,
    variant: Option<&fission_command_core::NativeVariant>,
) -> Result<PathBuf> {
    let products = products
        .iter()
        .filter(|product| {
            include_driver_packages || product.kind != NativeWindowsProductKind::DriverPackage
        })
        .collect::<Vec<_>>();
    let directory = variant_output_path(
        project_dir.join(".fission/native/windows/manifests"),
        variant,
    );
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{profile}-{package_format}.json"));
    let manifest = WindowsNativeProductsManifest {
        schema_version: 1,
        profile,
        package_format,
        products,
    };
    fs::write(&path, serde_json::to_vec_pretty(&manifest)?).with_context(|| {
        format!(
            "failed to write Windows native product manifest {}",
            path.display()
        )
    })?;
    Ok(path)
}

fn windows_packaging_environment(
    project_dir: &Path,
    binary: &Path,
    manifest: &Path,
) -> Result<Vec<(OsString, PathBuf)>> {
    let mut environment = vec![
        (OsString::from("WINDOWS_BINARY"), binary.to_path_buf()),
        (
            OsString::from("FISSION_WINDOWS_NATIVE_PRODUCTS_MANIFEST"),
            manifest.to_path_buf(),
        ),
    ];
    let assets = project_dir.join("assets");
    if assets.exists() && !assets.is_dir() {
        bail!(
            "project assets path {} must be a directory",
            assets.display()
        );
    }
    if assets.exists() {
        environment.push((OsString::from("FISSION_WINDOWS_ASSETS_DIR"), assets));
    }
    Ok(environment)
}

fn linux_packaging_environment(
    payload_dir: &Path,
    binary: &Path,
    manifest: &Path,
) -> Vec<(OsString, PathBuf)> {
    vec![
        (
            OsString::from("FISSION_LINUX_PAYLOAD_DIR"),
            payload_dir.to_path_buf(),
        ),
        (OsString::from("LINUX_BINARY"), binary.to_path_buf()),
        (
            OsString::from("FISSION_LINUX_NATIVE_PRODUCTS_MANIFEST"),
            manifest.to_path_buf(),
        ),
    ]
}

fn run_packaging_script(
    project_dir: &Path,
    script: &Path,
    release: bool,
    variant: Option<&fission_command_core::NativeVariant>,
) -> Result<Option<PathBuf>> {
    run_packaging_script_with_env(project_dir, script, release, variant, &[])
}

fn run_packaging_script_with_env(
    project_dir: &Path,
    script: &Path,
    release: bool,
    variant: Option<&fission_command_core::NativeVariant>,
    environment: &[(OsString, PathBuf)],
) -> Result<Option<PathBuf>> {
    if !script.exists() {
        bail!("packaging script is missing at {}", script.display());
    }
    let working_dir = absolute_path(project_dir)?;
    let script = absolute_path(script)?;
    let extension = script.extension().and_then(OsStr::to_str);
    let mut command = if extension == Some("ps1") {
        let program = if cfg!(windows) {
            "powershell"
        } else if find_in_path("pwsh").is_some() {
            "pwsh"
        } else {
            bail!(
                "{} requires PowerShell; install pwsh or run this package format on Windows",
                script.display()
            );
        };
        let mut command = Command::new(program);
        if cfg!(windows) {
            command.args(["-ExecutionPolicy", "Bypass", "-File"]);
        } else {
            command.arg("-File");
        }
        command.arg(&script);
        command
    } else if cfg!(windows) || extension == Some("sh") {
        let mut command = Command::new("bash");
        command.arg(&script);
        command
    } else {
        Command::new(&script)
    };
    command.current_dir(&working_dir);
    command.env_remove("FISSION_VARIANT");
    if let Some(variant) = variant {
        command.env("FISSION_VARIANT", variant.as_str());
    }
    if release {
        command.env("ANDROID_PROFILE", "release");
        command.env("IOS_PROFILE", "release");
        command.env("LINUX_PROFILE", "release");
        command.env("WINDOWS_PROFILE", "release");
    }
    for (key, path) in environment {
        command.env(key, absolute_path(path)?);
    }
    let output = command
        .output()
        .with_context(|| format!("failed to run {}", script.display()))?;
    io::stderr().write_all(&output.stderr).ok();
    if !output.status.success() {
        bail!("{} failed with {}", script.display(), output.status);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .rev()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let path = PathBuf::from(line);
            if path.is_absolute() {
                path
            } else {
                working_dir.join(path)
            }
        })
        .find(|path| path.exists()))
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(env::current_dir()
        .context("failed to resolve the current directory")?
        .join(path))
}

fn sanitize_file_stem(value: &str) -> String {
    let stem = value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '-',
        })
        .collect::<String>()
        .trim_matches(['-', '.', '_'])
        .to_string();
    if stem.is_empty() {
        "app".to_string()
    } else {
        stem
    }
}

fn display_app_name(value: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in value.chars() {
        match ch {
            '-' | '_' | '.' | ' ' => {
                if !out.ends_with(' ') && !out.is_empty() {
                    out.push(' ');
                }
                uppercase_next = true;
            }
            _ if uppercase_next => {
                out.extend(ch.to_uppercase());
                uppercase_next = false;
            }
            _ => out.push(ch),
        }
    }
    if out.trim().is_empty() {
        "Fission App".to_string()
    } else {
        out.trim().to_string()
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
#[path = "package_tests.rs"]
mod tests;
