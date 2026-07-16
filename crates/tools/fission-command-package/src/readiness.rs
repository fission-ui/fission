use super::*;

pub(super) fn readiness_package(
    project_dir: &Path,
    target: Option<Target>,
    format: Option<PackageFormat>,
    release: bool,
) -> Result<Vec<ReadinessCheck>> {
    let target = target.unwrap_or(Target::Site);
    let format = format.unwrap_or(PackageFormat::Static);
    let mut checks = Vec::new();
    let format_supported = package_format_supported(target, format);
    checks.push(check(
        "release.package.profile_selected",
        CheckSeverity::Info,
        CheckStatus::Passed,
        "package readiness profile is selected",
        Some(if release { "release" } else { "debug" }.to_string()),
        Vec::new(),
    ));
    checks.push(check(
        "release.package.format_supported",
        CheckSeverity::Error,
        if format_supported {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "package format is supported for the selected target",
        Some(format!(
            "--target {} --format {}",
            target.as_str(),
            format.as_str()
        )),
        vec!["Use a valid target/format pair, such as static-site/static, web/static, static-site/docker-image, ssr/docker-image, linux/run, terminal/run, macos/app, macos/pkg, windows/exe, windows/msi, windows/msix, android/apk, android/aab, or ios/ipa."],
    ));
    checks.push(check_path(
        "release.package.fission_toml_exists",
        project_dir.join("fission.toml"),
        "fission.toml exists",
        "Run `fission init .` or point --project-dir at a Fission project.",
    ));
    checks.push(package_output_location_writable(
        project_dir,
        target,
        format,
        release,
    ));
    checks.push(package_existing_artifact_manifest_current(
        project_dir,
        target,
        format,
        release,
    ));
    if let Ok(project) = fission_command_core::read_project_config(project_dir) {
        checks.push(check(
            "release.package.target_configured",
            CheckSeverity::Error,
            if project.targets.contains(&target) {
                CheckStatus::Passed
            } else {
                CheckStatus::Missing
            },
            "target is configured in fission.toml",
            Some(format!("target = {}", target.as_str())),
            vec!["Run `fission add-target <target> --project-dir .` before packaging."],
        ));
        checks.extend(package_identity_checks(
            project_dir,
            target,
            release,
            &project,
        ));
        if target == Target::Windows
            && project
                .native
                .modules
                .iter()
                .any(|module| !module.windows.is_empty())
        {
            checks.push(check_tool(
                "release.package.windows_msbuild_available",
                "msbuild",
                "Install Visual Studio Build Tools and the required Windows SDK/WDK workloads, then use a Developer Command Prompt.",
            ));
            if project
                .native
                .modules
                .iter()
                .any(|module| module.windows.nuget_packages_config.is_some())
            {
                checks.push(check_tool(
                    "release.package.windows_nuget_available",
                    "nuget",
                    "Install the NuGet CLI so Fission can restore the Windows native module's pinned SDK/WDK packages.",
                ));
            }
        }
    }
    if matches!(target, Target::Site) {
        let has_content = project_dir.join("content").exists();
        let has_entry = load_publish_manifest(project_dir)
            .ok()
            .and_then(|manifest| manifest.site.and_then(|site| site.entry))
            .is_some_and(|entry| !entry.trim().is_empty());
        checks.push(check(
            "release.package.site_content_or_entry",
            CheckSeverity::Error,
            if has_content || has_entry {
                CheckStatus::Passed
            } else {
                CheckStatus::Missing
            },
            "default content directory exists or custom site entry handles routing",
            Some(format!(
                "content: {}, site.entry: {}",
                project_dir.join("content").display(),
                has_entry
            )),
            vec!["Add content/ or configure [site].entry for a custom static site."],
        ));
    }
    checks.push(readiness_application_icon(project_dir, target));
    readiness_package_tools(project_dir, target, format, &mut checks);
    package::readiness_secondary_artifacts(project_dir, &mut checks);
    Ok(checks)
}

fn package_output_location_writable(
    project_dir: &Path,
    target: Target,
    format: PackageFormat,
    release: bool,
) -> ReadinessCheck {
    let output_dir =
        default_artifact_manifest_path_for_format(project_dir, target, format, release)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| project_dir.to_path_buf());
    match probe_nearest_writable_ancestor(&output_dir) {
        Ok(ancestor) => check(
            "release.package.output_writable",
            CheckSeverity::Error,
            CheckStatus::Passed,
            "artifact output directory can be created and written",
            Some(format!(
                "output: {}; writable ancestor: {}",
                output_dir.display(),
                ancestor.display()
            )),
            vec!["Fission will stage package output under target/fission/<profile>/<target>/<format>."],
        ),
        Err(error) => check(
            "release.package.output_writable",
            CheckSeverity::Error,
            CheckStatus::Failed,
            "artifact output directory can be created and written",
            Some(format!("{}: {error}", output_dir.display())),
            vec![
                "Fix directory permissions or choose a writable project directory before packaging.",
            ],
        ),
    }
}

fn probe_nearest_writable_ancestor(output_dir: &Path) -> Result<PathBuf> {
    let mut current = output_dir;
    while !current.exists() {
        current = current
            .parent()
            .with_context(|| format!("no existing parent for {}", output_dir.display()))?;
    }
    if !current.is_dir() {
        bail!(
            "nearest existing output ancestor is not a directory: {}",
            current.display()
        );
    }
    for attempt in 0..100 {
        let probe = current.join(format!(
            ".fission-readiness-write-probe-{}-{attempt}",
            std::process::id()
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
        {
            Ok(mut file) => {
                file.write_all(b"")?;
                drop(file);
                let _ = fs::remove_file(&probe);
                return Ok(current.to_path_buf());
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to write readiness probe in {}", current.display())
                });
            }
        }
    }
    bail!(
        "could not allocate a unique readiness probe file in {}",
        current.display()
    )
}

fn package_existing_artifact_manifest_current(
    project_dir: &Path,
    target: Target,
    format: PackageFormat,
    release: bool,
) -> ReadinessCheck {
    let path = default_artifact_manifest_path_for_format(project_dir, target, format, release);
    if !path.exists() {
        return check(
            "release.package.existing_artifact_manifest_current",
            CheckSeverity::Info,
            CheckStatus::Passed,
            "existing artifact manifest does not conflict with current source config",
            Some(format!("no existing manifest at {}", path.display())),
            vec!["Fission will write a fresh artifact manifest during packaging."],
        );
    }

    let manifest = match read_artifact_manifest(&path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return check(
                "release.package.existing_artifact_manifest_current",
                CheckSeverity::Warning,
                CheckStatus::Warning,
                "existing artifact manifest does not conflict with current source config",
                Some(format!("{}: {error}", path.display())),
                vec![
                    "Remove the stale artifact manifest or rerun `fission package` so the manifest is regenerated.",
                ],
            );
        }
    };
    match artifact_source_config_mismatches(project_dir, &manifest) {
        Ok((count, mismatches)) => check(
            "release.package.existing_artifact_manifest_current",
            if mismatches.is_empty() {
                CheckSeverity::Info
            } else {
                CheckSeverity::Warning
            },
            if mismatches.is_empty() {
                CheckStatus::Passed
            } else {
                CheckStatus::Warning
            },
            "existing artifact manifest does not conflict with current source config",
            Some(if mismatches.is_empty() {
                format!("{} source config file(s) match", count)
            } else {
                mismatches.join("; ")
            }),
            if mismatches.is_empty() {
                vec!["The existing manifest matches the current release source configuration."]
            } else {
                vec![
                    "Rerun `fission package` before publishing; distribution readiness will block stale artifacts.",
                ]
            },
        ),
        Err(error) => check(
            "release.package.existing_artifact_manifest_current",
            CheckSeverity::Warning,
            CheckStatus::Warning,
            "existing artifact manifest does not conflict with current source config",
            Some(error),
            vec![
                "Fix source configuration files or rerun `fission package` so artifact-manifest.json can be regenerated.",
            ],
        ),
    }
}

fn package_identity_checks(
    project_dir: &Path,
    target: Target,
    release: bool,
    project: &FissionProject,
) -> Vec<ReadinessCheck> {
    let severity = if release {
        CheckSeverity::Error
    } else {
        CheckSeverity::Warning
    };
    let mut checks = vec![
        check(
            "release.package.app_name_configured",
            CheckSeverity::Error,
            if project.app.name.trim().is_empty() {
                CheckStatus::Missing
            } else {
                CheckStatus::Passed
            },
            "application name is configured",
            Some("[app].name".to_string()),
            vec!["Set [app].name in fission.toml before packaging."],
        ),
        check(
            "release.package.app_id_configured",
            CheckSeverity::Error,
            if project.app.app_id.trim().is_empty() {
                CheckStatus::Missing
            } else {
                CheckStatus::Passed
            },
            "application id is configured",
            Some("[app].app_id".to_string()),
            vec!["Set [app].app_id in fission.toml before packaging."],
        ),
    ];

    match resolve_release_version_config(project_dir, Some(target)) {
        Ok(version) => {
            let version_ready = version
                .version
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
            checks.push(check(
                "release.package.version_resolved",
                severity,
                if version_ready {
                    CheckStatus::Passed
                } else {
                    CheckStatus::Missing
                },
                "release version is resolved before packaging",
                version.version,
                vec![
                    "Set [app].version, target package version fields, or an active [[releases]] entry before packaging."
                ],
            ));
            checks.push(check(
                "release.package.build_resolved",
                severity,
                if version.build.is_some() {
                    CheckStatus::Passed
                } else {
                    CheckStatus::Missing
                },
                "release build number is resolved before packaging",
                version.build.map(|value| value.to_string()),
                vec![
                    "Set [app].build, target package build fields, or an active [[releases]] entry before release packaging."
                ],
            ));
        }
        Err(error) => {
            checks.push(check(
                "release.package.version_config_parses",
                CheckSeverity::Error,
                CheckStatus::Failed,
                "release version configuration can be read",
                Some(error.to_string()),
                vec!["Fix fission.toml so release version/build fields can be inspected."],
            ));
        }
    }

    checks.extend(package_target_identity_checks(
        project_dir,
        target,
        release,
        &project.app.app_id,
    ));
    checks
}

fn package_target_identity_checks(
    project_dir: &Path,
    target: Target,
    release: bool,
    app_id: &str,
) -> Vec<ReadinessCheck> {
    let severity = if release {
        CheckSeverity::Error
    } else {
        CheckSeverity::Warning
    };
    let Ok(root) = read_package_identity_toml(project_dir) else {
        return Vec::new();
    };
    match target {
        Target::Android => vec![target_identity_matches_app_id_check(
            &root,
            "release.package.android.package_name_matches_app_id",
            severity,
            &["package", "android", "package_name"],
            app_id,
            "Android package name matches app id",
            "Set [package.android].package_name to the same stable id as [app].app_id before packaging for Play.",
        )],
        Target::Ios => vec![target_identity_matches_app_id_check(
            &root,
            "release.package.ios.bundle_id_matches_app_id",
            severity,
            &["package", "ios", "bundle_id"],
            app_id,
            "iOS bundle id matches app id",
            "Set [package.ios].bundle_id to the same stable id as [app].app_id before packaging for App Store Connect.",
        )],
        Target::Macos => vec![target_identity_matches_app_id_check(
            &root,
            "release.package.macos.bundle_id_matches_app_id",
            severity,
            &["package", "macos", "bundle_id"],
            app_id,
            "macOS bundle id matches app id",
            "Set [package.macos].bundle_id to the same stable id as [app].app_id before packaging macOS .app or .pkg artifacts.",
        )],
        Target::Windows => vec![
            target_identity_present_check(
                &root,
                "release.package.windows.identity_name_configured",
                severity,
                &["package", "windows", "identity_name"],
                "Windows package identity name is configured",
                "Set [package.windows].identity_name to the Microsoft Store package identity before packaging MSIX artifacts.",
            ),
            target_identity_present_check(
                &root,
                "release.package.windows.publisher_configured",
                severity,
                &["package", "windows", "publisher"],
                "Windows publisher identity is configured",
                "Set [package.windows].publisher to the Windows package publisher identity before packaging signed artifacts.",
            ),
        ],
        _ => Vec::new(),
    }
}

fn read_package_identity_toml(project_dir: &Path) -> Result<toml::Value> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn target_identity_matches_app_id_check(
    root: &toml::Value,
    id: &str,
    severity: CheckSeverity,
    path: &[&str],
    app_id: &str,
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    let value = toml_path_str(root, path);
    let status = match value {
        Some(value) if value == app_id => CheckStatus::Passed,
        Some(_) => CheckStatus::Failed,
        None => CheckStatus::Missing,
    };
    check(
        id,
        severity,
        status,
        summary,
        Some(format!(
            "{}={}, [app].app_id={}",
            toml_path_display(path),
            value.unwrap_or("<missing>"),
            app_id
        )),
        vec![remediation],
    )
}

fn target_identity_present_check(
    root: &toml::Value,
    id: &str,
    severity: CheckSeverity,
    path: &[&str],
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    let value = toml_path_str(root, path);
    let present = value.is_some_and(|value| !value.trim().is_empty());
    check(
        id,
        severity,
        if present {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        Some(format!(
            "{}={}",
            toml_path_display(path),
            value.unwrap_or("<missing>")
        )),
        vec![remediation],
    )
}

fn toml_path_str<'a>(root: &'a toml::Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(root, |value, key| value.get(*key))
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn toml_path_display(path: &[&str]) -> String {
    format!(
        "[{}].{}",
        path[..path.len().saturating_sub(1)].join("."),
        path[path.len() - 1]
    )
}

pub(super) fn readiness_application_icon(project_dir: &Path, target: Target) -> ReadinessCheck {
    if !target_uses_application_icon(target) {
        return check(
            "release.package.icons.not_required",
            CheckSeverity::Info,
            CheckStatus::Passed,
            "application icon is not required for this package target",
            Some(target.as_str().to_string()),
            Vec::new(),
        );
    }
    match fission_command_core::resolve_app_icon(project_dir, target) {
        Ok(Some(icon)) => check(
            "release.package.icons.source_available",
            CheckSeverity::Error,
            CheckStatus::Passed,
            "application icon source is available",
            Some(display_project_path(project_dir, &icon.path)),
            vec!["The package command will record this icon source in target/fission/icons/icon-manifest.json."],
        ),
        Ok(None) => check(
            "release.package.icons.source_available",
            CheckSeverity::Error,
            CheckStatus::Missing,
            "application icon source is available",
            Some(target.as_str().to_string()),
            vec![
                "Configure [package].icon, [package.icons].source, a target-specific icon source, or restore assets/app-icon.png before packaging.",
            ],
        ),
        Err(error) => check(
            "release.package.icons.source_available",
            CheckSeverity::Error,
            CheckStatus::Failed,
            "application icon source is valid",
            Some(error.to_string()),
            vec!["Fix the icon path or file extension in fission.toml before packaging."],
        ),
    }
}

pub(super) fn package_format_supported(target: Target, format: PackageFormat) -> bool {
    matches!(
        (target, format),
        (Target::Site, PackageFormat::Static)
            | (Target::Web, PackageFormat::Static)
            | (Target::Site, PackageFormat::DockerImage)
            | (Target::Server, PackageFormat::DockerImage)
            | (Target::Linux, PackageFormat::Run)
            | (Target::Terminal, PackageFormat::Run)
            | (Target::Macos, PackageFormat::App)
            | (Target::Macos, PackageFormat::Pkg)
            | (Target::Windows, PackageFormat::Exe)
            | (Target::Windows, PackageFormat::Msi)
            | (Target::Windows, PackageFormat::Msix)
            | (Target::Android, PackageFormat::Apk)
            | (Target::Android, PackageFormat::Aab)
            | (Target::Ios, PackageFormat::Ipa)
    )
}

pub(super) fn readiness_package_tools(
    project_dir: &Path,
    target: Target,
    format: PackageFormat,
    checks: &mut Vec<ReadinessCheck>,
) {
    match (target, format) {
        (Target::Site, PackageFormat::Static) | (Target::Web, PackageFormat::Static) => {
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
        }
        (Target::Site, PackageFormat::DockerImage)
        | (Target::Server, PackageFormat::DockerImage) => {
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
            checks.push(check_tool(
                "release.package.docker_available",
                "docker",
                "Install Docker and ensure the docker CLI can reach a running Docker engine.",
            ));
            if target == Target::Server {
                checks.push(check(
                    "release.package.server_entry_configured",
                    CheckSeverity::Error,
                    if server_entry_configured(project_dir) {
                        CheckStatus::Passed
                    } else {
                        CheckStatus::Missing
                    },
                    "server entry is configured",
                    Some("[server].entry".to_string()),
                    vec!["Add [server].entry to fission.toml so the Docker image can run the server app."],
                ));
            }
        }
        (Target::Linux, PackageFormat::Run) => {
            checks.push(host_os_check("release.package.host_is_linux", "linux"));
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
        }
        (Target::Terminal, PackageFormat::Run) => {
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
        }
        (Target::Macos, PackageFormat::App) => {
            checks.push(host_os_check("release.package.host_is_macos", "macos"));
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
            checks.push(check_tool(
                "release.package.codesign_available",
                "codesign",
                "Install Xcode command line tools so Fission can verify signed .app bundles.",
            ));
        }
        (Target::Macos, PackageFormat::Pkg) => {
            checks.push(host_os_check("release.package.host_is_macos", "macos"));
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
            checks.push(check_tool(
                "release.package.pkgbuild_available",
                "pkgbuild",
                "Install Xcode command line tools.",
            ));
            checks.push(check_tool(
                "release.package.productbuild_available",
                "productbuild",
                "Install Xcode command line tools.",
            ));
            checks.push(check_tool(
                "release.package.pkgutil_available",
                "pkgutil",
                "Install macOS package tools so Fission can inspect produced .pkg files.",
            ));
        }
        (Target::Windows, PackageFormat::Exe) => {
            checks.push(host_os_check("release.package.host_is_windows", "windows"));
            checks.push(check_tool(
                "release.package.cargo_available",
                "cargo",
                "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
            ));
            checks.push(check_tool(
                "release.package.signtool_available",
                "signtool",
                "Install Windows SDK signing tools and ensure signtool is on PATH.",
            ));
            checks.push(windows_signing_source_check());
            if let Some(script) = windows_exe_installer_script(project_dir) {
                checks.push(check_path(
                    "release.package.windows_exe_installer_script_exists",
                    script,
                    "Windows EXE installer script exists",
                    "Restore package.windows.exe_installer_script or remove that field to package the application executable instead.",
                ));
            } else {
                checks.push(check(
                    "release.package.windows_exe_is_application_binary",
                    CheckSeverity::Warning,
                    CheckStatus::Warning,
                    "Windows EXE output is the application executable, not an installer",
                    None,
                    vec!["Set package.windows.exe_installer_script when release installation requires privileged components, services, drivers, repair, or uninstall behavior."],
                ));
            }
        }
        (Target::Windows, PackageFormat::Msi) => {
            checks.push(host_os_check("release.package.host_is_windows", "windows"));
            checks.push(check_path(
                "release.package.windows_msi_script_exists",
                project_dir.join("platforms/windows/package-msi.ps1"),
                "Windows MSI packaging script exists",
                "Configure platforms/windows/package-msi.ps1 or install the Windows packaging target template.",
            ));
            checks.push(check_any_tool(
                "release.package.windows_msi_builder_available",
                &["wix", "candle"],
                "WiX MSI packaging tooling is available",
                "Install WiX Toolset or configure platforms/windows/package-msi.ps1 to call the approved MSI packager.",
            ));
            checks.push(check_tool(
                "release.package.signtool_available",
                "signtool",
                "Install Windows SDK signing tools and ensure signtool is on PATH.",
            ));
            checks.push(windows_signing_source_check());
        }
        (Target::Windows, PackageFormat::Msix) => {
            checks.push(host_os_check("release.package.host_is_windows", "windows"));
            checks.push(check_path(
                "release.package.windows_msix_manifest_exists",
                project_dir.join("platforms/windows/Package.appxmanifest"),
                "Windows MSIX package manifest exists",
                "Run `fission add-target windows --project-dir .` or restore platforms/windows/Package.appxmanifest.",
            ));
            checks.push(check_path(
                "release.package.windows_msix_script_exists",
                project_dir.join("platforms/windows/package-msix.ps1"),
                "Windows MSIX packaging script exists",
                "Configure platforms/windows/package-msix.ps1 or install the Windows packaging target template.",
            ));
            checks.push(check_tool(
                "release.package.makeappx_available",
                "makeappx",
                "Install Windows SDK MSIX packaging tools and ensure makeappx is on PATH.",
            ));
            checks.push(check_tool(
                "release.package.signtool_available",
                "signtool",
                "Install Windows SDK signing tools and ensure signtool is on PATH.",
            ));
            checks.push(windows_signing_source_check());
        }
        (Target::Android, PackageFormat::Apk) => {
            checks.push(check_path(
                "release.package.android_apk_script_exists",
                project_dir.join("platforms/android/package-apk.sh"),
                "Android APK packaging script exists",
                "Run `fission add-target android --project-dir .` or restore platforms/android/package-apk.sh.",
            ));
            android_packaging_checks(checks);
        }
        (Target::Android, PackageFormat::Aab) => {
            checks.push(check_path(
                "release.package.android_aab_script_exists",
                project_dir.join("platforms/android/package-aab.sh"),
                "Android AAB packaging script exists",
                "Add platforms/android/package-aab.sh once release AAB packaging is configured.",
            ));
            android_packaging_checks(checks);
            checks.push(check_optional_env_or_tool(
                "release.package.bundletool_available",
                &["BUNDLETOOL"],
                &["bundletool"],
                "Android bundletool is available for optional AAB inspection",
                "Install bundletool or set BUNDLETOOL if you want local AAB inspection beyond jarsigner verification.",
            ));
        }
        (Target::Ios, PackageFormat::Ipa) => {
            checks.push(host_os_check("release.package.host_is_macos", "macos"));
            checks.push(check_path(
                "release.package.ios_ipa_script_exists",
                project_dir.join("platforms/ios/package-ipa.sh"),
                "iOS IPA packaging script exists",
                "Add platforms/ios/package-ipa.sh once release IPA export is configured.",
            ));
            checks.push(check_tool(
                "release.package.xcrun_available",
                "xcrun",
                "Install Xcode command line tools and select an Xcode installation.",
            ));
            checks.push(check_tool(
                "release.package.xcodebuild_available",
                "xcodebuild",
                "Install Xcode so Fission can archive and export iOS IPA files.",
            ));
            checks.push(check_tool(
                "release.package.codesign_available",
                "codesign",
                "Install Xcode command line tools so Fission can verify iOS signing.",
            ));
            checks.push(check_tool(
                "release.package.zip_available",
                "zip",
                "Install zip so Fission can assemble the signed .ipa Payload archive.",
            ));
        }
        _ => {}
    }
}

fn windows_exe_installer_script(project_dir: &Path) -> Option<PathBuf> {
    let root = read_package_identity_toml(project_dir).ok()?;
    let configured = root
        .get("package")?
        .get("windows")?
        .get("exe_installer_script")?
        .as_str()?
        .trim();
    if configured.is_empty() {
        return None;
    }
    let path = Path::new(configured);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_dir.join(path)
    })
}

pub(super) fn server_entry_configured(project_dir: &Path) -> bool {
    fs::read_to_string(project_dir.join("fission.toml"))
        .ok()
        .and_then(|data| toml::from_str::<toml::Value>(&data).ok())
        .and_then(|value| {
            value
                .get("server")
                .and_then(|server| server.get("entry"))
                .and_then(toml::Value::as_str)
                .map(|entry| !entry.trim().is_empty())
        })
        .unwrap_or(false)
}

pub(super) fn android_packaging_checks(checks: &mut Vec<ReadinessCheck>) {
    checks.push(check_tool(
        "release.package.cargo_available",
        "cargo",
        "Install Rust from https://rustup.rs/ and ensure cargo is on PATH.",
    ));
    checks.push(check_any_env(
        "release.package.android_sdk_configured",
        &["ANDROID_HOME", "ANDROID_SDK_ROOT"],
        "Android SDK path is configured",
        "Set ANDROID_HOME or ANDROID_SDK_ROOT to the installed Android SDK.",
    ));
    checks.push(check_android_ndk());
    checks.push(check_android_build_tool(
        "release.package.aapt2_available",
        "aapt2",
        "Install Android SDK build-tools; Fission checks PATH and $ANDROID_HOME/build-tools/*.",
    ));
    checks.push(check_android_build_tool(
        "release.package.zipalign_available",
        "zipalign",
        "Install Android SDK build-tools; Fission checks PATH and $ANDROID_HOME/build-tools/*.",
    ));
    checks.push(check_android_build_tool(
        "release.package.apksigner_available",
        "apksigner",
        "Install Android SDK build-tools; Fission checks PATH and $ANDROID_HOME/build-tools/*.",
    ));
}

fn windows_signing_source_check() -> ReadinessCheck {
    if env::var_os("WINDOWS_SKIP_SIGNING").as_deref() == Some(OsStr::new("1")) {
        return check(
            "release.package.windows_signing_source",
            CheckSeverity::Warning,
            CheckStatus::Warning,
            "Windows signing source is configured",
            Some("WINDOWS_SKIP_SIGNING=1".to_string()),
            vec!["Use WINDOWS_SKIP_SIGNING=1 only for local unsigned validation; release distribution should use WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_BASE64, or WINDOWS_CERTIFICATE_THUMBPRINT."],
        );
    }
    check_any_env(
        "release.package.windows_signing_source",
        &[
            "WINDOWS_CERTIFICATE",
            "WINDOWS_CERTIFICATE_BASE64",
            "WINDOWS_CERTIFICATE_THUMBPRINT",
        ],
        "Windows signing source is configured",
        "Set WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_BASE64, or WINDOWS_CERTIFICATE_THUMBPRINT from a local/CI secret source. Do not store PFX/P12 files or passwords in fission.toml.",
    )
}

pub(super) fn readiness_distribute(
    project_dir: &Path,
    provider: DistributionProvider,
    site: &str,
    track: Option<&str>,
    format: Option<PackageFormat>,
    artifact: Option<&Path>,
    config: &PublishManifest,
) -> Result<Vec<ReadinessCheck>> {
    let mut checks = Vec::new();
    checks.push(distribution_receipt_location_writable(
        project_dir,
        provider,
        site,
    ));
    checks.push(distribution_dry_run_supported(provider));
    if let Some(path) = artifact {
        checks.push(check_path(
            "release.distribution.artifact_manifest_exists",
            path.to_path_buf(),
            "artifact manifest exists",
            "Run `fission package --target <target> --format <format> --release` for the selected publish target first.",
        ));
        if path.exists() {
            let manifest = read_artifact_manifest(path)?;
            checks.push(artifact_manifest_validation_check(&manifest));
            checks.push(artifact_manifest_hashes_check(&manifest));
            checks.push(artifact_source_config_matches_check(project_dir, &manifest));
            if let Some(check) = debug_symbol_distribution_check(provider, &manifest) {
                checks.push(check);
            }
            if provider_requires_static_root(provider) {
                checks.push(static_provider_artifact_format_check(&manifest));
                checks.push(check(
                    "release.distribution.static_root_exists",
                    CheckSeverity::Error,
                    if Path::new(&manifest.root_dir).join("index.html").exists() {
                        CheckStatus::Passed
                    } else {
                        CheckStatus::Missing
                    },
                    "static artifact root contains index.html",
                    Some(manifest.root_dir),
                    vec!["Rebuild the static package and ensure the output includes index.html."],
                ));
            }
        }
    }

    match provider {
        DistributionProvider::GithubPages => {
            readiness_github_pages(project_dir, site, config, &mut checks)?
        }
        DistributionProvider::GithubReleases => {
            github_releases::readiness(project_dir, site, artifact, config, &mut checks)?
        }
        DistributionProvider::DockerRegistry => {
            docker_registry::readiness(site, artifact, config, &mut checks)?
        }
        DistributionProvider::CloudflarePages => {
            readiness_cloudflare_pages(site, config, &mut checks)?
        }
        DistributionProvider::Netlify => readiness_netlify(site, config, &mut checks)?,
        DistributionProvider::S3 => files::readiness_s3(site, config, &mut checks)?,
        DistributionProvider::GoogleDrive => {
            files::readiness_google_drive(site, config, &mut checks)?
        }
        DistributionProvider::OneDrive => files::readiness_onedrive(site, config, &mut checks)?,
        DistributionProvider::Dropbox => files::readiness_dropbox(site, config, &mut checks)?,
        DistributionProvider::PlayStore => {
            stores::readiness_play_store(project_dir, track, artifact, config, &mut checks)?
        }
        DistributionProvider::AppStore => {
            stores::readiness_app_store(project_dir, track, artifact, config, &mut checks)?
        }
        DistributionProvider::MicrosoftStore => stores::readiness_microsoft_store(
            project_dir,
            track,
            format,
            artifact,
            config,
            &mut checks,
        )?,
    }
    Ok(checks)
}

fn debug_symbol_distribution_check(
    provider: DistributionProvider,
    manifest: &ArtifactManifest,
) -> Option<ReadinessCheck> {
    let symbols = manifest
        .artifacts
        .iter()
        .filter(|artifact| is_debug_or_crash_artifact(&artifact.kind))
        .collect::<Vec<_>>();
    if symbols.is_empty() {
        return None;
    }

    let provider_name = provider.as_str();
    let selected_provider_uploads_all_assets = provider_uploads_all_manifest_assets(provider);
    let mut gaps = Vec::new();
    let mut details = Vec::new();
    for artifact in symbols {
        let provider_hint = artifact
            .upload_provider
            .as_deref()
            .filter(|value| !value.trim().is_empty());
        let uploaded_by_selected = match provider_hint {
            Some(expected) if expected == provider_name => selected_provider_uploads_all_assets,
            Some(_) => false,
            None => selected_provider_uploads_all_assets,
        };
        details.push(format!(
            "{} kind={} upload_provider={} selected_provider_uploads={}",
            artifact.relative_path,
            artifact.kind,
            provider_hint.unwrap_or("not-set"),
            uploaded_by_selected
        ));
        if !uploaded_by_selected {
            gaps.push(artifact.relative_path.clone());
        }
    }

    Some(check(
        "release.distribution.debug_symbols_upload_state",
        if gaps.is_empty() {
            CheckSeverity::Info
        } else {
            CheckSeverity::Warning
        },
        if gaps.is_empty() {
            CheckStatus::Passed
        } else {
            CheckStatus::Warning
        },
        "debug symbols and crash diagnostics are accounted for by the selected distributor",
        Some(details.join("; ")),
        if gaps.is_empty() {
            vec!["Keep symbol/crash artifacts in the artifact manifest so support diagnostics remain tied to the release."]
        } else {
            vec![
                "Publish the symbol/crash artifacts through their configured upload_provider or choose an artifact distributor such as github-releases, s3, google-drive, onedrive, or dropbox for those assets.",
                "If the selected store provider supports native symbol upload, add provider integration before treating this release as fully supportable.",
            ]
        },
    ))
}

fn is_debug_or_crash_artifact(kind: &str) -> bool {
    matches!(kind, "debug_symbols" | "crash_diagnostics" | "symbols")
}

fn provider_uploads_all_manifest_assets(provider: DistributionProvider) -> bool {
    matches!(
        provider,
        DistributionProvider::GithubReleases
            | DistributionProvider::S3
            | DistributionProvider::GoogleDrive
            | DistributionProvider::OneDrive
            | DistributionProvider::Dropbox
    )
}

fn distribution_dry_run_supported(provider: DistributionProvider) -> ReadinessCheck {
    check(
        "release.distribution.dry_run_supported",
        CheckSeverity::Info,
        CheckStatus::Passed,
        "distribution dry-run support is modeled",
        Some(provider.as_str().to_string()),
        vec![
            "Use `fission distribute --dry-run` or `fission publish --dry-run` to inspect the upload plan without mutating provider state.",
        ],
    )
}

fn artifact_source_config_matches_check(
    project_dir: &Path,
    manifest: &ArtifactManifest,
) -> ReadinessCheck {
    let (count, mismatches) = match artifact_source_config_mismatches(project_dir, manifest) {
        Ok(result) => result,
        Err(error) if manifest.source_config.is_empty() => {
            return check(
                "release.distribution.artifact_source_config_current",
                CheckSeverity::Error,
                CheckStatus::Missing,
                "artifact source configuration hashes are recorded and current",
                Some(error),
                vec![
                    "Rebuild the package so artifact-manifest.json records fission.toml, Cargo.toml, and target packaging file hashes.",
                ],
            );
        }
        Err(error) => {
            return check(
                "release.distribution.artifact_source_config_current",
                CheckSeverity::Error,
                CheckStatus::Failed,
                "artifact source configuration hashes are recorded and current",
                Some(error),
                vec![
                    "Fix source configuration files so Fission can hash them before distribution.",
                ],
            );
        }
    };

    check(
        "release.distribution.artifact_source_config_current",
        CheckSeverity::Error,
        if mismatches.is_empty() {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "artifact source configuration hashes are recorded and current",
        Some(if mismatches.is_empty() {
            format!("{} source config file(s) match", count)
        } else {
            mismatches.join("; ")
        }),
        vec![
            "Rebuild the package after changing fission.toml, Cargo.toml, or target packaging files before distribution.",
        ],
    )
}

fn artifact_source_config_mismatches(
    project_dir: &Path,
    manifest: &ArtifactManifest,
) -> std::result::Result<(usize, Vec<String>), String> {
    if manifest.source_config.is_empty() {
        return Err("artifact manifest has no source_config entries".to_string());
    }

    let target = Target::from_str(&manifest.target, true)
        .map_err(|error| format!("unknown manifest target {}: {error}", manifest.target))?;
    let current = artifact_source_config(project_dir, target).map_err(|error| error.to_string())?;

    let mismatches = current
        .iter()
        .filter_map(|entry| {
            let recorded = manifest
                .source_config
                .iter()
                .find(|recorded| recorded.path == entry.path);
            match recorded {
                Some(recorded) if recorded.sha256 == entry.sha256 => None,
                Some(recorded) => Some(format!(
                    "{} changed (manifest {}, current {})",
                    entry.path, recorded.sha256, entry.sha256
                )),
                None => Some(format!("{} missing from artifact manifest", entry.path)),
            }
        })
        .chain(manifest.source_config.iter().filter_map(|recorded| {
            if current.iter().any(|entry| entry.path == recorded.path) {
                None
            } else {
                Some(format!(
                    "{} was recorded in the artifact but is no longer part of the current source configuration",
                    recorded.path
                ))
            }
        }))
        .collect::<Vec<_>>();

    Ok((current.len(), mismatches))
}

fn static_provider_artifact_format_check(manifest: &ArtifactManifest) -> ReadinessCheck {
    check(
        "release.distribution.static_artifact_format",
        CheckSeverity::Error,
        if manifest.format == PackageFormat::Static.as_str() {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "static host artifact format is static",
        Some(format!("manifest format: {}", manifest.format)),
        vec![
            "Run `fission package --target static-site --format static --release` or `fission package --target web --format static --release` before publishing to a static host.",
        ],
    )
}

fn distribution_receipt_location_writable(
    project_dir: &Path,
    provider: DistributionProvider,
    site: &str,
) -> ReadinessCheck {
    let receipt_dir = project_dir
        .join("target/fission/distribution")
        .join(provider.as_str())
        .join(site);
    match probe_nearest_writable_ancestor(&receipt_dir) {
        Ok(ancestor) => check(
            "release.distribution.receipt_path_writable",
            CheckSeverity::Error,
            CheckStatus::Passed,
            "distribution receipt path can be created and written",
            Some(format!(
                "receipt_dir: {}; writable ancestor: {}",
                receipt_dir.display(),
                ancestor.display()
            )),
            vec!["Fission writes distribution receipts under target/fission/distribution/<provider>/<site>."],
        ),
        Err(error) => check(
            "release.distribution.receipt_path_writable",
            CheckSeverity::Error,
            CheckStatus::Failed,
            "distribution receipt path can be created and written",
            Some(format!("{}: {error}", receipt_dir.display())),
            vec!["Fix directory permissions or choose a writable project directory before distribution."],
        ),
    }
}

fn artifact_manifest_hashes_check(manifest: &ArtifactManifest) -> ReadinessCheck {
    let mismatches = manifest
        .artifacts
        .iter()
        .filter_map(|artifact| {
            let path = artifact_file_path(manifest, artifact);
            match hash_file(&path) {
                Ok((sha256, size_bytes))
                    if sha256 == artifact.sha256 && size_bytes == artifact.size_bytes =>
                {
                    None
                }
                Ok((sha256, size_bytes)) => Some(format!(
                    "{} expected sha256={} size={} got sha256={} size={}",
                    artifact.relative_path,
                    artifact.sha256,
                    artifact.size_bytes,
                    sha256,
                    size_bytes
                )),
                Err(error) => Some(format!(
                    "{} missing/unreadable: {error}",
                    artifact.relative_path
                )),
            }
        })
        .collect::<Vec<_>>();
    check(
        "release.distribution.artifact_hashes_match",
        CheckSeverity::Error,
        if mismatches.is_empty() {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "artifact manifest hashes match artifact files",
        (!mismatches.is_empty()).then(|| mismatches.join("; ")),
        vec![
            "Rebuild the artifact or restore the files listed in artifact-manifest.json before distribution.",
        ],
    )
}

fn artifact_file_path(manifest: &ArtifactManifest, artifact: &ArtifactFile) -> PathBuf {
    let path = PathBuf::from(&artifact.path);
    if path.is_absolute() {
        path
    } else {
        Path::new(&manifest.root_dir).join(path)
    }
}

fn artifact_manifest_validation_check(manifest: &ArtifactManifest) -> ReadinessCheck {
    let non_passed = manifest
        .validation
        .checks
        .iter()
        .filter(|check| check.status != CheckStatus::Passed)
        .map(|check| format!("{}={:?}", check.id, check.status))
        .collect::<Vec<_>>();
    let failed = manifest.validation.state == "failed"
        || manifest.validation.checks.iter().any(|check| {
            check.severity == CheckSeverity::Error && check.status != CheckStatus::Passed
        });
    let warning = manifest.validation.state != "passed" || !non_passed.is_empty();
    let status = if failed {
        CheckStatus::Failed
    } else if warning {
        CheckStatus::Warning
    } else {
        CheckStatus::Passed
    };
    check(
        "release.distribution.artifact_validation",
        if failed {
            CheckSeverity::Error
        } else if warning {
            CheckSeverity::Warning
        } else {
            CheckSeverity::Info
        },
        status,
        "artifact manifest package validation is acceptable for distribution",
        Some(format!(
            "state={}{}",
            manifest.validation.state,
            if non_passed.is_empty() {
                String::new()
            } else {
                format!("; {}", non_passed.join(", "))
            }
        )),
        vec!["Re-run package validation and resolve failed artifact checks before distribution."],
    )
}

pub(super) fn provider_requires_static_root(provider: DistributionProvider) -> bool {
    matches!(
        provider,
        DistributionProvider::GithubPages
            | DistributionProvider::CloudflarePages
            | DistributionProvider::Netlify
    )
}

pub(super) fn readiness_github_pages(
    project_dir: &Path,
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = github_config(config, site)?;
    let owner = cfg
        .owner
        .clone()
        .or_else(|| infer_github_owner(project_dir));
    let repo = cfg.repo.clone().or_else(|| infer_github_repo(project_dir));
    checks.push(required_value(
        "release.github_pages.owner_configured",
        owner.as_deref(),
        "GitHub owner is configured or inferable from git remote",
        "Set distribution.github_pages.<site>.owner or configure an origin GitHub remote.",
    ));
    checks.push(required_value(
        "release.github_pages.repo_configured",
        repo.as_deref(),
        "GitHub repository is configured or inferable from git remote",
        "Set distribution.github_pages.<site>.repo or configure an origin GitHub remote.",
    ));
    let mode = cfg.mode.as_deref().unwrap_or("actions");
    checks.push(check(
        "release.github_pages.mode_supported",
        CheckSeverity::Error,
        if matches!(mode, "actions" | "branch" | "manual") {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        "GitHub Pages mode is supported",
        Some(mode.to_string()),
        vec!["Use mode = \"actions\", \"branch\", or \"manual\"."],
    ));
    if mode == "branch" {
        checks.push(check_tool(
            "release.github_pages.git_available",
            "git",
            "Install Git and authenticate to the repository remote.",
        ));
    } else {
        checks.push(check(
            "release.github_pages.source_is_actions",
            CheckSeverity::Warning,
            if cfg.source.as_deref().unwrap_or("github-actions") == "github-actions" {
                CheckStatus::Passed
            } else {
                CheckStatus::Warning
            },
            "GitHub Pages source is configured for Actions publishing",
            cfg.source.clone(),
            vec!["Set distribution.github_pages.<site>.source = \"github-actions\" for Actions-based Pages publishing."],
        ));
        let workflow = cfg.workflow.as_deref().unwrap_or("fission-pages.yml");
        checks.push(check_path(
            "release.github_pages.workflow_exists",
            github_workflow_path(project_dir, &cfg, workflow),
            "GitHub Pages workflow exists",
            "Run `fission distribute setup --provider github-pages --site production` to generate it.",
        ));
        checks.push(check(
            "release.github_pages.local_api_token_optional",
            CheckSeverity::Info,
            if env::var_os("GH_TOKEN").is_some() || env::var_os("GITHUB_TOKEN").is_some() {
                CheckStatus::Passed
            } else {
                CheckStatus::Skipped
            },
            "GitHub API token is available for local status/domain setup",
            None,
            vec!["For local Pages status or future domain setup automation, set GH_TOKEN/GITHUB_TOKEN or authenticate gh with `gh auth login`."],
        ));
    }
    let base = cfg.base_path.as_deref().unwrap_or("/");
    let expected = expected_github_base_path(&cfg, repo.as_deref());
    checks.push(check(
        "release.github_pages.base_path_matches_domain_mode",
        CheckSeverity::Warning,
        if base == expected { CheckStatus::Passed } else { CheckStatus::Warning },
        "GitHub Pages base path matches custom-domain/project-site mode",
        Some(format!("configured {base}, expected {expected}")),
        vec!["Set distribution.github_pages.<site>.base_path to the expected value or adjust the site renderer base URL."],
    ));
    checks.push(check(
        "release.github_pages.https_policy_set",
        CheckSeverity::Info,
        if cfg.enforce_https.unwrap_or(true) {
            CheckStatus::Passed
        } else {
            CheckStatus::Warning
        },
        "GitHub Pages HTTPS policy is explicit",
        Some(format!("enforce_https = {}", cfg.enforce_https.unwrap_or(true))),
        vec!["Keep enforce_https = true for public production sites unless there is a provider limitation."],
    ));
    Ok(())
}

pub(super) fn readiness_cloudflare_pages(
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = cloudflare_config(config, site)?;
    let env_account_id = env::var("CLOUDFLARE_ACCOUNT_ID").ok();
    checks.push(required_value(
        "release.cloudflare_pages.account_id_configured",
        cfg.account_id.as_deref().or(env_account_id.as_deref()),
        "Cloudflare account id is configured",
        "Set distribution.cloudflare_pages.<site>.account_id or CLOUDFLARE_ACCOUNT_ID.",
    ));
    checks.push(required_value(
        "release.cloudflare_pages.project_name_configured",
        cfg.project_name.as_deref(),
        "Cloudflare Pages project name is configured",
        "Set distribution.cloudflare_pages.<site>.project_name.",
    ));
    checks.push(required_provider_env_secret(
        "release.cloudflare_pages.token_available",
        &["CLOUDFLARE_API_TOKEN"],
        "Create a Cloudflare API token with Pages Edit permission and provide it through CI secrets or CLOUDFLARE_API_TOKEN.",
    ));
    checks.push(check_tool(
        "release.cloudflare_pages.wrangler_available",
        "wrangler",
        "Install Wrangler and authenticate it; Cloudflare Pages upload intentionally uses the provider CLI backend.",
    ));
    checks.push(base_path_check(
        "release.cloudflare_pages.base_path_root",
        cfg.base_path.as_deref(),
    ));
    Ok(())
}

pub(super) fn readiness_netlify(
    site: &str,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = netlify_config(config, site)?;
    checks.push(required_value(
        "release.netlify.site_configured",
        cfg.site_id.as_deref(),
        "Netlify site id is configured",
        "Set distribution.netlify.<site>.site_id or run provider setup after creating a Netlify site.",
    ));
    checks.push(required_provider_env_secret(
        "release.netlify.token_available",
        &["NETLIFY_AUTH_TOKEN"],
        "Create a Netlify access token and provide it through CI secrets or NETLIFY_AUTH_TOKEN.",
    ));
    checks.push(base_path_check(
        "release.netlify.base_path_root",
        cfg.base_path.as_deref(),
    ));
    Ok(())
}
