use super::*;

pub(super) fn build_artifact_manifest(
    project: &FissionProject,
    options: &PackageOptions,
    root: &Path,
    profile: &str,
) -> Result<ArtifactManifest> {
    let mut files = Vec::new();
    collect_artifacts(root, root, &mut files)?;
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let release = resolve_release_version_config(&options.project_dir, Some(options.target))?;
    let icon_manifest = prepare_icon_manifest(&options.project_dir, options.target, root)?;
    let source_config = artifact_source_config(&options.project_dir, options.target)?;
    Ok(ArtifactManifest {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        project: ArtifactProject {
            app_id: project.app.app_id.clone(),
            name: project.app.name.clone(),
            build: release.build,
            version: release
                .version
                .or_else(|| cargo_package_version(&options.project_dir)),
        },
        target: options.target.as_str().to_string(),
        format: options.format.as_str().to_string(),
        profile: profile.to_string(),
        root_dir: root.display().to_string(),
        source_config,
        artifacts: files,
        icon_manifest,
        signing: None,
        notarization: None,
        validation: ArtifactValidation {
            state: "passed".to_string(),
            checks: Vec::new(),
        },
    })
}

pub(super) fn prepare_icon_manifest(
    project_dir: &Path,
    target: Target,
    package_root: &Path,
) -> Result<Option<ArtifactIconManifest>> {
    if !target_uses_application_icon(target) {
        return Ok(None);
    }
    let Some(icon) = fission_command_core::resolve_app_icon(project_dir, target)? else {
        return Ok(None);
    };

    let icon_root = project_dir.join("target/fission/icons");
    fs::create_dir_all(&icon_root)
        .with_context(|| format!("failed to create {}", icon_root.display()))?;
    let manifest_path = icon_root.join("icon-manifest.json");
    let (source_sha256, source_size_bytes) = hash_file(&icon.path)?;
    let outputs = collect_icon_package_outputs(package_root)?;
    let manifest = json!({
        "schema_version": 1,
        "mode": if icon.configured { "configured" } else { "fallback" },
        "target": target.as_str(),
        "sources": [{
            "role": icon_source_role(target),
            "path": display_project_path(project_dir, &icon.path),
            "sha256": source_sha256,
            "size_bytes": source_size_bytes
        }],
        "outputs": outputs,
    });
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;
    let (sha256, _) = hash_file(&manifest_path)?;
    Ok(Some(ArtifactIconManifest {
        path: manifest_path.display().to_string(),
        sha256,
        outputs: manifest
            .get("outputs")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default(),
    }))
}

pub(super) fn target_uses_application_icon(target: Target) -> bool {
    !matches!(target, Target::Server | Target::Terminal)
}

pub(super) fn icon_source_role(target: Target) -> &'static str {
    match target {
        Target::Android => "android_launcher_source",
        Target::Ios => "ios_app_icon_source",
        Target::Macos => "macos_app_icon_source",
        Target::Windows => "windows_app_icon_source",
        Target::Linux => "linux_app_icon_source",
        Target::Web | Target::Site => "web_icon_source",
        Target::Server | Target::Terminal => "application_icon_source",
    }
}

pub(super) fn package_signing_context(
    project_dir: &Path,
    target: Target,
    format: PackageFormat,
    checks: &[ReadinessCheck],
) -> Result<Option<ArtifactSigning>> {
    if !package_format_requires_signing(format) {
        return Ok(None);
    }
    let signing_checks = checks
        .iter()
        .filter(|check| check.id.starts_with("release.package.signature."))
        .collect::<Vec<_>>();
    let state = if signing_checks
        .iter()
        .any(|check| matches!(check.status, CheckStatus::Failed | CheckStatus::Missing))
    {
        "failed"
    } else if signing_checks
        .iter()
        .any(|check| check.status == CheckStatus::Passed)
    {
        "signed"
    } else if signing_checks.is_empty() {
        "not-validated"
    } else {
        "unverified"
    };
    Ok(Some(ArtifactSigning {
        state: state.to_string(),
        identity: package_signing_identity(project_dir, target, format)?,
        certificate_sha256: None,
    }))
}

pub(super) fn package_notarization_context(
    project_dir: &Path,
    target: Target,
) -> Result<Option<Value>> {
    if target != Target::Macos {
        return Ok(None);
    }
    let Some(value) = read_fission_toml_json(project_dir)? else {
        return Ok(None);
    };
    let notarize = value
        .pointer("/package/macos/notarize")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if notarize {
        Ok(Some(json!({
            "state": "configured",
            "tool": "notarytool"
        })))
    } else {
        Ok(None)
    }
}

pub(super) fn package_format_requires_signing(format: PackageFormat) -> bool {
    matches!(
        format,
        PackageFormat::App
            | PackageFormat::Pkg
            | PackageFormat::Exe
            | PackageFormat::Msi
            | PackageFormat::Msix
            | PackageFormat::Apk
            | PackageFormat::Aab
            | PackageFormat::Ipa
    )
}

pub(super) fn package_signing_identity(
    project_dir: &Path,
    target: Target,
    format: PackageFormat,
) -> Result<Option<String>> {
    let Some(value) = read_fission_toml_json(project_dir)? else {
        return Ok(None);
    };
    let pointer = |path: &str| {
        value
            .pointer(path)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    };
    Ok(match (target, format) {
        (Target::Android, PackageFormat::Apk | PackageFormat::Aab) => {
            pointer("/package/android/keystore_alias")
                .or_else(|| env::var("ANDROID_KEYSTORE_ALIAS").ok())
        }
        (Target::Macos, PackageFormat::Pkg) => pointer("/package/macos/installer_identity")
            .or_else(|| pointer("/package/macos/signing_identity")),
        (Target::Macos, PackageFormat::App) => pointer("/package/macos/signing_identity"),
        (Target::Ios, PackageFormat::Ipa) => pointer("/package/ios/signing_identity")
            .or_else(|| pointer("/package/ios/team_id"))
            .or_else(|| env::var("APPLE_TEAM_ID").ok()),
        (Target::Windows, PackageFormat::Exe | PackageFormat::Msi | PackageFormat::Msix) => {
            pointer("/package/windows/certificate_thumbprint")
                .or_else(|| pointer("/package/windows/publisher"))
                .or_else(|| env::var("WINDOWS_CERTIFICATE_THUMBPRINT").ok())
        }
        _ => None,
    })
}

pub(super) fn read_fission_toml_json(project_dir: &Path) -> Result<Option<Value>> {
    let path = project_dir.join("fission.toml");
    if !path.exists() {
        return Ok(None);
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))?;
    serde_json::to_value(value)
        .map(Some)
        .context("failed to convert fission.toml to JSON")
}

pub(super) fn collect_icon_package_outputs(package_root: &Path) -> Result<Vec<Value>> {
    let mut outputs = Vec::new();
    if package_root.exists() {
        collect_icon_package_outputs_inner(package_root, package_root, &mut outputs)?;
    }
    outputs.sort_by(|left, right| {
        left.get("package_path")
            .and_then(Value::as_str)
            .cmp(&right.get("package_path").and_then(Value::as_str))
    });
    Ok(outputs)
}

pub(super) fn collect_icon_package_outputs_inner(
    root: &Path,
    current: &Path,
    outputs: &mut Vec<Value>,
) -> Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("failed to read {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_icon_package_outputs_inner(root, &path, outputs)?;
        } else if file_type.is_file() && looks_like_icon_output(&path) {
            let package_path = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            outputs.push(json!({
                "role": icon_output_role(&path),
                "path": path.display().to_string(),
                "package_path": package_path,
            }));
        }
    }
    Ok(())
}

pub(super) fn looks_like_icon_output(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !(name.contains("icon") || name.contains("favicon")) {
        return false;
    }
    matches!(
        path.extension()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "png" | "svg" | "xml" | "icns" | "ico" | "json"
    )
}

pub(super) fn icon_output_role(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "icns" => "macos_icns",
        "ico" => "windows_ico",
        "xml" => "icon_metadata",
        "json" => "icon_metadata",
        "svg" => "vector_icon",
        _ => "raster_icon",
    }
}

pub(super) fn collect_artifacts(
    root: &Path,
    current: &Path,
    files: &mut Vec<ArtifactFile>,
) -> Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("failed to read {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            collect_artifacts(root, &path, files)?;
        } else if file_type.is_file() {
            if path.file_name().and_then(OsStr::to_str) == Some(ARTIFACT_MANIFEST) {
                continue;
            }
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            let (sha256, size_bytes) = hash_file(&path)?;
            files.push(ArtifactFile {
                kind: if relative == "index.html" {
                    "entry"
                } else {
                    "asset"
                }
                .to_string(),
                purpose: None,
                platform: None,
                upload_provider: None,
                path: path.display().to_string(),
                relative_path: relative,
                sha256,
                size_bytes,
                mime_type: content_type(&path).to_string(),
            });
        }
    }
    Ok(())
}

pub(super) fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buf = [0u8; 8192];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&buf[..read]);
    }
    Ok((hex_lower(&hasher.finalize()), size))
}

pub(super) fn display_project_path(project_dir: &Path, path: &Path) -> String {
    path.strip_prefix(project_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn artifact_source_config(
    project_dir: &Path,
    target: Target,
) -> Result<Vec<ArtifactSourceConfig>> {
    let mut paths = vec![
        ("fission_manifest", project_dir.join("fission.toml")),
        ("cargo_manifest", project_dir.join("Cargo.toml")),
    ];
    paths.extend(target_source_config_candidates(project_dir, target));
    let mut entries = Vec::new();
    for (kind, path) in paths {
        if path.is_file() {
            let (sha256, _) = hash_file(&path)?;
            entries.push(ArtifactSourceConfig {
                kind: kind.to_string(),
                path: display_project_path(project_dir, &path),
                sha256,
            });
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

pub(super) fn target_source_config_candidates(
    project_dir: &Path,
    target: Target,
) -> Vec<(&'static str, PathBuf)> {
    match target {
        Target::Android => vec![
            (
                "android_manifest",
                project_dir.join("platforms/android/AndroidManifest.xml"),
            ),
            (
                "android_package_script",
                project_dir.join("platforms/android/package-apk.sh"),
            ),
            (
                "android_package_script",
                project_dir.join("platforms/android/package-aab.sh"),
            ),
        ],
        Target::Ios => vec![
            (
                "ios_info_plist",
                project_dir.join("platforms/ios/Info.plist"),
            ),
            (
                "ios_package_script",
                project_dir.join("platforms/ios/package-ipa.sh"),
            ),
        ],
        Target::Windows => vec![
            (
                "windows_appx_manifest",
                project_dir.join("platforms/windows/Package.appxmanifest"),
            ),
            (
                "windows_appx_manifest",
                project_dir.join("platforms/windows/AppxManifest.xml"),
            ),
            (
                "windows_package_script",
                project_dir.join("platforms/windows/package-msix.ps1"),
            ),
            (
                "windows_package_script",
                project_dir.join("platforms/windows/package-msi.ps1"),
            ),
        ],
        Target::Macos => vec![(
            "macos_package_config",
            project_dir.join("platforms/macos/Info.plist"),
        )],
        Target::Site => vec![("site_config", project_dir.join("site.toml"))],
        Target::Web => vec![("web_index", project_dir.join("platforms/web/index.html"))],
        Target::Linux | Target::Server | Target::Terminal => Vec::new(),
    }
}

pub(super) fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

pub(super) fn content_type(path: &Path) -> &'static str {
    if path.file_name().and_then(OsStr::to_str) == Some("Dockerfile") {
        return "text/plain; charset=utf-8";
    }
    match path.extension().and_then(OsStr::to_str).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" | "webmanifest" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub(super) fn copy_dir_contents(source: &Path, dest: &Path) -> Result<()> {
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&dest_path)?;
            copy_dir_contents(&source_path, &dest_path)?;
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

pub(super) fn clean_publish_root(root: &Path) -> Result<()> {
    fs::create_dir_all(root)?;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
