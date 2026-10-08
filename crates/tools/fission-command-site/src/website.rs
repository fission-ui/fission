//! Structured finite site operations, sharing the shell's build/report APIs.

use fission_command_core::website::{self, Action, Data, ErrorCode, Failure, Result, Route};
use fission_command_core::Target;
use fission_command_process::diagnostic::{self, DiagnosticFailure, FailureKind};
use fission_shell_site::{SiteBuildOptions, SiteBuildReport};
use std::{fs, path::Path, process::Command};

const PAYLOAD_LIMIT: usize = 16 * 1024 * 1024;

pub fn execute(root: &Path, release: bool, action: Action) -> Result<Data> {
    let project = website::load_project(root)?;
    website::require_target(root, &project, Target::Site)?;
    let options = SiteBuildOptions::from_project_dir(root, &project.app.name).map_err(|_| {
        Failure::new(ErrorCode::InvalidConfiguration, "Cannot load [site] configuration or its referenced files; repair fission.toml and the referenced paths.")
    })?;
    let command = match action {
        Action::Build | Action::SiteBuild => "build",
        Action::SiteCheck => "check",
        Action::SiteRoutes => "routes",
        _ => {
            return Err(Failure::new(
                ErrorCode::InvalidConfiguration,
                "Unsupported finite site action.",
            ))
        }
    };
    let report = if super::site_entry_configured(root)
        .map_err(|_| Failure::new(ErrorCode::InvalidConfiguration, "Cannot read [site].entry."))?
    {
        builder_report(root, release, command)?
    } else {
        let report = match command {
            "build" => fission_shell_site::build_content_site(&options),
            "check" => fission_shell_site::check_content_site(&options),
            _ => fission_shell_site::list_content_routes(&options).map(|routes| SiteBuildReport {
                output_dir: options.output_dir.clone(),
                routes,
            }),
        };
        report.map_err(|error| {
            let mut failure = Failure::new(
                ErrorCode::SiteFailed,
                "Static route discovery, rendering or validation failed.",
            );
            failure.diagnostics = diagnostic::excerpt(&format!("{error:#}"));
            failure
        })?
    };
    data_from_report(report, command == "build")
}

fn data_from_report(report: SiteBuildReport, built: bool) -> Result<Data> {
    if built
        && (!report.output_dir.is_dir()
            || report.routes.iter().any(|route| !route.output.is_file()))
    {
        return Err(Failure::new(
            ErrorCode::ArtifactMissing,
            "The site builder did not create all reported output files.",
        ));
    }
    Ok(Data {
        artifact_dir: built.then(|| report.output_dir.clone()),
        planned_output_dir: Some(report.output_dir),
        artifacts: if built {
            report
                .routes
                .iter()
                .map(|route| route.output.clone())
                .collect()
        } else {
            Vec::new()
        },
        routes: report
            .routes
            .into_iter()
            .map(|route| Route {
                path: route.path,
                title: route.title,
                source: route.source,
                output: route.output,
            })
            .collect(),
        ..Data::default()
    })
}

/// Separates Cargo compilation from running the site builder. Binary paths come
/// from Cargo's artifact messages, including custom target directories/profiles.
fn builder_report(root: &Path, release: bool, command_name: &str) -> Result<SiteBuildReport> {
    let manifest = root.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(Failure::new(
            ErrorCode::InvalidConfiguration,
            "[site].entry requires Cargo.toml.",
        ));
    }
    let mut metadata = Command::new("cargo");
    metadata
        .current_dir(root)
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest);
    let bytes = diagnostic::capture_quiet(&mut metadata, PAYLOAD_LIMIT)
        .map_err(|error| process_failure(error, ErrorCode::InvalidConfiguration, "cargo"))?;
    let metadata: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| report_unavailable())?;
    let canonical_manifest = manifest.canonicalize().map_err(|_| report_unavailable())?;
    let package = metadata["packages"]
        .as_array()
        .and_then(|packages| {
            packages.iter().find(|package| {
                package["manifest_path"].as_str().is_some_and(|path| {
                    Path::new(path).canonicalize().ok().as_deref()
                        == Some(canonical_manifest.as_path())
                })
            })
        })
        .ok_or_else(report_unavailable)?;
    let bins = package["targets"]
        .as_array()
        .ok_or_else(report_unavailable)?
        .iter()
        .filter(|target| {
            target["kind"]
                .as_array()
                .is_some_and(|kind| kind.iter().any(|kind| kind == "bin"))
        })
        .collect::<Vec<_>>();
    let name = package["default_run"]
        .as_str()
        .or_else(|| {
            if bins.len() == 1 {
                bins[0]["name"].as_str()
            } else {
                None
            }
        })
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidConfiguration,
                "Select a single site binary or set package.default-run in Cargo.toml.",
            )
        })?;
    let mut compile = Command::new("cargo");
    compile
        .current_dir(root)
        .args(["build", "--manifest-path"])
        .arg(&manifest)
        .args(["--bin", name, "--message-format=json-render-diagnostics"]);
    if release {
        compile.arg("--release");
    }
    let bytes = diagnostic::capture(&mut compile, PAYLOAD_LIMIT)
        .map_err(|error| process_failure(error, ErrorCode::CompileFailed, "cargo"))?;
    let executable = bytes
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
        .filter(|value| value["reason"] == "compiler-artifact" && value["target"]["name"] == name)
        .find_map(|value| value["executable"].as_str().map(std::path::PathBuf::from))
        .ok_or_else(report_unavailable)?;
    let report_file = tempfile::NamedTempFile::new().map_err(|_| report_unavailable())?;
    let mut builder = Command::new(executable);
    builder
        .current_dir(root)
        .arg(command_name)
        .arg("--project-dir")
        .arg(root)
        .arg("--report-file")
        .arg(report_file.path());
    diagnostic::run(&mut builder)
        .map_err(|error| process_failure(error, ErrorCode::SiteFailed, "site builder"))?;
    let mut bytes = Vec::new();
    use std::io::Read;
    fs::File::open(report_file.path())
        .and_then(|file| file.take(PAYLOAD_LIMIT as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| report_unavailable())?;
    if bytes.len() > PAYLOAD_LIMIT {
        return Err(report_unavailable());
    }
    serde_json::from_slice(&bytes).map_err(|_| report_unavailable())
}

fn report_unavailable() -> Failure {
    Failure::new(ErrorCode::ReportUnavailable, "Cannot read the bounded typed site report; use a matching Fission shell revision supporting --report-file and retry.")
}

pub fn process_failure(error: DiagnosticFailure, exit_code: ErrorCode, tool: &str) -> Failure {
    let code = match error.kind {
        FailureKind::MissingExecutable => ErrorCode::MissingToolchain,
        FailureKind::Interrupted => ErrorCode::Interrupted,
        FailureKind::Io => ErrorCode::IoFailed,
        FailureKind::PayloadTooLarge => ErrorCode::ReportUnavailable,
        FailureKind::Exit => exit_code,
    };
    let mut failure = Failure::new(
        code,
        match code {
            ErrorCode::MissingToolchain => format!("Required executable `{tool}` is unavailable."),
            ErrorCode::Interrupted => "The operation was interrupted.".into(),
            ErrorCode::CompileFailed => {
                "Compilation failed; repair the Rust source or Cargo dependencies before retrying."
                    .into()
            }
            ErrorCode::InvalidConfiguration => {
                "Cargo could not load the project configuration.".into()
            }
            ErrorCode::SiteFailed => {
                "The compiled site builder failed during route discovery, rendering or validation."
                    .into()
            }
            _ => "The build process could not complete.".into(),
        },
    );
    if code != ErrorCode::InvalidConfiguration {
        failure.diagnostics = error.diagnostics;
    }
    failure
}
