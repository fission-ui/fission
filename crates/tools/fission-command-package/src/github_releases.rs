use super::*;
use anyhow::{bail, Context, Result};
use fission_credentials as credentials;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Clone, Debug)]
struct ReleaseAsset {
    path: PathBuf,
    name: String,
    mime_type: String,
    sha256: Option<String>,
    size_bytes: u64,
}

pub(super) fn setup(options: &DistributeOptions, config: &PublishManifest) -> Result<()> {
    let cfg = github_releases_config(config, &options.site)?;
    let owner = cfg
        .owner
        .clone()
        .or_else(|| infer_github_owner(&options.project_dir))
        .unwrap_or_else(|| "<missing>".to_string());
    let repo = cfg
        .repo
        .clone()
        .or_else(|| infer_github_repo(&options.project_dir))
        .unwrap_or_else(|| "<missing>".to_string());
    println!("GitHub Releases setup checks for `{}`", options.site);
    println!("owner: {owner}");
    println!("repo: {repo}");
    println!(
        "tag: {}",
        cfg.tag
            .as_deref()
            .unwrap_or("<from --deploy or artifact version>")
    );
    println!(
        "Run `fission readiness distribute --provider github-releases --site {} --artifact <artifact-manifest> --project-dir {}` before publishing.",
        options.site,
        options.project_dir.display()
    );
    Ok(())
}

pub(super) fn readiness(
    project_dir: &Path,
    site: &str,
    artifact: Option<&Path>,
    config: &PublishManifest,
    checks: &mut Vec<ReadinessCheck>,
) -> Result<()> {
    let cfg = github_releases_config(config, site)?;
    let owner = cfg
        .owner
        .clone()
        .or_else(|| infer_github_owner(project_dir));
    let repo = cfg.repo.clone().or_else(|| infer_github_repo(project_dir));
    checks.push(required_value(
        "release.github_releases.owner_configured",
        owner.as_deref(),
        "GitHub release owner is configured or inferable from git remote",
        "Set distribution.github_releases.<site>.owner or configure an origin GitHub remote.",
    ));
    checks.push(required_value(
        "release.github_releases.repo_configured",
        repo.as_deref(),
        "GitHub release repository is configured or inferable from git remote",
        "Set distribution.github_releases.<site>.repo or configure an origin GitHub remote.",
    ));
    checks.push(check_tool(
        "release.github_releases.gh_available",
        "gh",
        "Install GitHub CLI and authenticate with `gh auth login`.",
    ));
    checks.push(check(
        "release.github_releases.auth_available",
        CheckSeverity::Error,
        if gh_auth_available(project_dir) {
            CheckStatus::Passed
        } else {
            CheckStatus::Missing
        },
        "GitHub CLI authentication is available",
        Some("gh auth status, GH_TOKEN/GITHUB_TOKEN, or Fission vault credential".to_string()),
        vec!["Run `gh auth login`, set GH_TOKEN/GITHUB_TOKEN, or import github-releases credentials into the Fission vault."],
    ));
    checks.push(check(
        "release.github_releases.replace_assets_explicit",
        CheckSeverity::Info,
        if cfg.replace_assets.is_some() {
            CheckStatus::Passed
        } else {
            CheckStatus::Skipped
        },
        "duplicate release asset behavior is explicit",
        cfg.replace_assets
            .map(|value| format!("replace_assets = {value}")),
        vec!["Set replace_assets = true to overwrite same-named release assets during republish, or false to fail safely."],
    ));
    if let Some(artifact) = artifact.filter(|path| path.exists()) {
        let manifest = read_artifact_manifest(artifact)?;
        let assets = release_assets(&manifest, artifact, &cfg)?;
        checks.push(check(
            "release.github_releases.assets_available",
            CheckSeverity::Error,
            if assets.is_empty() {
                CheckStatus::Missing
            } else {
                CheckStatus::Passed
            },
            "artifact manifest contains uploadable release assets",
            Some(format!(
                "{} uploadable assets for {} {}",
                assets.len(),
                manifest.target,
                manifest.format
            )),
            vec!["Package the app artifact first. GitHub Releases accepts any artifact file, including .run, .pkg, .exe, .msi, .msix, .apk, .aab, .ipa, archives, symbol files, and zipped static-site outputs."],
        ));
        let tag = release_tag(&cfg, &manifest, None);
        checks.push(check(
            "release.github_releases.tag_resolved",
            CheckSeverity::Error,
            if tag.is_some() {
                CheckStatus::Passed
            } else {
                CheckStatus::Missing
            },
            "release tag can be resolved",
            tag,
            vec!["Set distribution.github_releases.<site>.tag, pass --deploy <tag>, or ensure Cargo.toml has package.version."],
        ));
    }
    Ok(())
}

pub(super) fn status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = github_releases_config(config, &options.site)?;
    let (owner, repo) = github_repo(&options.project_dir, &cfg)?;
    let tag = options.deploy.as_deref().or(cfg.tag.as_deref());
    let output = gh_release_view(&options.project_dir, &owner, &repo, tag)?;
    let release: Value = serde_json::from_slice(&output.stdout)
        .context("failed to parse gh release view JSON output")?;
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "github-releases".to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: release_id(&release),
        canonical_url: release_url(&release),
        preview_url: None,
        custom_domain: None,
        status: gh_release_state(&release),
        stdout: Some(serde_json::to_string_pretty(&release)?),
        stderr: (!output.stderr.is_empty())
            .then(|| String::from_utf8_lossy(&output.stderr).to_string()),
        manual_follow_up: Vec::new(),
    })
}

pub(super) fn publish(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = github_releases_config(config, &options.site)?;
    let (owner, repo) = github_repo(&options.project_dir, &cfg)?;
    let tag = release_tag(&cfg, manifest, options.deploy.as_deref()).context(
        "GitHub Releases publishing requires a tag from --deploy, config tag, or artifact version",
    )?;
    let assets = release_assets(manifest, artifact_path, &cfg)?;
    if assets.is_empty() {
        bail!("GitHub Releases publishing requires at least one artifact file");
    }
    if options.dry_run {
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: "github-releases".to_string(),
            site: options.site.clone(),
            action: "publish".to_string(),
            artifact_manifest: Some(artifact_path.display().to_string()),
            deployment_id: Some(tag.clone()),
            canonical_url: Some(format!(
                "https://github.com/{owner}/{repo}/releases/tag/{}",
                url_segment(&tag)
            )),
            preview_url: None,
            custom_domain: None,
            status: "dry-run".to_string(),
            stdout: Some(serde_json::to_string_pretty(&json!({
                "tag": tag,
                "repo": repo_arg(&owner, &repo),
                "assets": assets.iter().map(asset_json).collect::<Vec<_>>(),
                "command": "gh release create/edit/upload"
            }))?),
            stderr: None,
            manual_follow_up: Vec::new(),
        });
    }

    require_gh_authenticated(&options.project_dir)?;
    let existing = gh_release_view(&options.project_dir, &owner, &repo, Some(&tag));
    let release_output = match existing {
        Ok(_) => gh_release_edit(&options.project_dir, &owner, &repo, &tag, &cfg)?,
        Err(error) if is_not_found_error(&error) => {
            gh_release_create(&options.project_dir, &owner, &repo, &tag, &cfg)?
        }
        Err(error) => return Err(error),
    };
    let uploaded = gh_release_upload(
        &options.project_dir,
        &owner,
        &repo,
        &tag,
        &assets,
        cfg.replace_assets.unwrap_or(false),
    )?;
    let view = gh_release_view(&options.project_dir, &owner, &repo, Some(&tag))?;
    let release: Value = serde_json::from_slice(&view.stdout)
        .context("failed to parse gh release view JSON output after publish")?;
    let stdout = json!({
        "release": release,
        "release_command": command_output_json(&release_output),
        "upload_command": command_output_json(&uploaded),
        "uploaded_assets": assets.iter().map(asset_json).collect::<Vec<_>>(),
    });
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "github-releases".to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: release_id(stdout.get("release").unwrap_or(&Value::Null)).or(Some(tag)),
        canonical_url: release_url(stdout.get("release").unwrap_or(&Value::Null)),
        preview_url: None,
        custom_domain: None,
        status: gh_release_state(stdout.get("release").unwrap_or(&Value::Null)),
        stdout: Some(serde_json::to_string_pretty(&stdout)?),
        stderr: None,
        manual_follow_up: cfg
            .draft
            .unwrap_or(false)
            .then(|| {
                "Publish the draft release from GitHub when it is ready for users.".to_string()
            })
            .into_iter()
            .collect(),
    })
}

fn github_repo(project_dir: &Path, cfg: &GithubReleasesConfig) -> Result<(String, String)> {
    let owner = cfg
        .owner
        .clone()
        .or_else(|| infer_github_owner(project_dir))
        .context("distribution.github_releases.<site>.owner or GitHub origin remote is required")?;
    let repo = cfg
        .repo
        .clone()
        .or_else(|| infer_github_repo(project_dir))
        .context("distribution.github_releases.<site>.repo or GitHub origin remote is required")?;
    Ok((owner, repo))
}

fn release_tag(
    cfg: &GithubReleasesConfig,
    manifest: &ArtifactManifest,
    override_tag: Option<&str>,
) -> Option<String> {
    override_tag
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| cfg.tag.clone().filter(|value| !value.trim().is_empty()))
        .or_else(|| {
            manifest
                .project
                .version
                .as_ref()
                .map(|version| format!("v{version}"))
        })
}

fn gh_release_view(
    project_dir: &Path,
    owner: &str,
    repo: &str,
    tag: Option<&str>,
) -> Result<Output> {
    let repo_arg = repo_arg(owner, repo);
    let mut args = vec!["release", "view"];
    if let Some(tag) = tag.filter(|value| !value.trim().is_empty()) {
        args.push(tag);
    }
    args.extend([
        "--repo",
        &repo_arg,
        "--json",
        "apiUrl,assets,author,body,createdAt,databaseId,id,isDraft,isImmutable,isPrerelease,name,publishedAt,tagName,targetCommitish,uploadUrl,url,zipballUrl,tarballUrl",
    ]);
    run_gh(project_dir, &args)
}

fn gh_release_create(
    project_dir: &Path,
    owner: &str,
    repo: &str,
    tag: &str,
    cfg: &GithubReleasesConfig,
) -> Result<Output> {
    let mut args = vec!["release".to_string(), "create".to_string(), tag.to_string()];
    args.extend(["--repo".to_string(), repo_arg(owner, repo)]);
    args.extend(release_metadata_args(cfg, project_dir)?);
    run_gh_owned(project_dir, &args)
}

fn gh_release_edit(
    project_dir: &Path,
    owner: &str,
    repo: &str,
    tag: &str,
    cfg: &GithubReleasesConfig,
) -> Result<Output> {
    let mut args = vec!["release".to_string(), "edit".to_string(), tag.to_string()];
    args.extend(["--repo".to_string(), repo_arg(owner, repo)]);
    args.extend(release_metadata_args(cfg, project_dir)?);
    run_gh_owned(project_dir, &args)
}

fn gh_release_upload(
    project_dir: &Path,
    owner: &str,
    repo: &str,
    tag: &str,
    assets: &[ReleaseAsset],
    replace_assets: bool,
) -> Result<Output> {
    let mut args = vec!["release".to_string(), "upload".to_string(), tag.to_string()];
    args.extend(assets.iter().map(|asset| asset.path.display().to_string()));
    args.extend(["--repo".to_string(), repo_arg(owner, repo)]);
    if replace_assets {
        args.push("--clobber".to_string());
    }
    run_gh_owned(project_dir, &args)
}

fn release_metadata_args(cfg: &GithubReleasesConfig, project_dir: &Path) -> Result<Vec<String>> {
    let mut args = Vec::new();
    if let Some(name) = cfg.name.as_deref().filter(|value| !value.trim().is_empty()) {
        args.extend(["--title".to_string(), name.to_string()]);
    }
    if let Some(target) = cfg
        .target_commitish
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.extend(["--target".to_string(), target.to_string()]);
    }
    if let Some(notes) = cfg.notes.as_ref().filter(|value| !value.trim().is_empty()) {
        args.extend(["--notes".to_string(), notes.to_string()]);
    } else if let Some(path) = cfg
        .notes_file
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        args.extend([
            "--notes-file".to_string(),
            resolve_project_path(project_dir, path.clone())
                .display()
                .to_string(),
        ]);
    }
    if cfg.draft.unwrap_or(false) {
        args.push("--draft".to_string());
    }
    if cfg.prerelease.unwrap_or(false) {
        args.push("--prerelease".to_string());
    }
    if let Some(latest) = cfg
        .make_latest
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        match latest {
            "true" => args.push("--latest".to_string()),
            "false" => args.push("--latest=false".to_string()),
            "legacy" => {}
            other => bail!("unsupported github-releases make_latest value `{other}`; use true, false, or legacy"),
        }
    }
    Ok(args)
}

fn release_assets(
    manifest: &ArtifactManifest,
    artifact_path: &Path,
    cfg: &GithubReleasesConfig,
) -> Result<Vec<ReleaseAsset>> {
    let mut assets = manifest
        .artifacts
        .iter()
        .filter_map(release_asset_from_manifest_file)
        .collect::<Vec<_>>();
    if cfg.upload_artifact_manifest.unwrap_or(true) {
        let metadata = fs::metadata(artifact_path)
            .with_context(|| format!("failed to read {}", artifact_path.display()))?;
        assets.push(ReleaseAsset {
            path: artifact_path.to_path_buf(),
            name: artifact_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(ARTIFACT_MANIFEST)
                .to_string(),
            mime_type: content_type(artifact_path).to_string(),
            sha256: None,
            size_bytes: metadata.len(),
        });
    }
    Ok(assets)
}

fn release_asset_from_manifest_file(file: &ArtifactFile) -> Option<ReleaseAsset> {
    let path = PathBuf::from(&file.path);
    if !path.is_file() {
        return None;
    }
    let name = path.file_name()?.to_str()?.to_string();
    Some(ReleaseAsset {
        path,
        name,
        mime_type: file.mime_type.clone(),
        sha256: Some(file.sha256.clone()),
        size_bytes: file.size_bytes,
    })
}

fn require_gh_authenticated(project_dir: &Path) -> Result<()> {
    run_gh(project_dir, &["auth", "status"])
        .map(|_| ())
        .context("GitHub Releases requires an authenticated gh CLI session; run `gh auth login` or set GH_TOKEN/GITHUB_TOKEN")
}

fn gh_auth_available(project_dir: &Path) -> bool {
    env::var_os("GH_TOKEN").is_some()
        || env::var_os("GITHUB_TOKEN").is_some()
        || credentials::provider_secret(DistributionProvider::GithubReleases, &[])
            .ok()
            .flatten()
            .is_some()
        || run_gh(project_dir, &["auth", "status"]).is_ok()
}

fn run_gh(project_dir: &Path, args: &[&str]) -> Result<Output> {
    let args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    run_gh_owned(project_dir, &args)
}

fn run_gh_owned(project_dir: &Path, args: &[String]) -> Result<Output> {
    let output = Command::new("gh")
        .args(args)
        .current_dir(project_dir)
        .envs(gh_env())
        .output()
        .with_context(|| {
            "failed to run gh; install GitHub CLI and authenticate with `gh auth login`"
        })?;
    if output.status.success() {
        Ok(output)
    } else {
        bail!(
            "gh {} failed with {}: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

fn gh_env() -> Vec<(&'static str, String)> {
    let mut envs = Vec::new();
    if env::var_os("GH_TOKEN").is_none() && env::var_os("GITHUB_TOKEN").is_none() {
        if let Ok(Some(token)) =
            credentials::provider_secret(DistributionProvider::GithubReleases, &[])
        {
            envs.push(("GH_TOKEN", token));
        }
    }
    envs
}

fn is_not_found_error(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}").to_ascii_lowercase();
    text.contains("not found") || text.contains("http 404") || text.contains("could not find")
}

fn release_id(value: &Value) -> Option<String> {
    value
        .get("databaseId")
        .and_then(Value::as_i64)
        .map(|id| id.to_string())
        .or_else(|| value.get("id").and_then(Value::as_str).map(str::to_string))
        .or_else(|| {
            value
                .get("tagName")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn release_url(value: &Value) -> Option<String> {
    value.get("url").and_then(Value::as_str).map(str::to_string)
}

fn gh_release_state(value: &Value) -> String {
    if value
        .get("isDraft")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "draft".to_string()
    } else if value
        .get("isPrerelease")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "prerelease".to_string()
    } else {
        value
            .get("tagName")
            .and_then(Value::as_str)
            .map(|tag| format!("published:{tag}"))
            .unwrap_or_else(|| "published".to_string())
    }
}

fn command_output_json(output: &Output) -> Value {
    json!({
        "status": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout).trim(),
        "stderr": String::from_utf8_lossy(&output.stderr).trim(),
    })
}

fn repo_arg(owner: &str, repo: &str) -> String {
    format!("{owner}/{repo}")
}

fn asset_json(asset: &ReleaseAsset) -> Value {
    json!({
        "name": asset.name,
        "path": asset.path,
        "mime_type": asset.mime_type,
        "sha256": asset.sha256,
        "size_bytes": asset.size_bytes,
    })
}

fn url_segment(value: &str) -> String {
    percent_encode(value, false)
}

fn percent_encode(value: &str, encode_slash: bool) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b'/' if !encode_slash => out.push('/'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(path: &Path, relative_path: &str, kind: &str) -> ArtifactFile {
        ArtifactFile {
            kind: kind.to_string(),
            purpose: None,
            platform: None,
            upload_provider: None,
            path: path.display().to_string(),
            relative_path: relative_path.to_string(),
            sha256: "abc".to_string(),
            size_bytes: 3,
            mime_type: content_type(path).to_string(),
        }
    }

    fn manifest(files: Vec<ArtifactFile>) -> ArtifactManifest {
        ArtifactManifest {
            schema_version: 1,
            created_at_unix_seconds: 0,
            project: ArtifactProject {
                app_id: "com.example.app".to_string(),
                name: "app".to_string(),
                version: Some("1.2.3".to_string()),
            },
            target: "linux".to_string(),
            format: "run".to_string(),
            profile: "release".to_string(),
            root_dir: "/tmp".to_string(),
            artifacts: files,
            validation: ArtifactValidation {
                state: "passed".to_string(),
                checks: Vec::new(),
            },
        }
    }

    #[test]
    fn release_tag_defaults_to_version() {
        let manifest = manifest(Vec::new());
        assert_eq!(
            release_tag(&GithubReleasesConfig::default(), &manifest, None),
            Some("v1.2.3".to_string())
        );
        assert_eq!(
            release_tag(&GithubReleasesConfig::default(), &manifest, Some("nightly")),
            Some("nightly".to_string())
        );
    }

    #[test]
    fn release_assets_include_every_manifest_file_and_manifest() {
        let dir = std::env::temp_dir().join(format!(
            "fission-github-release-assets-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let run = dir.join("app.run");
        let html = dir.join("index.html");
        fs::write(&run, b"run").unwrap();
        fs::write(&html, b"html").unwrap();
        let manifest = manifest(vec![
            artifact(&run, "app.run", "asset"),
            artifact(&html, "index.html", "entry"),
        ]);
        let manifest_path = dir.join("artifact-manifest.json");
        fs::write(&manifest_path, b"{}").unwrap();
        let assets =
            release_assets(&manifest, &manifest_path, &GithubReleasesConfig::default()).unwrap();
        let names = assets
            .iter()
            .map(|asset| asset.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"app.run"));
        assert!(names.contains(&"index.html"));
        assert!(names.contains(&"artifact-manifest.json"));
        let _ = fs::remove_dir_all(&dir);
    }
}
