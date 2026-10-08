use crate::cli::{Command, SiteCommand};
use fission_command_core::{
    website::{self, Action, CommandResult, Data, ErrorCode, Failure, Outcome, Step},
    Target,
};
use std::path::Path;

pub(crate) fn dispatch(command: &Command) -> Option<anyhow::Result<()>> {
    match command {
        Command::Init {
            path,
            name,
            app_id,
            local_path,
            json: true,
        } => {
            let mut retry = vec![
                "init".into(),
                path.to_string_lossy().into_owned(),
                "--json".into(),
            ];
            if let Some(name) = name {
                retry.extend(["--name".into(), name.clone()]);
            }
            if let Some(app_id) = app_id {
                retry.extend(["--app-id".into(), app_id.clone()]);
            }
            if let Some(local_path) = local_path {
                retry.extend([
                    "--local-path".into(),
                    local_path.to_string_lossy().into_owned(),
                ]);
            }
            Some(emit(path, Action::Init, vec![], retry, |root| {
                website::init(root, name.clone(), app_id.clone(), local_path.clone())
            }))
        }
        Command::AddTarget {
            targets,
            project_dir,
            json: true,
        } => {
            let mut retry = vec!["add-target".into()];
            retry.extend(targets.iter().map(|target| target.as_str().to_owned()));
            retry.push("--json".into());
            Some(emit(
                project_dir,
                Action::AddTarget,
                targets.clone(),
                retry,
                |root| website::add_targets(root, targets),
            ))
        }
        Command::Build {
            target,
            project_dir,
            release,
            features,
            no_default_features,
            variant,
            json: true,
        } => {
            let mut retry = vec!["build".into(), "--json".into()];
            if let Some(target) = target {
                retry.extend(["--target".into(), target.as_str().into()]);
            }
            if *release {
                retry.push("--release".into());
            }
            if *no_default_features {
                retry.push("--no-default-features".into());
            }
            for feature in features {
                retry.extend(["--features".into(), feature.clone()]);
            }
            if let Some(variant) = variant {
                retry.extend(["--variant".into(), variant.as_str().into()]);
            }
            Some(emit(
                project_dir,
                Action::Build,
                target.iter().copied().collect(),
                retry,
                |root| {
                    fission_command_run::website::build(
                        fission_command_run::BuildOptions {
                            project_dir: root.to_path_buf(),
                            target: *target,
                            release: *release,
                            variant: variant.clone(),
                        },
                        fission_command_run::WebCargoOptions {
                            features: features.clone(),
                            no_default_features: *no_default_features,
                        },
                    )
                },
            ))
        }
        Command::Site { command } => {
            let (root, action, release, name) = match command {
                SiteCommand::Build {
                    project_dir,
                    release,
                    json: true,
                } => (project_dir, Action::SiteBuild, *release, "build"),
                SiteCommand::Check {
                    project_dir,
                    release,
                    json: true,
                } => (project_dir, Action::SiteCheck, *release, "check"),
                SiteCommand::Routes {
                    project_dir,
                    json: true,
                } => (project_dir, Action::SiteRoutes, false, "routes"),
                _ => return None,
            };
            let mut retry = vec!["site".into(), name.into(), "--json".into()];
            if release {
                retry.push("--release".into());
            }
            Some(emit(root, action, vec![Target::Site], retry, |root| {
                fission_command_site::website::execute(root, release, action)
            }))
        }
        _ => None,
    }
}

fn emit(
    root: &Path,
    action: Action,
    selected_targets: Vec<Target>,
    mut retry: Vec<String>,
    run: impl FnOnce(&Path) -> website::Result<Data>,
) -> anyhow::Result<()> {
    let resolved = website::absolute(root);
    let retry_cwd = std::env::current_dir()
        .ok()
        .filter(|cwd| cwd.to_str().is_some());
    let root = match resolved {
        Ok(root) if root.to_str().is_some() && retry_cwd.is_some() => root,
        other => {
            let (project_dir, mut error) = match other {
                Ok(root) => (root.to_str().map(|_| root.clone()), Failure::new(ErrorCode::InvalidConfiguration,
                    "Structured mode requires UTF-8 project paths and an accessible UTF-8 working directory.")),
                Err(error) => (None, error),
            };
            if let Some(cwd) = retry_cwd {
                error.recovery.push(Step { instruction: "Choose an accessible UTF-8 project/working directory, then retry; inspect supported options with this invocation.".into(),
                    invocation: website::Invocation { cwd, program: "fission".into(), argv: vec!["--help".into()] } });
            }
            return write_result(CommandResult {
                schema: website::SCHEMA.into(),
                action,
                project_dir,
                selected_targets,
                state: None,
                outcome: Outcome::Failure { error },
            });
        }
    };
    let retry_cwd = retry_cwd.expect("checked working directory");
    if action == Action::Init {
        retry[1] = root.to_string_lossy().into_owned();
    } else {
        retry.extend(["--project-dir".into(), root.to_string_lossy().into_owned()]);
    }
    let result = run(&root);
    let outcome = match result {
        Ok(mut data) => {
            if data.artifact_dir.is_some() && selected_targets == [Target::Site] {
                data.next_steps.push(website::step(
                    &root,
                    "Validate all routes before consuming the artifact directory.",
                    &["site", "check", "--json"],
                ));
            }
            Outcome::Success { data }
        }
        Err(mut error) => {
            add_recovery(&root, action, &selected_targets, &mut error);
            if error.code != ErrorCode::InvalidTarget {
                error.recovery.push(Step {
                instruction: "After completing the prerequisites above, retry the operation. Existing generated configuration and source files are retained.".into(),
                invocation: website::Invocation { cwd: retry_cwd, program: "fission".into(), argv: retry },
            });
            }
            Outcome::Failure { error }
        }
    };
    let result = CommandResult {
        schema: website::SCHEMA.into(),
        action,
        project_dir: Some(root.clone()),
        selected_targets,
        state: website::state(&root),
        outcome,
    };
    write_result(result)
}

fn write_result(mut result: CommandResult) -> anyhow::Result<()> {
    // Serialize before writing so even a non-UTF-8 discovered artifact cannot
    // leave a partial object on stdout.
    let bytes = match serde_json::to_vec(&result) {
        Ok(bytes) => bytes,
        Err(_) => {
            let mut error = Failure::new(ErrorCode::ReportUnavailable, "The report contains paths that cannot be represented in JSON; use UTF-8 project/source/output paths and retry.");
            if let Some(root) = &result.project_dir {
                error.recovery.push(website::step(root, "Rename non-UTF-8 source/output paths, then repeat the requested operation; inspect its accepted options here.", &["--help"]));
            }
            result.outcome = Outcome::Failure { error };
            serde_json::to_vec(&result)?
        }
    };
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    out.write_all(&bytes)?;
    writeln!(out)?;
    if matches!(result.outcome, Outcome::Failure { .. }) {
        anyhow::bail!("website command failed; see the structured result for recovery");
    }
    Ok(())
}

fn add_recovery(root: &Path, action: Action, targets: &[Target], error: &mut Failure) {
    match error.code {
        ErrorCode::MissingToolchain => {
            let target = targets.first().copied().unwrap_or(Target::Site);
            error.recovery.push(website::step(root, "Install or select a working toolchain for this project; doctor reports required executables and supported installation suggestions.", &["doctor", target.as_str(), "--strict"]));
        }
        ErrorCode::InvalidConfiguration => {
            if action == Action::Init || action == Action::AddTarget {
                error.recovery.push(website::step(root, "Repair the indicated project configuration or select a valid dependency path. Inspect the supported setup options here before retrying.",
                    if action == Action::Init { &["init", "--help"] } else { &["add-target", "--help"] }));
                return;
            }
            error.recovery.push(website::step(root, "Repair fission.toml/Cargo.toml and referenced files at the reported project directory. Preserve generated target wiring; use add-target for target changes. Recheck the static site after repairing configuration.",
                if targets == [Target::Web] { &["build", "--target", "web", "--json"] } else { &["site", "check", "--json"] }));
        }
        ErrorCode::CompileFailed
        | ErrorCode::SiteFailed
        | ErrorCode::ReportUnavailable
        | ErrorCode::ArtifactMissing => {
            if action == Action::Init || action == Action::AddTarget {
                error.recovery.push(website::step(root, "Repair paths that are directories or inaccessible where setup expects files. Inspect the setup command before retrying.",
                    if action == Action::Init { &["init", "--help"] } else { &["add-target", "--help"] }));
                return;
            }
            error.recovery.push(website::step(root, "Repair the source/dependency/rendering problem described in the diagnostic excerpt; programmatic sites require a matching shell revision supporting --report-file.",
                if targets == [Target::Web] { &["build", "--target", "web", "--json"] } else { &["site", "check", "--json"] }));
        }
        ErrorCode::InvalidTarget => {
            // Selection requires user intent; suggest only targets already configured.
            if let Some(state) = website::state(root) {
                for target in state
                    .configured_targets
                    .into_iter()
                    .filter(|target| matches!(target, Target::Web | Target::Site))
                {
                    error.recovery.push(website::step(
                        root,
                        "Choose an intended configured website target explicitly.",
                        &["build", "--target", target.as_str(), "--json"],
                    ));
                }
            }
            if error.recovery.is_empty() {
                error.recovery.push(website::step(root, "Select an intended website target (web or static-site) with the supported target command; inspect the options here.", &["add-target", "--help"]));
            }
        }
        _ => {}
    }
}
