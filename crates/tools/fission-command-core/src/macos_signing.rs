use crate::NativeVariant;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
#[cfg(any(target_os = "macos", test))]
use sha1::{Digest as _, Sha1};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(any(target_os = "macos", test))]
use std::time::SystemTime;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct MacosPackageConfig {
    pub bundle_id: Option<String>,
    pub team_id: Option<String>,
    pub minimum_os: Option<String>,
    pub application_category: Option<String>,
    pub entitlements: Option<String>,
    pub provisioning_profile: Option<String>,
    pub signing_identity: Option<String>,
    pub installer_identity: Option<String>,
    pub notarize: Option<bool>,
    pub pkg_builder: Option<String>,
    #[serde(default)]
    pub cargo_features: Vec<String>,
    #[serde(default)]
    pub cargo_no_default_features: bool,
}

#[derive(Debug, Default, Deserialize)]
struct PackageManifest {
    package: Option<PackageRoot>,
    run: Option<RunRoot>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageRoot {
    macos: Option<MacosPackageManifest>,
}

#[derive(Debug, Default, Deserialize)]
struct RunRoot {
    macos: Option<MacosRunConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct MacosRunConfig {
    entitlements: Option<String>,
    provisioning_profile: Option<String>,
    signing_identity: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
struct MacosPackageManifest {
    #[serde(flatten)]
    base: MacosPackageConfig,
    release: Option<MacosPackageOverlay>,
    #[serde(default)]
    variants: BTreeMap<NativeVariant, MacosPackageOverlay>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
struct MacosPackageOverlay {
    application_category: Option<String>,
    entitlements: Option<String>,
    provisioning_profile: Option<String>,
    signing_identity: Option<String>,
    installer_identity: Option<String>,
    notarize: Option<bool>,
    pkg_builder: Option<String>,
    cargo_features: Option<Vec<String>>,
    cargo_no_default_features: Option<bool>,
}

impl MacosPackageManifest {
    fn effective(&self, release: bool, variant: Option<&NativeVariant>) -> MacosPackageConfig {
        let mut config = self.base.clone();
        if release {
            if let Some(overlay) = &self.release {
                overlay.apply_to(&mut config);
            }
        }
        if let Some(overlay) = variant.and_then(|variant| self.variants.get(variant)) {
            overlay.apply_to(&mut config);
        }
        config
    }
}

impl MacosPackageOverlay {
    fn apply_to(&self, config: &mut MacosPackageConfig) {
        if self.application_category.is_some() {
            config
                .application_category
                .clone_from(&self.application_category);
        }
        if self.entitlements.is_some() {
            config.entitlements.clone_from(&self.entitlements);
        }
        if self.provisioning_profile.is_some() {
            config
                .provisioning_profile
                .clone_from(&self.provisioning_profile);
        }
        if self.signing_identity.is_some() {
            config.signing_identity.clone_from(&self.signing_identity);
        }
        if self.installer_identity.is_some() {
            config
                .installer_identity
                .clone_from(&self.installer_identity);
        }
        if self.notarize.is_some() {
            config.notarize = self.notarize;
        }
        if self.pkg_builder.is_some() {
            config.pkg_builder.clone_from(&self.pkg_builder);
        }
        if let Some(features) = &self.cargo_features {
            config.cargo_features.clone_from(features);
        }
        if let Some(no_default_features) = self.cargo_no_default_features {
            config.cargo_no_default_features = no_default_features;
        }
    }
}

pub fn read_macos_package_config(project_dir: &Path) -> Result<MacosPackageConfig> {
    read_macos_package_config_for_profile(project_dir, false)
}

pub fn read_macos_package_config_for_profile(
    project_dir: &Path,
    release: bool,
) -> Result<MacosPackageConfig> {
    read_macos_package_config_for_profile_and_variant(project_dir, release, None)
}

pub fn read_macos_package_config_for_profile_and_variant(
    project_dir: &Path,
    release: bool,
    variant: Option<&NativeVariant>,
) -> Result<MacosPackageConfig> {
    let manifest = read_manifest(project_dir)?;
    Ok(package_config(manifest.package.as_ref(), release, variant))
}

pub fn read_macos_run_config(project_dir: &Path) -> Result<MacosPackageConfig> {
    read_macos_run_config_for_profile(project_dir, false)
}

pub fn read_macos_run_config_for_profile(
    project_dir: &Path,
    release: bool,
) -> Result<MacosPackageConfig> {
    Ok(run_config(&read_manifest(project_dir)?, release))
}

fn run_config(manifest: &PackageManifest, release: bool) -> MacosPackageConfig {
    let run = manifest.run.as_ref().and_then(|run| run.macos.as_ref());
    let mut config = package_config(manifest.package.as_ref(), release, None);
    if let Some(run) = run {
        if run.entitlements.is_some() {
            config.entitlements.clone_from(&run.entitlements);
        }
        if run.provisioning_profile.is_some() {
            config
                .provisioning_profile
                .clone_from(&run.provisioning_profile);
        }
        if run.signing_identity.is_some() {
            config.signing_identity.clone_from(&run.signing_identity);
        }
    }
    config
}

fn read_manifest(project_dir: &Path) -> Result<PackageManifest> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn package_config(
    package: Option<&PackageRoot>,
    release: bool,
    variant: Option<&NativeVariant>,
) -> MacosPackageConfig {
    package
        .and_then(|package| package.macos.as_ref())
        .map(|macos| macos.effective(release, variant))
        .unwrap_or_default()
}

pub fn sign_macos_app_if_configured(
    project_dir: &Path,
    app_bundle: &Path,
    macos: &MacosPackageConfig,
) -> Result<()> {
    let identity = macos
        .signing_identity
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let profile = macos
        .provisioning_profile
        .as_deref()
        .filter(|value| !value.trim().is_empty());

    if profile.is_some() && identity.is_none_or(|value| value == "-") {
        bail!(
            "macOS provisioning_profile requires a real effective package.macos signing identity or run.macos.signing_identity; ad-hoc signing with `-` cannot embed a provisioning profile"
        );
    }
    if let Some(profile) = profile {
        validate_macos_provisioning_profile(project_dir, macos, identity.expect("validated"))?;
        embed_macos_provisioning_profile(project_dir, app_bundle, profile)?;
    }
    remove_macos_bundle_extended_attributes(app_bundle)?;
    make_macos_bundle_world_readable(app_bundle)?;

    let Some(identity) = identity else {
        return Ok(());
    };

    let status = Command::new("codesign")
        .args(codesign_arguments(project_dir, identity, macos))
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

#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Deserialize)]
struct DecodedMacosProvisioningProfile {
    #[serde(rename = "TeamIdentifier")]
    team_identifiers: Vec<String>,
    #[serde(rename = "DeveloperCertificates")]
    developer_certificates: Vec<plist::Value>,
    #[serde(rename = "ProvisionedDevices", default)]
    provisioned_devices: Vec<String>,
    #[serde(rename = "ExpirationDate")]
    expiration_date: plist::Date,
    #[serde(rename = "Entitlements")]
    entitlements: BTreeMap<String, plist::Value>,
}

#[cfg(target_os = "macos")]
fn validate_macos_provisioning_profile(
    project_dir: &Path,
    macos: &MacosPackageConfig,
    signing_identity: &str,
) -> Result<()> {
    let profile = macos
        .provisioning_profile
        .as_deref()
        .expect("profile validation is called only when configured");
    let profile_path = resolve_project_path(project_dir, profile);
    if !profile_path.is_file() {
        bail!(
            "macOS provisioning profile does not exist or is not a file: {}",
            profile_path.display()
        );
    }
    let decoded = Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(&profile_path)
        .output()
        .context("failed to decode the macOS provisioning profile with `security cms`")?;
    if !decoded.status.success() {
        bail!(
            "macOS provisioning profile could not be verified by `security cms`: {}",
            String::from_utf8_lossy(&decoded.stderr).trim()
        );
    }
    let profile: DecodedMacosProvisioningProfile = plist::from_bytes(&decoded.stdout)
        .context("failed to parse decoded provisioning profile")?;
    let identity = resolve_codesigning_identity(signing_identity)?;
    let host_udid = current_macos_provisioning_udid()?;
    validate_macos_profile_bindings(&profile, macos, &identity, host_udid.as_deref())
}

#[cfg(not(target_os = "macos"))]
fn validate_macos_provisioning_profile(
    _project_dir: &Path,
    _macos: &MacosPackageConfig,
    _signing_identity: &str,
) -> Result<()> {
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Eq, PartialEq)]
struct ResolvedCodesigningIdentity {
    certificate_sha1: String,
    display_name: String,
}

#[cfg(target_os = "macos")]
fn resolve_codesigning_identity(requested: &str) -> Result<ResolvedCodesigningIdentity> {
    let output = Command::new("security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output()
        .context("failed to query macOS code-signing identities")?;
    if !output.status.success() {
        bail!("macOS code-signing identities could not be queried");
    }
    let mut matches =
        matching_codesigning_identities(&String::from_utf8_lossy(&output.stdout), requested);
    if matches.len() != 1 {
        bail!(
            "macOS signing identity `{requested}` resolved to {} valid identities; configure one unambiguous certificate name or SHA-1 fingerprint",
            matches.len()
        );
    }
    Ok(matches.remove(0))
}

#[cfg(any(target_os = "macos", test))]
fn parse_codesigning_identity(line: &str) -> Option<ResolvedCodesigningIdentity> {
    let trimmed = line.trim();
    let (_, after_index) = trimmed.split_once(')')?;
    let (fingerprint, quoted_name) = after_index.trim().split_once(' ')?;
    let display_name = quoted_name.strip_prefix('"')?.strip_suffix('"')?;
    if fingerprint.len() != 40
        || !fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit())
        || display_name.is_empty()
    {
        return None;
    }
    Some(ResolvedCodesigningIdentity {
        certificate_sha1: fingerprint.to_ascii_uppercase(),
        display_name: display_name.to_owned(),
    })
}

#[cfg(any(target_os = "macos", test))]
fn matching_codesigning_identities(
    output: &str,
    requested: &str,
) -> Vec<ResolvedCodesigningIdentity> {
    let mut matches = Vec::new();
    for identity in output.lines().filter_map(parse_codesigning_identity) {
        let is_match = identity.certificate_sha1.eq_ignore_ascii_case(requested)
            || identity.display_name == requested
            || identity.display_name.contains(requested);
        let certificate_already_matched =
            matches.iter().any(|matched: &ResolvedCodesigningIdentity| {
                matched
                    .certificate_sha1
                    .eq_ignore_ascii_case(&identity.certificate_sha1)
            });
        if is_match && !certificate_already_matched {
            matches.push(identity);
        }
    }
    matches
}

#[cfg(target_os = "macos")]
fn current_macos_provisioning_udid() -> Result<Option<String>> {
    let output = Command::new("system_profiler")
        .arg("SPHardwareDataType")
        .output()
        .context("failed to query this Mac's Provisioning UDID")?;
    if !output.status.success() {
        bail!("this Mac's Provisioning UDID could not be queried");
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.trim().strip_prefix("Provisioning UDID:"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned))
}

#[cfg(any(target_os = "macos", test))]
fn validate_macos_profile_bindings(
    profile: &DecodedMacosProvisioningProfile,
    macos: &MacosPackageConfig,
    identity: &ResolvedCodesigningIdentity,
    host_udid: Option<&str>,
) -> Result<()> {
    if SystemTime::from(profile.expiration_date) <= SystemTime::now() {
        bail!("macOS provisioning profile has expired");
    }
    let bundle_id = macos
        .bundle_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .context("macOS provisioning profile validation requires package.macos.bundle_id")?;
    let profile_team = profile
        .team_identifiers
        .first()
        .filter(|value| !value.trim().is_empty())
        .context("macOS provisioning profile has no TeamIdentifier")?;
    if profile
        .team_identifiers
        .iter()
        .any(|team| team != profile_team)
    {
        bail!("macOS provisioning profile contains conflicting team identifiers");
    }
    if let Some(configured_team) = macos
        .team_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if configured_team != profile_team {
            bail!(
                "macOS provisioning profile team `{profile_team}` does not match configured team `{configured_team}`"
            );
        }
    }
    let application_identifier = profile
        .entitlements
        .get("com.apple.application-identifier")
        .and_then(plist::Value::as_string)
        .context("macOS provisioning profile has no application identifier entitlement")?;
    let expected_application_identifier = format!("{profile_team}.{bundle_id}");
    let application_matches = application_identifier == expected_application_identifier
        || application_identifier
            .strip_suffix('*')
            .is_some_and(|prefix| expected_application_identifier.starts_with(prefix));
    if !application_matches {
        bail!(
            "macOS provisioning profile application identifier `{application_identifier}` does not authorize `{expected_application_identifier}`"
        );
    }
    let identity_in_profile = profile
        .developer_certificates
        .iter()
        .filter_map(plist::Value::as_data)
        .any(|certificate| format!("{:X}", Sha1::digest(certificate)) == identity.certificate_sha1);
    if !identity_in_profile {
        bail!(
            "macOS provisioning profile does not include signing certificate `{}` ({})",
            identity.display_name,
            identity.certificate_sha1
        );
    }
    if !profile.provisioned_devices.is_empty() {
        let host_udid = host_udid.context(
            "macOS development provisioning profile is device-bound but this Mac's Provisioning UDID is unavailable",
        )?;
        if !profile
            .provisioned_devices
            .iter()
            .any(|device| device == host_udid)
        {
            bail!(
                "macOS development provisioning profile does not include this Mac's Provisioning UDID `{host_udid}`"
            );
        }
    }
    Ok(())
}

fn embed_macos_provisioning_profile(
    project_dir: &Path,
    app_bundle: &Path,
    profile: &str,
) -> Result<()> {
    let source = resolve_project_path(project_dir, profile);
    if !source.is_file() {
        bail!(
            "macOS provisioning profile does not exist or is not a file: {}",
            source.display()
        );
    }

    let destination = app_bundle.join("Contents/embedded.provisionprofile");
    fs::copy(&source, &destination).with_context(|| {
        format!(
            "failed to embed macOS provisioning profile {} at {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn remove_macos_bundle_extended_attributes(_app_bundle: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("xattr")
            .args(["-c", "-r"])
            .arg(_app_bundle)
            .status()
            .context("failed to remove extended attributes from macOS app bundle")?;
        if !status.success() {
            bail!("xattr failed with {status}");
        }
    }
    Ok(())
}

fn make_macos_bundle_world_readable(app_bundle: &Path) -> Result<()> {
    let mut pending = vec![app_bundle.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect macOS bundle path {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(&path)
                .with_context(|| format!("failed to read macOS bundle path {}", path.display()))?
            {
                pending.push(entry?.path());
            }
        }
        make_world_readable(&path, metadata.permissions())?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_world_readable(path: &Path, mut permissions: fs::Permissions) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let readable = if path.is_dir() { 0o0555 } else { 0o0444 };
    permissions.set_mode(permissions.mode() | readable);
    fs::set_permissions(path, permissions).with_context(|| {
        format!(
            "failed to make macOS bundle path readable: {}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn make_world_readable(_path: &Path, _permissions: fs::Permissions) -> Result<()> {
    Ok(())
}

fn codesign_arguments(
    project_dir: &Path,
    identity: &str,
    macos: &MacosPackageConfig,
) -> Vec<OsString> {
    let mut arguments = vec![
        "--force".into(),
        "--timestamp".into(),
        "--options".into(),
        "runtime".into(),
        "--sign".into(),
        identity.into(),
    ];
    if let Some(entitlements) = macos
        .entitlements
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        arguments.push("--entitlements".into());
        arguments.push(resolve_project_path(project_dir, entitlements).into_os_string());
    }
    arguments
}

fn resolve_project_path(project_dir: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parses_macos_package_signing_configuration() {
        let manifest: PackageManifest = toml::from_str(
            r#"
[package.macos]
bundle_id = "com.example.app"
minimum_os = "14.0"
application_category = "public.app-category.developer-tools"
entitlements = "platforms/macos/App.entitlements"
provisioning_profile = "profiles/Developer.provisionprofile"
signing_identity = "Apple Development"
installer_identity = "Developer ID Installer"
notarize = true

[run.macos]
entitlements = "platforms/macos/Development.entitlements"
provisioning_profile = "profiles/Developer-Local.provisionprofile"
signing_identity = "-"
"#,
        )
        .unwrap();

        assert_eq!(
            package_config(manifest.package.as_ref(), false, None),
            MacosPackageConfig {
                bundle_id: Some("com.example.app".into()),
                minimum_os: Some("14.0".into()),
                application_category: Some("public.app-category.developer-tools".into()),
                entitlements: Some("platforms/macos/App.entitlements".into()),
                provisioning_profile: Some("profiles/Developer.provisionprofile".into()),
                signing_identity: Some("Apple Development".into()),
                installer_identity: Some("Developer ID Installer".into()),
                notarize: Some(true),
                ..Default::default()
            }
        );
        let run = manifest.run.as_ref().unwrap().macos.as_ref().unwrap();
        assert_eq!(
            run.entitlements.as_deref(),
            Some("platforms/macos/Development.entitlements")
        );
        assert_eq!(
            run.provisioning_profile.as_deref(),
            Some("profiles/Developer-Local.provisionprofile")
        );
        assert_eq!(run.signing_identity.as_deref(), Some("-"));
    }

    #[test]
    fn parses_codesigning_identity_output() {
        assert_eq!(
            parse_codesigning_identity(
                r#"  1) A678CDF82B5E6D0031FB0690F74FD365C01FE43D "Apple Development: Example (TEAM123)""#,
            ),
            Some(ResolvedCodesigningIdentity {
                certificate_sha1: "A678CDF82B5E6D0031FB0690F74FD365C01FE43D".into(),
                display_name: "Apple Development: Example (TEAM123)".into(),
            })
        );
        assert!(parse_codesigning_identity("0 valid identities found").is_none());
    }

    #[test]
    fn duplicate_keychain_results_are_one_codesigning_identity() {
        let output = r#"
  1) 00112233445566778899AABBCCDDEEFF00112233 "Developer ID Application: Example (TEAM123)"
  2) 00112233445566778899AABBCCDDEEFF00112233 "Developer ID Application: Example Duplicate Label (TEAM123)"
  3) FFEEDDCCBBAA99887766554433221100FFEEDDCC "Apple Development: Example (TEAM123)"
     3 valid identities found
"#;

        assert_eq!(
            matching_codesigning_identities(output, "00112233445566778899AABBCCDDEEFF00112233"),
            vec![ResolvedCodesigningIdentity {
                certificate_sha1: "00112233445566778899AABBCCDDEEFF00112233".into(),
                display_name: "Developer ID Application: Example (TEAM123)".into(),
            }]
        );
    }

    #[test]
    fn provisioning_profile_parses_apple_certificate_data_values() {
        let decoded: DecodedMacosProvisioningProfile = plist::from_bytes(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>TeamIdentifier</key><array><string>TEAM123</string></array>
<key>DeveloperCertificates</key><array><data>Y2VydGlmaWNhdGU=</data></array>
<key>ExpirationDate</key><date>2030-01-01T00:00:00Z</date>
<key>Entitlements</key><dict>
<key>com.apple.application-identifier</key><string>TEAM123.com.example.app</string>
</dict>
</dict></plist>"#,
        )
        .expect("Apple profile data should deserialize");

        assert_eq!(
            decoded.developer_certificates[0].as_data(),
            Some(b"certificate".as_slice())
        );
    }

    #[test]
    fn profile_bindings_require_bundle_team_certificate_and_device() {
        let certificate = b"certificate-der".to_vec();
        let identity = ResolvedCodesigningIdentity {
            certificate_sha1: format!("{:X}", Sha1::digest(&certificate)),
            display_name: "Apple Development: Example (TEAM123)".into(),
        };
        let mut profile = DecodedMacosProvisioningProfile {
            team_identifiers: vec!["TEAM123".into()],
            developer_certificates: vec![plist::Value::Data(certificate)],
            provisioned_devices: vec!["MAC-UDID".into()],
            expiration_date: (SystemTime::now() + Duration::from_secs(3_600)).into(),
            entitlements: BTreeMap::from([(
                "com.apple.application-identifier".into(),
                plist::Value::String("TEAM123.com.example.app".into()),
            )]),
        };
        let config = MacosPackageConfig {
            bundle_id: Some("com.example.app".into()),
            team_id: Some("TEAM123".into()),
            ..Default::default()
        };

        validate_macos_profile_bindings(&profile, &config, &identity, Some("MAC-UDID")).unwrap();

        let error =
            validate_macos_profile_bindings(&profile, &config, &identity, Some("OTHER-MAC"))
                .unwrap_err();
        assert!(error.to_string().contains("Provisioning UDID"));

        profile.entitlements.insert(
            "com.apple.application-identifier".into(),
            plist::Value::String("TEAM123.com.other.app".into()),
        );
        let error = validate_macos_profile_bindings(&profile, &config, &identity, Some("MAC-UDID"))
            .unwrap_err();
        assert!(error.to_string().contains("does not authorize"));
    }

    #[test]
    fn profile_bindings_reject_a_certificate_not_embedded_in_the_profile() {
        let profile = DecodedMacosProvisioningProfile {
            team_identifiers: vec!["TEAM123".into()],
            developer_certificates: vec![plist::Value::Data(b"different-certificate".to_vec())],
            provisioned_devices: Vec::new(),
            expiration_date: (SystemTime::now() + Duration::from_secs(3_600)).into(),
            entitlements: BTreeMap::from([(
                "com.apple.application-identifier".into(),
                plist::Value::String("TEAM123.com.example.app".into()),
            )]),
        };
        let identity = ResolvedCodesigningIdentity {
            certificate_sha1: format!("{:X}", Sha1::digest(b"selected-certificate")),
            display_name: "Apple Development: Example (TEAM123)".into(),
        };
        let config = MacosPackageConfig {
            bundle_id: Some("com.example.app".into()),
            team_id: Some("TEAM123".into()),
            ..Default::default()
        };

        let error =
            validate_macos_profile_bindings(&profile, &config, &identity, None).unwrap_err();
        assert!(error
            .to_string()
            .contains("does not include signing certificate"));
    }

    #[test]
    fn release_signing_overlay_is_ignored_for_debug_packages() {
        let manifest: PackageManifest = toml::from_str(
            r#"
[package.macos]
bundle_id = "com.example.app"
minimum_os = "14.0"
entitlements = "platforms/macos/Development.entitlements"
signing_identity = "-"

[package.macos.release]
application_category = "public.app-category.utilities"
entitlements = "platforms/macos/Release.entitlements"
provisioning_profile = "profiles/Distribution.provisionprofile"
signing_identity = "Developer ID Application: Example Ltd"
installer_identity = "Developer ID Installer: Example Ltd"
notarize = true
"#,
        )
        .unwrap();

        let debug = package_config(manifest.package.as_ref(), false, None);
        assert_eq!(
            debug.entitlements.as_deref(),
            Some("platforms/macos/Development.entitlements")
        );
        assert_eq!(debug.signing_identity.as_deref(), Some("-"));
        assert_eq!(debug.provisioning_profile, None);
        assert_eq!(debug.installer_identity, None);
        assert_eq!(debug.notarize, None);

        let release = package_config(manifest.package.as_ref(), true, None);
        assert_eq!(
            release.entitlements.as_deref(),
            Some("platforms/macos/Release.entitlements")
        );
        assert_eq!(
            release.application_category.as_deref(),
            Some("public.app-category.utilities")
        );
        assert_eq!(
            release.provisioning_profile.as_deref(),
            Some("profiles/Distribution.provisionprofile")
        );
        assert_eq!(
            release.signing_identity.as_deref(),
            Some("Developer ID Application: Example Ltd")
        );
        assert_eq!(
            release.installer_identity.as_deref(),
            Some("Developer ID Installer: Example Ltd")
        );
        assert_eq!(release.notarize, Some(true));

        let release_run = run_config(&manifest, true);
        assert_eq!(
            release_run.signing_identity.as_deref(),
            Some("Developer ID Application: Example Ltd")
        );
    }

    #[test]
    fn selected_variant_overrides_effective_release_signing() {
        let manifest: PackageManifest = toml::from_str(
            r#"
[package.macos]
bundle_id = "com.example.app"
signing_identity = "-"

[package.macos.release]
entitlements = "platforms/macos/DeveloperId.entitlements"
provisioning_profile = "profiles/DeveloperId.provisionprofile"
signing_identity = "Developer ID Application: Example Ltd"
installer_identity = "Developer ID Installer: Example Ltd"
notarize = true

[package.macos.variants.app-store]
entitlements = "platforms/macos/AppStore.entitlements"
provisioning_profile = "profiles/AppStore.provisionprofile"
signing_identity = "Apple Distribution: Example Ltd"
installer_identity = "3rd Party Mac Developer Installer: Example Ltd"
notarize = false
pkg_builder = "productbuild"
cargo_features = ["macos-app-store"]
cargo_no_default_features = true
"#,
        )
        .unwrap();
        let variant: NativeVariant = "app-store".parse().unwrap();

        let config = package_config(manifest.package.as_ref(), true, Some(&variant));

        assert_eq!(
            config.entitlements.as_deref(),
            Some("platforms/macos/AppStore.entitlements")
        );
        assert_eq!(
            config.provisioning_profile.as_deref(),
            Some("profiles/AppStore.provisionprofile")
        );
        assert_eq!(
            config.signing_identity.as_deref(),
            Some("Apple Distribution: Example Ltd")
        );
        assert_eq!(
            config.installer_identity.as_deref(),
            Some("3rd Party Mac Developer Installer: Example Ltd")
        );
        assert_eq!(config.notarize, Some(false));
        assert_eq!(config.pkg_builder.as_deref(), Some("productbuild"));
        assert_eq!(config.cargo_features, ["macos-app-store"]);
        assert!(config.cargo_no_default_features);
    }

    #[test]
    fn codesign_arguments_resolve_relative_entitlements() {
        let config = MacosPackageConfig {
            entitlements: Some("platforms/macos/App.entitlements".into()),
            ..Default::default()
        };

        assert_eq!(
            codesign_arguments(Path::new("/project"), "-", &config),
            vec![
                OsString::from("--force"),
                OsString::from("--timestamp"),
                OsString::from("--options"),
                OsString::from("runtime"),
                OsString::from("--sign"),
                OsString::from("-"),
                OsString::from("--entitlements"),
                OsString::from("/project/platforms/macos/App.entitlements"),
            ]
        );
    }

    #[test]
    fn run_signing_overrides_package_signing_only() {
        let manifest: PackageManifest = toml::from_str(
            r#"
[package.macos]
bundle_id = "com.example.app"
minimum_os = "14.0"
entitlements = "platforms/macos/Release.entitlements"
provisioning_profile = "profiles/Release.provisionprofile"
signing_identity = "Apple Development"

[run.macos]
entitlements = "platforms/macos/Development.entitlements"
provisioning_profile = "profiles/Development.provisionprofile"
signing_identity = "-"
"#,
        )
        .unwrap();
        let config = run_config(&manifest, false);

        assert_eq!(config.bundle_id.as_deref(), Some("com.example.app"));
        assert_eq!(config.minimum_os.as_deref(), Some("14.0"));
        assert_eq!(
            config.entitlements.as_deref(),
            Some("platforms/macos/Development.entitlements")
        );
        assert_eq!(
            config.provisioning_profile.as_deref(),
            Some("profiles/Development.provisionprofile")
        );
        assert_eq!(config.signing_identity.as_deref(), Some("-"));
    }

    #[test]
    fn provisioning_profile_rejects_ad_hoc_signing_identity() {
        let config = MacosPackageConfig {
            provisioning_profile: Some("profiles/Development.provisionprofile".into()),
            signing_identity: Some("-".into()),
            ..Default::default()
        };

        let error = sign_macos_app_if_configured(
            Path::new("/project"),
            Path::new("/project/Demo.app"),
            &config,
        )
        .unwrap_err();

        assert!(error.to_string().contains("ad-hoc signing"));
    }

    #[test]
    fn embeds_relative_macos_provisioning_profile() {
        let root =
            std::env::temp_dir().join(format!("fission-macos-profile-{}", std::process::id()));
        let project = root.join("project");
        let app = root.join("Demo.app");
        fs::remove_dir_all(&root).ok();
        fs::create_dir_all(project.join("profiles")).unwrap();
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(
            project.join("profiles/Development.provisionprofile"),
            b"profile-data",
        )
        .unwrap();

        embed_macos_provisioning_profile(&project, &app, "profiles/Development.provisionprofile")
            .unwrap();

        assert_eq!(
            fs::read(app.join("Contents/embedded.provisionprofile")).unwrap(),
            b"profile-data"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn macos_bundle_resources_are_readable_by_non_root_users() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "fission-macos-readable-bundle-{}",
            std::process::id()
        ));
        let app = root.join("Demo.app");
        let resource = app.join("Contents/Resources/private.dat");
        fs::remove_dir_all(&root).ok();
        fs::create_dir_all(resource.parent().unwrap()).unwrap();
        fs::write(&resource, b"resource").unwrap();
        fs::set_permissions(&resource, fs::Permissions::from_mode(0o600)).unwrap();

        make_macos_bundle_world_readable(&app).unwrap();

        assert_eq!(
            fs::metadata(&resource).unwrap().permissions().mode() & 0o004,
            0o004
        );
        assert_eq!(
            fs::metadata(resource.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o001,
            0o001
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_bundle_extended_attributes_are_removed_before_signing() {
        let root =
            std::env::temp_dir().join(format!("fission-macos-clean-bundle-{}", std::process::id()));
        let app = root.join("Demo.app");
        let resource = app.join("Contents/Resources/downloaded.dat");
        fs::remove_dir_all(&root).ok();
        fs::create_dir_all(resource.parent().unwrap()).unwrap();
        fs::write(&resource, b"resource").unwrap();
        let status = Command::new("xattr")
            .args(["-w", "com.apple.quarantine", "0081;test;Fission;"])
            .arg(&resource)
            .status()
            .unwrap();
        assert!(status.success());

        remove_macos_bundle_extended_attributes(&app).unwrap();

        let status = Command::new("xattr")
            .args(["-p", "com.apple.quarantine"])
            .arg(&resource)
            .status()
            .unwrap();
        assert!(!status.success());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn provisioning_profile_requires_signing_identity() {
        let config = MacosPackageConfig {
            provisioning_profile: Some("profiles/Development.provisionprofile".into()),
            ..Default::default()
        };

        let error = sign_macos_app_if_configured(
            Path::new("/project"),
            Path::new("/project/Demo.app"),
            &config,
        )
        .unwrap_err();

        assert!(error.to_string().contains("requires"));
    }
}
