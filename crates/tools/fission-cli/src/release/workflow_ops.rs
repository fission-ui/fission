use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Deserialize, Default)]
struct WorkflowRootToml {
    #[serde(default)]
    release_workflows: BTreeMap<String, ReleaseWorkflowToml>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct ReleaseWorkflowToml {
    #[serde(default)]
    commands: Vec<String>,
}

#[derive(Debug, Serialize)]
struct WorkflowReceipt {
    schema_version: u32,
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
    status: String,
    exit_code: Option<i32>,
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
            println!("{name}: {} command(s)", workflow.commands.len());
            for command in workflow.commands {
                println!("  {command}");
            }
        }
    }
    Ok(())
}

pub(super) fn run(project_dir: &Path, name: &str, dry_run: bool, json: bool) -> Result<()> {
    let workflows = read_workflows(project_dir)?;
    let workflow = workflows
        .release_workflows
        .get(name)
        .with_context(|| format!("release workflow `{name}` is not configured"))?;
    if workflow.commands.is_empty() {
        bail!("release workflow `{name}` has no commands");
    }
    let exe = env::current_exe().context("failed to resolve current fission executable")?;
    let mut receipt = WorkflowReceipt {
        schema_version: 1,
        workflow: name.to_string(),
        status: "passed".to_string(),
        dry_run,
        commands: Vec::new(),
    };
    for (index, command) in workflow.commands.iter().enumerate() {
        let mut argv = split_command(command)?;
        if argv.is_empty() {
            continue;
        }
        if !has_project_dir(&argv) {
            argv.push("--project-dir".to_string());
            argv.push(project_dir.display().to_string());
        }
        if dry_run {
            receipt.commands.push(WorkflowCommandReceipt {
                index,
                command: command.clone(),
                argv,
                status: "dry-run".to_string(),
                exit_code: None,
            });
            continue;
        }
        let status = Command::new(&exe)
            .args(&argv)
            .status()
            .with_context(|| format!("failed to run release workflow command `{command}`"))?;
        let success = status.success();
        receipt.commands.push(WorkflowCommandReceipt {
            index,
            command: command.clone(),
            argv,
            status: if success { "passed" } else { "failed" }.to_string(),
            exit_code: status.code(),
        });
        if !success {
            receipt.status = "failed".to_string();
            write_workflow_receipt(project_dir, &receipt)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&receipt)?);
            }
            bail!("release workflow `{name}` failed at command {}", index + 1);
        }
    }
    write_workflow_receipt(project_dir, &receipt)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else if dry_run {
        println!(
            "Release workflow `{name}` dry run: {} command(s)",
            receipt.commands.len()
        );
        for command in &receipt.commands {
            println!("  {}", command.argv.join(" "));
        }
    } else {
        println!("Release workflow `{name}` completed");
    }
    Ok(())
}

fn read_workflows(project_dir: &Path) -> Result<WorkflowRootToml> {
    let path = project_dir.join("fission.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_workflow_receipt(project_dir: &Path, receipt: &WorkflowReceipt) -> Result<()> {
    let path = project_dir
        .join("target/fission/release-workflows")
        .join(format!("{}.json", receipt.workflow));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(receipt)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn has_project_dir(argv: &[String]) -> bool {
    argv.iter()
        .any(|arg| arg == "--project-dir" || arg.starts_with("--project-dir="))
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
}
