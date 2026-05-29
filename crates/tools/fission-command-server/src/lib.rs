use anyhow::{bail, Context, Result};
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;

pub fn check(project_dir: &Path, release: bool) -> Result<()> {
    ensure_server_entry_configured(project_dir)?;
    artifacts(project_dir, release, true).context("failed to build server browser artifacts")?;
    run_server_builder(project_dir, release, "check", &[])
}

pub fn build(project_dir: &Path, release: bool) -> Result<()> {
    ensure_server_entry_configured(project_dir)?;
    artifacts(project_dir, release, true).context("failed to build server browser artifacts")?;
    build_server_binary(project_dir, release)
}

pub fn routes(project_dir: &Path) -> Result<()> {
    ensure_server_entry_configured(project_dir)?;
    run_server_builder(project_dir, false, "routes", &[])
}

pub fn serve(project_dir: &Path, release: bool, host: String, port: u16) -> Result<()> {
    ensure_server_entry_configured(project_dir)?;
    ensure_server_address_available(&host, port)?;
    artifacts(project_dir, release, true).context("failed to build server browser artifacts")?;
    let port = port.to_string();
    run_server_builder(
        project_dir,
        release,
        "serve",
        &["--host", host.as_str(), "--port", port.as_str()],
    )
}

fn ensure_server_address_available(host: &str, port: u16) -> Result<()> {
    let address = format!("{host}:{port}");
    let listener = TcpListener::bind(&address).with_context(|| {
        format!(
            "server address {address} is already in use; stop the existing process or choose another port with --port"
        )
    })?;
    drop(listener);
    Ok(())
}

pub fn artifacts(project_dir: &Path, release: bool, compile: bool) -> Result<()> {
    ensure_server_entry_configured(project_dir)?;
    let package_name = package_name(project_dir)?;
    let features = package_features(project_dir)?;
    let mut args = vec!["--package-name", package_name.as_str()];
    if features.iter().any(|feature| feature == "browser") {
        args.push("--package-no-default-features");
        args.push("--package-feature");
        args.push("browser");
    }
    if !compile {
        args.push("--no-compile");
    }
    run_server_builder(project_dir, release, "artifacts", &args)
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

fn package_name(project_dir: &Path) -> Result<String> {
    let path = project_dir.join("Cargo.toml");
    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    value
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("{} is missing [package].name", path.display()))
}

fn package_features(project_dir: &Path) -> Result<Vec<String>> {
    let path = project_dir.join("Cargo.toml");
    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(value
        .get("features")
        .and_then(|features| features.as_table())
        .map(|features| features.keys().cloned().collect())
        .unwrap_or_default())
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
    let manifest_path = manifest_path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", manifest_path.display()))?;
    let mut command = Command::new("cargo");
    command.current_dir(project_dir);
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

fn build_server_binary(project_dir: &Path, release: bool) -> Result<()> {
    let manifest_path = project_dir.join("Cargo.toml");
    if !manifest_path.exists() {
        bail!(
            "server entry is configured but {} is missing",
            manifest_path.display()
        );
    }
    let manifest_path = manifest_path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", manifest_path.display()))?;
    let mut command = Command::new("cargo");
    command.current_dir(project_dir);
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path);
    if release {
        command.arg("--release");
    }
    let status = command.status().context("failed to build server app")?;
    if !status.success() {
        bail!("server app build failed with {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_project(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn server_entry_configuration_is_required() {
        let dir = temp_project("fission-server-config-missing");
        fs::write(dir.join("fission.toml"), "[app]\nname = \"Test\"\n").unwrap();

        let error = ensure_server_entry_configured(&dir).unwrap_err();
        assert!(error.to_string().contains("[server].entry"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_package_name_and_browser_feature_for_artifact_shims() {
        let dir = temp_project("fission-server-config-package");
        fs::write(
            dir.join("Cargo.toml"),
            r#"[package]
name = "server-app"
version = "0.1.0"
edition = "2021"

[features]
default = ["server"]
server = []
browser = []
"#,
        )
        .unwrap();

        assert_eq!(package_name(&dir).unwrap(), "server-app");
        assert!(package_features(&dir)
            .unwrap()
            .iter()
            .any(|feature| feature == "browser"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn serve_preflight_reports_busy_port_before_building_artifacts() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let error = ensure_server_address_available("127.0.0.1", port).unwrap_err();
        assert!(error.to_string().contains("already in use"));
    }
}
