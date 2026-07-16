use super::*;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use fission_command_core::{
    build_linux_native_modules, build_windows_native_modules, cargo_package_name,
    embed_and_sign_macos_native_modules, normalized_extension, read_macos_package_config,
    read_project_config, resolve_app_icon, sign_macos_app_if_configured,
    stage_linux_native_products, stage_windows_runtime_products, sync_platform_config,
    BuiltLinuxNativeProduct, BuiltWindowsNativeProduct, FissionProject, MacosNativeBundleMode,
    MacosPackageConfig, NativeLinuxProductKind, NativeWindowsProductKind, PlatformCapability,
    Target,
};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
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

pub(super) fn package_artifact(options: &PackageOptions) -> Result<ArtifactManifest> {
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
    let binary = build_desktop_binary(&options.project_dir, options.release)?;
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
    let native_products =
        build_linux_native_modules(&options.project_dir, &project, options.release)?;
    stage_linux_native_products(&payload_dir, &native_products)?;
    let native_products_manifest =
        write_linux_native_products_manifest(&payload_dir, &native_products, profile, "run")?;
    copy_optional_assets(&options.project_dir, &payload_dir)?;

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
    let binary = build_desktop_binary(&options.project_dir, options.release)?;
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
    copy_optional_assets(&options.project_dir, &payload_dir)?;

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
    let macos = read_macos_package_config(&options.project_dir)?;
    let app_bundle = create_macos_app_bundle(options, &project, &staging_dir, &macos)?;
    embed_and_sign_macos_native_modules(
        &options.project_dir,
        &app_bundle,
        &project,
        &macos,
        MacosNativeBundleMode::Package,
        options.release,
    )?;
    sign_macos_app_if_configured(&options.project_dir, &app_bundle, &macos)?;
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
    let macos = read_macos_package_config(&options.project_dir)?;
    let app_bundle = create_macos_app_bundle(options, &project, &app_staging, &macos)?;
    embed_and_sign_macos_native_modules(
        &options.project_dir,
        &app_bundle,
        &project,
        &macos,
        MacosNativeBundleMode::Package,
        options.release,
    )?;
    sign_macos_app_if_configured(&options.project_dir, &app_bundle, &macos)?;
    let version = resolved_package_version(&options.project_dir, options.target)?;
    let pkg_path = staging_dir.join(format!(
        "{}-{}.pkg",
        sanitize_file_stem(&project.app.name),
        version
    ));
    if find_in_path("pkgbuild").is_none() {
        bail!("pkgbuild was not found; install Xcode command line tools to create macOS .pkg packages");
    }
    let status = Command::new("pkgbuild")
        .arg("--component")
        .arg(&app_bundle)
        .arg("--install-location")
        .arg("/Applications")
        .args(pkgbuild_signing_args(&macos))
        .arg(&pkg_path)
        .status()
        .context("failed to run pkgbuild")?;
    if !status.success() {
        bail!("pkgbuild failed with {status}");
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
    let binary = build_desktop_binary(&options.project_dir, options.release)?;
    let native_products =
        build_windows_native_modules(&options.project_dir, &project, options.release)?;
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
        )?;
        let environment = windows_packaging_environment(&binary, &manifest);
        let output_path = run_packaging_script_with_env(
            &options.project_dir,
            &script,
            options.release,
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
        copy_optional_assets(&options.project_dir, &staging_dir)?;
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
    let output_path = run_packaging_script(&options.project_dir, &script, options.release)?
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
        let binary = build_desktop_binary(&options.project_dir, options.release)?;
        let native_products =
            build_windows_native_modules(&options.project_dir, &project, options.release)?;
        let include_driver_packages = options.format != PackageFormat::Msix;
        let manifest = write_windows_native_products_manifest(
            &options.project_dir,
            &native_products,
            include_driver_packages,
            profile,
            options.format.as_str(),
        )?;
        environment = windows_packaging_environment(&binary, &manifest);
    }
    let output_path = run_packaging_script_with_env(
        &options.project_dir,
        &script,
        options.release,
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
        &manifest.validation.checks,
    )?;
    manifest.notarization = package_notarization_context(&options.project_dir, options.target)?;
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

fn docker_image_tags(
    options: &PackageOptions,
    project: &FissionProject,
    config: &DockerPackageConfig,
) -> Vec<String> {
    let configured = config
        .tags
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|tag| !tag.trim().is_empty())
        .collect::<Vec<_>>();
    if !configured.is_empty() {
        return configured;
    }
    let version =
        cargo_package_version(&options.project_dir).unwrap_or_else(|| "latest".to_string());
    vec![format!(
        "{}:{}",
        sanitize_docker_image_name(&project.app.name),
        version
    )]
}

fn write_server_docker_context(
    options: &PackageOptions,
    project: &FissionProject,
    config: &DockerPackageConfig,
    staging_dir: &Path,
    tags: &[String],
) -> Result<()> {
    let workspace_root = cargo_workspace_root(&options.project_dir)
        .unwrap_or_else(|| options.project_dir.clone())
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", options.project_dir.display()))?;
    let project_dir = options
        .project_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", options.project_dir.display()))?;
    let project_relative = project_dir
        .strip_prefix(&workspace_root)
        .unwrap_or(Path::new("."))
        .to_string_lossy()
        .replace('\\', "/");
    let package_name =
        cargo_package_name(&options.project_dir).unwrap_or_else(|| project.app.name.clone());
    let binary_name = sanitize_file_stem(&package_name);
    let artifact_args = server_artifact_args(&options.project_dir, options.release)?;
    let context_workspace = staging_dir.join("workspace");
    copy_docker_source_tree(&workspace_root, &context_workspace)?;
    write_dockerfile(
        staging_dir,
        &render_server_dockerfile(
            config.base_image(),
            config.port(),
            &project_relative,
            &package_name,
            &binary_name,
            &artifact_args,
        ),
    )?;
    fs::write(
        staging_dir.join(".dockerignore"),
        "target/\n.git/\n**/.DS_Store\n**/target/\n",
    )?;
    write_docker_context_readme(
        staging_dir,
        options,
        tags,
        "Server image context. The Dockerfile compiles the Fission server app inside a Rust builder stage, then runs the resulting binary in a minimal runtime stage.",
    )
}

fn write_static_site_docker_context(
    options: &PackageOptions,
    _project: &FissionProject,
    config: &DockerPackageConfig,
    staging_dir: &Path,
    tags: &[String],
) -> Result<()> {
    fission_command_site::build(&options.project_dir, options.release)?;
    let source_dir = site_output_dir(&options.project_dir)?;
    if !source_dir.join("index.html").exists() {
        bail!(
            "static site output {} does not contain index.html",
            source_dir.display()
        );
    }
    copy_dir_contents(&source_dir, &staging_dir.join("site"))?;
    write_static_server_crate(staging_dir, config.adapter())?;
    write_dockerfile(
        staging_dir,
        &render_static_site_dockerfile(config.base_image(), config.port()),
    )?;
    fs::write(
        staging_dir.join(".dockerignore"),
        "target/\n.git/\n**/.DS_Store\n",
    )?;
    write_docker_context_readme(
        staging_dir,
        options,
        tags,
        "Static-site image context. The Dockerfile builds a small Rust static-file server and copies the generated site into the runtime image.",
    )
}

fn write_dockerfile(staging_dir: &Path, content: &str) -> Result<()> {
    fs::write(staging_dir.join("Dockerfile"), content).with_context(|| {
        format!(
            "failed to write {}",
            staging_dir.join("Dockerfile").display()
        )
    })
}

fn write_static_server_crate(staging_dir: &Path, adapter: DockerStaticAdapter) -> Result<()> {
    let server_dir = staging_dir.join("server");
    fs::create_dir_all(server_dir.join("src"))?;
    match adapter {
        DockerStaticAdapter::Axum => {
            fs::write(
                server_dir.join("Cargo.toml"),
                r#"[package]
name = "fission-static-server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tower-http = { version = "0.6", features = ["fs"] }
"#,
            )?;
            fs::write(server_dir.join("src/main.rs"), AXUM_STATIC_SERVER)?;
        }
        DockerStaticAdapter::Actix => {
            fs::write(
                server_dir.join("Cargo.toml"),
                r#"[package]
name = "fission-static-server"
version = "0.1.0"
edition = "2021"

[dependencies]
actix-files = "0.6"
actix-web = "4"
"#,
            )?;
            fs::write(server_dir.join("src/main.rs"), ACTIX_STATIC_SERVER)?;
        }
    }
    Ok(())
}

fn render_server_dockerfile(
    base_image: &str,
    port: u16,
    project_relative: &str,
    package_name: &str,
    binary_name: &str,
    artifact_args: &str,
) -> String {
    format!(
        r#"FROM rust:1-bookworm AS builder
WORKDIR /workspace
COPY workspace/ .
WORKDIR /workspace/{project_relative}
RUN rustup target add wasm32-unknown-unknown
RUN cargo build --release --package {package_name} --bin {binary_name}
RUN mkdir -p target/fission/server && cargo run --release --package {package_name} --bin {binary_name} -- artifacts --package-name {package_name}{artifact_args}

FROM {base_image}
RUN useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin fission
WORKDIR /app
COPY --from=builder /workspace/target/release/{binary_name} /usr/local/bin/{binary_name}
COPY --from=builder /workspace/{project_relative}/target/fission/server /app/server-artifacts
COPY --from=builder /workspace/{project_relative}/fission.toml /app/fission.toml
ENV HOST=0.0.0.0
ENV PORT={port}
ENV FISSION_SERVER_ARTIFACTS=/app/server-artifacts
EXPOSE {port}
USER fission
CMD ["sh", "-c", "exec /usr/local/bin/{binary_name} serve --host ${{HOST:-0.0.0.0}} --port ${{PORT:-{port}}}"]
"#
    )
}

fn server_artifact_args(project_dir: &Path, release: bool) -> Result<String> {
    let manifest_path = project_dir.join("Cargo.toml");
    let data = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let value: toml::Value = toml::from_str(&data)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    let has_browser_feature = value
        .get("features")
        .and_then(toml::Value::as_table)
        .is_some_and(|features| features.contains_key("browser"));
    let mut args = Vec::new();
    if release {
        args.push("--release".to_string());
    }
    if has_browser_feature {
        args.push("--package-no-default-features".to_string());
        args.push("--package-feature browser".to_string());
    }
    Ok(if args.is_empty() {
        String::new()
    } else {
        format!(" {}", args.join(" "))
    })
}

fn render_static_site_dockerfile(base_image: &str, port: u16) -> String {
    format!(
        r#"FROM rust:1-bookworm AS builder
WORKDIR /workspace
COPY server/ server/
RUN cargo build --release --manifest-path server/Cargo.toml

FROM {base_image}
RUN useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin fission
WORKDIR /srv/fission-site
COPY site/ /srv/fission-site/
COPY --from=builder /workspace/server/target/release/fission-static-server /usr/local/bin/fission-static-server
ENV HOST=0.0.0.0
ENV PORT={port}
ENV FISSION_STATIC_ROOT=/srv/fission-site
EXPOSE {port}
USER fission
CMD ["sh", "-c", "exec /usr/local/bin/fission-static-server --host ${{HOST:-0.0.0.0}} --port ${{PORT:-{port}}} --root ${{FISSION_STATIC_ROOT:-/srv/fission-site}}"]
"#
    )
}

fn build_docker_image(staging_dir: &Path, tags: &[String]) -> Result<()> {
    if tags.is_empty() {
        bail!("docker-image packaging requires at least one image tag");
    }
    if find_in_path("docker").is_none() {
        bail!("docker was not found on PATH; install Docker or set [package.docker].build = false to generate the image context only");
    }
    let mut command = Command::new("docker");
    command.arg("build");
    for tag in tags {
        command.arg("--tag").arg(tag);
    }
    command.arg(staging_dir);
    let status = command.status().context("failed to run docker build")?;
    if !status.success() {
        bail!("docker build failed with {status}");
    }
    Ok(())
}

fn write_docker_image_metadata(
    options: &PackageOptions,
    project: &FissionProject,
    config: &DockerPackageConfig,
    staging_dir: &Path,
    tags: &[String],
    built: bool,
) -> Result<()> {
    let metadata = json!({
        "schema_version": 1,
        "app_id": project.app.app_id,
        "app_name": project.app.name,
        "target": options.target.as_str(),
        "format": options.format.as_str(),
        "adapter": match config.adapter() {
            DockerStaticAdapter::Actix => "actix",
            DockerStaticAdapter::Axum => "axum",
        },
        "port": config.port(),
        "base_image": config.base_image(),
        "tags": tags,
        "built": built,
    });
    fs::write(
        staging_dir.join("image-metadata.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;
    Ok(())
}

fn write_docker_context_readme(
    staging_dir: &Path,
    options: &PackageOptions,
    tags: &[String],
    description: &str,
) -> Result<()> {
    fs::write(
        staging_dir.join("README.md"),
        format!(
            "# Fission Docker image context\n\n{description}\n\nTarget: `{}`\nFormat: `{}`\nTags: `{}`\n\nBuild manually with:\n\n```sh\ndocker build {}\n```\n",
            options.target.as_str(),
            options.format.as_str(),
            tags.join("`, `"),
            tags.iter()
                .map(|tag| format!("--tag {tag}"))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    )?;
    Ok(())
}

fn copy_docker_source_tree(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if matches!(
            name_str.as_ref(),
            ".git"
                | ".tmp"
                | "target"
                | "dist"
                | "node_modules"
                | "platforms"
                | ".idea"
                | ".vscode"
        ) {
            continue;
        }
        let source_path = entry.path();
        let dest_path = dest.join(&name);
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            let Ok(metadata) = fs::metadata(&source_path) else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
        }
        if file_type.is_dir() {
            copy_docker_source_tree(&source_path, &dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &dest_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn cargo_workspace_root(project_dir: &Path) -> Option<PathBuf> {
    let output = Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .current_dir(project_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let manifest = String::from_utf8_lossy(&output.stdout);
    let manifest = manifest.trim();
    if manifest.is_empty() {
        return None;
    }
    PathBuf::from(manifest).parent().map(Path::to_path_buf)
}

fn sanitize_docker_image_name(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        let valid = matches!(ch, 'a'..='z' | '0'..='9' | '.' | '_' | '-');
        if valid {
            out.push(ch);
            last_dash = ch == '-';
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches(['-', '.', '_']).to_string();
    if out.is_empty() {
        "fission-app".to_string()
    } else {
        out
    }
}

const AXUM_STATIC_SERVER: &str = r#"use axum::Router;
use std::env;
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let mut port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    let mut root = env::var("FISSION_STATIC_ROOT").unwrap_or_else(|_| "/srv/fission-site".to_string());
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => host = args.next().unwrap_or(host),
            "--port" => port = args.next().and_then(|value| value.parse().ok()).unwrap_or(port),
            "--root" => root = args.next().unwrap_or(root),
            _ => {}
        }
    }
    let app = Router::new().fallback_service(ServeDir::new(root).append_index_html_on_directories(true));
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
"#;

const ACTIX_STATIC_SERVER: &str = r#"use actix_files::Files;
use actix_web::{App, HttpServer};
use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let mut host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let mut port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    let mut root = env::var("FISSION_STATIC_ROOT").unwrap_or_else(|_| "/srv/fission-site".to_string());
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => host = args.next().unwrap_or(host),
            "--port" => port = args.next().and_then(|value| value.parse().ok()).unwrap_or(port),
            "--root" => root = args.next().unwrap_or(root),
            _ => {}
        }
    }
    HttpServer::new(move || App::new().service(Files::new("/", root.clone()).index_file("index.html")))
        .bind((host, port))?
        .run()
        .await
}
"#;

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
    let staging_dir = options
        .project_dir
        .join("target/fission")
        .join(profile_name(options.release))
        .join(options.target.as_str())
        .join(options.format.as_str());
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

fn build_desktop_binary(project_dir: &Path, release: bool) -> Result<PathBuf> {
    let project_dir = fs::canonicalize(project_dir).with_context(|| {
        format!(
            "failed to resolve project directory {}",
            project_dir.display()
        )
    })?;
    let manifest_path = project_dir.join("Cargo.toml");
    let name = cargo_package_name(&project_dir).context("Cargo.toml package.name is required")?;
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--package")
        .arg(&name)
        .current_dir(&project_dir);
    if release {
        command.arg("--release");
    }
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
    let path = target_directory
        .join(profile_name(release))
        .join(executable);
    if !path.exists() {
        bail!("expected built binary at {}", path.display());
    }
    Ok(path)
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
    let binary = build_desktop_binary(&options.project_dir, options.release)?;
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
        render_macos_info_plist_capability_entries(project)
    )
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

fn notarize_macos_artifact_if_configured(
    artifact: &Path,
    macos: &MacosPackageConfig,
) -> Result<()> {
    if !macos.notarize.unwrap_or(false) {
        return Ok(());
    }
    let key_file = app_store_connect_key_file_for_notarization()?;
    let key_id = env::var("APP_STORE_CONNECT_KEY_ID")
        .context("APP_STORE_CONNECT_KEY_ID is required when package.macos.notarize = true")?;
    let issuer = env::var("APP_STORE_CONNECT_ISSUER_ID")
        .context("APP_STORE_CONNECT_ISSUER_ID is required when package.macos.notarize = true")?;
    let submit = Command::new("xcrun")
        .args([
            "notarytool",
            "submit",
            artifact.to_string_lossy().as_ref(),
            "--key",
            key_file.path.to_string_lossy().as_ref(),
            "--key-id",
            &key_id,
            "--issuer",
            &issuer,
            "--wait",
        ])
        .status()
        .context("failed to run xcrun notarytool")?;
    if !submit.success() {
        bail!("notarytool submit failed with {submit}");
    }
    let staple = Command::new("xcrun")
        .args(["stapler", "staple"])
        .arg(artifact)
        .status()
        .context("failed to run xcrun stapler")?;
    if !staple.success() {
        bail!("stapler failed with {staple}");
    }
    Ok(())
}

struct TemporarySecretFile {
    path: PathBuf,
    temp_dir: Option<PathBuf>,
}

impl Drop for TemporarySecretFile {
    fn drop(&mut self) {
        if let Some(dir) = self.temp_dir.take() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

fn app_store_connect_key_file_for_notarization() -> Result<TemporarySecretFile> {
    if let Some(path) = env::var_os("APP_STORE_CONNECT_API_KEY_PATH") {
        return Ok(TemporarySecretFile {
            path: PathBuf::from(path),
            temp_dir: None,
        });
    }
    let key_id = env::var("APP_STORE_CONNECT_KEY_ID")
        .context("APP_STORE_CONNECT_KEY_ID is required when package.macos.notarize = true")?;
    let file_name = format!("AuthKey_{key_id}.p8");
    if let Some(raw) = env::var("APP_STORE_CONNECT_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return temporary_notary_secret_file(&file_name, raw.as_bytes());
    } else if let Some(encoded) = env::var("APP_STORE_CONNECT_API_KEY_BASE64")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        let bytes = BASE64_STANDARD
            .decode(encoded.trim())
            .context("failed to decode APP_STORE_CONNECT_API_KEY_BASE64")?;
        return temporary_notary_secret_file(&file_name, &bytes);
    }
    bail!("APP_STORE_CONNECT_API_KEY_PATH, APP_STORE_CONNECT_API_KEY, or APP_STORE_CONNECT_API_KEY_BASE64 is required when package.macos.notarize = true")
}

fn temporary_notary_secret_file(file_name: &str, contents: &[u8]) -> Result<TemporarySecretFile> {
    let temp_dir = env::temp_dir().join(format!(
        "fission-notary-key-{}-{}",
        std::process::id(),
        now_unix_seconds()
    ));
    fs::create_dir_all(&temp_dir).with_context(|| {
        format!(
            "failed to create temporary App Store key directory {}",
            temp_dir.display()
        )
    })?;
    set_private_dir_permissions(&temp_dir)?;
    let path = temp_dir.join(file_name);
    fs::write(&path, contents).with_context(|| {
        format!(
            "failed to write temporary App Store key file {}",
            path.display()
        )
    })?;
    set_private_file_permissions(&path)?;
    Ok(TemporarySecretFile {
        path,
        temp_dir: Some(temp_dir),
    })
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
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

fn copy_optional_assets(project_dir: &Path, dest: &Path) -> Result<()> {
    let assets = project_dir.join("assets");
    if assets.exists() {
        copy_dir_contents(&assets, &dest.join("assets"))?;
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
) -> Result<PathBuf> {
    let products = products
        .iter()
        .filter(|product| {
            include_driver_packages || product.kind != NativeWindowsProductKind::DriverPackage
        })
        .collect::<Vec<_>>();
    let directory = project_dir.join(".fission/native/windows/manifests");
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

fn windows_packaging_environment(binary: &Path, manifest: &Path) -> Vec<(OsString, OsString)> {
    vec![
        (
            OsString::from("WINDOWS_BINARY"),
            binary.as_os_str().to_os_string(),
        ),
        (
            OsString::from("FISSION_WINDOWS_NATIVE_PRODUCTS_MANIFEST"),
            manifest.as_os_str().to_os_string(),
        ),
    ]
}

fn linux_packaging_environment(
    payload_dir: &Path,
    binary: &Path,
    manifest: &Path,
) -> Vec<(OsString, OsString)> {
    vec![
        (
            OsString::from("FISSION_LINUX_PAYLOAD_DIR"),
            payload_dir.as_os_str().to_os_string(),
        ),
        (
            OsString::from("LINUX_BINARY"),
            binary.as_os_str().to_os_string(),
        ),
        (
            OsString::from("FISSION_LINUX_NATIVE_PRODUCTS_MANIFEST"),
            manifest.as_os_str().to_os_string(),
        ),
    ]
}

fn run_packaging_script(
    project_dir: &Path,
    script: &Path,
    release: bool,
) -> Result<Option<PathBuf>> {
    run_packaging_script_with_env(project_dir, script, release, &[])
}

fn run_packaging_script_with_env(
    project_dir: &Path,
    script: &Path,
    release: bool,
    environment: &[(OsString, OsString)],
) -> Result<Option<PathBuf>> {
    if !script.exists() {
        bail!("packaging script is missing at {}", script.display());
    }
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
        command.arg(script);
        command
    } else if cfg!(windows) || extension == Some("sh") {
        let mut command = Command::new("bash");
        command.arg(script);
        command
    } else {
        Command::new(script)
    };
    command.current_dir(project_dir);
    if release {
        command.env("ANDROID_PROFILE", "release");
        command.env("IOS_PROFILE", "release");
        command.env("LINUX_PROFILE", "release");
        command.env("WINDOWS_PROFILE", "release");
    }
    for (key, value) in environment {
        command.env(key, value);
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
                project_dir.join(path)
            }
        })
        .find(|path| path.exists()))
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
