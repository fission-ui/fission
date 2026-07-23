use crate::activity_bar::ActivityBar;
use crate::breadcrumb::Breadcrumb;
use crate::command_palette::CommandPalette;
use crate::completion_popup::CompletionPopup;
use crate::context_menu::ContextMenu;
use crate::editor_surface::EditorSurface;
use crate::file_tree::FileTree;
use crate::find_replace_bar::FindReplaceBar;
use crate::git_panel::GitPanel;
use crate::hover_tooltip::HoverTooltip;
use crate::layout::{
    DIVIDER_THICKNESS, EDITOR_HORIZONTAL_RESERVE, PANEL_HEADER_HEIGHT, SIDEBAR_MAX_WIDTH,
    SIDEBAR_MIN_WIDTH,
};
use crate::menu_bar::MenuBar;
use crate::model::*;
use crate::palette::{BORDER_COLOR, DIM_TEXT, SIDEBAR_HEADING, SURFACE_BG};
use crate::search_panel::SearchPanel;
use crate::status_bar::StatusBar;
use crate::tab_bar::TabBar;
use crate::terminal_panel::TerminalPanel;
use fission::core::ui::{Column, Container, Row, Text, Widget};
use fission::core::{reduce_with, JobResource, ReducerContext, ResourceKey, TimerResource};
use fission::widgets::Spacer;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct EditorApp;

impl From<EditorApp> for Widget {
    fn from(_component: EditorApp) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
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

        let viewport = view.viewport_size();
        let sidebar_width = view.state().sidebar_width.min(
            (viewport.width - EDITOR_HORIZONTAL_RESERVE)
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
        );

        // ── Menu bar (topmost) ──
        let menu_bar = MenuBar.into();

        // ── Activity bar (leftmost strip) ──
        let activity_bar = ActivityBar.into();

        // ── Sidebar (content depends on active section) ──
        let sidebar = if view.state().sidebar_visible {
            let (header_text, panel_content) = match view.state().sidebar_section {
                SidebarSection::Explorer => ("EXPLORER", FileTree.into()),
                SidebarSection::Search => ("SEARCH", SearchPanel.into()),
                SidebarSection::Git => ("SOURCE CONTROL", GitPanel.into()),
                SidebarSection::Extensions => (
                    "EXTENSIONS",
                    Container::new(
                        Text::new("No extensions installed")
                            .size(tokens.typography.font_size_xs)
                            .color(DIM_TEXT),
                    )
                    .padding_all(tokens.spacing.s)
                    .flex_grow(1.0)
                    .into(),
                ),
            };

            let header = Container::new(
                Text::new(header_text)
                    .size(tokens.typography.font_size_xs)
                    .color(SIDEBAR_HEADING),
            )
            .bg(SURFACE_BG)
            .height(PANEL_HEADER_HEIGHT)
            .padding_all(tokens.spacing.s)
            .flex_shrink(0.0)
            .into();

            Container::new(Column {
                children: vec![header, panel_content],
                flex_grow: 1.0,
                ..Default::default()
            })
            .width(sidebar_width)
            .bg(SURFACE_BG)
            .flex_shrink(0.0)
            .into()
        } else {
            Spacer {
                width: Some(0.0),
                ..Default::default()
            }
            .into()
        };

        // 1px vertical divider between sidebar and editor
        let sidebar_divider = if view.state().sidebar_visible {
            Container::new(Spacer::default())
                .width(DIVIDER_THICKNESS)
                .bg(BORDER_COLOR)
                .flex_shrink(0.0)
                .into()
        } else {
            Spacer {
                width: Some(0.0),
                ..Default::default()
            }
            .into()
        };

        // ── Editor area: tabs + breadcrumb + find/replace + surface ──
        let tab_bar_node = TabBar.into();
        let breadcrumb_node = Breadcrumb.into();
        let find_replace_node = FindReplaceBar.into();
        let editor_surface_node = EditorSurface.into();

        let editor_area: Widget = Column {
            children: vec![
                tab_bar_node,
                breadcrumb_node,
                find_replace_node,
                editor_surface_node,
            ],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into();

        // 1px horizontal divider above terminal
        let terminal_divider = Container::new(Spacer::default())
            .height(DIVIDER_THICKNESS)
            .bg(BORDER_COLOR)
            .flex_shrink(0.0)
            .into();

        // Center: editor area + terminal
        let center = Column {
            children: if view.state().terminal_visible {
                vec![
                    Container::new(editor_area).flex_grow(1.0).into(),
                    terminal_divider,
                    TerminalPanel.into(),
                ]
            } else {
                vec![Container::new(editor_area).flex_grow(1.0).into()]
            },
            flex_grow: 1.0,
            ..Default::default()
        }
        .into();

        // Main layout: activity bar | sidebar | divider | center
        let main_layout: Widget = Row {
            children: vec![activity_bar, sidebar, sidebar_divider, center],
            align_items: fission::op::AlignItems::Stretch,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into();

        // Root: menu bar + main + status bar
        let root = Column {
            children: vec![
                menu_bar,
                Container::new(main_layout).flex_grow(1.0).into(),
                StatusBar.into(),
            ],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into();

        // ── Overlays (portals) ──
        let _: Widget = CommandPalette.into();
        let _: Widget = ContextMenu.into();
        let _: Widget = CompletionPopup.into();
        let _: Widget = HoverTooltip.into();

        root
    }
}
