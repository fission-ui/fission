use super::state::UiState;
use fission_command_core::Target;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const DEFAULT_SCROLLBACK_LINES: usize = 100_000;
const MAX_SCROLLBACK_LINE_CHARS: usize = 4096;
pub type CommandSessionId = u64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum UiCommand {
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

impl UiCommand {
    pub fn label(&self) -> String {
        match self {
            Self::InitProject => "initialise this project".to_string(),
            Self::AddTarget(target) => format!("add the {} target", target.as_str()),
            Self::DoctorAll => "run doctor checks".to_string(),
            Self::DoctorTarget(target) => format!("run {} doctor checks", target.as_str()),
            Self::Refresh => "refresh project state".to_string(),
            Self::RunSelected => "run the selected target".to_string(),
            Self::BuildSelected => "build the selected target".to_string(),
            Self::TestSelected => "test the selected target".to_string(),
            Self::SiteBuild => "build the static site".to_string(),
            Self::SiteCheck => "check the static site".to_string(),
            Self::SiteRoutes => "list static site routes".to_string(),
            Self::SiteServe => "serve the static site".to_string(),
            Self::LogsSnapshot => "read logs".to_string(),
            Self::LogsFollow => "follow logs".to_string(),
        }
    }

    pub fn confirmation_message(&self) -> String {
        match self {
            Self::InitProject => {
                "This writes missing Fission project files only when they are absent.".to_string()
            }
            Self::AddTarget(target) => format!(
                "This scaffolds or updates the {} platform files for this project.",
                target.as_str()
            ),
            Self::RunSelected => {
                "This builds, launches, and attaches output for the selected target unless detach is enabled.".to_string()
            }
            Self::BuildSelected => "This starts a build and streams compiler output.".to_string(),
            Self::TestSelected => "This runs the generated target smoke test.".to_string(),
            Self::SiteServe => {
                "This starts a local site server and keeps its output in a command tab.".to_string()
            }
            Self::LogsFollow => {
                "This starts a log follower and keeps streaming output into a command tab.".to_string()
            }
            Self::Refresh => "This refreshes project, target, and device state.".to_string(),
            Self::DoctorAll | Self::DoctorTarget(_) => {
                "This checks local tooling and reports missing setup.".to_string()
            }
            Self::SiteBuild | Self::SiteCheck | Self::SiteRoutes | Self::LogsSnapshot => {
                "This runs the selected workflow and stores the result in a command tab.".to_string()
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandRecord {
    pub title: String,
    pub status: CommandStatus,
    pub output: ScrollbackBuffer,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CommandStatus {
    #[default]
    Ready,
    Running,
    Ok,
    Failed,
}

#[derive(Clone, Debug)]
pub struct ScrollbackBuffer {
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
    pub fn new(limit: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ScrollbackBufferData {
                limit: limit.max(1),
                dropped_lines: 0,
                lines: VecDeque::new(),
            })),
        }
    }

    pub fn from_text(limit: usize, text: impl AsRef<str>) -> Self {
        let mut buffer = Self::new(limit);
        for line in text.as_ref().lines() {
            buffer.push_line(line);
        }
        if buffer.display_line_count() == 0 {
            buffer.push_line("");
        }
        buffer
    }

    pub fn set_limit(&mut self, limit: usize) {
        let mut data = self.inner.lock().expect("scrollback lock poisoned");
        data.limit = limit.max(1);
        data.trim_to_limit();
    }

    pub fn push_line(&mut self, line: &str) {
        let mut data = self.inner.lock().expect("scrollback lock poisoned");
        data.lines.push_back(truncate_line(line));
        data.trim_to_limit();
    }

    pub fn display_line_count(&self) -> usize {
        let data = self.inner.lock().expect("scrollback lock poisoned");
        data.lines.len() + usize::from(data.dropped_lines > 0)
    }

    pub fn visible_lines(&self, start: usize, count: usize) -> Vec<String> {
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
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Running => "Running",
            Self::Ok => "OK",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandSnapshot {
    pub id: CommandSessionId,
    pub revision: u64,
    pub record: CommandRecord,
    pub finished: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandRuntimeSnapshot {
    pub active_session_id: Option<CommandSessionId>,
    pub sessions: Vec<CommandSnapshot>,
}

#[derive(Clone, Default)]
pub struct CommandRuntime {
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
    next_session_id: CommandSessionId,
    active_session_id: Option<CommandSessionId>,
    sessions: Vec<CommandSnapshot>,
}

impl CommandRuntime {
    fn begin(&self, mut record: CommandRecord, limit: usize) -> CommandSessionId {
        let mut state = self.inner.lock().expect("command runtime lock poisoned");
        state.next_session_id = state.next_session_id.saturating_add(1);
        let session_id = state.next_session_id;
        record.output.set_limit(limit);
        state.active_session_id = Some(session_id);
        state.sessions.push(CommandSnapshot {
            id: session_id,
            revision: 0,
            record,
            finished: false,
        });
        session_id
    }

    fn update(
        &self,
        session_id: CommandSessionId,
        update: impl FnOnce(&mut CommandRecord, &mut bool),
    ) {
        let mut state = self.inner.lock().expect("command runtime lock poisoned");
        let Some(snapshot) = state.sessions.iter_mut().find(|item| item.id == session_id) else {
            return;
        };
        update(&mut snapshot.record, &mut snapshot.finished);
        snapshot.revision = snapshot.revision.saturating_add(1);
    }

    pub fn snapshot(&self) -> CommandRuntimeSnapshot {
        let state = self.inner.lock().expect("command runtime lock poisoned");
        CommandRuntimeSnapshot {
            active_session_id: state.active_session_id,
            sessions: state.sessions.clone(),
        }
    }

    pub fn set_active(&self, session_id: CommandSessionId) {
        let mut state = self.inner.lock().expect("command runtime lock poisoned");
        if state.sessions.iter().any(|item| item.id == session_id) {
            state.active_session_id = Some(session_id);
        }
    }

    pub fn record_completed(
        &self,
        mut record: CommandRecord,
        limit: usize,
        finished: bool,
    ) -> CommandSessionId {
        let mut state = self.inner.lock().expect("command runtime lock poisoned");
        state.next_session_id = state.next_session_id.saturating_add(1);
        let session_id = state.next_session_id;
        record.output.set_limit(limit);
        state.active_session_id = Some(session_id);
        state.sessions.push(CommandSnapshot {
            id: session_id,
            revision: 0,
            record,
            finished,
        });
        session_id
    }

    pub fn set_limit(&self, limit: usize) {
        let mut state = self.inner.lock().expect("command runtime lock poisoned");
        for snapshot in &mut state.sessions {
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
}

pub fn execute_ui_command(state: &mut UiState, command: UiCommand) {
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
        state
            .command_runtime
            .record_completed(record, state.scrollback_limit, true);
        state.sync_command_sessions();
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
        state
            .command_runtime
            .record_completed(record, state.scrollback_limit, true);
        state.sync_command_sessions();
        return;
    };

    let record = match plan.mode {
        CommandMode::Capture => start_capture_command(state, plan),
    };
    if matches!(record.status, CommandStatus::Failed) {
        state
            .command_runtime
            .record_completed(record, state.scrollback_limit, true);
    }
    state.sync_command_sessions();
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
                mode: CommandMode::Capture,
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
                mode: CommandMode::Capture,
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

    #[test]
    fn command_runtime_keeps_independent_sessions() {
        let runtime = CommandRuntime::default();
        let first = runtime.record_completed(
            CommandRecord {
                title: "Doctor".to_string(),
                status: CommandStatus::Ok,
                output: ScrollbackBuffer::from_text(10, "doctor output"),
            },
            10,
            true,
        );
        let second = runtime.record_completed(
            CommandRecord {
                title: "Serve".to_string(),
                status: CommandStatus::Running,
                output: ScrollbackBuffer::from_text(10, "serve output"),
            },
            10,
            false,
        );

        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.active_session_id, Some(second));
        assert_eq!(snapshot.sessions.len(), 2);
        assert_eq!(snapshot.sessions[0].id, first);
        assert_eq!(snapshot.sessions[1].id, second);
        assert_eq!(
            snapshot.sessions[0].record.output.visible_lines(0, 1),
            vec!["doctor output".to_string()]
        );
        assert_eq!(
            snapshot.sessions[1].record.output.visible_lines(0, 1),
            vec!["serve output".to_string()]
        );
    }
}
