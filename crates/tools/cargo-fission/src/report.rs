//! Envelope construction and two renderers; this module never executes commands.
pub(crate) use fission_command_core::report::Action;
use fission_command_core::report::{
    self, CommandResult, Data, ErrorCode, Failure, Invocation, Outcome, Result, Step,
};
use fission_command_core::Target;
use std::{io::Write, path::Path};

pub(crate) fn finish(
    root: &Path,
    action: Action,
    mut selected_targets: Vec<Target>,
    outcome: Result<Data>,
    json: bool,
    retry: Vec<String>,
) -> anyhow::Result<()> {
    let project_dir = report::absolute(root).ok();
    let state = project_dir.as_deref().and_then(report::state);
    let outcome = match outcome {
        Ok(data) => {
            if !data.targets.is_empty() {
                selected_targets = data.targets.iter().map(|data| data.target).collect();
            }
            Outcome::Success { data }
        }
        Err(mut error) => {
            if let Ok(cwd) = std::env::current_dir() {
                if cwd.to_str().is_some() && root.to_str().is_some() {
                    error.recovery.push(Step {
                        instruction: "Repair the reported failure, then retry this invocation."
                            .into(),
                        invocation: Invocation {
                            cwd,
                            program: "fission".into(),
                            argv: retry,
                        },
                    });
                }
            }
            Outcome::Failure { error }
        }
    };
    render(
        CommandResult {
            schema: report::SCHEMA.into(),
            action,
            project_dir,
            selected_targets,
            state,
            outcome,
        },
        json,
    )
}

fn render(mut report: CommandResult, json: bool) -> anyhow::Result<()> {
    if json {
        let bytes = json_bytes(&mut report)?;
        let mut out = std::io::stdout().lock();
        out.write_all(&bytes)?;
        writeln!(out)?;
    } else {
        human(&report);
    }
    match report.outcome {
        Outcome::Success { .. } => Ok(()),
        Outcome::Failure { error } => Err(error.into()),
    }
}

fn json_bytes(report: &mut CommandResult) -> anyhow::Result<Vec<u8>> {
    // Any envelope path can fail serialization; discard every path-bearing field.
    match serde_json::to_vec(&report) {
        Ok(bytes) => Ok(bytes),
        Err(_) => {
            report.project_dir = None;
            report.state = None;
            report.outcome = Outcome::Failure { error: Failure::new(ErrorCode::ReportUnavailable,
                "The command outcome contains paths that cannot be represented in JSON; use human output to inspect it.") };
            Ok(serde_json::to_vec(&report)?)
        }
    }
}

fn human(report: &CommandResult) {
    let Outcome::Success { data } = &report.outcome else {
        if let Outcome::Failure { error } = &report.outcome {
            if !error.diagnostics.is_empty() {
                eprintln!("{}", error.diagnostics);
            }
            for step in &error.recovery {
                eprintln!("{}", step.instruction);
                eprintln!(
                    "{} {:?} (cwd {})",
                    step.invocation.program,
                    step.invocation.argv,
                    step.invocation.cwd.display()
                );
            }
        }
        return;
    };
    match report.action {
        Action::Init => {
            println!(
                "Initialized project at {}",
                report
                    .project_dir
                    .as_ref()
                    .expect("successful root")
                    .display()
            );
            for path in &data.instructions {
                println!("Read {}", path.display());
            }
        }
        Action::AddTarget => println!("Configured {} target(s)", report.selected_targets.len()),
        Action::SiteBuild | Action::Build => {
            for target in &data.targets {
                if target.target == Target::Site {
                    println!(
                        "Built {} static route(s) into {}",
                        target.routes.len(),
                        target
                            .artifact_dir
                            .as_ref()
                            .expect("site build directory")
                            .display()
                    );
                    for route in &target.routes {
                        println!("{} -> {}", route.path, route.output.display());
                    }
                } else {
                    println!("Built {}", target.target.as_str());
                    for path in &target.artifacts {
                        println!("{}", path.display());
                    }
                }
            }
        }
        Action::SiteCheck => {
            for target in &data.targets {
                println!(
                    "Checked {} static route(s); output would be {}",
                    target.routes.len(),
                    target
                        .planned_output_dir
                        .as_ref()
                        .expect("site planned path")
                        .display()
                );
            }
        }
        Action::SiteRoutes => {
            for target in &data.targets {
                for route in &target.routes {
                    println!(
                        "{}  {}  {}",
                        route.path,
                        route.title,
                        route.source.display()
                    );
                }
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn serialization_failure_removes_unrepresentable_project_state_and_data_paths() {
        let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(b"/bad-\xff".to_vec()));
        let mut report = CommandResult {
            schema: report::SCHEMA.into(),
            action: Action::Init,
            project_dir: Some(path.clone()),
            selected_targets: vec![],
            state: Some(report::ProjectState {
                name: "app".into(),
                config_path: path.join("fission.toml"),
                configured_targets: vec![],
            }),
            outcome: Outcome::Success {
                data: Data {
                    artifacts: vec![path],
                    ..Data::default()
                },
            },
        };
        let bytes = json_bytes(&mut report).unwrap();
        let decoded: CommandResult = serde_json::from_slice(&bytes).unwrap();
        assert!(decoded.project_dir.is_none() && decoded.state.is_none());
        assert!(
            matches!(decoded.outcome, Outcome::Failure { error } if error.code == ErrorCode::ReportUnavailable)
        );
    }
}
