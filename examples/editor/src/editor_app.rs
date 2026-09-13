use crate::command_palette::CommandPalette;
use crate::completion_popup::CompletionPopup;
use crate::context_menu::ContextMenu;
use crate::editor_workspace::EditorWorkspace;
use crate::hover_tooltip::HoverTooltip;
use crate::model::*;
use fission::core::ui::Widget;
use fission::core::{reduce_with, JobResource, ResourceKey, TimerResource};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone)]
pub struct EditorApp;

impl From<EditorApp> for Widget {
    fn from(_component: EditorApp) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let state = view.state();

        // Registers the reducer for the EditorStarted action dispatched at launch.
        ctx.bind(
            EditorStarted {
                root_path: PathBuf::from("."),
            },
            reduce_with!(on_editor_started),
        );

        let tree_scan_loaded = ctx.bind(TreeScanCompleted, reduce_with!(on_tree_scan_completed));
        let tree_scan_failed = ctx.bind(TreeScanFailed, reduce_with!(on_tree_scan_failed));
        let git_status_loaded = ctx.bind(GitStatusLoaded, reduce_with!(on_git_status_loaded));
        let git_status_failed = ctx.bind(GitStatusFailed, reduce_with!(on_git_status_failed));
        let fs_completed = ctx.bind(FsCompleted, reduce_with!(on_fs_completed));
        let fs_failed = ctx.bind(FsFailed, reduce_with!(on_fs_failed));
        let poll_terminal = ctx.bind(PollTerminal, reduce_with!(on_poll_terminal));
        let poll_lsp = ctx.bind(PollLsp, reduce_with!(on_poll_lsp));

        ctx.with_resources(|resources| {
            if state.tree_scan_pending() {
                resources.job(
                    JobResource::new(
                        ResourceKey::new("editor-tree-scan"),
                        TREE_SCAN_JOB,
                        TreeScanRequest {
                            root_path: state.root_path.clone(),
                            generation: state.tree_scan_generation,
                        },
                    )
                    .deps((state.root_path.clone(), state.tree_scan_generation))
                    .on_ok(tree_scan_loaded)
                    .on_err(tree_scan_failed),
                );
            }

            if state.git_status_pending() {
                resources.job(
                    JobResource::new(
                        ResourceKey::new("editor-git-status"),
                        GIT_STATUS_JOB,
                        GitStatusRequest {
                            root_path: state.root_path.clone(),
                            generation: state.git_status_generation,
                        },
                    )
                    .deps((state.root_path.clone(), state.git_status_generation))
                    .on_ok(git_status_loaded)
                    .on_err(git_status_failed),
                );
            }

            for request in &state.pending_fs {
                resources.job(
                    JobResource::new(
                        ResourceKey::new(format!("editor-fs-{}", request.id)),
                        FS_JOB,
                        request.clone(),
                    )
                    .deps(request.id)
                    .on_ok(fs_completed.clone())
                    .on_err(fs_failed.clone()),
                );
            }

            if state.terminal_visible
                && state.bottom_panel_tab == BottomPanelTab::Terminal
                && state.terminal_session.is_some()
            {
                resources.timer(
                    TimerResource::new(
                        ResourceKey::new("editor-terminal-poll"),
                        Duration::from_millis(16),
                        PollTerminalTick,
                    )
                    .on_tick(poll_terminal),
                );
            }

            if state.lsp_enabled() {
                resources.timer(
                    TimerResource::new(
                        ResourceKey::new("editor-lsp-poll"),
                        Duration::from_secs(1),
                        PollLspTick,
                    )
                    .immediate()
                    .on_tick(poll_lsp),
                );
            }
        });

        let root: Widget = EditorWorkspace.into();

        // Overlays register themselves as portals while they build.
        let _: Widget = CommandPalette.into();
        let _: Widget = ContextMenu.into();
        let _: Widget = CompletionPopup.into();
        let _: Widget = HoverTooltip.into();

        root
    }
}
