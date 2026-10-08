use super::{BuildOptions, WebCargoOptions};
use fission_command_core::{
    website::{self, Action, Data, ErrorCode, Failure, Result},
    Target,
};
use fission_command_process::diagnostic;
use fission_command_site::website::process_failure;
use std::{fs, process::Command};

/// The finite website build API; leaves human/native build APIs unchanged.
pub fn build(options: BuildOptions, web: WebCargoOptions) -> Result<Data> {
    let root = website::absolute(&options.project_dir)?;
    let target = options.target.ok_or_else(|| {
        Failure::new(
            ErrorCode::InvalidTarget,
            "Structured website build requires explicit --target web or --target static-site.",
        )
    })?;
    website::require_website_target(target)?;
    if options.variant.is_some()
        || (target != Target::Web && (!web.features.is_empty() || web.no_default_features))
    {
        return Err(Failure::new(
            ErrorCode::InvalidConfiguration,
            "Website builds do not accept --variant; Cargo feature overrides require --target web.",
        ));
    }
    let project = website::load_project(&root)?;
    website::require_target(&root, &project, target)?;
    if target == Target::Site {
        return fission_command_site::website::execute(&root, options.release, Action::Build);
    }
    let mut cargo = Command::new("cargo");
    cargo.current_dir(&root).arg("--version");
    diagnostic::run(&mut cargo)
        .map_err(|error| process_failure(error, ErrorCode::MissingToolchain, "cargo"))?;
    let mut rustc = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    rustc.current_dir(&root).args([
        "--print",
        "target-libdir",
        "--target",
        "wasm32-unknown-unknown",
    ]);
    let bytes = diagnostic::capture_quiet(&mut rustc, 16 * 1024)
        .map_err(|error| process_failure(error, ErrorCode::MissingToolchain, "rustc"))?;
    let libdir = std::str::from_utf8(&bytes)
        .ok()
        .map(|value| value.trim_end_matches(['\r', '\n']));
    let installed = libdir
        .and_then(|path| fs::read_dir(path).ok())
        .is_some_and(|entries| {
            entries.filter_map(|entry| entry.ok()).any(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with("libcore-") && name.ends_with(".rlib")
            })
        });
    if !installed {
        let mut error = Failure::new(
            ErrorCode::MissingToolchain,
            "The selected Rust compiler has no wasm32-unknown-unknown standard library.",
        );
        error.recovery.push(website::Step {
            instruction: "Install the Web Rust target for the selected compiler. For a rustup-managed compiler, use this invocation.".into(),
            invocation: website::Invocation { cwd: root.clone(), program: "rustup".into(), argv: vec!["target".into(), "add".into(), "wasm32-unknown-unknown".into()] },
        });
        return Err(error);
    }
    super::sync_target_platform_config(&root, &project, target).map_err(|_| {
        Failure::new(ErrorCode::InvalidConfiguration, "Cannot prepare the configured Web target; check Cargo.toml, dependencies and Web storage configuration.")
    })?;
    let package_dir = root.join("platforms/web/pkg");
    let mut command = super::web_build_command(
        &root,
        &package_dir,
        options.release,
        &web.features,
        web.no_default_features,
    );
    diagnostic::run(&mut command)
        .map_err(|error| process_failure(error, ErrorCode::CompileFailed, "wasm-pack"))?;
    let mut files = fs::read_dir(&package_dir)
        .map_err(|_| artifact_missing())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    if !files
        .iter()
        .any(|path| path.extension().is_some_and(|ext| ext == "wasm"))
        || !files
            .iter()
            .any(|path| path.extension().is_some_and(|ext| ext == "js"))
    {
        return Err(artifact_missing());
    }
    let artifact_dir = root.join("platforms/web");
    if !artifact_dir.join("index.html").is_file() {
        return Err(artifact_missing());
    }
    files.push(artifact_dir.join("index.html"));
    if artifact_dir.join("bootstrap.mjs").is_file() {
        files.push(artifact_dir.join("bootstrap.mjs"));
    }
    Ok(Data {
        artifact_dir: Some(artifact_dir),
        artifacts: files,
        ..Data::default()
    })
}

fn artifact_missing() -> Failure {
    Failure::new(
        ErrorCode::ArtifactMissing,
        "The Web build did not create the expected HTML, JavaScript and WASM files.",
    )
}
