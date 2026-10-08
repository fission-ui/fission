//! Maps subprocess outcomes without selecting a renderer.
use crate::report::{ErrorCode, Failure};
use fission_command_process::diagnostic::{DiagnosticFailure, FailureKind};

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
        ErrorCode::MissingToolchain => format!("Required toolchain command `{tool}` is unavailable or cannot run for this project."),
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
