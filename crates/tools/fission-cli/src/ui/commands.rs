use super::state::UiState;
use crate::Target;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) const DEFAULT_SCROLLBACK_LINES: usize = 100_000;
const MAX_SCROLLBACK_LINE_CHARS: usize = 4096;

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
    pub(crate) output: ScrollbackBuffer,
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

#[derive(Clone, Debug)]
pub(crate) struct ScrollbackBuffer {
    inner: Arc<Mutex<ScrollbackBufferData>>,
}

#[derive(Debug, Eq, PartialEq)]
struct ScrollbackBufferData {
    limit: usize,
    dropped_lines: usize,
    lines: VecDeque<String>,
}

impl Default for ScrollbackBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_SCROLLBACK_LINES)
    }
}

impl ScrollbackBuffer {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ScrollbackBufferData {
                limit: limit.max(1),
                dropped_lines: 0,
                lines: VecDeque::new(),
            })),
        }
    }

    pub(crate) fn from_text(limit: usize, text: impl AsRef<str>) -> Self {
        let mut buffer = Self::new(limit);
        for line in text.as_ref().lines() {
            buffer.push_line(line);
        }
        if buffer.display_line_count() == 0 {
            buffer.push_line("");
        }
        buffer
    }

    pub(crate) fn set_limit(&mut self, limit: usize) {
        let mut data = self.inner.lock().expect("scrollback lock poisoned");
        data.limit = limit.max(1);
        data.trim_to_limit();
    }

    pub(crate) fn push_line(&mut self, line: &str) {
        let mut data = self.inner.lock().expect("scrollback lock poisoned");
        data.lines.push_back(truncate_line(line));
        data.trim_to_limit();
    }

    pub(crate) fn display_line_count(&self) -> usize {
        let data = self.inner.lock().expect("scrollback lock poisoned");
        data.lines.len() + usize::from(data.dropped_lines > 0)
    }

    pub(crate) fn visible_lines(&self, start: usize, count: usize) -> Vec<String> {
        let mut visible = Vec::new();
        if count == 0 {
            return visible;
        }
        let data = self.inner.lock().expect("scrollback lock poisoned");
        let marker_lines = usize::from(data.dropped_lines > 0);
        if data.dropped_lines > 0 && start == 0 {
            visible.push(format!(
                "... {} older lines discarded by scrollback limit ...",
                data.dropped_lines
            ));
        }
        let first_buffer_line = start.saturating_sub(marker_lines);
        let remaining = count.saturating_sub(visible.len());
        visible.extend(
            data.lines
                .iter()
                .skip(first_buffer_line)
                .take(remaining)
                .cloned(),
        );
        visible
    }
}

impl PartialEq for ScrollbackBuffer {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.inner, &other.inner) {
            return true;
        }
        let data = self.inner.lock().expect("scrollback lock poisoned");
        let other_data = other.inner.lock().expect("scrollback lock poisoned");
        *data == *other_data
    }
}

impl Eq for ScrollbackBuffer {}

impl ScrollbackBufferData {
    fn trim_to_limit(&mut self) {
        while self.lines.len() > self.limit {
            self.lines.pop_front();
            self.dropped_lines = self.dropped_lines.saturating_add(1);
        }
    }
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
    pub(crate) revision: u64,
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
    fn begin(&self, mut record: CommandRecord, limit: usize) -> u64 {
        let mut state = self.inner.lock().expect("command runtime lock poisoned");
        state.next_generation = state.next_generation.saturating_add(1);
        let generation = state.next_generation;
        record.output.set_limit(limit);
        state.snapshot = Some(CommandSnapshot {
            generation,
            revision: 0,
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
        snapshot.revision = snapshot.revision.saturating_add(1);
    }

    pub(crate) fn snapshot(&self) -> Option<CommandSnapshot> {
        self.inner
            .lock()
            .expect("command runtime lock poisoned")
            .snapshot
            .clone()
    }

    pub(crate) fn set_limit(&self, limit: usize) {
        let mut state = self.inner.lock().expect("command runtime lock poisoned");
        if let Some(snapshot) = state.snapshot.as_mut() {
            snapshot.record.output.set_limit(limit);
            snapshot.revision = snapshot.revision.saturating_add(1);
        }
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
        let record = CommandRecord {
            title: "Refresh".to_string(),
            status: CommandStatus::Ok,
            output: ScrollbackBuffer::from_text(
                state.scrollback_limit,
                "Project, target, and device state refreshed.",
            ),
        };
        state.last_log_line_count = record.output.display_line_count();
        state.last_command = Some(record);
        return;
    }

    let Some(plan) = command_plan(state, command) else {
        let record = CommandRecord {
            title: "Action unavailable".to_string(),
            status: CommandStatus::Failed,
            output: ScrollbackBuffer::from_text(
                state.scrollback_limit,
                "Select a target or device before running this action.",
            ),
        };
        state.last_log_line_count = record.output.display_line_count();
        state.last_command = Some(record);
        return;
    };

    let record = match plan.mode {
        CommandMode::Capture => start_capture_command(state, plan),
        CommandMode::Spawn { log_name } => spawn_command(state, &plan, log_name),
    };
    state.last_log_line_count = record.output.display_line_count();
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
        output: ScrollbackBuffer::from_text(
            state.scrollback_limit,
            format!("Running `{command_line}`..."),
        ),
    };
    let generation = state
        .command_runtime
        .begin(initial_record.clone(), state.scrollback_limit);
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
                record
                    .output
                    .push_line(&format!("Failed to start command: {error}"));
                *finished = true;
            });
            return;
        }
    };

    runtime.update(generation, |record, _| {
        record
            .output
            .push_line(&format!("Started process {}.", child.id()));
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
                    record
                        .output
                        .push_line(&format!("Failed to wait for command: {error}"));
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
            record.output.push_line(&line);
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
        record.output.push_line(&format!(
            "Command exited with {}.",
            status
                .code()
                .map(|code| format!("status {code}"))
                .unwrap_or_else(|| "no status code".to_string())
        ));
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
            output: ScrollbackBuffer::from_text(
                state.scrollback_limit,
                format!("Failed to create log file {}", log_path.display()),
            ),
        };
    };
    let err = match log.try_clone() {
        Ok(err) => err,
        Err(error) => {
            return CommandRecord {
                title: plan.title.clone(),
                status: CommandStatus::Failed,
                output: ScrollbackBuffer::from_text(
                    state.scrollback_limit,
                    format!("Failed to prepare log file: {error}"),
                ),
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
            output: ScrollbackBuffer::from_text(
                state.scrollback_limit,
                format!(
                    "Started process {}. Output is being written to {}.",
                    child.id(),
                    log_path.display()
                ),
            ),
        },
        Err(error) => CommandRecord {
            title: plan.title.clone(),
            status: CommandStatus::Failed,
            output: ScrollbackBuffer::from_text(
                state.scrollback_limit,
                format!("Failed to start command: {error}"),
            ),
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

fn truncate_line(line: &str) -> String {
    if line.chars().count() <= MAX_SCROLLBACK_LINE_CHARS {
        return line.to_string();
    }
    let mut truncated = line
        .chars()
        .take(MAX_SCROLLBACK_LINE_CHARS)
        .collect::<String>();
    truncated.push_str(" ...");
    truncated
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrollback_buffer_discards_oldest_lines_at_limit() {
        let mut buffer = ScrollbackBuffer::new(3);
        for line in ["one", "two", "three", "four", "five"] {
            buffer.push_line(line);
        }

        assert_eq!(buffer.display_line_count(), 4);
        assert_eq!(
            buffer.visible_lines(0, 4),
            vec![
                "... 2 older lines discarded by scrollback limit ...".to_string(),
                "three".to_string(),
                "four".to_string(),
                "five".to_string()
            ]
        );
    }

    #[test]
    fn scrollback_buffer_limit_can_be_reduced() {
        let mut buffer = ScrollbackBuffer::new(5);
        for line in ["one", "two", "three", "four"] {
            buffer.push_line(line);
        }

        buffer.set_limit(2);

        assert_eq!(
            buffer.visible_lines(0, 3),
            vec![
                "... 2 older lines discarded by scrollback limit ...".to_string(),
                "three".to_string(),
                "four".to_string()
            ]
        );
    }
}
