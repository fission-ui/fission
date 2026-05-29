use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

pub fn check(project_dir: &Path, release: bool) -> Result<()> {
    ensure_server_entry_configured(project_dir)?;
    run_server_builder(project_dir, release, "check", &[])
}

pub fn routes(project_dir: &Path) -> Result<()> {
    ensure_server_entry_configured(project_dir)?;
    run_server_builder(project_dir, false, "routes", &[])
}

pub fn serve(project_dir: &Path, release: bool, host: String, port: u16) -> Result<()> {
    ensure_server_entry_configured(project_dir)?;
    let port = port.to_string();
    run_server_builder(
        project_dir,
        release,
        "serve",
        &["--host", host.as_str(), "--port", port.as_str()],
    )
}

fn ensure_server_entry_configured(project_dir: &Path) -> Result<()> {
    let path = project_dir.join("fission.toml");
    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    if value
        .get("server")
        .and_then(|server| server.get("entry"))
        .and_then(|entry| entry.as_str())
        .is_some()
    {
        Ok(())
    } else {
        bail!("fission.toml is missing [server].entry")
    }
}

fn run_server_builder(
    project_dir: &Path,
    release: bool,
    command_name: &str,
    extra_args: &[&str],
) -> Result<()> {
    let manifest_path = project_dir.join("Cargo.toml");
    if !manifest_path.exists() {
        bail!(
            "server entry is configured but {} is missing",
            manifest_path.display()
        );
    }
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("--manifest-path")
        .arg(&manifest_path);
    if release {
        command.arg("--release");
    }
    command.arg("--").arg(command_name);
    for arg in extra_args {
        command.arg(arg);
    }
    let status = command.status().context("failed to run server app")?;
    if !status.success() {
        bail!("server app failed with {status}");
    }
    Ok(())
}
