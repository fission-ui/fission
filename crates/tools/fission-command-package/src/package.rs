use super::*;
use anyhow::{bail, Context, Result};
use fission_command_core::{cargo_package_name, read_project_config, FissionProject, Target};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Deserialize;
use serde_json::json;
use std::env;
use std::ffi::OsStr;
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
    macos: Option<MacosPackageConfig>,
    #[serde(default)]
    secondary_artifacts: Vec<SecondaryArtifactConfig>,
    #[serde(default)]
    symbols: Vec<SecondaryArtifactConfig>,
    #[serde(default)]
    crash_assets: Vec<SecondaryArtifactConfig>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct SecondaryArtifactConfig {
    kind: Option<String>,
    purpose: Option<String>,
    platform: Option<String>,
    path: Option<String>,
    upload_provider: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MacosPackageConfig {
    bundle_id: Option<String>,
    minimum_os: Option<String>,
    entitlements: Option<String>,
    signing_identity: Option<String>,
    installer_identity: Option<String>,
    notarize: Option<bool>,
}

pub(super) fn package_artifact(options: &PackageOptions) -> Result<ArtifactManifest> {
    match options.format {
        PackageFormat::Static => package_static(options),
        PackageFormat::Run => package_linux_run(options),
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
            "static packaging currently supports site and web targets, not `{}`",
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
    copy_optional_assets(&options.project_dir, &payload_dir)?;

    let package_name = sanitize_file_stem(&project.app.name);
    let run_path = staging_dir.join(format!(
        "{package_name}-{}-{}.run",
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
    let macos = macos_package_config(&options.project_dir)?;
    let app_bundle = create_macos_app_bundle(options, &project, &staging_dir, &macos)?;
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
    let macos = macos_package_config(&options.project_dir)?;
    let app_bundle = create_macos_app_bundle(options, &project, &app_staging, &macos)?;
    sign_macos_app_if_configured(&options.project_dir, &app_bundle, &macos)?;
    let pkg_path = staging_dir.join(format!(
        "{}-{}.pkg",
        sanitize_file_stem(&project.app.name),
        cargo_package_version(&options.project_dir).unwrap_or_else(|| "0.0.0".to_string())
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
    let dest = staging_dir.join(binary.file_name().unwrap_or_else(|| OsStr::new("app.exe")));
    fs::copy(&binary, &dest)
        .with_context(|| format!("failed to copy {} to {}", binary.display(), dest.display()))?;
    copy_optional_assets(&options.project_dir, &staging_dir)?;
    finish_artifact_manifest(&project, options, &staging_dir, profile)
}

fn package_android_apk(options: &PackageOptions) -> Result<ArtifactManifest> {
    ensure_package_target(options, Target::Android, PackageFormat::Apk)?;
    let project = read_project_config(&options.project_dir)?;
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
    let output_path = run_packaging_script(&options.project_dir, &script, options.release)?
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
    let mut manifest = build_artifact_manifest(project, options, staging_dir, profile)?;
    add_configured_secondary_artifacts(&options.project_dir, &mut manifest)?;
    manifest.validation.checks = package_artifact_checks(options, staging_dir, &manifest);
    manifest.validation.state = manifest_validation_state(&manifest.validation.checks).to_string();
    let manifest_path = staging_dir.join(ARTIFACT_MANIFEST);
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).with_context(|| {
        format!(
            "failed to write artifact manifest {}",
            manifest_path.display()
        )
    })?;
    Ok(manifest)
}

fn package_artifact_checks(
    options: &PackageOptions,
    staging_dir: &Path,
    manifest: &ArtifactManifest,
) -> Vec<ReadinessCheck> {
    let mut checks = Vec::new();
    checks.push(package_primary_artifact_check(
        options.format,
        staging_dir,
        manifest,
    ));
    checks.push(package_artifact_bytes_check(manifest));
    checks.extend(package_signature_checks(options, staging_dir, manifest));
    checks.push(package_install_smoke_check(options.format, staging_dir));
    checks
}

fn package_primary_artifact_check(
    format: PackageFormat,
    staging_dir: &Path,
    manifest: &ArtifactManifest,
) -> ReadinessCheck {
    let found = match format {
        PackageFormat::Static => staging_dir.join("index.html").exists(),
        PackageFormat::App => has_child_with_extension(staging_dir, "app"),
        PackageFormat::Run
        | PackageFormat::Pkg
        | PackageFormat::Exe
        | PackageFormat::Apk
        | PackageFormat::Aab
        | PackageFormat::Ipa
        | PackageFormat::Msi
        | PackageFormat::Msix => manifest.artifacts.iter().any(|file| {
            Path::new(&file.path).extension().and_then(OsStr::to_str) == Some(format.as_str())
        }),
    };
    check(
        "release.package.artifact.primary_present",
        CheckSeverity::Error,
        if found {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "primary package artifact exists",
        Some(format!(
            "{} package output in {}",
            format.as_str(),
            staging_dir.display()
        )),
        vec![
            "Re-run the package command and ensure the packager emits the requested artifact type.",
        ],
    )
}

fn package_artifact_bytes_check(manifest: &ArtifactManifest) -> ReadinessCheck {
    let empty = manifest
        .artifacts
        .iter()
        .filter(|file| file.size_bytes == 0)
        .map(|file| file.relative_path.as_str())
        .collect::<Vec<_>>();
    check(
        "release.package.artifact.files_non_empty",
        CheckSeverity::Warning,
        if empty.is_empty() {
            CheckStatus::Passed
        } else {
            CheckStatus::Warning
        },
        "artifact files have non-zero bytes",
        (!empty.is_empty()).then(|| empty.join(", ")),
        vec![
            "Inspect the listed zero-byte files and remove or regenerate them before distribution.",
        ],
    )
}

fn package_signature_checks(
    options: &PackageOptions,
    staging_dir: &Path,
    manifest: &ArtifactManifest,
) -> Vec<ReadinessCheck> {
    match options.format {
        PackageFormat::App => vec![verify_with_tool(
            "release.package.signature.macos_app",
            "codesign",
            &["--verify", "--deep", "--strict"],
            primary_child_with_extension(staging_dir, "app"),
            "macOS .app signature verifies",
            "Sign the .app bundle with package.macos.signing_identity or disable signed distribution for this package.",
        )],
        PackageFormat::Pkg => vec![verify_with_tool(
            "release.package.signature.macos_pkg",
            "pkgutil",
            &["--check-signature"],
            primary_file_with_extension(manifest, "pkg"),
            "macOS .pkg signature verifies",
            "Sign the package with package.macos.installer_identity before distribution.",
        )],
        PackageFormat::Apk => vec![verify_with_tool(
            "release.package.signature.android_apk",
            "apksigner",
            &["verify"],
            primary_file_with_extension(manifest, "apk"),
            "Android APK signature verifies",
            "Configure Android signing and run the platform packager again.",
        )],
        PackageFormat::Aab => vec![verify_with_tool(
            "release.package.signature.android_aab",
            "jarsigner",
            &["-verify"],
            primary_file_with_extension(manifest, "aab"),
            "Android AAB jar signature verifies",
            "Configure Android upload signing and regenerate the AAB.",
        )],
        PackageFormat::Msix => vec![verify_with_tool(
            "release.package.signature.windows_msix",
            "signtool",
            &["verify", "/pa"],
            primary_file_with_extension(manifest, "msix"),
            "Windows MSIX signature verifies",
            "Sign the MSIX with the Windows package certificate before distribution.",
        )],
        PackageFormat::Msi => vec![verify_with_tool(
            "release.package.signature.windows_msi",
            "signtool",
            &["verify", "/pa"],
            primary_file_with_extension(manifest, "msi"),
            "Windows MSI signature verifies",
            "Sign the MSI with the Windows package certificate before distribution.",
        )],
        PackageFormat::Exe => vec![verify_with_tool(
            "release.package.signature.windows_exe",
            "signtool",
            &["verify", "/pa"],
            primary_file_with_extension(manifest, "exe"),
            "Windows executable signature verifies",
            "Sign the executable or installer with the Windows package certificate before distribution.",
        )],
        _ => Vec::new(),
    }
}

fn package_install_smoke_check(format: PackageFormat, staging_dir: &Path) -> ReadinessCheck {
    if matches!(format, PackageFormat::Static) {
        return check(
            "release.package.install_smoke.not_required",
            CheckSeverity::Info,
            CheckStatus::Passed,
            "install smoke receipt is not required for static packages",
            Some(staging_dir.display().to_string()),
            Vec::new(),
        );
    }
    let candidates = [
        staging_dir.join("install-smoke.json"),
        staging_dir.join("package-validation/install-smoke.json"),
    ];
    let receipt = candidates.iter().find(|path| path.exists());
    check(
        "release.package.install_smoke.receipt",
        CheckSeverity::Warning,
        if receipt.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Skipped
        },
        "package install smoke receipt exists",
        receipt
            .map(|path| path.display().to_string())
            .or_else(|| Some(staging_dir.display().to_string())),
        vec!["Run the platform install/smoke workflow and write install-smoke.json next to the artifact before release distribution."],
    )
}

fn verify_with_tool(
    id: &str,
    tool: &str,
    args: &[&str],
    path: Option<PathBuf>,
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    let Some(path) = path else {
        return check(
            id,
            CheckSeverity::Warning,
            CheckStatus::Skipped,
            summary,
            Some("primary artifact was not found".to_string()),
            vec![remediation],
        );
    };
    let Some(tool_path) = find_in_path(tool) else {
        return check(
            id,
            CheckSeverity::Warning,
            CheckStatus::Skipped,
            summary,
            Some(format!("{tool} is not available on PATH")),
            vec![remediation],
        );
    };
    let output = Command::new(&tool_path).args(args).arg(&path).output();
    match output {
        Ok(output) => check(
            id,
            CheckSeverity::Error,
            if output.status.success() {
                CheckStatus::Passed
            } else {
                CheckStatus::Failed
            },
            summary,
            Some(format!(
                "{} {} {}: {}{}",
                tool_path.display(),
                args.join(" "),
                path.display(),
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            vec![remediation],
        ),
        Err(error) => check(
            id,
            CheckSeverity::Warning,
            CheckStatus::Skipped,
            summary,
            Some(error.to_string()),
            vec![remediation],
        ),
    }
}

fn manifest_validation_state(checks: &[ReadinessCheck]) -> &'static str {
    if checks
        .iter()
        .any(|check| check.severity == CheckSeverity::Error && check.status != CheckStatus::Passed)
    {
        "failed"
    } else if checks
        .iter()
        .any(|check| check.status == CheckStatus::Warning || check.status == CheckStatus::Skipped)
    {
        "warning"
    } else {
        "passed"
    }
}

fn has_child_with_extension(root: &Path, extension: &str) -> bool {
    primary_child_with_extension(root, extension).is_some()
}

fn primary_child_with_extension(root: &Path, extension: &str) -> Option<PathBuf> {
    fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(OsStr::to_str) == Some(extension)).then_some(path)
        })
}

fn primary_file_with_extension(manifest: &ArtifactManifest, extension: &str) -> Option<PathBuf> {
    manifest.artifacts.iter().find_map(|file| {
        let path = Path::new(&file.path);
        (path.extension().and_then(OsStr::to_str) == Some(extension)).then(|| path.to_path_buf())
    })
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
    let mut command = Command::new("cargo");
    command.arg("build").current_dir(project_dir);
    if release {
        command.arg("--release");
    }
    let status = command.status().context("failed to run cargo build")?;
    if !status.success() {
        bail!("desktop build failed with {status}");
    }
    let name = cargo_package_name(project_dir).context("Cargo.toml package.name is required")?;
    let executable = if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name
    };
    let path = project_dir
        .join("target")
        .join(profile_name(release))
        .join(executable);
    if !path.exists() {
        bail!("expected built binary at {}", path.display());
    }
    Ok(path)
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
    if let Some(icon) = app_icon_path(&options.project_dir) {
        let _ = fs::copy(icon, resources.join("AppIcon.png"));
    }
    let version =
        cargo_package_version(&options.project_dir).unwrap_or_else(|| "0.1.0".to_string());
    let plist = render_info_plist(project, &app_name, &executable, macos, &version);
    fs::write(contents.join("Info.plist"), plist)?;
    fs::write(contents.join("PkgInfo"), "APPL????")?;
    Ok(app_bundle)
}

fn render_info_plist(
    project: &FissionProject,
    app_name: &str,
    executable: &str,
    macos: &MacosPackageConfig,
    version: &str,
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
  <key>CFBundleShortVersionString</key>
  <string>{}</string>
  <key>CFBundleVersion</key>
  <string>{}</string>
  <key>LSMinimumSystemVersion</key>
  <string>{}</string>
</dict>
</plist>
"#,
        escape_xml(bundle_id),
        escape_xml(app_name),
        escape_xml(app_name),
        escape_xml(executable),
        escape_xml(version),
        escape_xml(version),
        escape_xml(minimum_os)
    )
}

fn macos_package_config(project_dir: &Path) -> Result<MacosPackageConfig> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: PackageManifest =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(manifest
        .package
        .and_then(|package| package.macos)
        .unwrap_or_default())
}

fn sign_macos_app_if_configured(
    project_dir: &Path,
    app_bundle: &Path,
    macos: &MacosPackageConfig,
) -> Result<()> {
    let Some(identity) = macos
        .signing_identity
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(());
    };
    let mut command = Command::new("codesign");
    command
        .arg("--force")
        .arg("--timestamp")
        .arg("--options")
        .arg("runtime")
        .arg("--sign")
        .arg(identity);
    if let Some(entitlements) = macos
        .entitlements
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        command
            .arg("--entitlements")
            .arg(resolve_project_path(project_dir, entitlements.to_string()));
    }
    let status = command
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
    let key = env::var("APP_STORE_CONNECT_API_KEY_PATH")
        .context("APP_STORE_CONNECT_API_KEY_PATH is required when package.macos.notarize = true")?;
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
            &key,
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
mkdir -p "$DEST"
ARCHIVE_LINE=$(awk '/^__FISSION_ARCHIVE_BELOW__$/ {{ print NR + 1; exit 0; }}' "$0")
tail -n +"$ARCHIVE_LINE" "$0" | tar -xz -C "$DEST"
chmod +x "$DEST/$EXECUTABLE" 2>/dev/null || true
echo "Installed $APP_NAME to $DEST"
echo "Run: $DEST/$EXECUTABLE"
exit 0
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

fn run_packaging_script(
    project_dir: &Path,
    script: &Path,
    release: bool,
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
        command.env("WINDOWS_PROFILE", "release");
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

fn app_icon_path(project_dir: &Path) -> Option<PathBuf> {
    [
        "assets/app-icon.icns",
        "assets/AppIcon.icns",
        "assets/app-icon.png",
        "assets/icon.png",
    ]
    .into_iter()
    .map(|relative| project_dir.join(relative))
    .find(|path| path.exists())
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
