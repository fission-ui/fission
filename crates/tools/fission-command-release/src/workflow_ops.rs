use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize, Default)]
struct WorkflowRootToml {
    #[serde(default)]
    release_workflows: BTreeMap<String, ReleaseWorkflowToml>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct ReleaseWorkflowToml {
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    steps: Vec<ReleaseWorkflowStepConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct ReleaseWorkflowStepToml {
    command: String,
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    readiness: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum ReleaseWorkflowStepConfig {
    Command(String),
    Detailed(ReleaseWorkflowStepToml),
}

#[derive(Debug, Serialize)]
struct WorkflowReceipt {
    schema_version: u32,
    created_at_unix_seconds: u64,
    workflow: String,
    status: String,
    dry_run: bool,
    commands: Vec<WorkflowCommandReceipt>,
}

#[derive(Debug, Serialize)]
struct WorkflowCommandReceipt {
    index: usize,
    command: String,
    argv: Vec<String>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    readiness: Vec<WorkflowReadinessReceipt>,
    status: String,
    exit_code: Option<i32>,
}

#[derive(Debug, Serialize)]
struct WorkflowReadinessReceipt {
    command: String,
    argv: Vec<String>,
    status: String,
    exit_code: Option<i32>,
}

struct WorkflowStepPlan {
    index: usize,
    step: ReleaseWorkflowStepToml,
    argv: Vec<String>,
    readiness: Vec<WorkflowGatePlan>,
}

struct WorkflowGatePlan {
    command: String,
    argv: Vec<String>,
}

pub(super) fn list(project_dir: &Path, json: bool) -> Result<()> {
    let workflows = read_workflows(project_dir)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&workflows.release_workflows)?
        );
    } else if workflows.release_workflows.is_empty() {
        println!("No [release_workflows.<name>] entries configured");
    } else {
        for (name, workflow) in workflows.release_workflows {
            let steps = workflow_steps(&workflow)?;
            println!("{name}: {} step(s)", steps.len());
            for step in steps {
                println!("  {}", step.command);
                for gate in step.readiness {
                    println!("    readiness: {gate}");
                }
            }
        }
    }
    Ok(())
}

pub(super) fn run(
    project_dir: &Path,
    name: &str,
    dry_run: bool,
    yes: bool,
    json: bool,
) -> Result<()> {
    if !dry_run && !yes {
        bail!("release-workflow run can execute mutating Fission commands; pass --yes after reviewing the dry run");
    }
    let workflows = read_workflows(project_dir)?;
    let workflow = workflows
        .release_workflows
        .get(name)
        .with_context(|| format!("release workflow `{name}` is not configured"))?;
    let steps = workflow_steps(workflow)?;
    if steps.is_empty() {
        bail!("release workflow `{name}` has no steps");
    }
    let step_plans = workflow_step_plans(&steps, project_dir)?;
    let exe = env::current_exe().context("failed to resolve current fission executable")?;
    let mut receipt = WorkflowReceipt {
        schema_version: 1,
        created_at_unix_seconds: now_unix_seconds(),
        workflow: name.to_string(),
        status: "passed".to_string(),
        dry_run,
        commands: Vec::new(),
    };
    for plan in &step_plans {
        if dry_run {
            receipt.commands.push(WorkflowCommandReceipt {
                index: plan.index,
                command: plan.step.command.clone(),
                argv: plan.argv.clone(),
                inputs: plan.step.inputs.clone(),
                outputs: plan.step.outputs.clone(),
                readiness: plan
                    .readiness
                    .iter()
                    .map(|gate| WorkflowReadinessReceipt {
                        command: gate.command.clone(),
                        argv: gate.argv.clone(),
                        status: "dry-run".to_string(),
                        exit_code: None,
                    })
                    .collect(),
                status: "dry-run".to_string(),
                exit_code: None,
            });
            continue;
        }
        let mut readiness = Vec::new();
        for gate in &plan.readiness {
            let status = Command::new(&exe)
                .args(&gate.argv)
                .status()
                .with_context(|| {
                    format!(
                        "failed to run release workflow readiness `{}`",
                        gate.command
                    )
                })?;
            let success = status.success();
            readiness.push(WorkflowReadinessReceipt {
                command: gate.command.clone(),
                argv: gate.argv.clone(),
                status: if success { "passed" } else { "failed" }.to_string(),
                exit_code: status.code(),
            });
            if !success {
                receipt.status = "failed".to_string();
                receipt.commands.push(WorkflowCommandReceipt {
                    index: plan.index,
                    command: plan.step.command.clone(),
                    argv: plan.argv.clone(),
                    inputs: plan.step.inputs.clone(),
                    outputs: plan.step.outputs.clone(),
                    readiness,
                    status: "blocked".to_string(),
                    exit_code: None,
                });
                let receipt_path = write_workflow_receipt(project_dir, &receipt)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&receipt)?);
                }
                bail!(
                    "release workflow `{name}` failed readiness before command {}; workflow receipt: {}",
                    plan.index + 1,
                    receipt_path.display()
                );
            }
        }
        let status = Command::new(&exe)
            .args(&plan.argv)
            .status()
            .with_context(|| {
                format!(
                    "failed to run release workflow command `{}`",
                    plan.step.command
                )
            })?;
        let success = status.success();
        receipt.commands.push(WorkflowCommandReceipt {
            index: plan.index,
            command: plan.step.command.clone(),
            argv: plan.argv.clone(),
            inputs: plan.step.inputs.clone(),
            outputs: plan.step.outputs.clone(),
            readiness,
            status: if success { "passed" } else { "failed" }.to_string(),
            exit_code: status.code(),
        });
        if !success {
            receipt.status = "failed".to_string();
            let receipt_path = write_workflow_receipt(project_dir, &receipt)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&receipt)?);
            }
            bail!(
                "release workflow `{name}` failed at command {}; workflow receipt: {}",
                plan.index + 1,
                receipt_path.display()
            );
        }
    }
    let receipt_path = write_workflow_receipt(project_dir, &receipt)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else if dry_run {
        println!(
            "Release workflow `{name}` dry run: {} command(s)",
            receipt.commands.len()
        );
        for command in &receipt.commands {
            println!("  {}", command.argv.join(" "));
            for gate in &command.readiness {
                println!("    readiness: {}", gate.argv.join(" "));
            }
        }
    } else {
        println!("Release workflow `{name}` completed");
        println!("Workflow receipt: {}", receipt_path.display());
    }
    Ok(())
}

fn read_workflows(project_dir: &Path) -> Result<WorkflowRootToml> {
    let path = project_dir.join("fission.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_workflow_receipt(
    project_dir: &Path,
    receipt: &WorkflowReceipt,
) -> Result<std::path::PathBuf> {
    let dir = project_dir
        .join("target/fission/release-workflows")
        .join(&receipt.workflow);
    fs::create_dir_all(&dir)?;
    let path =
        unique_receipt_path(dir.join(format!("workflow-{}.json", receipt.created_at_unix_seconds)));
    fs::write(&path, serde_json::to_vec_pretty(receipt)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn unique_receipt_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("receipt");
    let extension = path.extension().and_then(|value| value.to_str());
    for index in 2.. {
        let file_name = match extension {
            Some(extension) => format!("{stem}-{index}.{extension}"),
            None => format!("{stem}-{index}"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("unbounded receipt path search should always return")
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn workflow_steps(workflow: &ReleaseWorkflowToml) -> Result<Vec<ReleaseWorkflowStepToml>> {
    let mut steps = workflow
        .commands
        .iter()
        .map(|command| ReleaseWorkflowStepToml {
            command: command.clone(),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    steps.extend(workflow.steps.iter().cloned().map(|step| match step {
        ReleaseWorkflowStepConfig::Command(command) => ReleaseWorkflowStepToml {
            command,
            ..Default::default()
        },
        ReleaseWorkflowStepConfig::Detailed(step) => step,
    }));
    if let Some(step) = steps.iter().find(|step| step.command.trim().is_empty()) {
        bail!("release workflow step command cannot be empty: {step:?}");
    }
    Ok(steps)
}

fn workflow_step_plans(
    steps: &[ReleaseWorkflowStepToml],
    project_dir: &Path,
) -> Result<Vec<WorkflowStepPlan>> {
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let argv = workflow_command_argv(&step.command, project_dir)?;
            let readiness = step
                .readiness
                .iter()
                .map(|command| {
                    workflow_command_argv(command, project_dir).map(|argv| WorkflowGatePlan {
                        command: command.clone(),
                        argv,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(WorkflowStepPlan {
                index,
                step: step.clone(),
                argv,
                readiness,
            })
        })
        .collect()
}

fn workflow_command_argv(command: &str, project_dir: &Path) -> Result<Vec<String>> {
    let mut argv = split_command(command)?;
    normalize_workflow_argv(&mut argv);
    validate_fission_workflow_command(command, &argv)?;
    if !has_project_dir(&argv) {
        argv.push("--project-dir".to_string());
        argv.push(project_dir.display().to_string());
    }
    Ok(argv)
}

fn validate_fission_workflow_command(command: &str, argv: &[String]) -> Result<()> {
    let Some(first) = argv.first() else {
        bail!("release workflow command cannot be empty");
    };
    if is_supported_fission_command(first) {
        return Ok(());
    }
    bail!(
        "release workflow command `{command}` is not a Fission command; workflows may only run declarative `fission ...` commands, not shell scripts or arbitrary programs"
    )
}

fn is_supported_fission_command(command: &str) -> bool {
    matches!(
        command,
        "init"
            | "add-target"
            | "add-capability"
            | "doctor"
            | "devices"
            | "run"
            | "build"
            | "test"
            | "site"
            | "server"
            | "package"
            | "distribute"
            | "publish"
            | "readiness"
            | "release-config"
            | "release-content"
            | "beta"
            | "signing"
            | "reviews"
            | "release-workflow"
            | "auth"
            | "logs"
            | "ui"
            | "serve-web"
    )
}

fn has_project_dir(argv: &[String]) -> bool {
    argv.iter()
        .any(|arg| arg == "--project-dir" || arg.starts_with("--project-dir="))
}

fn normalize_workflow_argv(argv: &mut Vec<String>) {
    if argv.len() >= 2 && argv[0] == "cargo" && argv[1] == "fission" {
        argv.drain(0..2);
        return;
    }
    let Some(first) = argv.first() else {
        return;
    };
    let stem = Path::new(first)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(first);
    if matches!(stem, "fission" | "cargo-fission") {
        argv.remove(0);
    }
}

fn split_command(command: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote = None;
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => quote = Some(ch),
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            (None, c) => current.push(c),
        }
    }
    if let Some(q) = quote {
        bail!("unterminated {q} quote in workflow command `{command}`");
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fission-workflow-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn workflow_receipt_path(dir: &Path, name: &str) -> std::path::PathBuf {
        fs::read_dir(dir.join("target/fission/release-workflows").join(name))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("workflow-") && name.ends_with(".json"))
            })
            .unwrap()
    }

    #[test]
    fn split_command_keeps_quoted_values() {
        let args =
            split_command("release-config push --provider play-store --locales 'en-US,fr-FR'")
                .unwrap();
        assert_eq!(
            args,
            vec![
                "release-config",
                "push",
                "--provider",
                "play-store",
                "--locales",
                "en-US,fr-FR"
            ]
        );
    }

    #[test]
    fn project_dir_detection_accepts_split_or_equals() {
        assert!(has_project_dir(&[
            "--project-dir".to_string(),
            ".".to_string()
        ]));
        assert!(has_project_dir(&["--project-dir=.".to_string()]));
        assert!(!has_project_dir(&["release-config".to_string()]));
    }

    #[test]
    fn workflow_commands_accept_fission_prefixes() {
        let mut direct = split_command("fission package --target web").unwrap();
        normalize_workflow_argv(&mut direct);
        assert_eq!(direct, vec!["package", "--target", "web"]);

        let mut cargo = split_command("cargo fission package --target web").unwrap();
        normalize_workflow_argv(&mut cargo);
        assert_eq!(cargo, vec!["package", "--target", "web"]);
    }

    #[test]
    fn workflow_rejects_non_fission_commands() {
        let dir = unique_dir("reject-shell");
        let err = workflow_command_argv("sh -c 'echo no'", &dir).unwrap_err();
        assert!(err
            .to_string()
            .contains("workflows may only run declarative `fission ...` commands"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn workflow_run_rejects_all_non_fission_steps_before_execution() {
        let dir = unique_dir("prevalidate-all");
        fs::write(
            dir.join("fission.toml"),
            r#"[release_workflows.beta]
commands = [
  "fission readiness package --target static-site --format static --json",
  "sh -c 'echo should-not-run'"
]
"#,
        )
        .unwrap();

        let err = run(&dir, "beta", false, true, true).unwrap_err();

        assert!(err
            .to_string()
            .contains("workflows may only run declarative `fission ...` commands"));
        assert!(!dir.join("target/fission/release-workflows/beta").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn workflow_run_requires_yes_before_mutating_execution() {
        let dir = unique_dir("requires-yes");
        let err = run(&dir, "beta", false, false, false).unwrap_err();
        assert!(err.to_string().contains("pass --yes"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn workflow_steps_can_declare_inputs_outputs_and_readiness_gates() {
        let workflow: ReleaseWorkflowToml = toml::from_str(
            r#"
commands = ["fission release-config validate"]

[[steps]]
command = "fission publish --provider play-store --dry-run --yes"
inputs = ["fission.toml", "release-content/metadata"]
outputs = ["target/fission/distribution/play-store"]
readiness = ["fission readiness release --provider play-store --target android --format aab --json"]
"#,
        )
        .unwrap();

        let steps = workflow_steps(&workflow).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].command, "fission release-config validate");
        assert_eq!(
            steps[1].inputs,
            vec!["fission.toml", "release-content/metadata"]
        );
        assert_eq!(
            steps[1].readiness,
            vec!["fission readiness release --provider play-store --target android --format aab --json"]
        );
    }

    #[test]
    fn workflow_steps_accept_string_array_syntax() {
        let workflow: ReleaseWorkflowToml = toml::from_str(
            r#"
steps = [
  "fission readiness release --provider play-store --target android --format aab --json",
  "fission package --target android --format aab --release"
]
"#,
        )
        .unwrap();

        let steps = workflow_steps(&workflow).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(
            steps[0].command,
            "fission readiness release --provider play-store --target android --format aab --json"
        );
    }

    #[test]
    fn workflow_dry_run_receipt_includes_step_metadata_and_gates() {
        let dir = unique_dir("dry-run-steps");
        fs::write(
            dir.join("fission.toml"),
            r#"[release_workflows.beta]

[[release_workflows.beta.steps]]
command = "fission release-config validate"
inputs = ["fission.toml"]
outputs = ["target/fission/release-workflows/beta.json"]
readiness = ["fission readiness release --provider play-store --target android --format aab --json"]
"#,
        )
        .unwrap();

        run(&dir, "beta", true, false, true).unwrap();

        let receipt_path = workflow_receipt_path(&dir, "beta");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();
        assert!(value
            .get("created_at_unix_seconds")
            .and_then(serde_json::Value::as_u64)
            .is_some());
        let command = &value["commands"][0];
        assert_eq!(command["status"], "dry-run");
        assert_eq!(command["inputs"][0], "fission.toml");
        assert_eq!(
            command["outputs"][0],
            "target/fission/release-workflows/beta.json"
        );
        assert_eq!(command["readiness"][0]["status"], "dry-run");
        assert_eq!(command["readiness"][0]["argv"][0], "readiness");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn workflow_receipts_do_not_overwrite_repeated_runs() {
        let dir = unique_dir("unique-receipts");
        fs::write(
            dir.join("fission.toml"),
            r#"[release_workflows.beta]
commands = ["fission release-config validate"]
"#,
        )
        .unwrap();

        run(&dir, "beta", true, false, true).unwrap();
        run(&dir, "beta", true, false, true).unwrap();

        let receipt_count = fs::read_dir(dir.join("target/fission/release-workflows/beta"))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("workflow-") && name.ends_with(".json"))
            })
            .count();
        assert_eq!(receipt_count, 2);

        let _ = fs::remove_dir_all(&dir);
    }
}
