//! Web build facts; used by the common build authority.
use super::WebCargoOptions;
use fission_command_core::report::process_failure;
use fission_command_core::{
    report::{self, ErrorCode, Failure, Result, TargetData},
    Target,
};
use fission_command_process::diagnostic;
use std::{fs, path::Path, process::Command};

pub(super) fn build(root: &Path, release: bool, web: &WebCargoOptions) -> Result<TargetData> {
    let mut cargo = Command::new("cargo");
    cargo.current_dir(root).arg("--version");
    diagnostic::run(&mut cargo)
        .map_err(|error| process_failure(error, ErrorCode::MissingToolchain, "cargo"))?;
    let mut rustc = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    rustc.current_dir(root).args([
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
        error.recovery.push(report::Step {
            instruction: "Install the Web Rust target for the selected compiler. For a rustup-managed compiler, use this invocation.".into(),
            invocation: report::Invocation { cwd: root.to_path_buf(), program: "rustup".into(), argv: vec!["target".into(), "add".into(), "wasm32-unknown-unknown".into()] },
        });
        return Err(error);
    }
    let package_dir = root.join("platforms/web/pkg");
    let mut command = super::web_build_command(
        root,
        &package_dir,
        release,
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
    Ok(TargetData {
        target: Target::Web,
        artifact_dir: Some(artifact_dir),
        artifacts: files,
        planned_output_dir: None,
        routes: Vec::new(),
    })
}

fn artifact_missing() -> Failure {
    Failure::new(
        ErrorCode::ArtifactMissing,
        "The Web build did not create the expected HTML, JavaScript and WASM files.",
    )
}
