use super::*;
use anyhow::{bail, Context, Result};
use fission_credentials as credentials;
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use std::time::Duration;
use zip::write::SimpleFileOptions;

const NETLIFY_API: &str = "https://api.netlify.com/api/v1";

pub(super) fn publish_netlify(
    options: &DistributeOptions,
    config: &PublishManifest,
    artifact_path: &Path,
    manifest: &ArtifactManifest,
) -> Result<DistributionReceipt> {
    let cfg = netlify_config(config, &options.site)?;
    let site_id = cfg
        .site_id
        .as_deref()
        .context("distribution.netlify.<site>.site_id is required")?;
    let token = netlify_token()?;
    let zip = zip_directory(Path::new(&manifest.root_dir))?;
    if options.dry_run {
        return Ok(receipt(
            options,
            artifact_path,
            "dry-run",
            None,
            netlify_configured_url(&cfg),
            Some(format!(
                "{} bytes zipped for Netlify site {site_id}",
                zip.len()
            )),
            None,
            Vec::new(),
        ));
    }
    let client = http_client()?;
    let deploy_url = if cfg.production.unwrap_or(true) {
        format!("{NETLIFY_API}/sites/{site_id}/deploys")
    } else {
        format!("{NETLIFY_API}/sites/{site_id}/deploys?draft=true")
    };
    let response = client
        .post(deploy_url)
        .bearer_auth(token)
        .header("Content-Type", "application/zip")
        .body(zip)
        .send()
        .context("failed to create Netlify deploy")?;
    let value = json_response(response, "Netlify deploy")?;
    let deployment_id = value.get("id").and_then(Value::as_str).map(str::to_string);
    let canonical_url = value
        .get("ssl_url")
        .or_else(|| value.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| netlify_configured_url(&cfg));
    let preview_url = value
        .get("deploy_ssl_url")
        .or_else(|| value.get("deploy_url"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let status = value
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("uploaded");
    Ok(receipt(
        options,
        artifact_path,
        status,
        deployment_id,
        canonical_url,
        Some(value.to_string()),
        None,
        preview_url
            .into_iter()
            .map(|url| format!("Preview URL: {url}"))
            .collect(),
    ))
}

pub(super) fn netlify_status(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = netlify_config(config, &options.site)?;
    let site_id = cfg
        .site_id
        .as_deref()
        .context("distribution.netlify.<site>.site_id is required")?;
    let client = http_client()?;
    let token = netlify_token()?;
    let url = if let Some(deploy) = options.deploy.as_deref() {
        format!("{NETLIFY_API}/sites/{site_id}/deploys/{deploy}")
    } else {
        format!("{NETLIFY_API}/sites/{site_id}/deploys")
    };
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .context("failed to fetch Netlify status")?;
    let value = json_response(response, "Netlify status")?;
    let first = value
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or(&value);
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "netlify".to_string(),
        site: options.site.clone(),
        action: "status".to_string(),
        artifact_manifest: None,
        deployment_id: first.get("id").and_then(Value::as_str).map(str::to_string),
        canonical_url: first
            .get("ssl_url")
            .or_else(|| first.get("url"))
            .and_then(Value::as_str)
            .map(str::to_string),
        preview_url: first
            .get("deploy_ssl_url")
            .or_else(|| first.get("deploy_url"))
            .and_then(Value::as_str)
            .map(str::to_string),
        custom_domain: cfg.custom_domain.clone(),
        status: first
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("ok")
            .to_string(),
        stdout: Some(value.to_string()),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
}

pub(super) fn netlify_lifecycle(
    options: &DistributeOptions,
    config: &PublishManifest,
) -> Result<DistributionReceipt> {
    let cfg = netlify_config(config, &options.site)?;
    let site_id = cfg
        .site_id
        .as_deref()
        .context("distribution.netlify.<site>.site_id is required")?;
    let deploy = options
        .deploy
        .as_deref()
        .context("netlify promote/rollback requires --deploy <deploy-id>")?;
    if options.dry_run {
        return Ok(DistributionReceipt {
            schema_version: 1,
            created_at_unix_seconds: now_unix_seconds(),
            provider: "netlify".to_string(),
            site: options.site.clone(),
            action: action_name(options).to_string(),
            artifact_manifest: None,
            deployment_id: Some(deploy.to_string()),
            canonical_url: netlify_configured_url(&cfg),
            preview_url: None,
            custom_domain: cfg.custom_domain.clone(),
            status: "dry-run".to_string(),
            stdout: None,
            stderr: None,
            manual_follow_up: vec![format!(
                "Would restore Netlify deploy {deploy} as the live site."
            )],
        });
    }
    let client = http_client()?;
    let token = netlify_token()?;
    let url = format!("{NETLIFY_API}/sites/{site_id}/deploys/{deploy}/restore");
    let response = client
        .post(url)
        .bearer_auth(token)
        .send()
        .context("failed to restore Netlify deploy")?;
    let value = json_response(response, "Netlify restore deploy")?;
    Ok(DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "netlify".to_string(),
        site: options.site.clone(),
        action: action_name(options).to_string(),
        artifact_manifest: None,
        deployment_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        canonical_url: value
            .get("ssl_url")
            .or_else(|| value.get("url"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| netlify_configured_url(&cfg)),
        preview_url: value
            .get("deploy_ssl_url")
            .or_else(|| value.get("deploy_url"))
            .and_then(Value::as_str)
            .map(str::to_string),
        custom_domain: cfg.custom_domain.clone(),
        status: value
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("restored")
            .to_string(),
        stdout: Some(value.to_string()),
        stderr: None,
        manual_follow_up: Vec::new(),
    })
}

fn zip_directory(root: &Path) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    add_zip_entries(root, root, &mut writer, options)?;
    Ok(writer.finish()?.into_inner())
}

fn add_zip_entries<W: Write + std::io::Seek>(
    root: &Path,
    current: &Path,
    writer: &mut zip::ZipWriter<W>,
    options: SimpleFileOptions,
) -> Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("failed to read {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        if entry.file_type()?.is_dir() {
            if !relative.is_empty() {
                writer.add_directory(format!("{relative}/"), options)?;
            }
            add_zip_entries(root, &path, writer, options)?;
        } else if entry.file_type()?.is_file() {
            writer.start_file(relative, options)?;
            writer.write_all(&fs::read(&path)?)?;
        }
    }
    Ok(())
}

fn netlify_token() -> Result<String> {
    credentials::provider_secret(DistributionProvider::Netlify, &["NETLIFY_AUTH_TOKEN"])?
        .context("NETLIFY_AUTH_TOKEN or Fission vault credentials are required for Netlify")
}

fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent("cargo-fission-release/0.1")
        .build()
        .context("failed to build Netlify HTTP client")
}

fn json_response(response: reqwest::blocking::Response, operation: &str) -> Result<Value> {
    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        bail!("{operation} failed with {status}: {text}");
    }
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {operation} response: {text}"))
}

fn receipt(
    options: &DistributeOptions,
    artifact_path: &Path,
    status: &str,
    deployment_id: Option<String>,
    canonical_url: Option<String>,
    stdout: Option<String>,
    stderr: Option<String>,
    manual_follow_up: Vec<String>,
) -> DistributionReceipt {
    DistributionReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        provider: "netlify".to_string(),
        site: options.site.clone(),
        action: "publish".to_string(),
        artifact_manifest: Some(artifact_path.display().to_string()),
        deployment_id,
        canonical_url,
        preview_url: None,
        custom_domain: None,
        status: status.to_string(),
        stdout,
        stderr,
        manual_follow_up,
    }
}

fn action_name(options: &DistributeOptions) -> &'static str {
    match options.action {
        DistributeAction::Promote => "promote",
        DistributeAction::Rollback => "rollback",
        _ => "lifecycle",
    }
}

fn netlify_configured_url(cfg: &NetlifyConfig) -> Option<String> {
    cfg.custom_domain
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .map(|domain| format!("https://{}", domain.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fission-static-hosts-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn zip_directory_includes_nested_static_assets() {
        let dir = unique_dir("zip");
        fs::create_dir_all(dir.join("assets")).unwrap();
        fs::write(dir.join("index.html"), "<h1>Home</h1>").unwrap();
        fs::write(dir.join("assets/site.css"), "body {}").unwrap();
        let zip = zip_directory(&dir).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
        assert!(archive.by_name("index.html").is_ok());
        assert!(archive.by_name("assets/site.css").is_ok());
    }
}
