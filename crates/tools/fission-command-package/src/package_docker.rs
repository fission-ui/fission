use super::*;

pub(super) fn docker_image_tags(
    options: &PackageOptions,
    project: &FissionProject,
    config: &DockerPackageConfig,
) -> Vec<String> {
    let configured = config
        .tags
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|tag| !tag.trim().is_empty())
        .collect::<Vec<_>>();
    if !configured.is_empty() {
        return configured;
    }
    let version =
        cargo_package_version(&options.project_dir).unwrap_or_else(|| "latest".to_string());
    vec![format!(
        "{}:{}",
        sanitize_docker_image_name(&project.app.name),
        version
    )]
}

pub(super) fn write_server_docker_context(
    options: &PackageOptions,
    project: &FissionProject,
    config: &DockerPackageConfig,
    staging_dir: &Path,
    tags: &[String],
) -> Result<()> {
    let workspace_root = cargo_workspace_root(&options.project_dir)
        .unwrap_or_else(|| options.project_dir.clone())
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", options.project_dir.display()))?;
    let project_dir = options
        .project_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", options.project_dir.display()))?;
    let project_relative = project_dir
        .strip_prefix(&workspace_root)
        .unwrap_or(Path::new("."))
        .to_string_lossy()
        .replace('\\', "/");
    let package_name =
        cargo_package_name(&options.project_dir).unwrap_or_else(|| project.app.name.clone());
    let binary_name = sanitize_file_stem(&package_name);
    let artifact_args = server_artifact_args(&options.project_dir, options.release)?;
    let context_workspace = staging_dir.join("workspace");
    copy_docker_source_tree(&workspace_root, &context_workspace)?;
    write_dockerfile(
        staging_dir,
        &render_server_dockerfile(
            config.base_image(),
            config.port(),
            &project_relative,
            &package_name,
            &binary_name,
            &artifact_args,
        ),
    )?;
    fs::write(
        staging_dir.join(".dockerignore"),
        "target/\n.git/\n**/.DS_Store\n**/target/\n",
    )?;
    write_docker_context_readme(
        staging_dir,
        options,
        tags,
        "Server image context. The Dockerfile compiles the Fission server app inside a Rust builder stage, then runs the resulting binary in a minimal runtime stage.",
    )
}

pub(super) fn write_static_site_docker_context(
    options: &PackageOptions,
    _project: &FissionProject,
    config: &DockerPackageConfig,
    staging_dir: &Path,
    tags: &[String],
) -> Result<()> {
    fission_command_site::build(&options.project_dir, options.release)?;
    let source_dir = site_output_dir(&options.project_dir)?;
    if !source_dir.join("index.html").exists() {
        bail!(
            "static site output {} does not contain index.html",
            source_dir.display()
        );
    }
    copy_dir_contents(&source_dir, &staging_dir.join("site"))?;
    write_static_server_crate(staging_dir, config.adapter())?;
    write_dockerfile(
        staging_dir,
        &render_static_site_dockerfile(config.base_image(), config.port()),
    )?;
    fs::write(
        staging_dir.join(".dockerignore"),
        "target/\n.git/\n**/.DS_Store\n",
    )?;
    write_docker_context_readme(
        staging_dir,
        options,
        tags,
        "Static-site image context. The Dockerfile builds a small Rust static-file server and copies the generated site into the runtime image.",
    )
}

fn write_dockerfile(staging_dir: &Path, content: &str) -> Result<()> {
    fs::write(staging_dir.join("Dockerfile"), content).with_context(|| {
        format!(
            "failed to write {}",
            staging_dir.join("Dockerfile").display()
        )
    })
}

pub(super) fn write_static_server_crate(
    staging_dir: &Path,
    adapter: DockerStaticAdapter,
) -> Result<()> {
    let server_dir = staging_dir.join("server");
    fs::create_dir_all(server_dir.join("src"))?;
    match adapter {
        DockerStaticAdapter::Axum => {
            fs::write(
                server_dir.join("Cargo.toml"),
                r#"[package]
name = "fission-static-server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tower-http = { version = "0.6", features = ["fs"] }
"#,
            )?;
            fs::write(server_dir.join("src/main.rs"), AXUM_STATIC_SERVER)?;
        }
        DockerStaticAdapter::Actix => {
            fs::write(
                server_dir.join("Cargo.toml"),
                r#"[package]
name = "fission-static-server"
version = "0.1.0"
edition = "2021"

[dependencies]
actix-files = "0.6"
actix-web = "4"
"#,
            )?;
            fs::write(server_dir.join("src/main.rs"), ACTIX_STATIC_SERVER)?;
        }
    }
    Ok(())
}

pub(super) fn render_server_dockerfile(
    base_image: &str,
    port: u16,
    project_relative: &str,
    package_name: &str,
    binary_name: &str,
    artifact_args: &str,
) -> String {
    format!(
        r#"FROM rust:1-bookworm AS builder
WORKDIR /workspace
COPY workspace/ .
WORKDIR /workspace/{project_relative}
RUN rustup target add wasm32-unknown-unknown
RUN cargo build --release --package {package_name} --bin {binary_name}
RUN mkdir -p target/fission/server && cargo run --release --package {package_name} --bin {binary_name} -- artifacts --package-name {package_name}{artifact_args}

FROM {base_image}
RUN useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin fission
WORKDIR /app
COPY --from=builder /workspace/target/release/{binary_name} /usr/local/bin/{binary_name}
COPY --from=builder /workspace/{project_relative}/target/fission/server /app/server-artifacts
COPY --from=builder /workspace/{project_relative}/fission.toml /app/fission.toml
ENV HOST=0.0.0.0
ENV PORT={port}
ENV FISSION_SERVER_ARTIFACTS=/app/server-artifacts
EXPOSE {port}
USER fission
CMD ["sh", "-c", "exec /usr/local/bin/{binary_name} serve --host ${{HOST:-0.0.0.0}} --port ${{PORT:-{port}}}"]
"#
    )
}

fn server_artifact_args(project_dir: &Path, release: bool) -> Result<String> {
    let manifest_path = project_dir.join("Cargo.toml");
    let data = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let value: toml::Value = toml::from_str(&data)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    let has_browser_feature = value
        .get("features")
        .and_then(toml::Value::as_table)
        .is_some_and(|features| features.contains_key("browser"));
    let mut args = Vec::new();
    if release {
        args.push("--release".to_string());
    }
    if has_browser_feature {
        args.push("--package-no-default-features".to_string());
        args.push("--package-feature browser".to_string());
    }
    Ok(if args.is_empty() {
        String::new()
    } else {
        format!(" {}", args.join(" "))
    })
}

fn render_static_site_dockerfile(base_image: &str, port: u16) -> String {
    format!(
        r#"FROM rust:1-bookworm AS builder
WORKDIR /workspace
COPY server/ server/
RUN cargo build --release --manifest-path server/Cargo.toml

FROM {base_image}
RUN useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin fission
WORKDIR /srv/fission-site
COPY site/ /srv/fission-site/
COPY --from=builder /workspace/server/target/release/fission-static-server /usr/local/bin/fission-static-server
ENV HOST=0.0.0.0
ENV PORT={port}
ENV FISSION_STATIC_ROOT=/srv/fission-site
EXPOSE {port}
USER fission
CMD ["sh", "-c", "exec /usr/local/bin/fission-static-server --host ${{HOST:-0.0.0.0}} --port ${{PORT:-{port}}} --root ${{FISSION_STATIC_ROOT:-/srv/fission-site}}"]
"#
    )
}

pub(super) fn build_docker_image(staging_dir: &Path, tags: &[String]) -> Result<()> {
    if tags.is_empty() {
        bail!("docker-image packaging requires at least one image tag");
    }
    if find_in_path("docker").is_none() {
        bail!("docker was not found on PATH; install Docker or set [package.docker].build = false to generate the image context only");
    }
    let mut command = Command::new("docker");
    command.arg("build");
    for tag in tags {
        command.arg("--tag").arg(tag);
    }
    command.arg(staging_dir);
    let status = command.status().context("failed to run docker build")?;
    if !status.success() {
        bail!("docker build failed with {status}");
    }
    Ok(())
}

pub(super) fn write_docker_image_metadata(
    options: &PackageOptions,
    project: &FissionProject,
    config: &DockerPackageConfig,
    staging_dir: &Path,
    tags: &[String],
    built: bool,
) -> Result<()> {
    let metadata = json!({
        "schema_version": 1,
        "app_id": project.app.app_id,
        "app_name": project.app.name,
        "target": options.target.as_str(),
        "format": options.format.as_str(),
        "adapter": match config.adapter() {
            DockerStaticAdapter::Actix => "actix",
            DockerStaticAdapter::Axum => "axum",
        },
        "port": config.port(),
        "base_image": config.base_image(),
        "tags": tags,
        "built": built,
    });
    fs::write(
        staging_dir.join("image-metadata.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;
    Ok(())
}

pub(super) fn write_docker_context_readme(
    staging_dir: &Path,
    options: &PackageOptions,
    tags: &[String],
    description: &str,
) -> Result<()> {
    fs::write(
        staging_dir.join("README.md"),
        format!(
            "# Fission Docker image context\n\n{description}\n\nTarget: `{}`\nFormat: `{}`\nTags: `{}`\n\nBuild manually with:\n\n```sh\ndocker build {}\n```\n",
            options.target.as_str(),
            options.format.as_str(),
            tags.join("`, `"),
            tags.iter()
                .map(|tag| format!("--tag {tag}"))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    )?;
    Ok(())
}

pub(super) fn copy_docker_source_tree(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if matches!(
            name_str.as_ref(),
            ".git"
                | ".tmp"
                | "target"
                | "dist"
                | "node_modules"
                | "platforms"
                | ".idea"
                | ".vscode"
        ) {
            continue;
        }
        let source_path = entry.path();
        let dest_path = dest.join(&name);
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            let Ok(metadata) = fs::metadata(&source_path) else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
        }
        if file_type.is_dir() {
            copy_docker_source_tree(&source_path, &dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &dest_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn cargo_workspace_root(project_dir: &Path) -> Option<PathBuf> {
    let output = Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .current_dir(project_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let manifest = String::from_utf8_lossy(&output.stdout);
    let manifest = manifest.trim();
    if manifest.is_empty() {
        return None;
    }
    PathBuf::from(manifest).parent().map(Path::to_path_buf)
}

pub(super) fn sanitize_docker_image_name(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        let valid = matches!(ch, 'a'..='z' | '0'..='9' | '.' | '_' | '-');
        if valid {
            out.push(ch);
            last_dash = ch == '-';
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches(['-', '.', '_']).to_string();
    if out.is_empty() {
        "fission-app".to_string()
    } else {
        out
    }
}

const AXUM_STATIC_SERVER: &str = r#"use axum::Router;
use std::env;
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let mut port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    let mut root = env::var("FISSION_STATIC_ROOT").unwrap_or_else(|_| "/srv/fission-site".to_string());
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => host = args.next().unwrap_or(host),
            "--port" => port = args.next().and_then(|value| value.parse().ok()).unwrap_or(port),
            "--root" => root = args.next().unwrap_or(root),
            _ => {}
        }
    }
    let app = Router::new().fallback_service(ServeDir::new(root).append_index_html_on_directories(true));
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
"#;

const ACTIX_STATIC_SERVER: &str = r#"use actix_files::Files;
use actix_web::{App, HttpServer};
use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let mut host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let mut port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    let mut root = env::var("FISSION_STATIC_ROOT").unwrap_or_else(|_| "/srv/fission-site".to_string());
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => host = args.next().unwrap_or(host),
            "--port" => port = args.next().and_then(|value| value.parse().ok()).unwrap_or(port),
            "--root" => root = args.next().unwrap_or(root),
            _ => {}
        }
    }
    HttpServer::new(move || App::new().service(Files::new("/", root.clone()).index_file("index.html")))
        .bind((host, port))?
        .run()
        .await
}
"#;
