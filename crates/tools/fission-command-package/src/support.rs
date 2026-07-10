use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    CheckSeverity, CheckStatus, CloudflarePagesConfig, GithubPagesConfig, ReadinessCheck,
    ReadinessReport,
};

pub(super) fn print_readiness_report(report: &ReadinessReport) {
    println!("Readiness: {}", report.status);
    print_checks(&report.checks);
}

pub(super) fn print_checks(checks: &[ReadinessCheck]) {
    for check in checks {
        println!(
            "[{:?}/{:?}] {} - {}",
            check.severity, check.status, check.id, check.summary
        );
        if let Some(details) = &check.details {
            println!("  {details}");
        }
        for remediation in &check.remediation {
            println!("  fix: {remediation}");
        }
    }
}

pub(super) fn report_status(checks: &[ReadinessCheck]) -> &'static str {
    if checks
        .iter()
        .any(|check| check.severity == CheckSeverity::Error && check.status != CheckStatus::Passed)
    {
        "blocked"
    } else if checks
        .iter()
        .any(|check| !matches!(check.status, CheckStatus::Passed | CheckStatus::Skipped))
    {
        "warning"
    } else {
        "ready"
    }
}

pub(super) fn check(
    id: impl Into<String>,
    severity: CheckSeverity,
    status: CheckStatus,
    summary: impl Into<String>,
    details: Option<String>,
    remediation: Vec<&str>,
) -> ReadinessCheck {
    ReadinessCheck {
        id: id.into(),
        severity,
        status,
        summary: summary.into(),
        details,
        remediation: remediation.into_iter().map(str::to_string).collect(),
    }
}

pub(super) fn check_android_ndk() -> ReadinessCheck {
    let env_path = ["ANDROID_NDK", "ANDROID_NDK_HOME", "ANDROID_NDK_ROOT"]
        .iter()
        .find_map(|name| {
            env::var_os(name).map(|value| ((*name).to_string(), PathBuf::from(value)))
        });
    let sdk_ndk = env::var_os("ANDROID_HOME")
        .or_else(|| env::var_os("ANDROID_SDK_ROOT"))
        .and_then(|sdk| latest_child_dir(PathBuf::from(sdk).join("ndk")));
    let detail = env_path
        .as_ref()
        .map(|(name, path)| format!("environment variable {name}: {}", path.display()))
        .or_else(|| {
            sdk_ndk
                .as_ref()
                .map(|path| format!("detected {}", path.display()))
        });
    check(
        "release.package.android_ndk_configured",
        CheckSeverity::Error,
        if detail.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "Android NDK path is configured",
        detail,
        vec![
            "Set ANDROID_NDK or install an NDK under $ANDROID_HOME/ndk for Rust cross-compilation.",
        ],
    )
}

pub(super) fn check_android_build_tool(id: &str, tool: &str, remediation: &str) -> ReadinessCheck {
    let found = find_in_path(tool).or_else(|| find_android_build_tool(tool));
    check(
        id,
        CheckSeverity::Error,
        if found.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        format!("{tool} is available"),
        found.map(|path| path.display().to_string()),
        vec![remediation],
    )
}

pub(super) fn find_android_build_tool(tool: &str) -> Option<PathBuf> {
    let sdk = env::var_os("ANDROID_HOME").or_else(|| env::var_os("ANDROID_SDK_ROOT"))?;
    let build_tools = PathBuf::from(sdk).join("build-tools");
    let dir = latest_child_dir(build_tools)?;
    let direct = dir.join(tool);
    if direct.exists() {
        return Some(direct);
    }
    #[cfg(windows)]
    {
        let exe = dir.join(format!("{tool}.exe"));
        if exe.exists() {
            return Some(exe);
        }
    }
    None
}

pub(super) fn latest_child_dir(root: PathBuf) -> Option<PathBuf> {
    let mut dirs = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    dirs.pop()
}

pub(super) fn check_path(
    id: &str,
    path: PathBuf,
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    check(
        id,
        CheckSeverity::Error,
        if path.exists() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        Some(path.display().to_string()),
        vec![remediation],
    )
}

pub(super) fn required_value(
    id: &str,
    value: Option<&str>,
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    check(
        id,
        CheckSeverity::Error,
        if value.is_some_and(|value| !value.trim().is_empty()) {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        value.map(str::to_string),
        vec![remediation],
    )
}

pub(super) fn required_provider_env_secret(
    id: &str,
    env_names: &[&str],
    remediation: &str,
) -> ReadinessCheck {
    let env_name = env_names.iter().find(|name| env::var_os(name).is_some());
    check(
        id,
        CheckSeverity::Error,
        if env_name.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "provider credentials are available",
        env_name.map(|name| format!("environment variable {name}")),
        vec![remediation],
    )
}

pub(super) fn env_secret(env_names: &[&str]) -> Result<Option<String>> {
    for name in env_names {
        if env::var_os(name).is_some() {
            return env::var(name)
                .map(Some)
                .with_context(|| format!("environment variable {name} is not valid UTF-8"));
        }
    }
    Ok(None)
}

pub(super) fn base_path_check(id: &str, base_path: Option<&str>) -> ReadinessCheck {
    let value = base_path.unwrap_or("/");
    check(
        id,
        CheckSeverity::Warning,
        if value == "/" {
            CheckStatus::Passed
        } else {
            CheckStatus::Warning
        },
        "static hosting provider base path is root",
        Some(format!("base_path = {value}")),
        vec!["Dedicated static hosting providers usually serve production sites from `/`; use a non-root base path only when deliberately hosting below a subpath."],
    )
}

pub(super) fn host_os_check(id: &str, expected: &str) -> ReadinessCheck {
    let current = env::consts::OS;
    check(
        id,
        CheckSeverity::Error,
        if current == expected {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        format!("host operating system is {expected}"),
        Some(format!("current host: {current}")),
        vec!["Run this package format on the platform that owns the native packaging/signing toolchain."],
    )
}

pub(super) fn check_tool(id: &str, tool: &str, remediation: &str) -> ReadinessCheck {
    check(
        id,
        CheckSeverity::Error,
        if find_in_path(tool).is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        format!("{tool} is available on PATH"),
        find_in_path(tool).map(|path| path.display().to_string()),
        vec![remediation],
    )
}

pub(super) fn check_any_tool(
    id: &str,
    tools: &[&str],
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    let found = tools
        .iter()
        .find_map(|tool| find_in_path(tool).map(|path| (*tool, path)));
    check(
        id,
        CheckSeverity::Error,
        if found.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        found
            .map(|(tool, path)| format!("{tool}: {}", path.display()))
            .or_else(|| Some(format!("checked: {}", tools.join(", ")))),
        vec![remediation],
    )
}

pub(super) fn check_any_env(
    id: &str,
    names: &[&str],
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    let found = names.iter().find(|name| env::var_os(name).is_some());
    check(
        id,
        CheckSeverity::Error,
        if found.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        summary,
        found.map(|name| format!("environment variable {name}")),
        vec![remediation],
    )
}

pub(super) fn check_optional_env_or_tool(
    id: &str,
    env_names: &[&str],
    tools: &[&str],
    summary: &str,
    remediation: &str,
) -> ReadinessCheck {
    let found_env = env_names.iter().find(|name| env::var_os(name).is_some());
    let found_tool = tools
        .iter()
        .find_map(|tool| find_in_path(tool).map(|path| (*tool, path)));
    check(
        id,
        CheckSeverity::Warning,
        if found_env.is_some() || found_tool.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Warning
        },
        summary,
        found_env
            .map(|name| format!("environment variable {name}"))
            .or_else(|| found_tool.map(|(tool, path)| format!("{tool}: {}", path.display())))
            .or_else(|| Some(format!("checked: {}", tools.join(", ")))),
        vec![remediation],
    )
}

pub(super) fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
        if cfg!(windows) {
            for extension in ["exe", "cmd", "bat", "ps1"] {
                let candidate = dir.join(format!("{name}.{extension}"));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

pub(super) fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) fn cargo_package_version(project_dir: &Path) -> Option<String> {
    let data = fs::read_to_string(project_dir.join("Cargo.toml")).ok()?;
    let value: toml::Value = toml::from_str(&data).ok()?;
    value
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(|version| version.as_str())
        .map(str::to_string)
}

pub(super) fn release_version(project_dir: &Path) -> Option<String> {
    let data = fs::read_to_string(project_dir.join("fission.toml")).ok()?;
    let value: toml::Value = toml::from_str(&data).ok()?;
    value
        .get("package")
        .and_then(|package| package.get("android"))
        .and_then(|android| android.get("version_name"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            value
                .get("package")
                .and_then(|package| package.get("ios"))
                .and_then(|ios| ios.get("marketing_version"))
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            value
                .get("app")
                .and_then(|app| app.get("version"))
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| active_release_version(&value))
}

pub(super) fn active_release_version(value: &toml::Value) -> Option<String> {
    let active = value
        .get("release")
        .and_then(|release| release.get("active_release"))
        .and_then(toml::Value::as_str)?;
    value
        .get("releases")
        .and_then(toml::Value::as_array)?
        .iter()
        .find(|release| {
            release
                .get("id")
                .and_then(toml::Value::as_str)
                .is_some_and(|id| id == active)
        })
        .and_then(|release| release.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
}

pub(super) fn release_build_number(project_dir: &Path) -> Option<u64> {
    let data = fs::read_to_string(project_dir.join("fission.toml")).ok()?;
    let value: toml::Value = toml::from_str(&data).ok()?;
    value
        .get("package")
        .and_then(|package| package.get("android"))
        .and_then(|android| android.get("version_code"))
        .and_then(toml::Value::as_integer)
        .and_then(|value| u64::try_from(value).ok())
        .or_else(|| {
            value
                .get("package")
                .and_then(|package| package.get("ios"))
                .and_then(|ios| ios.get("build_number"))
                .and_then(|value| {
                    value
                        .as_str()
                        .and_then(|value| value.parse::<u64>().ok())
                        .or_else(|| {
                            value
                                .as_integer()
                                .and_then(|value| u64::try_from(value).ok())
                        })
                })
        })
        .or_else(|| {
            value
                .get("app")
                .and_then(|app| app.get("build"))
                .and_then(toml::Value::as_integer)
                .and_then(|value| u64::try_from(value).ok())
        })
        .or_else(|| active_release_build(&value))
}

pub(super) fn active_release_build(value: &toml::Value) -> Option<u64> {
    let active = value
        .get("release")
        .and_then(|release| release.get("active_release"))
        .and_then(toml::Value::as_str)?;
    value
        .get("releases")
        .and_then(toml::Value::as_array)?
        .iter()
        .find(|release| {
            release
                .get("id")
                .and_then(toml::Value::as_str)
                .is_some_and(|id| id == active)
        })
        .and_then(|release| release.get("build"))
        .and_then(toml::Value::as_integer)
        .and_then(|value| u64::try_from(value).ok())
}

pub(super) fn resolve_project_path(project_dir: &Path, path: String) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        project_dir.join(path)
    }
}

pub(super) fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

pub(super) fn expected_github_base_path(cfg: &GithubPagesConfig, repo: Option<&str>) -> String {
    if cfg
        .custom_domain
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        "/".to_string()
    } else if cfg.site_kind.as_deref() == Some("user")
        || cfg.site_kind.as_deref() == Some("organization")
    {
        "/".to_string()
    } else {
        repo.map(|repo| format!("/{repo}/"))
            .unwrap_or_else(|| "/".to_string())
    }
}

pub(super) fn github_pages_url(
    cfg: &GithubPagesConfig,
    owner: Option<&str>,
    repo: Option<&str>,
) -> Option<String> {
    if let Some(domain) = cfg
        .custom_domain
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(format!("https://{}", domain.trim()));
    }
    let owner = owner?;
    if cfg.site_kind.as_deref() == Some("user") || cfg.site_kind.as_deref() == Some("organization")
    {
        Some(format!("https://{owner}.github.io/"))
    } else {
        repo.map(|repo| format!("https://{owner}.github.io/{repo}/"))
    }
}

pub(super) fn cloudflare_url(cfg: &CloudflarePagesConfig) -> Option<String> {
    if let Some(domain) = cfg
        .custom_domain
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        Some(format!("https://{}", domain.trim()))
    } else {
        cfg.project_name
            .as_ref()
            .map(|name| format!("https://{name}.pages.dev"))
    }
}

pub(super) fn github_pages_follow_up(cfg: &GithubPagesConfig) -> Vec<String> {
    let mut follow_up = Vec::new();
    if cfg
        .custom_domain
        .as_deref()
        .filter(|value| !value.is_empty())
        .is_some()
    {
        follow_up.push(
            "Verify the GitHub Pages custom domain and HTTPS state in repository settings."
                .to_string(),
        );
    }
    follow_up
}

pub(super) fn infer_github_owner(project_dir: &Path) -> Option<String> {
    parse_github_remote(project_dir).map(|(owner, _)| owner)
}

pub(super) fn infer_github_repo(project_dir: &Path) -> Option<String> {
    parse_github_remote(project_dir).map(|(_, repo)| repo)
}

pub(super) fn parse_github_remote(project_dir: &Path) -> Option<(String, String)> {
    let remote = git_output(project_dir, ["remote", "get-url", "origin"]).ok()?;
    let remote = remote.trim().trim_end_matches(".git");
    if let Some(rest) = remote.strip_prefix("git@github.com:") {
        let (owner, repo) = rest.split_once('/')?;
        return Some((owner.to_string(), repo.to_string()));
    }
    if let Some(rest) = remote.strip_prefix("https://github.com/") {
        let (owner, repo) = rest.split_once('/')?;
        return Some((owner.to_string(), repo.to_string()));
    }
    None
}

pub(super) fn git_repo_root(project_dir: &Path) -> Option<PathBuf> {
    git_output(project_dir, ["rev-parse", "--show-toplevel"])
        .ok()
        .map(|value| PathBuf::from(value.trim()))
}

pub(super) fn git_output<'a, I>(dir: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        bail!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(super) fn run_git<'a, I>(dir: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = &'a str>,
{
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .context("failed to run git")?;
    if !status.success() {
        bail!("git command failed with {status}");
    }
    Ok(())
}

pub(super) fn first_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|part| part.starts_with("https://") || part.starts_with("http://"))
        .map(|value| {
            value
                .trim_matches(|c| c == ',' || c == ')' || c == '(')
                .to_string()
        })
}
