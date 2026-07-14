use super::*;

pub(super) fn setup_provider(options: &DistributeOptions, config: &PublishManifest) -> Result<()> {
    match options.provider {
        DistributionProvider::GithubPages => setup_github_pages(options, config),
        DistributionProvider::GithubReleases => github_releases::setup(options, config),
        DistributionProvider::DockerRegistry => setup_non_static_provider(options, config),
        DistributionProvider::CloudflarePages => {
            let cfg = cloudflare_config(config, &options.site)?;
            println!("Cloudflare Pages setup checks for `{}`", options.site);
            println!(
                "account_id: {}",
                cfg.account_id.as_deref().unwrap_or("<missing>")
            );
            println!(
                "project_name: {}",
                cfg.project_name.as_deref().unwrap_or("<missing>")
            );
            println!("Run `fission readiness distribute --provider cloudflare-pages --site {} --project-dir {}` before publishing.", options.site, options.project_dir.display());
            Ok(())
        }
        DistributionProvider::Netlify => {
            let cfg = netlify_config(config, &options.site)?;
            println!("Netlify setup checks for `{}`", options.site);
            println!("site_id: {}", cfg.site_id.as_deref().unwrap_or("<missing>"));
            println!(
                "team_slug: {}",
                cfg.team_slug.as_deref().unwrap_or("<missing>")
            );
            println!("Run `fission readiness distribute --provider netlify --site {} --project-dir {}` before publishing.", options.site, options.project_dir.display());
            Ok(())
        }
        DistributionProvider::S3
        | DistributionProvider::GoogleDrive
        | DistributionProvider::OneDrive
        | DistributionProvider::Dropbox
        | DistributionProvider::PlayStore
        | DistributionProvider::AppStore
        | DistributionProvider::MicrosoftStore => setup_non_static_provider(options, config),
    }
}

pub(super) fn publish_artifact(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<()> {
    let (receipt, receipt_value) = publish_artifact_receipt(options, config)?;
    print_distribution_receipt(options, &receipt, Some(&receipt_value))
}

pub(super) fn publish_artifact_receipt(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<(DistributionReceipt, Value)> {
    let mut events = Vec::new();
    match publish_artifact_receipt_with_events(options, config, &mut events) {
        Ok(value) => Ok(value),
        Err(error) => {
            let message = error.to_string();
            let receipt_path = write_failed_distribution_receipt(
                options,
                &mut events,
                "distribution.publish",
                &message,
            )?;
            bail!(
                "distribution publish failed: {}; distribution receipt: {}",
                redact_sensitive_text(&message),
                receipt_path.display()
            );
        }
    }
}

pub(super) fn publish_artifact_receipt_with_events(
    options: &DistributeOptions,
    config: &PublishManifest,
    events: &mut Vec<DistributionEvent>,
) -> Result<(DistributionReceipt, Value)> {
    let artifact_path = options
        .artifact
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if options.provider == DistributionProvider::DockerRegistry {
                default_artifact_manifest_path_for_format(
                    &options.project_dir,
                    Target::Server,
                    PackageFormat::DockerImage,
                    true,
                )
            } else {
                default_artifact_manifest_path(&options.project_dir, Target::Site, true)
            }
        });
    push_distribution_event(
        events,
        "distribution.artifact_manifest",
        "started",
        Some(artifact_path.display().to_string()),
    );
    let manifest = read_artifact_manifest(&artifact_path)?;
    push_distribution_event(
        events,
        "distribution.artifact_manifest",
        "completed",
        Some(format!(
            "{} {} with {} artifact(s)",
            manifest.target,
            manifest.format,
            manifest.artifacts.len()
        )),
    );
    push_distribution_event(
        events,
        "distribution.readiness",
        "started",
        Some(options.provider.as_str().to_string()),
    );
    let checks = readiness_distribute(
        &options.project_dir,
        options.provider,
        &options.site,
        options.track.as_deref(),
        options.format,
        Some(&artifact_path),
        config,
    )?;
    let errors = checks
        .iter()
        .filter(|check| {
            check.severity == CheckSeverity::Error && check.status != CheckStatus::Passed
        })
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        push_distribution_event(
            events,
            "distribution.readiness",
            "blocked",
            Some(format!("{} blocking check(s)", errors.len())),
        );
        print_checks(&checks);
        bail!("distribution readiness failed");
    }
    push_distribution_event(
        events,
        "distribution.readiness",
        "completed",
        Some(format!("{} check(s)", checks.len())),
    );

    push_distribution_event(
        events,
        "provider.publish",
        "started",
        Some(options.provider.as_str().to_string()),
    );
    push_provider_request_event(events, options, "publish");
    push_distribution_event(
        events,
        "provider.upload_plan",
        "planned",
        Some(format!(
            "{} manifest artifact(s) plus artifact-manifest.json",
            manifest.artifacts.len()
        )),
    );
    let receipt = match options.provider {
        DistributionProvider::GithubPages => {
            publish_github_pages(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::GithubReleases => {
            github_releases::publish(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::DockerRegistry => {
            docker_registry::publish(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::CloudflarePages => {
            publish_cloudflare_pages(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::Netlify => {
            static_hosts::publish_netlify(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::S3 => {
            files::publish_s3(options, config, &artifact_path, &manifest, events)?
        }
        DistributionProvider::GoogleDrive => {
            files::publish_google_drive(options, config, &artifact_path, &manifest, events)?
        }
        DistributionProvider::OneDrive => {
            files::publish_onedrive(options, config, &artifact_path, &manifest, events)?
        }
        DistributionProvider::Dropbox => {
            files::publish_dropbox(options, config, &artifact_path, &manifest, events)?
        }
        DistributionProvider::PlayStore => {
            stores::publish_play_store(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::AppStore => {
            stores::publish_app_store(options, config, &artifact_path, &manifest)?
        }
        DistributionProvider::MicrosoftStore => {
            stores::publish_microsoft_store(options, config, &artifact_path, &manifest)?
        }
    };
    push_provider_response_event(events, &receipt);
    push_distribution_event(
        events,
        "provider.publish",
        receipt.status.as_str(),
        Some(provider_event_detail(&receipt)),
    );
    let provider_uploaded = provider_uploaded_assets_by_relative(&receipt);
    if !provider_uploaded.is_empty() {
        push_distribution_event(
            events,
            "provider.uploaded_assets",
            "captured",
            Some(format!(
                "{} uploaded/planned asset(s)",
                provider_uploaded.len()
            )),
        );
    }
    push_provider_uploaded_asset_events(events, options, &manifest, &provider_uploaded);
    if let Some(stdout) = receipt
        .stdout
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_distribution_event(
            events,
            "provider.stdout",
            "captured",
            Some(truncate_event_detail(stdout)),
        );
        push_provider_stdio_line_events(events, "provider.stdout.line", stdout);
    }
    if let Some(stderr) = receipt
        .stderr
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_distribution_event(
            events,
            "provider.stderr",
            "captured",
            Some(truncate_event_detail(stderr)),
        );
        push_provider_stdio_line_events(events, "provider.stderr.line", stderr);
    }
    let receipt_path = receipt_output_path(&options.project_dir, &receipt);
    push_distribution_event(
        events,
        "distribution.receipt",
        "written",
        Some(receipt_path.display().to_string()),
    );
    let receipt_value = distribution_receipt_value_with_events(
        options,
        &receipt,
        Some(&artifact_path),
        Some(&manifest),
        events,
    )?;
    write_receipt(&options.project_dir, &receipt, &receipt_value)?;
    Ok((receipt, receipt_value))
}

pub(super) fn print_distribution_receipt(
    options: &DistributeOptions,
    receipt: &DistributionReceipt,
    value: Option<&Value>,
) -> Result<()> {
    if options.json {
        let value = match value {
            Some(value) => value.clone(),
            None => distribution_receipt_value(options, receipt, None, None)?,
        };
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!(
            "{} {} status: {}",
            receipt.provider, receipt.action, receipt.status
        );
        if let Some(url) = &receipt.canonical_url {
            println!("URL: {url}");
        }
        print_provider_upload_plan(receipt);
        for item in &receipt.manual_follow_up {
            println!("Follow-up: {item}");
        }
    }
    Ok(())
}

pub(super) fn print_provider_upload_plan(receipt: &DistributionReceipt) {
    let uploaded = provider_uploaded_assets_by_relative(receipt);
    if uploaded.is_empty() {
        return;
    }
    println!("Uploaded/planned assets: {}", uploaded.len());
    for asset in uploaded.values().take(10) {
        let relative = asset
            .get("relative_path")
            .and_then(Value::as_str)
            .unwrap_or("<asset>");
        let destination = asset
            .get("url")
            .or_else(|| asset.get("provider_id"))
            .and_then(Value::as_str)
            .unwrap_or("<provider destination>");
        println!("  {relative} -> {destination}");
    }
    if uploaded.len() > 10 {
        println!("  ... {} more", uploaded.len() - 10);
    }
}

pub(super) fn provider_status(options: &DistributeOptions, config: &PublishManifest) -> Result<()> {
    let _ = config;
    let outcome = distribute_status_outcome(options.clone())?;
    let value = outcome.receipt;
    if options.json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        let provider = value
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or(options.provider.as_str());
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        println!("{provider} status: {status}");
        if let Some(stdout) = value.get("stdout").and_then(Value::as_str) {
            print!("{stdout}");
        }
    }
    Ok(())
}

pub(super) fn provider_status_receipt(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    Ok(match options.provider {
        DistributionProvider::GithubPages => github_pages_status(options, config)?,
        DistributionProvider::GithubReleases => github_releases::status(options, config)?,
        DistributionProvider::DockerRegistry => docker_registry::status(options, config)?,
        DistributionProvider::CloudflarePages => cloudflare_pages_status(options, config)?,
        DistributionProvider::Netlify => static_hosts::netlify_status(options, config)?,
        DistributionProvider::PlayStore => stores::play_store_status(options, config)?,
        DistributionProvider::S3 => files::s3_status(options, config)?,
        DistributionProvider::GoogleDrive => files::google_drive_status(options, config)?,
        DistributionProvider::OneDrive => files::onedrive_status(options, config)?,
        DistributionProvider::Dropbox => files::dropbox_status(options, config)?,
        DistributionProvider::AppStore => stores::app_store_status(options, config)?,
        DistributionProvider::MicrosoftStore => stores::microsoft_store_status(options, config)?,
    })
}

pub(super) fn provider_lifecycle(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<()> {
    let mut events = Vec::new();
    let operation = match options.action {
        DistributeAction::Promote => "promote",
        DistributeAction::Rollback => "rollback",
        _ => "lifecycle",
    };
    push_distribution_event(
        &mut events,
        "provider.lifecycle",
        "started",
        Some(options.provider.as_str().to_string()),
    );
    push_provider_request_event(&mut events, options, operation);
    let receipt = match options.provider {
        DistributionProvider::Netlify => static_hosts::netlify_lifecycle(options, config)?,
        DistributionProvider::CloudflarePages => cloudflare_pages_lifecycle(options, config)?,
        DistributionProvider::AppStore => stores::app_store_lifecycle(options, config)?,
        _ => bail!(
            "{} currently supports setup, publish, and status; {} is not exposed by this provider backend",
            options.provider.as_str(),
            match options.action {
                DistributeAction::Promote => "promote",
                DistributeAction::Rollback => "rollback",
                _ => "this operation",
            }
        ),
    };
    push_provider_response_event(&mut events, &receipt);
    push_distribution_event(
        &mut events,
        "provider.lifecycle",
        receipt.status.as_str(),
        Some(provider_event_detail(&receipt)),
    );
    if let Some(stdout) = receipt
        .stdout
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_distribution_event(
            &mut events,
            "provider.stdout",
            "captured",
            Some(truncate_event_detail(stdout)),
        );
        push_provider_stdio_line_events(&mut events, "provider.stdout.line", stdout);
    }
    if let Some(stderr) = receipt
        .stderr
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_distribution_event(
            &mut events,
            "provider.stderr",
            "captured",
            Some(truncate_event_detail(stderr)),
        );
        push_provider_stdio_line_events(&mut events, "provider.stderr.line", stderr);
    }
    let (artifact_path, manifest) = receipt_artifact_context(&receipt)?;
    let receipt_path = receipt_output_path(&options.project_dir, &receipt);
    push_distribution_event(
        &mut events,
        "distribution.receipt",
        "written",
        Some(receipt_path.display().to_string()),
    );
    let receipt_value = distribution_receipt_value_with_events(
        options,
        &receipt,
        artifact_path.as_deref(),
        manifest.as_ref(),
        &events,
    )?;
    write_receipt(&options.project_dir, &receipt, &receipt_value)?;
    print_distribution_receipt(options, &receipt, Some(&receipt_value))
}

pub(super) fn cloudflare_pages_lifecycle(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = cloudflare_config(config, &options.site)?;
    let account_id = cfg
        .account_id
        .clone()
        .or_else(|| env::var("CLOUDFLARE_ACCOUNT_ID").ok())
        .context(
            "distribution.cloudflare_pages.<site>.account_id or CLOUDFLARE_ACCOUNT_ID is required",
        )?;
    let project_name = cfg
        .project_name
        .as_deref()
        .context("distribution.cloudflare_pages.<site>.project_name is required")?;
    let deploy = options
        .deploy
        .as_deref()
        .context("cloudflare-pages promote/rollback requires --deploy <deployment-id>")?;
    if options.dry_run {
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: "cloudflare-pages".to_string(),
            site: options.site.clone(),
            action: match options.action {
                DistributeAction::Promote => "promote",
                DistributeAction::Rollback => "rollback",
                _ => "lifecycle",
            }
            .to_string(),
            artifact_manifest: None,
            deployment_id: Some(deploy.to_string()),
            canonical_url: cloudflare_url(&cfg),
            preview_url: None,
            custom_domain: non_empty(cfg.custom_domain.clone()),
            status: "dry-run".to_string(),
            stdout: None,
            stderr: None,
            manual_follow_up: vec![format!(
                "Would make Cloudflare Pages deployment {deploy} live by calling the provider rollback endpoint."
            )],
        });
    }
    let token =
        env_secret(&["CLOUDFLARE_API_TOKEN"])?.context("CLOUDFLARE_API_TOKEN is required")?;
    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/pages/projects/{project_name}/deployments/{deploy}/rollback"
    );
    let response = reqwest::blocking::Client::builder()
        .user_agent("cargo-fission-publish/0.1")
        .build()?
        .post(url)
        .bearer_auth(token)
        .send()
        .context("failed to rollback Cloudflare Pages deployment")?;
    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        bail!("Cloudflare Pages rollback failed with {status}: {text}");
    }
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse Cloudflare Pages rollback response: {text}"))?;
    let result = value.get("result").unwrap_or(&value);
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "cloudflare-pages".to_string(),
        site: options.site.clone(),
        action: match options.action {
            DistributeAction::Promote => "promote",
            DistributeAction::Rollback => "rollback",
            _ => "lifecycle",
        }
        .to_string(),
        artifact_manifest: None,
        deployment_id: result
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .or_else(|| Some(deploy.to_string())),
        canonical_url: cloudflare_url(&cfg),
        preview_url: result
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        custom_domain: non_empty(cfg.custom_domain.clone()),
        status: result
            .pointer("/latest_stage/status")
            .or_else(|| result.get("status"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("rollback-requested")
            .to_string(),
        stdout: Some(serde_json::to_string_pretty(&value)?),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
}

pub(super) fn setup_non_static_provider(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<()> {
    let checks = readiness_distribute(
        &options.project_dir,
        options.provider,
        &options.site,
        options.track.as_deref(),
        options.format,
        options.artifact.as_deref(),
        config,
    )?;
    if options.json {
        let report = ReadinessReport {
            project_dir: options.project_dir.display().to_string(),
            target: None,
            format: None,
            provider: Some(options.provider.as_str().to_string()),
            site: Some(options.site.clone()),
            status: report_status(&checks).to_string(),
            checks,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "{} setup checks for `{}`",
            options.provider.as_str(),
            options.site
        );
        print_checks(&checks);
    }
    Ok(())
}

pub(super) fn setup_github_pages(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<()> {
    let cfg = github_config(config, &options.site)?;
    let workflow = cfg
        .workflow
        .clone()
        .unwrap_or_else(|| "fission-pages.yml".to_string());
    let workflow_path = github_workflow_path(&options.project_dir, &cfg, &workflow);
    let content = render_github_pages_workflow(&options.project_dir, &cfg);
    if options.dry_run {
        println!("Would write {}:\n{}", workflow_path.display(), content);
        return Ok(());
    }
    if workflow_path.exists() && !options.yes {
        bail!(
            "{} already exists; pass --yes to overwrite or edit it manually",
            workflow_path.display()
        );
    }
    if let Some(parent) = workflow_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&workflow_path, content)
        .with_context(|| format!("failed to write {}", workflow_path.display()))?;
    println!("Wrote {}", workflow_path.display());
    if cfg
        .custom_domain
        .as_deref()
        .filter(|s| !s.is_empty())
        .is_some()
    {
        println!("Custom domains for GitHub Actions Pages must be configured in repository Pages settings or via the GitHub Pages API.");
    }
    Ok(())
}

pub(super) fn publish_github_pages(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = github_config(config, &options.site)?;
    let mode = cfg.mode.as_deref().unwrap_or("actions");
    match mode {
        "branch" => publish_github_pages_branch(options, &cfg, artifact_path, manifest),
        "actions" => {
            let owner = cfg
                .owner
                .clone()
                .or_else(|| infer_github_owner(&options.project_dir));
            let repo = cfg
                .repo
                .clone()
                .or_else(|| infer_github_repo(&options.project_dir));
            let workflow = cfg
                .workflow
                .clone()
                .unwrap_or_else(|| "fission-pages.yml".to_string());
            let mut follow_up = vec![format!(
                "Commit the generated workflow and push the configured production branch so GitHub Actions deploys the exact static site build."
            )];
            if let (Some(owner), Some(repo)) = (&owner, &repo) {
                follow_up.push(format!(
                    "Repository Pages URL should be https://{owner}.github.io/{repo}/ unless a custom domain is configured."
                ));
            }
            let workflow_path = github_workflow_path(&options.project_dir, &cfg, &workflow);
            if !workflow_path.exists() {
                follow_up.push(format!(
                    "Run `fission distribute setup --provider github-pages --site {} --project-dir {}` to generate {}.",
                    options.site,
                    options.project_dir.display(),
                    workflow_path.display()
                ));
            }
            Ok(DistributionReceipt {
                schema_version: 1,
                created_at_unix_seconds: now_unix_seconds(),
                provider: "github-pages".to_string(),
                site: options.site.clone(),
                action: "publish".to_string(),
                artifact_manifest: Some(artifact_path.display().to_string()),
                deployment_id: None,
                canonical_url: github_pages_url(&cfg, owner.as_deref(), repo.as_deref()),
                preview_url: None,
                custom_domain: non_empty(cfg.custom_domain.clone()),
                status: "workflow-required".to_string(),
                stdout: None,
                stderr: None,
                manual_follow_up: follow_up,
            })
        }
        other => bail!("unsupported github-pages mode `{other}`; expected actions or branch"),
    }
}

pub(super) fn publish_github_pages_branch(
    options: &DistributeOptions,
    cfg: &GithubPagesConfig,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let remote = cfg.remote.as_deref().unwrap_or("origin");
    let branch = cfg.source_branch.as_deref().unwrap_or("gh-pages");
    let source_path = cfg.source_path.as_deref().unwrap_or("/");
    let repo_root = git_output(&options.project_dir, ["rev-parse", "--show-toplevel"])?;
    let repo_root = PathBuf::from(repo_root.trim());
    let remote_url = git_output(&repo_root, ["remote", "get-url", remote])?;
    let worktree = options
        .project_dir
        .join("target/fission/publish/github-pages")
        .join(&options.site);
    if options.dry_run {
        println!(
            "Would publish {} to {remote}:{branch} at {}",
            manifest.root_dir, source_path
        );
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: "github-pages".to_string(),
            site: options.site.clone(),
            action: "publish".to_string(),
            artifact_manifest: Some(artifact_path.display().to_string()),
            deployment_id: Some(format!("{remote}:{branch}")),
            canonical_url: github_pages_url(cfg, cfg.owner.as_deref(), cfg.repo.as_deref()),
            preview_url: None,
            custom_domain: non_empty(cfg.custom_domain.clone()),
            status: "dry-run".to_string(),
            stdout: None,
            stderr: None,
            manual_follow_up: Vec::new(),
        });
    }
    if worktree.exists() {
        fs::remove_dir_all(&worktree)
            .with_context(|| format!("failed to clean {}", worktree.display()))?;
    }
    if let Some(parent) = worktree.parent() {
        fs::create_dir_all(parent)?;
    }

    let clone_status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            branch,
            remote_url.trim(),
        ])
        .arg(&worktree)
        .status()
        .context("failed to run git clone for GitHub Pages branch")?;
    if !clone_status.success() {
        let status = Command::new("git")
            .args(["clone", "--depth", "1", remote_url.trim()])
            .arg(&worktree)
            .status()
            .context("failed to run git clone for GitHub Pages repository")?;
        if !status.success() {
            bail!("failed to clone {remote} for GitHub Pages publishing");
        }
        run_git(&worktree, ["checkout", "--orphan", branch])?;
    }

    let publish_root = if source_path == "/" || source_path == "." {
        worktree.clone()
    } else {
        worktree.join(source_path.trim_start_matches('/'))
    };
    clean_publish_root(&publish_root)?;
    copy_dir_contents(Path::new(&manifest.root_dir), &publish_root)?;
    fs::write(publish_root.join(".nojekyll"), "")?;
    if let Some(domain) = non_empty(cfg.custom_domain.clone()) {
        fs::write(publish_root.join("CNAME"), format!("{}\n", domain.trim()))?;
    }

    run_git(&worktree, ["add", "--all"])?;
    let commit = Command::new("git")
        .args(["commit", "-m", "Publish Fission static site"])
        .current_dir(&worktree)
        .output()
        .context("failed to run git commit for GitHub Pages")?;
    if !commit.status.success() {
        let stderr = String::from_utf8_lossy(&commit.stderr);
        if !stderr.contains("nothing to commit") && !stderr.contains("no changes added") {
            io::stderr().write_all(&commit.stderr).ok();
            bail!("git commit failed for GitHub Pages publish");
        }
    }
    run_git(&worktree, ["push", remote, branch])?;
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "github-pages".to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: Some(format!("{remote}:{branch}")),
        canonical_url: github_pages_url(cfg, cfg.owner.as_deref(), cfg.repo.as_deref()),
        preview_url: None,
        custom_domain: non_empty(cfg.custom_domain.clone()),
        status: "published".to_string(),
        stdout: None,
        stderr: None,
        manual_follow_up: github_pages_follow_up(cfg),
    })
}

pub(super) fn publish_cloudflare_pages(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = cloudflare_config(config, &options.site)?;
    let project_name = cfg
        .project_name
        .as_deref()
        .context("distribution.cloudflare_pages.<site>.project_name is required")?;
    let mut args = vec![
        "pages".to_string(),
        "deploy".to_string(),
        manifest.root_dir.clone(),
        "--project-name".to_string(),
        project_name.to_string(),
    ];
    if let Some(environment) = cfg
        .environment
        .as_deref()
        .filter(|value| *value != "production")
    {
        args.push("--branch".to_string());
        args.push(environment.to_string());
    }
    run_publish_command(
        options,
        "cloudflare-pages",
        "wrangler",
        args,
        artifact_path,
        || cloudflare_url(&cfg),
    )
}

pub(super) fn run_publish_command<F>(
    options: &DistributeOptions,
    provider: &str,
    program: &str,
    args: Vec<String>,
    artifact_path: &Path,
    canonical_url: F,
) -> Result<DistributionReceipt>
where
    F: FnOnce() -> Option<String>,
{
    if options.dry_run {
        println!("Would run: {} {}", program, args.join(" "));
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: provider.to_string(),
            site: options.site.clone(),
            action: "publish".to_string(),
            artifact_manifest: Some(artifact_path.display().to_string()),
            deployment_id: None,
            canonical_url: canonical_url(),
            preview_url: None,
            custom_domain: None,
            status: "dry-run".to_string(),
            stdout: None,
            stderr: None,
            manual_follow_up: Vec::new(),
        });
    }
    let output = Command::new(program)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!("failed to run {program}; install it or run readiness for remediation")
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        eprint!("{stderr}");
        bail!("{provider} publish failed with {}", output.status);
    }
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: provider.to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id: None,
        canonical_url: canonical_url(),
        preview_url: first_url(&stdout),
        custom_domain: None,
        status: "published".to_string(),
        stdout: Some(stdout),
        stderr: (!stderr.trim().is_empty()).then_some(stderr),
        manual_follow_up: Vec::new(),
    })
}

pub(super) fn github_pages_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = github_config(config, &options.site)?;
    let owner = cfg
        .owner
        .clone()
        .or_else(|| infer_github_owner(&options.project_dir));
    let repo = cfg
        .repo
        .clone()
        .or_else(|| infer_github_repo(&options.project_dir));
    let (Some(owner), Some(repo)) = (owner, repo) else {
        bail!("github-pages status requires owner and repo in fission.toml or a GitHub remote");
    };
    command_status_receipt(
        options,
        "github-pages",
        "gh",
        vec!["api".to_string(), format!("repos/{owner}/{repo}/pages")],
    )
}

pub(super) fn cloudflare_pages_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = cloudflare_config(config, &options.site)?;
    let account_id = cfg
        .account_id
        .clone()
        .or_else(|| env::var("CLOUDFLARE_ACCOUNT_ID").ok())
        .context(
            "distribution.cloudflare_pages.<site>.account_id or CLOUDFLARE_ACCOUNT_ID is required",
        )?;
    let project_name = cfg
        .project_name
        .as_deref()
        .context("distribution.cloudflare_pages.<site>.project_name is required")?;
    let token =
        env_secret(&["CLOUDFLARE_API_TOKEN"])?.context("CLOUDFLARE_API_TOKEN is required")?;
    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/pages/projects/{project_name}/deployments"
    );
    let response = reqwest::blocking::Client::builder()
        .user_agent("cargo-fission-publish/0.1")
        .build()?
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to query Cloudflare Pages deployments")?;
    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        bail!("Cloudflare Pages status failed with {status}: {text}");
    }
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse Cloudflare Pages status response: {text}"))?;
    let latest = value
        .get("result")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first());
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "cloudflare-pages".to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: latest
            .and_then(|item| item.get("id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        canonical_url: cloudflare_url(&cfg),
        preview_url: latest
            .and_then(|item| item.get("url"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        custom_domain: non_empty(cfg.custom_domain.clone()),
        status: latest
            .and_then(|item| item.pointer("/latest_stage/status"))
            .or_else(|| latest.and_then(|item| item.get("deployment_trigger")))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("ok")
            .to_string(),
        stdout: Some(serde_json::to_string_pretty(&value)?),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
}

pub(super) fn command_status_receipt(
    options: &DistributeOptions,
    provider: &str,
    program: &str,
    args: Vec<String>,
) -> Result<DistributionReceipt> {
    let output = Command::new(program)
        .args(&args)
        .output()
        .with_context(|| format!("failed to run {program}; install it or authenticate first"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: provider.to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: options.deploy.clone(),
        canonical_url: first_url(&stdout),
        preview_url: None,
        custom_domain: None,
        status: if output.status.success() {
            "ok"
        } else {
            "failed"
        }
        .to_string(),
        stdout: Some(stdout),
        stderr: (!stderr.trim().is_empty()).then_some(stderr),
        manual_follow_up: Vec::new(),
    })
}
