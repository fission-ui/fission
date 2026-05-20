use super::state::UiState;
use crate::Target;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum UiCommand {
    InitProject,
    AddTarget(Target),
    DoctorAll,
    DoctorTarget(Target),
    Refresh,
    RunSelected,
    BuildSelected,
    TestSelected,
    SiteBuild,
    SiteCheck,
    SiteRoutes,
    SiteServe,
    LogsSnapshot,
    LogsFollow,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CommandRecord {
    pub(crate) title: String,
    pub(crate) status: CommandStatus,
    pub(crate) output: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CommandStatus {
    #[default]
    Ready,
    Running,
    Ok,
    Failed,
    Started,
}

impl CommandStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Running => "Running",
            Self::Ok => "OK",
            Self::Failed => "Failed",
            Self::Started => "Started",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CommandSnapshot {
    pub(crate) generation: u64,
    pub(crate) record: CommandRecord,
    pub(crate) finished: bool,
}

#[derive(Clone, Default)]
pub(crate) struct CommandRuntime {
    inner: Arc<Mutex<CommandRuntimeState>>,
}

impl std::fmt::Debug for CommandRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandRuntime").finish_non_exhaustive()
    }
}

impl PartialEq for CommandRuntime {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

#[derive(Default)]
struct CommandRuntimeState {
    next_generation: u64,
    snapshot: Option<CommandSnapshot>,
}

impl CommandRuntime {
    fn begin(&self, record: CommandRecord) -> u64 {
        let mut state = self.inner.lock().expect("command runtime lock poisoned");
        state.next_generation = state.next_generation.saturating_add(1);
        let generation = state.next_generation;
        state.snapshot = Some(CommandSnapshot {
            generation,
            record,
            finished: false,
        });
        generation
    }

    fn update(&self, generation: u64, update: impl FnOnce(&mut CommandRecord, &mut bool)) {
        let mut state = self.inner.lock().expect("command runtime lock poisoned");
        let Some(snapshot) = state.snapshot.as_mut() else {
            return;
        };
        if snapshot.generation != generation {
            return;
        }
        update(&mut snapshot.record, &mut snapshot.finished);
    }

    pub(crate) fn snapshot(&self) -> Option<CommandSnapshot> {
        self.inner
            .lock()
            .expect("command runtime lock poisoned")
            .snapshot
            .clone()
    }
}

struct CommandPlan {
    title: String,
    args: Vec<String>,
    mode: CommandMode,
}

#[derive(Clone, Copy)]
enum CommandMode {
    Capture,
    Spawn { log_name: &'static str },
}

pub(crate) fn execute_ui_command(state: &mut UiState, command: UiCommand) {
    if matches!(command, UiCommand::Refresh) {
        state.refresh();
        state.last_command = Some(CommandRecord {
            title: "Refresh".to_string(),
            status: CommandStatus::Ok,
            output: "Project, target, and device state refreshed.".to_string(),
        });
        return;
    }

    let Some(plan) = command_plan(state, command) else {
        state.last_command = Some(CommandRecord {
            title: "Action unavailable".to_string(),
            status: CommandStatus::Failed,
            output: "Select a target or device before running this action.".to_string(),
        });
        return;
    };

    let record = match plan.mode {
        CommandMode::Capture => start_capture_command(state, plan),
        CommandMode::Spawn { log_name } => spawn_command(state, &plan, log_name),
    };
    state.last_command = Some(record);
}

fn command_plan(state: &UiState, command: UiCommand) -> Option<CommandPlan> {
    let project_dir = state.project_dir.display().to_string();
    match command {
        UiCommand::InitProject => {
            let mut args = vec!["init".into(), project_dir];
            push_optional_flag(&mut args, "--name", &state.init_name);
            push_optional_flag(&mut args, "--app-id", &state.init_app_id);
            push_optional_flag(&mut args, "--local-path", &state.init_local_path);
            Some(CommandPlan {
                title: "Initialise project".to_string(),
                args,
                mode: CommandMode::Capture,
            })
        }
        UiCommand::AddTarget(target) => Some(CommandPlan {
            title: format!("Add {} target", target.as_str()),
            args: vec![
                "add-target".into(),
                target.as_str().into(),
                "--project-dir".into(),
                project_dir,
            ],
            mode: CommandMode::Capture,
        }),
        UiCommand::DoctorAll => Some(CommandPlan {
            title: "Doctor".to_string(),
            args: {
                let mut args = vec!["doctor".into(), "--project-dir".into(), project_dir];
                if state.strict {
                    args.push("--strict".into());
                }
                args
            },
            mode: CommandMode::Capture,
        }),
        UiCommand::DoctorTarget(target) => Some(CommandPlan {
            title: format!("Doctor {}", target.as_str()),
            args: {
                let mut args = vec![
                    "doctor".into(),
                    target.as_str().into(),
                    "--project-dir".into(),
                    project_dir,
                ];
                if state.strict {
                    args.push("--strict".into());
                }
                args
            },
            mode: CommandMode::Capture,
        }),
        UiCommand::RunSelected => {
            let target = state.selected_target?;
            let mut args = vec![
                "run".into(),
                "--target".into(),
                target.as_str().into(),
                "--project-dir".into(),
                project_dir,
                "--host".into(),
                state.host.clone(),
                "--port".into(),
                state.port.clone(),
            ];
            push_common_run_flags(state, &mut args);
            Some(CommandPlan {
                title: format!("Run {}", target.as_str()),
                args,
                mode: CommandMode::Capture,
            })
        }
        UiCommand::BuildSelected => {
            let target = state.selected_target?;
            let mut args = vec![
                "build".into(),
                "--target".into(),
                target.as_str().into(),
                "--project-dir".into(),
                project_dir,
            ];
            if state.release {
                args.push("--release".into());
            }
            Some(CommandPlan {
                title: format!("Build {}", target.as_str()),
                args,
                mode: CommandMode::Capture,
            })
        }
        UiCommand::TestSelected => {
            let target = state.selected_target?;
            let mut args = vec![
                "test".into(),
                "--target".into(),
                target.as_str().into(),
                "--project-dir".into(),
                project_dir,
            ];
            if state.headless {
                args.push("--headless".into());
            }
            Some(CommandPlan {
                title: format!("Test {}", target.as_str()),
                args,
                mode: CommandMode::Capture,
            })
        }
        UiCommand::SiteBuild => {
            let mut args = vec![
                "site".into(),
                "build".into(),
                "--project-dir".into(),
                project_dir,
            ];
            if state.release {
                args.push("--release".into());
            }
            Some(CommandPlan {
                title: "Build static site".to_string(),
                args,
                mode: CommandMode::Capture,
            })
        }
        UiCommand::SiteCheck => {
            let mut args = vec![
                "site".into(),
                "check".into(),
                "--project-dir".into(),
                project_dir,
            ];
            if state.release {
                args.push("--release".into());
            }
            Some(CommandPlan {
                title: "Check static site".to_string(),
                args,
                mode: CommandMode::Capture,
            })
        }
        UiCommand::SiteRoutes => Some(CommandPlan {
            title: "List static site routes".to_string(),
            args: vec![
                "site".into(),
                "routes".into(),
                "--project-dir".into(),
                project_dir,
            ],
            mode: CommandMode::Capture,
        }),
        UiCommand::SiteServe => {
            let mut args = vec![
                "site".into(),
                "serve".into(),
                "--project-dir".into(),
                project_dir,
                "--host".into(),
                state.host.clone(),
                "--port".into(),
                state.port.clone(),
            ];
            if state.release {
                args.push("--release".into());
            }
            if state.no_open {
                args.push("--no-open".into());
            }
            Some(CommandPlan {
                title: "Serve static site".to_string(),
                args,
                mode: CommandMode::Spawn {
                    log_name: "site-serve",
                },
            })
        }
        UiCommand::LogsSnapshot => {
            let target = state.selected_target?;
            let mut args = vec![
                "logs".into(),
                "--target".into(),
                target.as_str().into(),
                "--project-dir".into(),
                project_dir,
            ];
            if let Some(device) = selected_device_arg(state) {
                args.extend(["--device".into(), device]);
            }
            Some(CommandPlan {
                title: format!("Logs {}", target.as_str()),
                args,
                mode: CommandMode::Capture,
            })
        }
        UiCommand::LogsFollow => {
            let target = state.selected_target?;
            let mut args = vec![
                "logs".into(),
                "--target".into(),
                target.as_str().into(),
                "--project-dir".into(),
                project_dir,
                "--follow".into(),
            ];
            if let Some(device) = selected_device_arg(state) {
                args.extend(["--device".into(), device]);
            }
            Some(CommandPlan {
                title: format!("Follow {} logs", target.as_str()),
                args,
                mode: CommandMode::Spawn {
                    log_name: "logs-follow",
                },
            })
        }
        UiCommand::Refresh => None,
    }
}

fn push_common_run_flags(state: &UiState, args: &mut Vec<String>) {
    if let Some(device) = selected_device_arg(state) {
        args.extend(["--device".into(), device]);
    }
    if state.detach {
        args.push("--detach".into());
    }
    if state.release {
        args.push("--release".into());
    }
    if state.no_open {
        args.push("--no-open".into());
    }
    if state.headless {
        args.push("--headless".into());
    }
}

fn push_optional_flag(args: &mut Vec<String>, name: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        args.extend([name.to_string(), value.to_string()]);
    }
}

fn selected_device_arg(state: &UiState) -> Option<String> {
    let selected = state.selected_device.as_ref()?;
    if selected == "auto" {
        None
    } else {
        Some(selected.clone())
    }
}

fn start_capture_command(state: &UiState, plan: CommandPlan) -> CommandRecord {
    let command_line = format_command_line(&plan.args);
    let initial_record = CommandRecord {
        title: plan.title.clone(),
        status: CommandStatus::Running,
        output: format!("Running `{command_line}`..."),
    };
    let generation = state.command_runtime.begin(initial_record.clone());
    let runtime = state.command_runtime.clone();
    let mut command = command_base(state);
    command
        .args(&plan.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    thread::spawn(move || run_capture_command(runtime, generation, command));
    initial_record
}

fn run_capture_command(runtime: CommandRuntime, generation: u64, mut command: Command) {
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            runtime.update(generation, |record, finished| {
                record.status = CommandStatus::Failed;
                record.output = format!("Failed to start command: {error}");
                *finished = true;
            });
            return;
        }
    };

    runtime.update(generation, |record, _| {
        record.output = format!("Started process {}.\n{}", child.id(), record.output);
    });

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        pipe_lines(stdout, tx.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        pipe_lines(stderr, tx.clone());
    }
    drop(tx);

    let status = loop {
        drain_output(&runtime, generation, &rx);
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                runtime.update(generation, |record, finished| {
                    record.status = CommandStatus::Failed;
                    append_output(record, &format!("Failed to wait for command: {error}"));
                    *finished = true;
                });
                return;
            }
        }
    };
    drain_output(&runtime, generation, &rx);
    finish_capture_command(&runtime, generation, status);
}

fn pipe_lines<R>(reader: R, tx: std::sync::mpsc::Sender<String>)
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = tx.send(format!("Failed to read command output: {error}"));
                    break;
                }
            }
        }
    });
}

fn drain_output(runtime: &CommandRuntime, generation: u64, rx: &std::sync::mpsc::Receiver<String>) {
    let mut lines = Vec::new();
    while let Ok(line) = rx.try_recv() {
        lines.push(line);
    }
    if lines.is_empty() {
        return;
    }
    runtime.update(generation, |record, _| {
        for line in lines {
            append_output(record, &line);
        }
    });
}

fn finish_capture_command(runtime: &CommandRuntime, generation: u64, status: ExitStatus) {
    runtime.update(generation, |record, finished| {
        record.status = if status.success() {
            CommandStatus::Ok
        } else {
            CommandStatus::Failed
        };
        append_output(
            record,
            &format!(
                "Command exited with {}.",
                status
                    .code()
                    .map(|code| format!("status {code}"))
                    .unwrap_or_else(|| "no status code".to_string())
            ),
        );
        *finished = true;
    });
}

fn spawn_command(state: &UiState, plan: &CommandPlan, log_name: &str) -> CommandRecord {
    let log_path = ui_log_path(state, log_name);
    let log = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&log_path);
    let Ok(log) = log else {
        return CommandRecord {
            title: plan.title.clone(),
            status: CommandStatus::Failed,
            output: format!("Failed to create log file {}", log_path.display()),
        };
    };
    let err = match log.try_clone() {
        Ok(err) => err,
        Err(error) => {
            return CommandRecord {
                title: plan.title.clone(),
                status: CommandStatus::Failed,
                output: format!("Failed to prepare log file: {error}"),
            };
        }
    };
    match command_base(state)
        .args(&plan.args)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err))
        .spawn()
    {
        Ok(child) => CommandRecord {
            title: plan.title.clone(),
            status: CommandStatus::Started,
            output: format!(
                "Started process {}. Output is being written to {}.",
                child.id(),
                log_path.display()
            ),
        },
        Err(error) => CommandRecord {
            title: plan.title.clone(),
            status: CommandStatus::Failed,
            output: format!("Failed to start command: {error}"),
        },
    }
}

fn command_base(state: &UiState) -> Command {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("fission"));
    let mut command = Command::new(exe);
    if state.project_dir.exists() {
        command.current_dir(&state.project_dir);
    } else if let Some(parent) = state.project_dir.parent().filter(|parent| parent.exists()) {
        command.current_dir(parent);
    }
    command
}

fn ui_log_path(state: &UiState, name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let dir = state.project_dir.join(".fission/ui");
    let _ = fs::create_dir_all(&dir);
    dir.join(format!("{name}-{stamp}.log"))
}

fn trim_output(mut output: String) -> String {
    const MAX: usize = 7000;
    while output.ends_with('\n') || output.ends_with('\r') {
        output.pop();
    }
    if output.is_empty() {
        return "Command completed without output.".to_string();
    }
    if output.len() > MAX {
        let start = output.len().saturating_sub(MAX);
        format!("... output truncated ...\n{}", &output[start..])
    } else {
        output
    }
}

fn append_output(record: &mut CommandRecord, line: &str) {
    if record.output.is_empty() {
        record.output.push_str(line);
    } else {
        record.output.push('\n');
        record.output.push_str(line);
    }
    record.output = trim_output(std::mem::take(&mut record.output));
}

fn format_command_line(args: &[String]) -> String {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "fission".to_string());
    std::iter::once(exe)
        .chain(args.iter().map(|arg| shell_word(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_word(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
