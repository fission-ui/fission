use super::*;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize, Default)]
struct SigningToml {
    package: Option<PackageToml>,
}

#[derive(Debug, Deserialize, Default)]
struct PackageToml {
    android: Option<AndroidPackageToml>,
    ios: Option<ApplePackageToml>,
    macos: Option<MacosPackageToml>,
    windows: Option<WindowsPackageToml>,
}

#[derive(Debug, Deserialize, Default)]
struct AndroidPackageToml {
    keystore_alias: Option<String>,
    keystore_env: Option<String>,
    keystore_base64_env: Option<String>,
    keystore_password_env: Option<String>,
    key_password_env: Option<String>,
    package_name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ApplePackageToml {
    bundle_id: Option<String>,
    team_id: Option<String>,
    entitlements: Option<String>,
    provisioning_profile: Option<String>,
    signing_identity: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MacosPackageToml {
    bundle_id: Option<String>,
    team_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WindowsPackageToml {
    identity_name: Option<String>,
    publisher: Option<String>,
    certificate_thumbprint: Option<String>,
    certificate_thumbprint_env: Option<String>,
    certificate_env: Option<String>,
    certificate_base64_env: Option<String>,
    certificate_password_env: Option<String>,
}

pub(super) fn status(project_dir: &Path, target: Target, json: bool) -> Result<()> {
    print_report(
        build_status_report("signing.status", project_dir, target),
        json,
    )
}

pub(super) fn status_checks(project_dir: &Path, target: Target) -> Vec<LifecycleCheck> {
    build_status_report("signing.status", project_dir, target).checks
}

pub(super) fn sync(project_dir: &Path, target: Target, readonly: bool, json: bool) -> Result<()> {
    let mut report = build_status_report("signing.sync", project_dir, target);
    report.checks.push(ok_check(
        "signing.sync.mode",
        if readonly {
            "readonly"
        } else {
            "write status snapshot"
        },
    ));
    if !readonly {
        let output = project_dir
            .join("release-content/signing")
            .join(format!("{}.status.json", target.as_str()));
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output, serde_json::to_vec_pretty(&report)?)
            .with_context(|| format!("failed to write {}", output.display()))?;
        report.checks.push(ok_check(
            "signing.sync.snapshot_written",
            output.display().to_string(),
        ));
    }
    print_report(report, json)
}

pub(super) fn import(
    project_dir: &Path,
    target: Target,
    keystore: Option<PathBuf>,
    alias: Option<String>,
    dry_run: bool,
    yes: bool,
    json: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("signing import rewrites fission.toml signing references; pass --yes after reviewing the target and alias");
    }
    let mut report = base_report("signing.import", None, Some(target));
    report.checks.push(path_check(
        "signing.project_config_exists",
        project_dir.join("fission.toml"),
        "fission.toml exists",
    ));
    match target {
        Target::Android => import_android(project_dir, keystore, alias, dry_run, &mut report)?,
        Target::Ios | Target::Macos | Target::Windows => report.checks.push(failed_check(
            "signing.import.target_requires_platform_store",
            format!(
                "{} signing import is intentionally read-only for now; use the platform certificate/keychain tooling and record references in fission.toml",
                target.as_str()
            ),
        )),
        _ => report.checks.push(warning_check(
            "signing.import.target",
            format!("{} does not require signing by default", target.as_str()),
        )),
    }
    print_report(report, json)
}

fn import_android(
    project_dir: &Path,
    keystore: Option<PathBuf>,
    alias: Option<String>,
    dry_run: bool,
    report: &mut LifecycleReport,
) -> Result<()> {
    let alias = alias.context("signing import --target android requires --alias")?;
    if let Some(keystore) = keystore {
        report.checks.push(path_check(
            "signing.android.keystore_exists",
            keystore,
            "Android upload keystore exists",
        ));
        report.checks.push(warning_check(
            "signing.android.keystore_not_recorded",
            "Android keystore paths are secrets-adjacent and machine-specific; Fission does not write them to fission.toml".to_string(),
        ));
    }
    let path = project_dir.join("fission.toml");
    let data = fs::read_to_string(&path).unwrap_or_default();
    let mut root = if data.trim().is_empty() {
        toml_edit::DocumentMut::new()
    } else {
        parse_toml_edit_document(&data, &path)?
    };
    set_toml_edit_path(
        &mut root,
        "package.android.keystore_alias",
        toml_edit::value(alias.clone()),
    )?;
    set_toml_edit_path(
        &mut root,
        "package.android.keystore_env",
        toml_edit::value("ANDROID_KEYSTORE"),
    )?;
    set_toml_edit_path(
        &mut root,
        "package.android.keystore_base64_env",
        toml_edit::value("ANDROID_KEYSTORE_BASE64"),
    )?;
    if !dry_run {
        write_toml_edit_document(&path, &root)?;
    }
    report.checks.push(ok_check(
        if dry_run {
            "signing.android.config_would_write"
        } else {
            "signing.android.config_written"
        },
        format!("package.android.keystore_alias = {alias}; keystore source comes from ANDROID_KEYSTORE or ANDROID_KEYSTORE_BASE64"),
    ));
    report.checks.push(warning_check(
        "signing.android.secret_not_imported",
        "keystore files and passwords were not stored in fission.toml; use CI secrets, environment variables, or an OS-backed secret store".to_string(),
    ));
    Ok(())
}

fn build_status_report(area: &str, project_dir: &Path, target: Target) -> LifecycleReport {
    let mut report = base_report(area, None, Some(target));
    let config_path = project_dir.join("fission.toml");
    report.checks.push(path_check(
        "signing.project_config_exists",
        config_path.clone(),
        "fission.toml exists",
    ));
    let config = signing_config(&config_path).unwrap_or_default();
    match target {
        Target::Android => android_checks(
            project_dir,
            config.package.and_then(|p| p.android),
            &mut report,
        ),
        Target::Ios => {
            apple_ios_checks(project_dir, config.package.and_then(|p| p.ios), &mut report)
        }
        Target::Macos => {
            let cfg = config.package.and_then(|p| p.macos);
            match fission_command_core::read_macos_package_config_for_profile(project_dir, true) {
                Ok(signing) => macos_checks(project_dir, cfg, &signing, &mut report),
                Err(error) => {
                    report.checks.push(failed_check(
                        "signing.macos.config",
                        format!("failed to resolve macOS release signing configuration: {error}"),
                    ));
                    macos_checks(
                        project_dir,
                        cfg,
                        &fission_command_core::MacosPackageConfig::default(),
                        &mut report,
                    );
                }
            }
        }
        Target::Windows => windows_checks(
            project_dir,
            config.package.and_then(|p| p.windows),
            &mut report,
        ),
        _ => report.checks.push(warning_check(
            "signing.target",
            format!("{} does not require signing by default", target.as_str()),
        )),
    }
    finalize_status(&mut report);
    report
}

fn android_checks(
    project_dir: &Path,
    cfg: Option<AndroidPackageToml>,
    report: &mut LifecycleReport,
) {
    let _ = project_dir;
    let keystore_env = cfg
        .as_ref()
        .and_then(|cfg| cfg.keystore_env.as_deref())
        .unwrap_or("ANDROID_KEYSTORE");
    let keystore_base64_env = cfg
        .as_ref()
        .and_then(|cfg| cfg.keystore_base64_env.as_deref())
        .unwrap_or("ANDROID_KEYSTORE_BASE64");
    let store_password_env = cfg
        .as_ref()
        .and_then(|cfg| cfg.keystore_password_env.as_deref())
        .unwrap_or("ANDROID_KEYSTORE_PASSWORD");
    let key_password_env = cfg
        .as_ref()
        .and_then(|cfg| cfg.key_password_env.as_deref())
        .unwrap_or("ANDROID_KEY_PASSWORD");
    report.checks.push(required_text(
        "signing.android.package_name",
        cfg.as_ref().and_then(|cfg| cfg.package_name.as_deref()),
        "Android package name is configured",
        "Set package.android.package_name in fission.toml.",
    ));
    report.checks.push(required_text(
        "signing.android.alias",
        cfg.as_ref().and_then(|cfg| cfg.keystore_alias.as_deref()),
        "Android keystore alias is configured",
        "Set package.android.keystore_alias or ANDROID_KEYSTORE_ALIAS.",
    ));
    report.checks.push(android_keystore_source_check(
        keystore_env,
        keystore_base64_env,
    ));
    report.checks.push(env_or_warning(
        "signing.android.keystore_password",
        &[store_password_env, key_password_env],
        "Android signing password source is configured",
        "Set Android signing passwords through CI/local environment variables; do not write passwords to fission.toml.",
    ));
    report.checks.push(tool_check(
        "signing.android.keytool_available",
        "keytool",
        "Install a JDK so Fission can inspect Android keystores.",
    ));
    report.checks.push(tool_check(
        "signing.android.apksigner_available",
        "apksigner",
        "Install Android build-tools and ensure apksigner is on PATH.",
    ));
}

fn apple_ios_checks(
    project_dir: &Path,
    cfg: Option<ApplePackageToml>,
    report: &mut LifecycleReport,
) {
    report.checks.push(host_os_check_local(
        "signing.apple.host_is_macos",
        "Apple signing and provisioning checks require macOS.",
    ));
    report.checks.push(required_text(
        "signing.ios.bundle_id",
        cfg.as_ref().and_then(|cfg| cfg.bundle_id.as_deref()),
        "iOS bundle identifier is configured",
        "Set package.ios.bundle_id.",
    ));
    report.checks.push(required_text(
        "signing.ios.team_id",
        cfg.as_ref().and_then(|cfg| cfg.team_id.as_deref()),
        "Apple team id is configured",
        "Set package.ios.team_id.",
    ));
    check_optional_path(
        project_dir,
        &mut report.checks,
        "signing.ios.entitlements",
        cfg.as_ref().and_then(|cfg| cfg.entitlements.as_deref()),
        "iOS entitlements file exists",
    );
    check_optional_path(
        project_dir,
        &mut report.checks,
        "signing.ios.provisioning_profile",
        cfg.as_ref()
            .and_then(|cfg| cfg.provisioning_profile.as_deref()),
        "iOS provisioning profile exists",
    );
    report.checks.push(tool_check(
        "signing.apple.xcrun_available",
        "xcrun",
        "Install Xcode command line tools.",
    ));
    report.checks.push(tool_check(
        "signing.apple.security_available",
        "security",
        "Run on macOS with the security tool available.",
    ));
    report.checks.push(apple_identity_check(
        cfg.as_ref().and_then(|cfg| cfg.signing_identity.as_deref()),
    ));
}

fn macos_checks(
    project_dir: &Path,
    cfg: Option<MacosPackageToml>,
    signing: &fission_command_core::MacosPackageConfig,
    report: &mut LifecycleReport,
) {
    report.checks.push(host_os_check_local(
        "signing.apple.host_is_macos",
        "macOS signing and notarization checks require macOS.",
    ));
    report.checks.push(required_text(
        "signing.macos.bundle_id",
        cfg.as_ref().and_then(|cfg| cfg.bundle_id.as_deref()),
        "macOS bundle identifier is configured",
        "Set package.macos.bundle_id.",
    ));
    report.checks.push(required_text(
        "signing.macos.team_id",
        cfg.as_ref().and_then(|cfg| cfg.team_id.as_deref()),
        "Apple team id is configured for macOS signing",
        "Set package.macos.team_id when signing or notarizing macOS packages.",
    ));
    check_optional_path(
        project_dir,
        &mut report.checks,
        "signing.macos.entitlements",
        signing.entitlements.as_deref(),
        "macOS entitlements file exists",
    );
    report.checks.push(required_text(
        "signing.macos.identity",
        signing.signing_identity.as_deref(),
        "Developer ID Application signing identity is configured",
        "Set package.macos.release.signing_identity for release-only signing.",
    ));
    report.checks.push(tool_check(
        "signing.apple.codesign_available",
        "codesign",
        "Run on macOS with Xcode command line tools installed.",
    ));
    report
        .checks
        .push(apple_identity_check(signing.signing_identity.as_deref()));
    if signing.notarize.unwrap_or(false) {
        report.checks.push(required_text(
            "signing.macos.installer_identity",
            signing.installer_identity.as_deref(),
            "Developer ID Installer identity is configured for pkg signing",
            "Set package.macos.release.installer_identity when release notarization is enabled.",
        ));
        report.checks.push(env_or_missing(
            "signing.apple.notary_key",
            &[
                "APP_STORE_CONNECT_API_KEY_PATH",
                "APP_STORE_CONNECT_API_KEY",
                "APP_STORE_CONNECT_API_KEY_BASE64",
            ],
            "App Store Connect API key material is configured for notarization",
            "Set APP_STORE_CONNECT_API_KEY_PATH, APP_STORE_CONNECT_API_KEY, or APP_STORE_CONNECT_API_KEY_BASE64 in the release environment.",
        ));
        report.checks.push(env_or_missing(
            "signing.apple.notary_key_id",
            &["APP_STORE_CONNECT_KEY_ID"],
            "APP_STORE_CONNECT_KEY_ID is configured for notarization",
            "Set APP_STORE_CONNECT_KEY_ID in the release environment.",
        ));
        report.checks.push(env_or_missing(
            "signing.apple.notary_issuer_id",
            &["APP_STORE_CONNECT_ISSUER_ID"],
            "APP_STORE_CONNECT_ISSUER_ID is configured for notarization",
            "Set APP_STORE_CONNECT_ISSUER_ID in the release environment.",
        ));
        report.checks.push(tool_check(
            "signing.apple.xcrun_available",
            "xcrun",
            "Install Xcode command line tools for notarytool and stapler.",
        ));
    }
}

fn windows_checks(
    project_dir: &Path,
    cfg: Option<WindowsPackageToml>,
    report: &mut LifecycleReport,
) {
    let _ = project_dir;
    report.checks.push(required_text(
        "signing.windows.identity_name",
        cfg.as_ref().and_then(|cfg| cfg.identity_name.as_deref()),
        "Windows package identity name is configured",
        "Set package.windows.identity_name.",
    ));
    report.checks.push(required_text(
        "signing.windows.publisher",
        cfg.as_ref().and_then(|cfg| cfg.publisher.as_deref()),
        "Windows publisher identity is configured",
        "Set package.windows.publisher to the certificate subject.",
    ));
    if windows_certificate_thumbprint(&cfg).is_none() {
        let certificate_env = cfg
            .as_ref()
            .and_then(|cfg| cfg.certificate_env.as_deref())
            .unwrap_or("WINDOWS_CERTIFICATE");
        let certificate_base64_env = cfg
            .as_ref()
            .and_then(|cfg| cfg.certificate_base64_env.as_deref())
            .unwrap_or("WINDOWS_CERTIFICATE_BASE64");
        let certificate_thumbprint_env = cfg
            .as_ref()
            .and_then(|cfg| cfg.certificate_thumbprint_env.as_deref())
            .unwrap_or("WINDOWS_CERTIFICATE_THUMBPRINT");
        report.checks.push(required_text(
            "signing.windows.certificate_reference",
            env::var(certificate_env)
                .ok()
                .or_else(|| env::var(certificate_base64_env).ok())
                .or_else(|| env::var(certificate_thumbprint_env).ok())
                .as_deref(),
            "Windows signing certificate reference is configured",
            "Set package.windows.certificate_thumbprint, package.windows.certificate_thumbprint_env, WINDOWS_CERTIFICATE_THUMBPRINT, WINDOWS_CERTIFICATE, or WINDOWS_CERTIFICATE_BASE64.",
        ));
        let password_env = cfg
            .as_ref()
            .and_then(|cfg| cfg.certificate_password_env.as_deref())
            .unwrap_or("WINDOWS_CERTIFICATE_PASSWORD");
        report.checks.push(env_or_warning(
            "signing.windows.certificate_password",
            &[password_env],
            "Windows certificate password source is configured",
            "Set the Windows certificate password through CI/local environment variables; do not write passwords to fission.toml.",
        ));
    } else {
        report.checks.push(ok_check(
            "signing.windows.certificate_reference",
            "Windows signing certificate thumbprint configured",
        ));
    }
    report.checks.push(tool_check(
        "signing.windows.signtool_available",
        "signtool",
        "Install Windows SDK signing tools and ensure signtool is on PATH.",
    ));
}

fn signing_config(path: &Path) -> Result<SigningToml> {
    if !path.exists() {
        return Ok(SigningToml::default());
    }
    let data = fs::read_to_string(path)?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn check_optional_path(
    project_dir: &Path,
    checks: &mut Vec<LifecycleCheck>,
    id: &str,
    path: Option<&str>,
    summary: &str,
) {
    if let Some(path) = path.filter(|path| !path.trim().is_empty()) {
        checks.push(path_check(id, project_dir.join(path), summary));
    } else {
        checks.push(required_text(
            id,
            None,
            summary,
            "Configure this path in fission.toml if the app requires the capability.",
        ));
    }
}

fn windows_certificate_thumbprint(cfg: &Option<WindowsPackageToml>) -> Option<String> {
    cfg.as_ref()
        .and_then(|cfg| cfg.certificate_thumbprint.clone())
        .or_else(|| {
            let env_name = cfg
                .as_ref()
                .and_then(|cfg| cfg.certificate_thumbprint_env.as_deref())
                .unwrap_or("WINDOWS_CERTIFICATE_THUMBPRINT");
            env::var(env_name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
}

fn android_keystore_source_check(keystore_env: &str, keystore_base64_env: &str) -> LifecycleCheck {
    if env::var_os(keystore_base64_env).is_some() {
        return LifecycleCheck {
            id: "signing.android.keystore_source".to_string(),
            status: "passed".to_string(),
            summary: "Android upload keystore source is configured".to_string(),
            details: Some(format!("environment variable {keystore_base64_env}")),
            remediation: Vec::new(),
        };
    }
    if let Some(path) = env::var_os(keystore_env) {
        let path = PathBuf::from(path);
        return path_check(
            "signing.android.keystore_source",
            path,
            "Android upload keystore source is configured",
        );
    }
    LifecycleCheck {
        id: "signing.android.keystore_source".to_string(),
        status: "missing".to_string(),
        summary: "Android upload keystore source is configured".to_string(),
        details: None,
        remediation: vec![format!(
            "Set {keystore_env} to a local keystore path or {keystore_base64_env} to base64 keystore bytes from CI secrets."
        )],
    }
}

fn apple_identity_check(expected: Option<&str>) -> LifecycleCheck {
    if !cfg!(target_os = "macos") {
        return LifecycleCheck {
            id: "signing.apple.identity_available".to_string(),
            status: "warning".to_string(),
            summary: "Apple code signing identity is available".to_string(),
            details: Some("identity lookup requires macOS".to_string()),
            remediation: vec![
                "Run this check on a macOS release machine or remote builder.".to_string(),
            ],
        };
    }
    let output = Command::new("security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let found = expected.is_some_and(|needle| stdout.contains(needle));
            LifecycleCheck {
                id: "signing.apple.identity_available".to_string(),
                status: if expected.is_none() || found { "passed" } else { "missing" }.to_string(),
                summary: "Apple code signing identity is available".to_string(),
                details: expected.map(|expected| format!("expected identity: {expected}")),
                remediation: vec!["Install the certificate in the login keychain or update the configured signing identity.".to_string()],
            }
        }
        Ok(output) => LifecycleCheck {
            id: "signing.apple.identity_available".to_string(),
            status: "failed".to_string(),
            summary: "Apple code signing identity lookup succeeds".to_string(),
            details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            remediation: vec![
                "Unlock the keychain and ensure Xcode command line tools are installed."
                    .to_string(),
            ],
        },
        Err(error) => failed_check("signing.apple.identity_available", error.to_string()),
    }
}

fn required_text(
    id: &str,
    value: Option<&str>,
    summary: &str,
    remediation: &str,
) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: if value.is_some_and(|value| !value.trim().is_empty()) {
            "passed"
        } else {
            "missing"
        }
        .to_string(),
        summary: summary.to_string(),
        details: value.map(str::to_string),
        remediation: vec![remediation.to_string()],
    }
}

fn env_or_missing(id: &str, vars: &[&str], summary: &str, remediation: &str) -> LifecycleCheck {
    let found = vars.iter().find(|name| env::var_os(name).is_some());
    LifecycleCheck {
        id: id.to_string(),
        status: if found.is_some() { "passed" } else { "missing" }.to_string(),
        summary: summary.to_string(),
        details: found.map(|name| (*name).to_string()),
        remediation: vec![remediation.to_string()],
    }
}

fn env_or_warning(id: &str, vars: &[&str], summary: &str, remediation: &str) -> LifecycleCheck {
    let found = vars.iter().find(|name| env::var_os(name).is_some());
    LifecycleCheck {
        id: id.to_string(),
        status: if found.is_some() { "passed" } else { "warning" }.to_string(),
        summary: summary.to_string(),
        details: found.map(|name| (*name).to_string()),
        remediation: vec![remediation.to_string()],
    }
}

fn host_os_check_local(id: &str, remediation: &str) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: if cfg!(target_os = "macos") {
            "passed"
        } else {
            "missing"
        }
        .to_string(),
        summary: "host operating system supports this signing flow".to_string(),
        details: Some(env::consts::OS.to_string()),
        remediation: vec![remediation.to_string()],
    }
}

fn tool_check(id: &str, program: &str, remediation: &str) -> LifecycleCheck {
    LifecycleCheck {
        id: id.to_string(),
        status: if command_exists(program) {
            "passed"
        } else {
            "missing"
        }
        .to_string(),
        summary: format!("{program} is available on PATH"),
        details: env::var_os("PATH").map(|_| program.to_string()),
        remediation: vec![remediation.to_string()],
    }
}

fn command_exists(program: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|dir| {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return true;
        }
        if cfg!(windows) {
            ["exe", "cmd", "bat"]
                .iter()
                .any(|ext| dir.join(format!("{program}.{ext}")).is_file())
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_import_writes_only_env_references() {
        let dir = std::env::temp_dir().join(format!(
            "fission-signing-import-{}-{}",
            std::process::id(),
            now_unix_seconds()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("upload.jks"), "not a real keystore").unwrap();
        fs::write(
            &dir.join("fission.toml"),
            "[package.android]\npackage_name = \"com.example.todo\"\n",
        )
        .unwrap();
        let mut report = base_report("test", None, Some(Target::Android));
        import_android(
            &dir,
            Some(dir.join("upload.jks")),
            Some("upload".to_string()),
            false,
            &mut report,
        )
        .unwrap();
        let text = fs::read_to_string(dir.join("fission.toml")).unwrap();
        assert!(text.contains("keystore_alias = \"upload\""));
        assert!(text.contains("keystore_env = \"ANDROID_KEYSTORE\""));
        assert!(text.contains("keystore_base64_env = \"ANDROID_KEYSTORE_BASE64\""));
        assert!(!text.contains("upload.jks"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn android_import_dry_run_does_not_rewrite_manifest() {
        let dir = std::env::temp_dir().join(format!(
            "fission-signing-import-dry-run-{}-{}",
            std::process::id(),
            now_unix_seconds()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &dir.join("fission.toml"),
            "[package.android]\npackage_name = \"com.example.todo\"\n",
        )
        .unwrap();
        let original = fs::read_to_string(dir.join("fission.toml")).unwrap();
        let mut report = base_report("test", None, Some(Target::Android));
        import_android(&dir, None, Some("upload".to_string()), true, &mut report).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("fission.toml")).unwrap(),
            original
        );
        assert!(report
            .checks
            .iter()
            .any(|check| check.id == "signing.android.config_would_write"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn signing_import_requires_yes_before_manifest_rewrite() {
        let dir = std::env::temp_dir().join(format!(
            "fission-signing-import-requires-yes-{}-{}",
            std::process::id(),
            now_unix_seconds()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(&dir.join("fission.toml"), "").unwrap();
        let err = import(
            &dir,
            Target::Android,
            None,
            Some("upload".to_string()),
            false,
            false,
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("pass --yes"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn windows_certificate_thumbprint_can_come_from_named_env() {
        let env_name = format!("FISSION_WINDOWS_THUMBPRINT_{}", std::process::id());
        env::set_var(&env_name, "ABCDEF123456");
        let cfg = Some(WindowsPackageToml {
            certificate_thumbprint_env: Some(env_name.clone()),
            ..Default::default()
        });

        assert_eq!(
            windows_certificate_thumbprint(&cfg),
            Some("ABCDEF123456".to_string())
        );

        env::remove_var(env_name);
    }

    #[test]
    fn macos_signing_status_resolves_release_overlay() {
        let dir = std::env::temp_dir().join(format!(
            "fission-macos-release-signing-status-{}-{}",
            std::process::id(),
            now_unix_seconds()
        ));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("fission.toml"),
            r#"
[package.macos]
bundle_id = "com.example.app"
team_id = "ABCDE12345"
signing_identity = "-"

[package.macos.release]
entitlements = "platforms/macos/Release.entitlements"
signing_identity = "Developer ID Application: Example Ltd"
installer_identity = "Developer ID Installer: Example Ltd"
notarize = true
"#,
        )
        .unwrap();
        let report = build_status_report("test", &dir, Target::Macos);
        let check = |id: &str| report.checks.iter().find(|check| check.id == id).unwrap();

        assert_eq!(check("signing.macos.identity").status, "passed");
        assert_eq!(
            check("signing.macos.identity").details.as_deref(),
            Some("Developer ID Application: Example Ltd")
        );
        assert_eq!(
            check("signing.macos.installer_identity").details.as_deref(),
            Some("Developer ID Installer: Example Ltd")
        );
        assert!(report
            .checks
            .iter()
            .all(|check| check.id != "signing.macos.config"));
        fs::remove_dir_all(dir).unwrap();
    }
}
