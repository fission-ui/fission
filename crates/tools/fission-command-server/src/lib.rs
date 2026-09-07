use anyhow::{bail, Context, Result};
use fission_command_process::{run_status, SupervisedChild};
use serde_json::Value;
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};

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

pub fn spawn_serve(
    project_dir: &Path,
    release: bool,
    host: String,
    port: u16,
) -> Result<SupervisedChild> {
    ensure_server_entry_configured(project_dir)?;
    ensure_server_address_available(&host, port)?;
    artifacts(project_dir, release, true).context("failed to build server browser artifacts")?;
    let port_text = port.to_string();
    spawn_server_builder(
        project_dir,
        release,
        "serve",
        &["--host", host.as_str(), "--port", port_text.as_str()],
    )
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
    let package = server_package(project_dir)?;
    let mut args = vec!["--package-name", package.name.as_str()];
    if package.features.contains("browser") {
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct ServerPackage {
    name: String,
    features: BTreeSet<String>,
}

fn configured_server_package(project_dir: &Path) -> Result<Option<String>> {
    let path = project_dir.join("fission.toml");
    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(value
        .get("server")
        .and_then(|server| server.get("package"))
        .and_then(|package| package.as_str())
        .map(ToString::to_string))
}

fn server_package(project_dir: &Path) -> Result<ServerPackage> {
    let manifest_path = project_dir.join("Cargo.toml");
    if !manifest_path.exists() {
        bail!("{} is missing", manifest_path.display());
    }
    let manifest_path = manifest_path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", manifest_path.display()))?;
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(project_dir)
        .output()
        .context("failed to run cargo metadata for the server package")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed for {}: {}",
            manifest_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .context("failed to parse cargo metadata for the server package")?;
    select_server_package(
        &metadata,
        &manifest_path,
        configured_server_package(project_dir)?.as_deref(),
    )
}

fn select_server_package(
    metadata: &Value,
    root_manifest_path: &Path,
    configured_name: Option<&str>,
) -> Result<ServerPackage> {
    let workspace_members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .context("cargo metadata omitted workspace_members")?;
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .context("cargo metadata omitted packages")?;
    let selected = packages.iter().find(|package| {
        let is_workspace_member = package.get("id").is_some_and(|id| {
            workspace_members
                .iter()
                .any(|workspace_id| workspace_id == id)
        });
        if !is_workspace_member {
            return false;
        }
        if let Some(name) = configured_name {
            package.get("name").and_then(Value::as_str) == Some(name)
        } else {
            package
                .get("manifest_path")
                .and_then(Value::as_str)
                .is_some_and(|path| Path::new(path) == root_manifest_path)
        }
    });
    let selected = match (selected, configured_name) {
        (Some(package), _) => package,
        (None, Some(name)) => bail!(
            "[server].package `{name}` is not a package in the Cargo workspace rooted at {}",
            root_manifest_path.display()
        ),
        (None, None) => bail!(
            "{} is a virtual Cargo workspace; set [server].package to the server package name",
            root_manifest_path.display()
        ),
    };
    let name = selected
        .get("name")
        .and_then(Value::as_str)
        .context("selected Cargo package is missing its name")?
        .to_string();
    let features = selected
        .get("features")
        .and_then(Value::as_object)
        .map(|features| features.keys().cloned().collect())
        .unwrap_or_default();
    Ok(ServerPackage { name, features })
}

fn spawn_server_builder(
    project_dir: &Path,
    release: bool,
    command_name: &str,
    extra_args: &[&str],
) -> Result<SupervisedChild> {
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
    let package = server_package(project_dir)?;
    command.current_dir(project_dir);
    command
        .arg("run")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--package")
        .arg(&package.name);
    if release {
        command.arg("--release");
    }
    command.arg("--").arg(command_name);
    for arg in extra_args {
        command.arg(arg);
    }
    command.stdout(Stdio::null()).stderr(Stdio::null());
    SupervisedChild::spawn(&mut command).context("failed to spawn server app")
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
    let package = server_package(project_dir)?;
    command.current_dir(project_dir);
    command
        .arg("run")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--package")
        .arg(&package.name);
    if release {
        command.arg("--release");
    }
    command.arg("--").arg(command_name);
    for arg in extra_args {
        command.arg(arg);
    }
    run_status(&mut command, "server app")
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
    let package = server_package(project_dir)?;
    command.current_dir(project_dir);
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--package")
        .arg(&package.name);
    if release {
        command.arg("--release");
    }
    run_status(&mut command, "server app build")
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
    fn selects_configured_virtual_workspace_package_and_its_features() {
        let metadata = serde_json::json!({
            "workspace_members": ["path+file:///workspace/apps/server#server-app@0.1.0"],
            "packages": [
                {
                    "id": "path+file:///workspace/apps/server#server-app@0.1.0",
                    "name": "server-app",
                    "manifest_path": "/workspace/apps/server/Cargo.toml",
                    "features": {"browser": [], "server": []}
                },
                {
                    "id": "registry+https://example.invalid/dependency#1.0.0",
                    "name": "dependency",
                    "manifest_path": "/registry/dependency/Cargo.toml",
                    "features": {}
                }
            ]
        });

        let package = select_server_package(
            &metadata,
            Path::new("/workspace/Cargo.toml"),
            Some("server-app"),
        )
        .unwrap();

        assert_eq!(package.name, "server-app");
        assert!(package.features.contains("browser"));
    }

    #[test]
    fn virtual_workspace_requires_explicit_server_package() {
        let metadata = serde_json::json!({
            "workspace_members": ["server-id"],
            "packages": [{
                "id": "server-id",
                "name": "server-app",
                "manifest_path": "/workspace/apps/server/Cargo.toml",
                "features": {}
            }]
        });

        let error =
            select_server_package(&metadata, Path::new("/workspace/Cargo.toml"), None).unwrap_err();

        assert!(error.to_string().contains("set [server].package"));
    }

    #[test]
    fn serve_preflight_reports_busy_port_before_building_artifacts() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let error = ensure_server_address_available("127.0.0.1", port).unwrap_err();
        assert!(error.to_string().contains("already in use"));
    }
}
