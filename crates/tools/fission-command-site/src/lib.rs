use anyhow::{bail, Context, Result};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build(project_dir: &Path, release: bool) -> Result<()> {
    if site_entry_configured(project_dir)? {
        return run_site_builder(project_dir, release, "build", &[]);
    }
    let options = site_build_options(project_dir)?;
    let report = fission_shell_site::build_content_site(&options)?;
    println!(
        "Built {} static route(s) into {}",
        report.routes.len(),
        report.output_dir.display()
    );
    for route in report.routes {
        println!("{} -> {}", route.path, route.output.display());
    }
    Ok(())
}

pub fn check(project_dir: &Path, release: bool) -> Result<()> {
    if site_entry_configured(project_dir)? {
        return run_site_builder(project_dir, release, "check", &[]);
    }
    let options = site_build_options(project_dir)?;
    let report = fission_shell_site::check_content_site(&options)?;
    println!(
        "Checked {} static route(s); output would be {}",
        report.routes.len(),
        report.output_dir.display()
    );
    Ok(())
}

pub fn routes(project_dir: &Path) -> Result<()> {
    if site_entry_configured(project_dir)? {
        return run_site_builder(project_dir, false, "routes", &[]);
    }
    let options = site_build_options(project_dir)?;
    let routes = fission_shell_site::list_content_routes(&options)?;
    for route in routes {
        println!(
            "{}  {}  {}",
            route.path,
            route.title,
            route.source.display()
        );
    }
    Ok(())
}

pub fn serve(project_dir: &Path, release: bool, host: String, port: u16, open: bool) -> Result<()> {
    if site_entry_configured(project_dir)? {
        let port = port.to_string();
        let open_flag = if open { "" } else { "--no-open" };
        let mut args = vec!["--host", host.as_str(), "--port", port.as_str()];
        if !open {
            args.push(open_flag);
        }
        return run_site_builder(project_dir, release, "serve", &args);
    }
    build(project_dir, release)?;
    let options = site_build_options(project_dir)?;
    serve_static(options.output_dir, host, port, open)
}

pub fn serve_static(root: PathBuf, host: String, port: u16, open: bool) -> Result<()> {
    let listener = TcpListener::bind((host.as_str(), port))
        .with_context(|| format!("failed to bind {}:{}", host, port))?;
    let url = if root.join("index.html").exists() {
        format!("http://{host}:{port}/")
    } else {
        format!("http://{host}:{port}/platforms/web/")
    };
    println!("Serving {} at {}", root.display(), url);
    println!("Press Ctrl+C to stop.");
    if open {
        let _ = open_url(&url);
    }
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_http_request(stream, &root) {
                    eprintln!("request failed: {error}");
                }
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}

fn site_build_options(project_dir: &Path) -> Result<fission_shell_site::SiteBuildOptions> {
    let app_name = project_name(project_dir)?;
    fission_shell_site::SiteBuildOptions::from_project_dir(project_dir, app_name.clone()).or_else(
        |_| {
            Ok(fission_shell_site::SiteBuildOptions::for_project(
                project_dir,
                app_name,
            ))
        },
    )
}

fn project_name(project_dir: &Path) -> Result<String> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    value
        .get("app")
        .and_then(|app| app.get("name"))
        .and_then(|name| name.as_str())
        .map(ToString::to_string)
        .context("fission.toml is missing app.name")
}

fn site_entry_configured(project_dir: &Path) -> Result<bool> {
    let path = project_dir.join("fission.toml");
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(value
        .get("site")
        .and_then(|site| site.get("entry"))
        .and_then(|entry| entry.as_str())
        .is_some())
}

fn run_site_builder(
    project_dir: &Path,
    release: bool,
    command_name: &str,
    extra_args: &[&str],
) -> Result<()> {
    let manifest_path = project_dir.join("Cargo.toml");
    if !manifest_path.exists() {
        bail!(
            "site entry is configured but {} is missing",
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
    command
        .arg("--")
        .arg(command_name)
        .arg("--project-dir")
        .arg(project_dir);
    for arg in extra_args {
        if !arg.is_empty() {
            command.arg(arg);
        }
    }
    run_status(&mut command, "site builder")
}

fn run_status(command: &mut Command, label: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to run {label}"))?;
    if !status.success() {
        bail!("{label} failed with {status}");
    }
    Ok(())
}

fn handle_http_request(mut stream: TcpStream, root: &Path) -> Result<()> {
    let mut reader = io::BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");
    let response = static_response(root, path)?;
    stream.write_all(&response)?;
    Ok(())
}

fn static_response(root: &Path, request_path: &str) -> Result<Vec<u8>> {
    let mut relative = request_path.trim_start_matches('/').to_string();
    if relative.is_empty() {
        relative = if root.join("index.html").exists() {
            "index.html".to_string()
        } else {
            "platforms/web/".to_string()
        };
    }
    if relative.ends_with('/') {
        relative.push_str("index.html");
    }
    if !relative.ends_with(".html") && !relative.contains('.') {
        relative.push_str("/index.html");
    }
    let path = sanitize_static_path(root, &relative)?;
    if !path.exists() || !path.is_file() {
        println!("GET {} 404", request_path);
        return Ok(http_response(404, "text/plain", b"not found"));
    }
    let body = fs::read(&path)?;
    let content_type = content_type(&path);
    println!("GET {} 200", request_path);
    Ok(http_response(200, content_type, &body))
}

fn sanitize_static_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut path = PathBuf::from(root);
    for part in relative.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains('\\') {
            bail!("invalid static path `{relative}`");
        }
        path.push(part);
    }
    Ok(path)
}

fn http_response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Error",
    };
    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(OsStr::to_str).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "css" => "text/css; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn open_url(url: &str) -> Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("open");
        cmd.arg(url);
        cmd
    } else if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        cmd
    } else {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(url);
        cmd
    };
    command.spawn()?;
    Ok(())
}
