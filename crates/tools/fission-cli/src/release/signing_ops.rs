use super::*;
use anyhow::{Context, Result};
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
    keystore: Option<String>,
    upload_keystore: Option<String>,
    keystore_alias: Option<String>,
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
    entitlements: Option<String>,
    signing_identity: Option<String>,
    installer_identity: Option<String>,
    notarize: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct WindowsPackageToml {
    identity_name: Option<String>,
    publisher: Option<String>,
    certificate: Option<String>,
    certificate_thumbprint: Option<String>,
}

pub(super) fn status(project_dir: &Path, target: Target, json: bool) -> Result<()> {
    print_report(
        build_status_report("signing.status", project_dir, target),
        json,
    )
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
    json: bool,
) -> Result<()> {
    let mut report = base_report("signing.import", None, Some(target));
    report.checks.push(path_check(
        "signing.project_config_exists",
        project_dir.join("fission.toml"),
        "fission.toml exists",
    ));
    match target {
        Target::Android => import_android(project_dir, keystore, alias, &mut report)?,
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
    report: &mut LifecycleReport,
) -> Result<()> {
    let keystore = keystore.context("signing import --target android requires --keystore")?;
    let alias = alias.context("signing import --target android requires --alias")?;
    report.checks.push(path_check(
        "signing.android.keystore_exists",
        keystore.clone(),
        "Android upload keystore exists",
    ));
    if !keystore.exists() {
        return Ok(());
    }
    let relative = project_relative_or_absolute(project_dir, &keystore);
    let path = project_dir.join("fission.toml");
    let data = fs::read_to_string(&path).unwrap_or_default();
    let mut root: toml::Value = if data.trim().is_empty() {
        toml::Value::Table(Default::default())
    } else {
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?
    };
    set_toml_path(
        &mut root,
        "package.android.keystore",
        toml::Value::String(relative.clone()),
    )?;
    set_toml_path(
        &mut root,
        "package.android.keystore_alias",
        toml::Value::String(alias.clone()),
    )?;
    fs::write(&path, toml::to_string_pretty(&root)? + "\n")
        .with_context(|| format!("failed to write {}", path.display()))?;
    report.checks.push(ok_check(
        "signing.android.config_written",
        format!("package.android.keystore = {relative}, alias = {alias}"),
    ));
    report.checks.push(warning_check(
        "signing.android.password_not_imported",
        "keystore passwords were not stored in fission.toml; use ANDROID_KEYSTORE_PASSWORD and ANDROID_KEY_PASSWORD in CI or an OS-backed secret store".to_string(),
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
        Target::Macos => macos_checks(
            project_dir,
            config.package.and_then(|p| p.macos),
            &mut report,
        ),
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
    let keystore = env::var("ANDROID_KEYSTORE")
        .ok()
        .or_else(|| cfg.as_ref().and_then(|cfg| cfg.keystore.clone()))
        .or_else(|| cfg.as_ref().and_then(|cfg| cfg.upload_keystore.clone()));
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
        "Run `fission signing import --target android --keystore <file> --alias <alias>`.",
    ));
    match keystore {
        Some(path) => report.checks.push(path_check(
            "signing.android.keystore_exists",
            project_dir.join(path),
            "Android upload keystore exists",
        )),
        None => report.checks.push(required_text(
            "signing.android.keystore",
            None,
            "Android upload keystore is configured",
            "Set ANDROID_KEYSTORE or package.android.keystore.",
        )),
    }
    report.checks.push(env_or_warning(
        "signing.android.keystore_password",
        &["ANDROID_KEYSTORE_PASSWORD", "ANDROID_KEY_PASSWORD"],
        "Android signing password source is configured",
        "Set ANDROID_KEYSTORE_PASSWORD and ANDROID_KEY_PASSWORD in CI or an OS-backed secret store; do not write passwords to fission.toml.",
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

fn macos_checks(project_dir: &Path, cfg: Option<MacosPackageToml>, report: &mut LifecycleReport) {
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
    check_optional_path(
        project_dir,
        &mut report.checks,
        "signing.macos.entitlements",
        cfg.as_ref().and_then(|cfg| cfg.entitlements.as_deref()),
        "macOS entitlements file exists",
    );
    report.checks.push(required_text(
        "signing.macos.identity",
        cfg.as_ref().and_then(|cfg| cfg.signing_identity.as_deref()),
        "Developer ID Application signing identity is configured",
        "Set package.macos.signing_identity.",
    ));
    report.checks.push(tool_check(
        "signing.apple.codesign_available",
        "codesign",
        "Run on macOS with Xcode command line tools installed.",
    ));
    report.checks.push(apple_identity_check(
        cfg.as_ref().and_then(|cfg| cfg.signing_identity.as_deref()),
    ));
    if cfg.as_ref().and_then(|cfg| cfg.notarize).unwrap_or(false) {
        report.checks.push(required_text(
            "signing.macos.installer_identity",
            cfg.as_ref()
                .and_then(|cfg| cfg.installer_identity.as_deref()),
            "Developer ID Installer identity is configured for pkg signing",
            "Set package.macos.installer_identity when package.macos.notarize = true.",
        ));
        for (id, name) in [
            (
                "signing.apple.notary_key_path",
                "APP_STORE_CONNECT_API_KEY_PATH",
            ),
            ("signing.apple.notary_key_id", "APP_STORE_CONNECT_KEY_ID"),
            (
                "signing.apple.notary_issuer_id",
                "APP_STORE_CONNECT_ISSUER_ID",
            ),
        ] {
            report.checks.push(env_or_missing(
                id,
                &[name],
                &format!("{name} is configured for notarization"),
                &format!("Set {name} in the release environment."),
            ));
        }
    }
}

fn windows_checks(
    project_dir: &Path,
    cfg: Option<WindowsPackageToml>,
    report: &mut LifecycleReport,
) {
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
    if let Some(certificate) = cfg.as_ref().and_then(|cfg| cfg.certificate.as_deref()) {
        report.checks.push(path_check(
            "signing.windows.certificate_exists",
            project_dir.join(certificate),
            "Windows signing certificate file exists",
        ));
    } else {
        report.checks.push(required_text(
            "signing.windows.certificate_reference",
            cfg.as_ref()
                .and_then(|cfg| cfg.certificate_thumbprint.as_deref()),
            "Windows signing certificate reference is configured",
            "Set package.windows.certificate or package.windows.certificate_thumbprint.",
        ));
    }
    report.checks.push(env_or_warning(
        "signing.windows.certificate_password",
        &["WINDOWS_CERTIFICATE_PASSWORD"],
        "Windows certificate password source is configured",
        "Set WINDOWS_CERTIFICATE_PASSWORD in CI or use an OS certificate store; do not write passwords to fission.toml.",
    ));
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

fn project_relative_or_absolute(project_dir: &Path, path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .unwrap_or_else(|_| project_dir.to_path_buf())
            .join(path)
    };
    absolute
        .strip_prefix(project_dir)
        .unwrap_or(&absolute)
        .to_string_lossy()
        .trim_start_matches(std::path::MAIN_SEPARATOR)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_import_writes_non_secret_references() {
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
            &mut report,
        )
        .unwrap();
        let text = fs::read_to_string(dir.join("fission.toml")).unwrap();
        assert!(text.contains("keystore = \"upload.jks\""));
        assert!(text.contains("keystore_alias = \"upload\""));
        let _ = fs::remove_dir_all(&dir);
    }
}
