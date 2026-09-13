//! Reducers for results arriving from background jobs and timers: tree scans,
//! git status, file operations, terminal output and language-server polls.

use super::{
    EditorStarted, EditorState, FsCompleted, FsFailed, GitStatusFailed, GitStatusLoaded, LspHandle,
    PollLsp, PollLspTick, PollTerminal, PollTerminalTick, TreeScanCompleted, TreeScanFailed,
    FS_JOB, GIT_STATUS_JOB, TREE_SCAN_JOB,
};
use fission::core::ReducerContext;

type Cx<'a, 'b, 'c> = ReducerContext<'a, 'b, 'c, EditorState>;

/// Opens the workspace: scans the tree, reads git status, starts a terminal and,
/// outside automated test runs, a language server.
pub fn on_editor_started(state: &mut EditorState, action: EditorStarted, _: &mut Cx<'_, '_, '_>) {
    state.root_path = action.root_path;
    state.request_tree_refresh();
    state.refresh_git_status();
    state.ensure_terminal_session();
    if std::env::var("FISSION_TEST_CONTROL_PORT").is_err() && state.lsp_handle.is_none() {
        state.lsp_handle = Some(LspHandle::new(&state.root_path));
    }
}

/// Applies a tree scan unless a newer scan has been requested since.
pub fn on_tree_scan_completed(
    state: &mut EditorState,
    _: TreeScanCompleted,
    cx: &mut Cx<'_, '_, '_>,
) {
    if let Some(result) = cx.input.job_ok(TREE_SCAN_JOB) {
        if result.generation == state.tree_scan_generation {
            state.cached_tree_entries = result.entries;
            state.tree_scan_loaded_generation = result.generation;
        }
    }
}

pub fn on_tree_scan_failed(state: &mut EditorState, _: TreeScanFailed, cx: &mut Cx<'_, '_, '_>) {
    state.tree_scan_loaded_generation = state.tree_scan_generation;
    if let Some(message) = cx.input.job_error_message(TREE_SCAN_JOB) {
        state.status_message = Some(format!("Tree refresh failed: {}", message));
    }
}

/// Applies git status unless a newer refresh has been requested since.
pub fn on_git_status_loaded(state: &mut EditorState, _: GitStatusLoaded, cx: &mut Cx<'_, '_, '_>) {
    if let Some(result) = cx.input.job_ok(GIT_STATUS_JOB) {
        if result.generation == state.git_status_generation {
            state.git_status_lines = result.entries;
            state.git_status_loaded_generation = result.generation;
        }
    }
}

pub fn on_git_status_failed(state: &mut EditorState, _: GitStatusFailed, cx: &mut Cx<'_, '_, '_>) {
    state.git_status_loaded_generation = state.git_status_generation;
    if let Some(message) = cx.input.job_error_message(GIT_STATUS_JOB) {
        state.status_message = Some(format!("Git status refresh failed: {}", message));
    }
}

pub fn on_fs_completed(state: &mut EditorState, _: FsCompleted, cx: &mut Cx<'_, '_, '_>) {
    if let Some(outcome) = cx.input.job_ok(FS_JOB) {
        state.apply_fs_outcome(outcome);
    }
}

pub fn on_fs_failed(state: &mut EditorState, _: FsFailed, cx: &mut Cx<'_, '_, '_>) {
    if let Some(failure) = cx.input.job_err(FS_JOB) {
        state.apply_fs_failure(failure);
    }
}

/// Redraws when the terminal has produced output since the last tick.
pub fn on_poll_terminal(state: &mut EditorState, _: PollTerminal, cx: &mut Cx<'_, '_, '_>) {
    let _tick: PollTerminalTick = cx.input.timer_tick().unwrap_or_default();
    if let Some(session) = state.terminal_session.as_ref() {
        if session.take_dirty() {
            state.redraw_epoch = state.redraw_epoch.wrapping_add(1);
        }
    }
}

/// Collects diagnostics and completions the language server has sent.
pub fn on_poll_lsp(state: &mut EditorState, _: PollLsp, cx: &mut Cx<'_, '_, '_>) {
    let _tick: PollLspTick = cx.input.timer_tick().unwrap_or_default();
    if let Some(handle) = state.lsp_handle.as_ref() {
        let (diags, completions) = handle.poll_diagnostics();
        for (path, file_diags) in diags {
            state.diagnostics.insert(path, file_diags);
        }
        if !completions.is_empty() {
            state.completions = completions;
            state.show_completions = true;
            state.selected_completion = 0;
        }
    }
}
