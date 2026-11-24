use anyhow::{bail, Context, Result};
use clap::Parser;
use std::{ffi::OsString, path::Path, thread};

mod cli;
mod serving;

#[cfg(test)]
use fission_command_core::{read_project_config, Target};

use cli::{Cli, Command, ServerCommand, SiteCommand};

const CLI_THREAD_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Runs the CLI dispatcher on a thread with a consistent cross-platform stack.
///
/// Some platform ABIs, notably Windows on ARM64, provide a smaller main-thread
/// stack than the command graph needs while parsing and dispatching packaging
/// operations. The reserve is virtual address space and is committed on demand.
pub fn run_from_env() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    let worker = thread::Builder::new()
        .name("fission-cli".into())
        .stack_size(CLI_THREAD_STACK_BYTES)
        .spawn(move || run(args))
        .context("failed to start the Fission CLI worker")?;

    worker
        .join()
        .map_err(|_| anyhow::anyhow!("the Fission CLI worker terminated unexpectedly"))?
}

pub fn run<I, S>(args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let mut argv: Vec<OsString> = args.into_iter().map(Into::into).collect();
    if let Some(bin) = argv.first() {
        if let Some(name) = Path::new(bin).file_name().and_then(|value| value.to_str()) {
            if name == "cargo-fission" {
                argv[0] = OsString::from("fission");
                if argv.get(1).and_then(|value| value.to_str()) == Some("fission") {
                    argv.remove(1);
                }
            }
        }
    }
    let cli = Cli::parse_from(argv.clone());
    warn_for_alpha_features(&cli.command);
    match cli.command {
        Command::TestVisualCase {
            url,
            mount_url,
            target,
            width,
            height,
            timeout_ms,
            screenshot,
            report,
        } => {
            fission_command_run::review::worker(fission_command_run::review::ReviewWorkerOptions {
                url,
                mount: mount_url,
                target,
                viewport: fission_command_run::review::Viewport { width, height },
                timeout_ms,
                screenshot,
                report,
            })
        }
        Command::Features => {
            print_feature_catalog();
            Ok(())
        }
        Command::Init {
            path,
            name,
            app_id,
            local_path,
            template,
        } => {
            fission_command_core::init_project_with_template(
                &path, name, app_id, local_path, template,
            )?;
            if template == Some(fission_command_core::ProjectTemplate::Website) {
                println!("Website ready. Read WEBSITE.md for checks, serving, routing, and GitHub Pages preparation. No website has been published.");
            }
            println!("Immediately read generated AGENTS.md (or AGENTS.fission.md beside user instructions) and its linked guidance before editing. Keep fission.toml; change targets with fission add-target.");
            Ok(())
        }
        Command::AddTarget {
            targets,
            project_dir,
        } => fission_command_core::add_targets(&project_dir, &targets),
        Command::AddCapability {
            capabilities,
            project_dir,
        } => fission_command_core::add_capabilities(&project_dir, &capabilities),
        Command::Doctor {
            targets,
            project_dir,
            strict,
        } => fission_command_run::doctor::run_doctor(&project_dir, &targets, strict),
        Command::Devices { project_dir, json } => {
            fission_command_run::list_devices(&project_dir, json)
        }
        Command::Run {
            target,
            device,
            project_dir,
            detach,
            release,
            features,
            no_default_features,
            variant,
            host,
            port,
            no_open,
            headless,
            serving,
        } => serving::execute("run", &project_dir.clone(), serving.json, &argv, |notify| {
            if serving.json
                && !matches!(
                    target,
                    Some(fission_command_core::Target::Web | fission_command_core::Target::Site)
                )
            {
                bail!(
                    "attached serving JSON currently requires --target web or --target static-site"
                );
            }
            fission_command_run::run_app_with_serving_options(
                fission_command_run::RunOptions {
                    project_dir,
                    target,
                    device,
                    detach,
                    release,
                    variant,
                    host,
                    port,
                    no_open,
                    headless,
                },
                fission_command_run::WebCargoOptions {
                    features,
                    no_default_features,
                },
                serving.options(),
                notify,
            )
        }),
        Command::Build {
            target,
            project_dir,
            release,
            features,
            no_default_features,
            variant,
        } => fission_command_run::build_app_with_web_cargo_options(
            fission_command_run::BuildOptions {
                project_dir,
                target,
                release,
                variant,
            },
            fission_command_run::WebCargoOptions {
                features,
                no_default_features,
            },
        ),
        Command::Test {
            target,
            project_dir,
            headless: _,
            features,
            no_default_features,
            variant: _,
            visual,
        } if visual.visual_review => {
            fission_command_run::review::run(fission_command_run::review::ReviewOptions {
                project_dir,
                target: target.context("visual review requires --target web or static-site")?,
                output_dir: visual
                    .output_dir
                    .context("visual review requires --output-dir")?,
                routes: visual.routes,
                viewports: visual.viewports,
                mount: visual.mount,
                port: visual.port,
                release: visual.release,
                web_cargo: fission_command_run::WebCargoOptions {
                    features,
                    no_default_features,
                },
                startup_timeout: std::time::Duration::from_secs(visual.startup_timeout_seconds),
                case_timeout: std::time::Duration::from_secs(visual.case_timeout_seconds),
                json: visual.json,
                strict: visual.strict,
            })
        }
        Command::Test {
            target,
            project_dir,
            headless,
            features,
            no_default_features,
            variant,
            ..
        } => fission_command_run::test_app_with_web_cargo_options(
            fission_command_run::TestOptions {
                project_dir,
                target,
                headless,
                variant,
            },
            fission_command_run::WebCargoOptions {
                features,
                no_default_features,
            },
        ),
        Command::Site { command } => match command {
            SiteCommand::Build {
                project_dir,
                release,
            } => fission_command_site::build(&project_dir, release),
            SiteCommand::Check {
                project_dir,
                release,
            } => fission_command_site::check(&project_dir, release),
            SiteCommand::Serve {
                project_dir,
                host,
                port,
                release,
                no_open,
                serving,
            } => serving::execute("site serve", &project_dir, serving.json, &argv, |notify| {
                fission_command_site::serve_with_options(
                    &project_dir,
                    release,
                    host,
                    port,
                    !no_open,
                    serving.options(),
                    notify,
                )
            }),
            SiteCommand::Routes { project_dir } => fission_command_site::routes(&project_dir),
        },
        Command::Server { command } => match command {
            ServerCommand::Build {
                project_dir,
                release,
            } => fission_command_server::build(&project_dir, release),
            ServerCommand::Check {
                project_dir,
                release,
            } => fission_command_server::check(&project_dir, release),
            ServerCommand::Serve {
                project_dir,
                host,
                port,
                release,
            } => fission_command_server::serve(&project_dir, release, host, port),
            ServerCommand::Routes { project_dir } => fission_command_server::routes(&project_dir),
            ServerCommand::Artifacts {
                project_dir,
                release,
                no_compile,
            } => fission_command_server::artifacts(&project_dir, release, !no_compile),
        },
        Command::Package {
            target,
            format,
            project_dir,
            release,
            variant,
            json,
        } => fission_command_package::package(fission_command_package::PackageOptions {
            project_dir,
            target,
            format,
            release,
            variant,
            json,
        }),
        Command::Distribute {
            action,
            provider,
            artifact,
            site,
            deploy,
            track,
            locales,
            dry_run,
            yes,
            project_dir,
            json,
        } => {
            let action = action.unwrap_or(fission_command_package::DistributeAction::Publish);
            if action == fission_command_package::DistributeAction::Publish {
                fission_command_release::publish_workflow(
                    fission_command_release::PublishWorkflowOptions {
                        project_dir,
                        provider,
                        target: None,
                        format: None,
                        artifact,
                        site,
                        deploy,
                        track,
                        locales,
                        overwrite_remote: false,
                        dry_run,
                        yes,
                        json,
                    },
                )
            } else {
                fission_command_package::distribute(fission_command_package::DistributeOptions {
                    project_dir,
                    provider,
                    action,
                    target: None,
                    format: None,
                    artifact,
                    site,
                    deploy,
                    track,
                    locales,
                    dry_run,
                    yes,
                    json,
                })
            }
        }
        Command::Publish {
            provider,
            target,
            format,
            artifact,
            site,
            deploy,
            track,
            locales,
            app,
            guided,
            dry_run,
            overwrite_remote,
            yes,
            project_dir,
            json,
        } => {
            if app {
                if guided || dry_run || yes || json || overwrite_remote {
                    bail!(
                        "publish --app opens the interactive windowed publish app; remove --app for --guided/--dry-run/--yes/--json/--overwrite-remote automation"
                    );
                }
                fission_command_ui::run_publish_window(fission_command_ui::PublishUiOptions {
                    project_dir,
                    provider,
                    target,
                    format,
                    artifact,
                    site,
                    deploy,
                    track,
                    locales,
                    screenshot: None,
                    exit_after_render: false,
                    width: None,
                    height: None,
                    native_file_dialog: true,
                })
            } else if artifact.is_some()
                && target.is_none()
                && format.is_none()
                && locales.is_empty()
                && !guided
            {
                fission_command_release::publish_workflow(
                    fission_command_release::PublishWorkflowOptions {
                        project_dir,
                        provider,
                        target,
                        format,
                        artifact,
                        site,
                        deploy,
                        track,
                        locales,
                        overwrite_remote,
                        dry_run,
                        yes,
                        json,
                    },
                )
            } else if !guided && (dry_run || yes || json) {
                fission_command_release::publish_workflow(
                    fission_command_release::PublishWorkflowOptions {
                        project_dir,
                        provider,
                        target,
                        format,
                        artifact,
                        site,
                        deploy,
                        track,
                        locales,
                        overwrite_remote,
                        dry_run,
                        yes,
                        json,
                    },
                )
            } else if guided || dry_run || yes || json {
                fission_command_package::run_publish_shell_with_hooks(
                    fission_command_package::PublishShellOptions {
                        project_dir,
                        provider,
                        target,
                        format,
                        artifact,
                        site,
                        deploy,
                        track,
                        locales,
                        overwrite_remote,
                        dry_run,
                        yes,
                        json,
                        app,
                    },
                    publish_shell_hooks(),
                )
            } else {
                fission_command_ui::run_publish_tui(fission_command_ui::PublishUiOptions {
                    project_dir,
                    provider,
                    target,
                    format,
                    artifact,
                    site,
                    deploy,
                    track,
                    locales,
                    screenshot: None,
                    exit_after_render: false,
                    width: None,
                    height: None,
                    native_file_dialog: false,
                })
            }
        }
        Command::Readiness {
            kind,
            target,
            format,
            release,
            provider,
            artifact,
            site,
            track,
            locales,
            project_dir,
            json,
        } => {
            if kind == fission_command_package::ReadinessKind::Release {
                let Some(provider) = provider else {
                    bail!("readiness release requires --provider");
                };
                fission_command_release::readiness_release(
                    fission_command_release::PublishWorkflowOptions {
                        project_dir,
                        provider,
                        target,
                        format,
                        artifact,
                        site,
                        deploy: None,
                        track,
                        locales,
                        overwrite_remote: false,
                        dry_run: true,
                        yes: true,
                        json,
                    },
                )
            } else {
                fission_command_package::readiness(fission_command_package::ReadinessOptions {
                    project_dir,
                    kind,
                    target,
                    format,
                    provider,
                    artifact,
                    site,
                    track,
                    release,
                    json,
                })
            }
        }
        Command::ReleaseConfig { command } => match command {
            fission_command_release::ReleaseConfigCommand::Edit {
                project_dir,
                tui: true,
                provider,
            } => fission_command_ui::run_release_config_tui(
                fission_command_ui::ReleaseConfigEditorOptions {
                    project_dir,
                    provider,
                    screenshot: None,
                    exit_after_render: false,
                    width: None,
                    height: None,
                },
            ),
            command => fission_command_release::release_config(command),
        },
        Command::ReleaseContent { command } => fission_command_release::release_content(command),
        Command::Beta { command } => fission_command_release::beta(command),
        Command::Signing { command } => fission_command_release::signing(command),
        Command::Reviews { command } => fission_command_release::reviews(command),
        Command::ReleaseWorkflow { command } => fission_command_release::release_workflow(command),
        Command::Auth { command } => fission_command_release::auth(command),
        Command::Logs {
            target,
            device,
            project_dir,
            follow,
        } => fission_command_run::attach_logs(fission_command_run::LogOptions {
            project_dir,
            target,
            device,
            follow,
        }),
        Command::Ui {
            provider,
            target,
            format,
            track,
            locales,
            project_dir,
            screenshot,
            exit_after_render,
            width,
            height,
        } => fission_command_ui::run_publish_tui(fission_command_ui::PublishUiOptions {
            project_dir,
            provider,
            target,
            format,
            artifact: None,
            site: "production".to_string(),
            deploy: None,
            track,
            locales,
            screenshot,
            exit_after_render,
            width,
            height,
            native_file_dialog: false,
        }),
        Command::ServeWeb {
            project_dir,
            host,
            port,
            open,
        } => fission_command_run::serve_web(fission_command_run::ServeWebOptions {
            project_dir,
            host,
            port,
            open,
        }),
    }
}

const ALPHA_FEATURES: &[(&str, &str)] = &[
    ("assets", "typed asset provenance and licence metadata"),
    ("game", "deterministic fixed-step game runtime"),
    ("scene", "shared renderer-neutral scene contracts"),
    ("scene2d", "retained interactive 2D scenes"),
    ("scene3d", "retained interactive 3D scenes"),
    ("physics", "provider-neutral 2D and 3D physics contracts"),
    ("physics-rapier2d", "optional Rapier 2D physics provider"),
    ("physics-rapier3d", "optional Rapier 3D physics provider"),
];

fn print_feature_catalog() {
    println!("Fission alpha features");
    println!();
    for (feature, summary) in ALPHA_FEATURES {
        println!("{feature:<18} alpha  {summary}");
    }
    println!();
    println!("Alpha APIs are usable but may change between alpha releases.");
}

fn warn_for_alpha_features(command: &Command) {
    let features = match command {
        Command::Run { features, .. }
        | Command::Build { features, .. }
        | Command::Test { features, .. } => features,
        _ => return,
    };
    let selected = ALPHA_FEATURES
        .iter()
        .filter_map(|(feature, _)| {
            features
                .iter()
                .any(|value| value == feature)
                .then_some(*feature)
        })
        .collect::<Vec<_>>();
    if !selected.is_empty() {
        eprintln!(
            "note: {} use alpha APIs that may change between alpha releases",
            selected.join(", ")
        );
    }
}

fn publish_shell_hooks() -> fission_command_package::PublishShellHooks {
    fission_command_package::PublishShellHooks {
        release_plan: publish_shell_release_plan,
        release_checks: publish_shell_release_checks,
        publish: publish_shell_publish,
        skip_requirement: publish_shell_skip_requirement,
        bump_build: publish_shell_bump_build,
    }
}

fn publish_shell_release_plan(
    request: fission_command_package::PublishShellWorkflowRequest,
) -> Result<fission_command_package::PublishShellReleasePlan> {
    let plan = fission_command_release::release_plan_snapshot(publish_workflow_options(request))?;
    Ok(fission_command_package::PublishShellReleasePlan {
        status: plan.status,
        provider: plan.provider,
        target: plan.target,
        format: plan.format,
        track: plan.track,
        locales: plan.locales,
        steps: plan
            .steps
            .into_iter()
            .map(|step| fission_command_package::PublishShellReleaseStep {
                id: step.id,
                title: step.title,
                status: step.status,
            })
            .collect(),
        capabilities: plan
            .capabilities
            .into_iter()
            .map(
                |capability| fission_command_package::PublishShellProviderCapability {
                    id: capability.id,
                    status: capability.status,
                    summary: capability.summary,
                },
            )
            .collect(),
        requirements: plan
            .requirements
            .into_iter()
            .map(publish_shell_requirement_check)
            .collect(),
    })
}

fn publish_shell_release_checks(
    request: fission_command_package::PublishShellWorkflowRequest,
) -> Result<Vec<fission_command_package::ReadinessCheck>> {
    fission_command_release::release_readiness_checks(publish_workflow_options(request))
}

fn publish_shell_publish(
    request: fission_command_package::PublishShellWorkflowRequest,
) -> Result<String> {
    let dry_run = request.dry_run;
    fission_command_release::publish_workflow(publish_workflow_options(request))?;
    Ok(if dry_run {
        "publish dry-run completed".to_string()
    } else {
        "publish workflow completed".to_string()
    })
}

fn publish_shell_skip_requirement(
    request: fission_command_package::PublishShellWorkflowRequest,
    id: String,
) -> Result<String> {
    fission_command_release::skip_release_requirement(&request.project_dir, &id, true)?;
    Ok(format!("recorded explicit skip for {id}"))
}

fn publish_shell_bump_build(
    request: fission_command_package::PublishShellWorkflowRequest,
) -> Result<String> {
    fission_command_release::bump_release_build(&request.project_dir, request.target, 1, true)?;
    Ok("bumped release build".to_string())
}

fn publish_workflow_options(
    request: fission_command_package::PublishShellWorkflowRequest,
) -> fission_command_release::PublishWorkflowOptions {
    fission_command_release::PublishWorkflowOptions {
        project_dir: request.project_dir,
        provider: request.provider,
        target: request.target,
        format: request.format,
        artifact: request.artifact,
        site: request.site,
        deploy: request.deploy,
        track: request.track,
        locales: request.locales,
        overwrite_remote: request.overwrite_remote,
        dry_run: request.dry_run,
        yes: request.yes,
        json: request.json,
    }
}

fn publish_shell_requirement_check(
    requirement: fission_command_release::ReleaseRequirementSnapshot,
) -> fission_command_package::ReadinessCheck {
    fission_command_package::ReadinessCheck {
        id: requirement.id,
        severity: match requirement.level.as_str() {
            "provider-required" => fission_command_package::CheckSeverity::Error,
            "fission-recommended" => fission_command_package::CheckSeverity::Warning,
            _ => fission_command_package::CheckSeverity::Info,
        },
        status: match requirement.status.as_str() {
            "passed" => fission_command_package::CheckStatus::Passed,
            "missing" => fission_command_package::CheckStatus::Missing,
            "failed" => fission_command_package::CheckStatus::Failed,
            "warning" => fission_command_package::CheckStatus::Warning,
            "skipped" => fission_command_package::CheckStatus::Skipped,
            _ => fission_command_package::CheckStatus::Warning,
        },
        summary: requirement.summary,
        details: requirement.details,
        remediation: requirement.remediation,
    }
}

#[cfg(test)]
mod tests;
