use crate::command_palette::CommandPalette;
use crate::completion_popup::CompletionPopup;
use crate::context_menu::ContextMenu;
use crate::editor_workspace::EditorWorkspace;
use crate::hover_tooltip::HoverTooltip;
use crate::model::*;
use fission::core::ui::Widget;
use fission::core::{reduce_with, JobResource, ReducerContext, ResourceKey, TimerResource};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct EditorApp;

impl From<EditorApp> for Widget {
    fn from(_component: EditorApp) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let _start_editor = ctx.bind(
            EditorStarted {
                root_path: PathBuf::from("."),
            },
            reduce_with!(
                (|state: &mut EditorState, action: EditorStarted, _| {
                    state.root_path = action.root_path;
                    state.request_tree_refresh();
                    state.refresh_git_status();
                    state.ensure_terminal_session();
                    if std::env::var("FISSION_TEST_CONTROL_PORT").is_err()
                        && state.lsp_handle.is_none()
                    {
                        state.lsp_handle = Some(LspHandle::new(&state.root_path));
                    }
                })
            ),
        );

        let tree_scan_loaded = ctx.bind(
            TreeScanCompleted,
            reduce_with!(
                (|state: &mut EditorState,
                  _: TreeScanCompleted,
                  reducer: &mut ReducerContext<EditorState>| {
                    if let Some(result) = reducer.input.job_ok(TREE_SCAN_JOB) {
                        if result.generation == state.tree_scan_generation {
                            state.cached_tree_entries = result.entries;
                            state.tree_scan_loaded_generation = result.generation;
                        }
                    }
                })
            ),
        );
        let tree_scan_failed = ctx.bind(
            TreeScanFailed,
            reduce_with!(
                (|state: &mut EditorState,
                  _: TreeScanFailed,
                  reducer: &mut ReducerContext<EditorState>| {
                    state.tree_scan_loaded_generation = state.tree_scan_generation;
                    if let Some(message) = reducer.input.job_error_message(TREE_SCAN_JOB) {
                        state.status_message = Some(format!("Tree refresh failed: {}", message));
                    }
                })
            ),
        );
        if view.state().tree_scan_pending() {
            ctx.with_resources(|resources| {
                resources.job(
                    JobResource::new(
                        ResourceKey::new("editor-tree-scan"),
                        TREE_SCAN_JOB,
                        TreeScanRequest {
                            root_path: view.state().root_path.clone(),
                            generation: view.state().tree_scan_generation,
                        },
                    )
                    .deps((
                        view.state().root_path.clone(),
                        view.state().tree_scan_generation,
                    ))
                    .on_ok(tree_scan_loaded)
                    .on_err(tree_scan_failed),
                );
            });
        }

        let git_status_loaded = ctx.bind(
            GitStatusLoaded,
            reduce_with!(
                (|state: &mut EditorState,
                  _: GitStatusLoaded,
                  reducer: &mut ReducerContext<EditorState>| {
                    if let Some(result) = reducer.input.job_ok(GIT_STATUS_JOB) {
                        if result.generation == state.git_status_generation {
                            state.git_status_lines = result.entries;
                            state.git_status_loaded_generation = result.generation;
                        }
                    }
                })
            ),
        );
        let git_status_failed = ctx.bind(
            GitStatusFailed,
            reduce_with!(
                (|state: &mut EditorState,
                  _: GitStatusFailed,
                  reducer: &mut ReducerContext<EditorState>| {
                    state.git_status_loaded_generation = state.git_status_generation;
                    if let Some(message) = reducer.input.job_error_message(GIT_STATUS_JOB) {
                        state.status_message =
                            Some(format!("Git status refresh failed: {}", message));
                    }
                })
            ),
        );
        if view.state().git_status_pending() {
            ctx.with_resources(|resources| {
                resources.job(
                    JobResource::new(
                        ResourceKey::new("editor-git-status"),
                        GIT_STATUS_JOB,
                        GitStatusRequest {
                            root_path: view.state().root_path.clone(),
                            generation: view.state().git_status_generation,
                        },
                    )
                    .deps((
                        view.state().root_path.clone(),
                        view.state().git_status_generation,
                    ))
                    .on_ok(git_status_loaded)
                    .on_err(git_status_failed),
                );
            });
        }

        let poll_terminal = ctx.bind(
            PollTerminal,
            reduce_with!(
                (|state: &mut EditorState,
                  _: PollTerminal,
                  reducer: &mut ReducerContext<EditorState>| {
                    let _tick: PollTerminalTick = reducer.input.timer_tick().unwrap_or_default();
                    if let Some(session) = state.terminal_session.as_ref() {
                        if session.take_dirty() {
                            state.redraw_epoch = state.redraw_epoch.wrapping_add(1);
                        }
                    }
                })
            ),
        );
        if view.state().terminal_visible
            && view.state().bottom_panel_tab == BottomPanelTab::Terminal
            && view.state().terminal_session.is_some()
        {
            ctx.with_resources(|resources| {
                resources.timer(
                    TimerResource::new(
                        ResourceKey::new("editor-terminal-poll"),
                        Duration::from_millis(16),
                        PollTerminalTick,
                    )
                    .on_tick(poll_terminal),
                );
            });
        }

        let poll_lsp = ctx.bind(
            PollLsp,
            reduce_with!(
                (|state: &mut EditorState,
                  _: PollLsp,
                  reducer: &mut ReducerContext<EditorState>| {
                    let _tick: PollLspTick = reducer.input.timer_tick().unwrap_or_default();
                    if let Some(handle) = state.lsp_handle.as_ref() {
                        let (diags, completions) = handle.poll_diagnostics();
                        if !diags.is_empty() {
                            for (path, file_diags) in diags {
                                state.diagnostics.insert(path, file_diags);
                            }
                        }
                        if !completions.is_empty() {
                            state.completions = completions;
                            state.show_completions = true;
                            state.selected_completion = 0;
                        }
                    }
                })
            ),
        );
        if view.state().lsp_enabled() {
            ctx.with_resources(|resources| {
                resources.timer(
                    TimerResource::new(
                        ResourceKey::new("editor-lsp-poll"),
                        Duration::from_secs(1),
                        PollLspTick,
                    )
                    .immediate()
                    .on_tick(poll_lsp),
                );
            });
        }

        let root: Widget = EditorWorkspace.into();

        // ── Overlays (portals) ──
        let _: Widget = CommandPalette.into();
        let _: Widget = ContextMenu.into();
        let _: Widget = CompletionPopup.into();
        let _: Widget = HoverTooltip.into();

        root
    }
}
