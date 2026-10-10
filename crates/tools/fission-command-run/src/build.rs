//! The build authority for every supported target and both CLI renderers.
use super::*;
use fission_command_core::report::{
    self, Data, ErrorCode, Failure, Result as ReportResult, TargetData,
};
use fission_command_process::diagnostic;

/// Preserves the existing host default when no target was requested.
pub fn resolve_build_target(target: Option<Target>) -> Target {
    target.unwrap_or_else(host_desktop_target)
}

pub fn build_app_with_web_cargo_options(
    mut options: BuildOptions,
    web: WebCargoOptions,
) -> ReportResult<Data> {
    options.project_dir = report::absolute(&options.project_dir)?;
    let root = &options.project_dir;
    let project = report::load_project(root)?;
    let target = resolve_build_target(options.target);
    ensure_native_variant_target(target, options.variant.as_ref())
        .map_err(|error| validation_failure(error, ErrorCode::InvalidTarget))?;
    ensure_web_cargo_feature_target(target, &web.features, web.no_default_features)
        .map_err(|error| validation_failure(error, ErrorCode::InvalidTarget))?;
    ensure_target_configured(&project, root, target).map_err(|error| {
        let code = if !project.targets.contains(&target) {
            ErrorCode::TargetNotConfigured
        } else {
            ErrorCode::ScaffoldMissing
        };
        validation_failure(error, code).recover(report::step(
            root,
            "Add or repair the selected target scaffold.",
            &["add-target", target.as_str(), "--json"],
        ))
    })?;
    sync_target_platform_config(root, &project, target)
        .map_err(|error| report::operation_failure(&error))?;

    if target == Target::Site {
        return fission_command_site::build(root, options.release);
    }
    if target == Target::Web {
        return super::web_build::build(root, options.release, &web).map(|target| Data {
            targets: vec![target],
            ..Data::default()
        });
    }
    if matches!(target, Target::Linux | Target::Windows | Target::Macos) {
        require_desktop_host(target)
            .map_err(|error| validation_failure(error, ErrorCode::InvalidTarget))?;
    } else if target == Target::Ios {
        require_host(target)
            .map_err(|error| validation_failure(error, ErrorCode::InvalidTarget))?;
    }
    let built: Result<TargetData> = (|| {
        let mut data = TargetData::completed(target);
        match target {
            Target::Linux | Target::Windows | Target::Macos | Target::Terminal => {
                let binary =
                    build_desktop_binary(root, options.release, target, options.variant.as_ref())?;
                match target {
                    Target::Linux => {
                        build_linux_native_modules(
                            root,
                            &project,
                            options.variant.as_ref(),
                            options.release,
                        )?;
                    }
                    Target::Windows => {
                        build_windows_native_modules(
                            root,
                            &project,
                            options.variant.as_ref(),
                            options.release,
                        )?;
                    }
                    Target::Macos => {
                        build_macos_native_modules(
                            root,
                            &project,
                            options.variant.as_ref(),
                            options.release,
                        )?;
                    }
                    _ => {}
                }
                data.artifacts.push(binary.path);
            }
            Target::Server => fission_command_server::build(root, options.release)?,
            Target::Ios => {
                let mut command = command_for_script(&root.join("platforms/ios/package-sim.sh"))?;
                command.current_dir(root);
                if options.release {
                    command.env("IOS_SIM_PROFILE", "release");
                }
                diagnostic::run(&mut command)?;
            }
            Target::Android => data.artifacts.push(package_android(root, options.release)?),
            Target::Site | Target::Web => unreachable!("handled above"),
        }
        if data.artifacts.iter().any(|path| !path.is_file()) {
            return Err(Failure::new(
                ErrorCode::ArtifactMissing,
                "The build did not create its reported artifacts.",
            )
            .into());
        }
        Ok(data)
    })();
    built
        .map(|target| Data {
            targets: vec![target],
            ..Data::default()
        })
        .map_err(build_failure)
}

fn validation_failure(error: anyhow::Error, code: ErrorCode) -> Failure {
    Failure::new(code, diagnostic::excerpt(&error.to_string()))
}

fn build_failure(error: anyhow::Error) -> Failure {
    if let Some(error) = error.downcast_ref::<diagnostic::DiagnosticFailure>() {
        return fission_command_core::report::process_failure(
            diagnostic::DiagnosticFailure {
                kind: error.kind,
                diagnostics: error.diagnostics.clone(),
            },
            ErrorCode::CompileFailed,
            "build tool",
        );
    }
    if let Some(failure) = error.downcast_ref::<Failure>() {
        return Failure::new(failure.code, failure.message.clone());
    }
    let mut failure = Failure::new(
        ErrorCode::CompileFailed,
        "The selected target build failed.",
    );
    failure.diagnostics = diagnostic::excerpt(&format!("{error:#}"));
    failure
}
