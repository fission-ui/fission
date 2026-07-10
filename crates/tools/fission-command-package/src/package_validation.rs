use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct InstallSmokeReceipt {
    target: Option<String>,
    format: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct InstallSmokeCheckOutcome {
    id: String,
    status: String,
    details: Option<String>,
}

pub(super) fn prepare_package_validation_inputs(
    options: &PackageOptions,
    staging_dir: &Path,
) -> Result<()> {
    if options.format == PackageFormat::Static {
        write_static_load_smoke_receipt(options.target, options.format, staging_dir)?;
    }
    if options.format == PackageFormat::Run
        && matches!(options.target, Target::Linux | Target::Terminal)
    {
        write_run_install_smoke_receipt(options.target, options.format, staging_dir)?;
    }
    Ok(())
}

fn write_static_load_smoke_receipt(
    target: Target,
    format: PackageFormat,
    staging_dir: &Path,
) -> Result<()> {
    let validation_dir = staging_dir.join("package-validation");
    fs::create_dir_all(&validation_dir)?;
    let receipt_path = validation_dir.join("load-smoke.json");
    let index = staging_dir.join("index.html");
    let mut outcomes = Vec::new();
    let index_exists = index.is_file();
    outcomes.push(InstallSmokeCheckOutcome {
        id: "static.index_present".to_string(),
        status: if index_exists { "passed" } else { "failed" }.to_string(),
        details: Some(index.display().to_string()),
    });
    let html = fs::read_to_string(&index).unwrap_or_default();
    let html_non_empty = !html.trim().is_empty();
    outcomes.push(InstallSmokeCheckOutcome {
        id: "static.index_non_empty".to_string(),
        status: if html_non_empty { "passed" } else { "failed" }.to_string(),
        details: Some(format!("{} bytes", html.len())),
    });
    let has_document =
        html.contains("<html") || html.contains("<!doctype") || html.contains("<body");
    outcomes.push(InstallSmokeCheckOutcome {
        id: "static.index_document_like".to_string(),
        status: if has_document { "passed" } else { "failed" }.to_string(),
        details: Some("expected html/body/doctype marker".to_string()),
    });
    let status = index_exists && html_non_empty && has_document;
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "target": target.as_str(),
            "format": format.as_str(),
            "status": if status { "passed" } else { "failed" },
            "checks": outcomes,
        }))?,
    )
    .with_context(|| format!("failed to write {}", receipt_path.display()))?;
    Ok(())
}

fn write_run_install_smoke_receipt(
    target: Target,
    format: PackageFormat,
    staging_dir: &Path,
) -> Result<()> {
    let validation_dir = staging_dir.join("package-validation");
    fs::create_dir_all(&validation_dir)?;
    let receipt_path = validation_dir.join("install-smoke.json");
    let artifact = primary_child_with_extension(staging_dir, "run");
    let mut outcomes = Vec::new();
    let status = match artifact {
        Some(artifact) => run_embedded_run_smoke(&artifact, &mut outcomes),
        None => {
            outcomes.push(InstallSmokeCheckOutcome {
                id: "artifact.run_present".to_string(),
                status: "failed".to_string(),
                details: Some(format!(
                    "no .run artifact found in {}",
                    staging_dir.display()
                )),
            });
            false
        }
    };
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "target": target.as_str(),
            "format": format.as_str(),
            "status": if status { "passed" } else { "failed" },
            "checks": outcomes,
        }))?,
    )
    .with_context(|| format!("failed to write {}", receipt_path.display()))?;
    Ok(())
}

fn run_embedded_run_smoke(artifact: &Path, outcomes: &mut Vec<InstallSmokeCheckOutcome>) -> bool {
    let verify = Command::new("sh").arg(artifact).arg("--verify").output();
    let verify_ok = command_outcome("artifact.verify", verify, outcomes);

    let install_root = std::env::temp_dir().join(format!(
        "fission-install-smoke-{}-{}",
        std::process::id(),
        now_unix_seconds()
    ));
    let _ = fs::remove_dir_all(&install_root);
    let install = Command::new("sh")
        .arg(artifact)
        .arg("--install")
        .env("FISSION_INSTALL_DIR", &install_root)
        .output();
    let install_ok = command_outcome("artifact.install", install, outcomes);
    let receipt_ok = install_root.join(".fission-install-receipt").is_file();
    outcomes.push(InstallSmokeCheckOutcome {
        id: "artifact.install_receipt".to_string(),
        status: if receipt_ok { "passed" } else { "failed" }.to_string(),
        details: Some(install_root.display().to_string()),
    });

    let uninstall = Command::new("sh")
        .arg(artifact)
        .arg("--uninstall")
        .env("FISSION_INSTALL_DIR", &install_root)
        .output();
    let uninstall_ok = command_outcome("artifact.uninstall", uninstall, outcomes);
    let removed_ok = !install_root.exists();
    outcomes.push(InstallSmokeCheckOutcome {
        id: "artifact.uninstall_removed".to_string(),
        status: if removed_ok { "passed" } else { "failed" }.to_string(),
        details: Some(install_root.display().to_string()),
    });
    let _ = fs::remove_dir_all(&install_root);

    verify_ok && install_ok && receipt_ok && uninstall_ok && removed_ok
}

fn command_outcome(
    id: &str,
    output: std::io::Result<std::process::Output>,
    outcomes: &mut Vec<InstallSmokeCheckOutcome>,
) -> bool {
    match output {
        Ok(output) => {
            let ok = output.status.success();
            let detail = format!(
                "status={} stdout={} stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
            outcomes.push(InstallSmokeCheckOutcome {
                id: id.to_string(),
                status: if ok { "passed" } else { "failed" }.to_string(),
                details: Some(detail),
            });
            ok
        }
        Err(error) => {
            outcomes.push(InstallSmokeCheckOutcome {
                id: id.to_string(),
                status: "failed".to_string(),
                details: Some(error.to_string()),
            });
            false
        }
    }
}

pub(super) fn package_artifact_checks(
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
    checks.extend(package_structure_checks(
        options.format,
        staging_dir,
        manifest,
    ));
    checks.push(package_artifact_bytes_check(manifest));
    checks.push(package_icon_manifest_check(options.target, manifest));
    checks.extend(package_signature_checks(options, staging_dir, manifest));
    if options.format == PackageFormat::Static {
        checks.push(package_static_load_smoke_check(
            options.target,
            options.format,
            staging_dir,
        ));
    } else {
        checks.push(package_install_smoke_check(
            options.target,
            options.format,
            staging_dir,
        ));
    }
    checks
}

fn package_structure_checks(
    format: PackageFormat,
    staging_dir: &Path,
    manifest: &ArtifactManifest,
) -> Vec<ReadinessCheck> {
    match format {
        PackageFormat::App => vec![macos_app_structure_check(staging_dir)],
        PackageFormat::Apk => vec![zip_structure_check(
            "release.package.structure.android_apk",
            primary_file_with_extension(manifest, "apk"),
            &["AndroidManifest.xml"],
            &[],
            "Android APK contains required package entries",
            "Regenerate the APK; a valid APK must contain AndroidManifest.xml.",
        )],
        PackageFormat::Aab => vec![zip_structure_check(
            "release.package.structure.android_aab",
            primary_file_with_extension(manifest, "aab"),
            &["BundleConfig.pb", "base/manifest/AndroidManifest.xml"],
            &[],
            "Android App Bundle contains required bundle entries",
            "Regenerate the AAB; a valid bundle must contain BundleConfig.pb and base/manifest/AndroidManifest.xml.",
        )],
        PackageFormat::Ipa => vec![zip_structure_check(
            "release.package.structure.ios_ipa",
            primary_file_with_extension(manifest, "ipa"),
            &[],
            &[("Payload/", ".app/Info.plist")],
            "iOS IPA contains a Payload app bundle and Info.plist",
            "Regenerate the IPA; a valid IPA must contain Payload/<App>.app/Info.plist.",
        )],
        PackageFormat::Msix => vec![zip_structure_check(
            "release.package.structure.windows_msix",
            primary_file_with_extension(manifest, "msix"),
            &["AppxManifest.xml"],
            &[],
            "Windows MSIX contains AppxManifest.xml",
            "Regenerate the MSIX; a valid MSIX must contain AppxManifest.xml.",
        )],
        _ => Vec::new(),
    }
}

fn macos_app_structure_check(staging_dir: &Path) -> ReadinessCheck {
    let Some(app) = primary_child_with_extension(staging_dir, "app") else {
        return check(
            "release.package.structure.macos_app",
            CheckSeverity::Error,
            CheckStatus::Missing,
            "macOS .app bundle contains required bundle entries",
            Some(staging_dir.display().to_string()),
            vec!["Regenerate the .app bundle; the package root must contain <Name>.app."],
        );
    };
    let info = app.join("Contents/Info.plist");
    let macos = app.join("Contents/MacOS");
    let passed = info.is_file() && macos.is_dir();
    check(
        "release.package.structure.macos_app",
        CheckSeverity::Error,
        if passed {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "macOS .app bundle contains required bundle entries",
        Some(format!(
            "Info.plist={} Contents/MacOS={}",
            info.is_file(),
            macos.is_dir()
        )),
        vec!["Regenerate the .app bundle; it must contain Contents/Info.plist and Contents/MacOS."],
    )
}

fn zip_structure_check(
    id: &str,
    path: Option<PathBuf>,
    required_exact: &[&str],
    required_patterns: &[(&str, &str)],
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    let Some(path) = path else {
        return check(
            id,
            CheckSeverity::Error,
            CheckStatus::Missing,
            summary,
            Some("primary artifact was not found".to_string()),
            vec![remediation],
        );
    };
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) => {
            return check(
                id,
                CheckSeverity::Error,
                CheckStatus::Failed,
                summary,
                Some(format!("{}: {error}", path.display())),
                vec![remediation],
            );
        }
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(error) => {
            return check(
                id,
                CheckSeverity::Error,
                CheckStatus::Failed,
                summary,
                Some(format!(
                    "{} is not a valid zip package: {error}",
                    path.display()
                )),
                vec![remediation],
            );
        }
    };
    let names = (0..archive.len())
        .filter_map(|index| {
            archive
                .by_index(index)
                .ok()
                .map(|file| file.name().to_string())
        })
        .collect::<Vec<_>>();
    let missing_exact = required_exact
        .iter()
        .filter(|required| !names.iter().any(|name| name == **required))
        .copied()
        .collect::<Vec<_>>();
    let missing_patterns = required_patterns
        .iter()
        .filter(|(prefix, suffix)| {
            !names
                .iter()
                .any(|name| name.starts_with(*prefix) && name.ends_with(*suffix))
        })
        .map(|(prefix, suffix)| format!("{prefix}*{suffix}"))
        .collect::<Vec<_>>();
    let mut missing = missing_exact
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    missing.extend(missing_patterns);
    check(
        id,
        CheckSeverity::Error,
        if missing.is_empty() {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        summary,
        Some(if missing.is_empty() {
            format!("{} entries checked", names.len())
        } else {
            format!("missing {}", missing.join(", "))
        }),
        vec![remediation],
    )
}

fn package_primary_artifact_check(
    format: PackageFormat,
    staging_dir: &Path,
    manifest: &ArtifactManifest,
) -> ReadinessCheck {
    let found = match format {
        PackageFormat::Static => staging_dir.join("index.html").exists(),
        PackageFormat::DockerImage => {
            staging_dir.join("Dockerfile").exists()
                && staging_dir.join("image-metadata.json").exists()
        }
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

fn package_icon_manifest_check(target: Target, manifest: &ArtifactManifest) -> ReadinessCheck {
    if !target_uses_application_icon(target) {
        return check(
            "release.package.icons.not_required",
            CheckSeverity::Info,
            CheckStatus::Passed,
            "application icon manifest is not required for this package target",
            Some(target.as_str().to_string()),
            Vec::new(),
        );
    }
    match &manifest.icon_manifest {
        Some(icon_manifest) => check(
            "release.package.icons.manifest_recorded",
            CheckSeverity::Error,
            CheckStatus::Passed,
            "application icon manifest is recorded in the artifact manifest",
            Some(format!(
                "{} ({} output(s))",
                icon_manifest.path, icon_manifest.outputs
            )),
            vec!["Keep icon generation deterministic before distributing the artifact."],
        ),
        None => check(
            "release.package.icons.manifest_recorded",
            CheckSeverity::Error,
            CheckStatus::Missing,
            "application icon manifest is recorded in the artifact manifest",
            Some(target.as_str().to_string()),
            vec![
                "Configure an application icon or restore assets/app-icon.png so package output records the icon source and generated package paths.",
            ],
        ),
    }
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

pub(super) fn package_install_smoke_check(
    target: Target,
    format: PackageFormat,
    staging_dir: &Path,
) -> ReadinessCheck {
    if matches!(format, PackageFormat::Static | PackageFormat::DockerImage) {
        return check(
            "release.package.install_smoke.not_required",
            CheckSeverity::Info,
            CheckStatus::Passed,
            "install smoke receipt is not required for this package format",
            Some(staging_dir.display().to_string()),
            Vec::new(),
        );
    }
    let candidates = [
        staging_dir.join("install-smoke.json"),
        staging_dir.join("package-validation/install-smoke.json"),
    ];
    let receipt = candidates.iter().find(|path| path.exists());
    let Some(receipt_path) = receipt else {
        return check(
            "release.package.install_smoke.receipt",
            CheckSeverity::Warning,
            CheckStatus::Skipped,
            "package install smoke receipt exists",
            Some(staging_dir.display().to_string()),
            vec!["Run the platform install/smoke workflow and write install-smoke.json next to the artifact before release distribution."],
        );
    };

    let outcome = fs::read_to_string(receipt_path)
        .with_context(|| format!("failed to read {}", receipt_path.display()))
        .and_then(|data| {
            serde_json::from_str::<InstallSmokeReceipt>(&data)
                .with_context(|| format!("failed to parse {}", receipt_path.display()))
        });
    let receipt = match outcome {
        Ok(receipt) => receipt,
        Err(error) => {
            return check(
                "release.package.install_smoke.receipt",
                CheckSeverity::Warning,
                CheckStatus::Failed,
                "package install smoke receipt is valid",
                Some(error.to_string()),
                vec!["Re-run the platform install/smoke workflow and write a valid install-smoke.json receipt."],
            );
        }
    };

    let expected_target = target.as_str();
    let expected_format = format.as_str();
    let target_ok = receipt.target.as_deref() == Some(expected_target);
    let format_ok = receipt.format.as_deref() == Some(expected_format);
    let status = receipt.status.as_deref().unwrap_or("missing");
    let status_ok = matches!(status, "passed" | "ok" | "success");
    let passed = target_ok && format_ok && status_ok;
    check(
        "release.package.install_smoke.receipt",
        CheckSeverity::Warning,
        if passed {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "package install smoke receipt proves target and format passed",
        Some(format!(
            "{} target={} format={} status={}",
            receipt_path.display(),
            receipt.target.as_deref().unwrap_or("missing"),
            receipt.format.as_deref().unwrap_or("missing"),
            status
        )),
        vec!["Re-run the platform install/smoke workflow for this exact target and format before release distribution."],
    )
}

fn package_static_load_smoke_check(
    target: Target,
    format: PackageFormat,
    staging_dir: &Path,
) -> ReadinessCheck {
    let receipt_path = staging_dir.join("package-validation/load-smoke.json");
    if !receipt_path.exists() {
        return check(
            "release.package.static_load_smoke.receipt",
            CheckSeverity::Warning,
            CheckStatus::Skipped,
            "static package load-smoke receipt exists",
            Some(staging_dir.display().to_string()),
            vec!["Re-run static packaging so Fission writes package-validation/load-smoke.json."],
        );
    }
    let outcome = fs::read_to_string(&receipt_path)
        .with_context(|| format!("failed to read {}", receipt_path.display()))
        .and_then(|data| {
            serde_json::from_str::<InstallSmokeReceipt>(&data)
                .with_context(|| format!("failed to parse {}", receipt_path.display()))
        });
    let receipt = match outcome {
        Ok(receipt) => receipt,
        Err(error) => {
            return check(
                "release.package.static_load_smoke.receipt",
                CheckSeverity::Warning,
                CheckStatus::Failed,
                "static package load-smoke receipt is valid",
                Some(error.to_string()),
                vec!["Re-run static packaging so Fission writes a valid load-smoke receipt."],
            );
        }
    };
    let passed = receipt.target.as_deref() == Some(target.as_str())
        && receipt.format.as_deref() == Some(format.as_str())
        && matches!(
            receipt.status.as_deref().unwrap_or("missing"),
            "passed" | "ok" | "success"
        );
    check(
        "release.package.static_load_smoke.receipt",
        CheckSeverity::Warning,
        if passed {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "static package root passed deterministic load-smoke checks",
        Some(format!(
            "{} target={} format={} status={}",
            receipt_path.display(),
            receipt.target.as_deref().unwrap_or("missing"),
            receipt.format.as_deref().unwrap_or("missing"),
            receipt.status.as_deref().unwrap_or("missing")
        )),
        vec![
            "Re-run static packaging and inspect index.html if the static load-smoke check fails.",
        ],
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

pub(super) fn manifest_validation_state(checks: &[ReadinessCheck]) -> &'static str {
    if checks
        .iter()
        .any(|check| check.severity == CheckSeverity::Error && check.status != CheckStatus::Passed)
    {
        "failed"
    } else if checks
        .iter()
        .any(|check| check.status != CheckStatus::Passed)
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
