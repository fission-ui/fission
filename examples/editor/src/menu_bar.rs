use crate::editor_menu_button::EditorMenuButton;
use crate::editor_menu_item::EditorMenuItem;
use crate::layout::{
    MENU_BAR_HEIGHT, MENU_BUTTON_WIDTH, MENU_FLYOUT_MAX_WIDTH, MENU_FLYOUT_MIN_WIDTH,
    OVERLAY_EDGE_GUTTER, OVERLAY_HORIZONTAL_RESERVE, OVERLAY_MIN_INSET,
};
use crate::model::*;
use crate::palette::{FLYOUT_BG, FLYOUT_BORDER, INTERACTION_BACKDROP, MENU_BAR_BG};
use fission::core::ui::{Column, Container, GestureDetector, Positioned, Row, Widget, ZStack};
use fission::core::{reduce_with, WidgetId};
use fission::widgets::Spacer;

pub(crate) struct MenuBar;

impl From<MenuBar> for Widget {
    fn from(_component: MenuBar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let viewport = view.viewport_size();
        let flyout_width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
            .clamp(MENU_FLYOUT_MIN_WIDTH, MENU_FLYOUT_MAX_WIDTH);

        // reduce_with: set active_menu (toggle logic)
        let set_menu = ctx.bind(
            SetActiveMenu(None),
            reduce_with!(
                (|s: &mut EditorState, a: SetActiveMenu, _| {
                    if s.active_menu == a.0 {
                        s.active_menu = None;
                    } else {
                        s.active_menu = a.0;
                    }
                })
            ),
        );
        let set_menu_id = set_menu.id;

        // ── Shared action handlers for flyout commands ──

        let dismiss_menu = ctx.bind(
            DismissMenu,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.active_menu = None;
                })
            ),
        );

        let save_file = ctx.bind(
            SaveFile,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.save_active_file();
                    s.active_menu = None;
                })
            ),
        );

        let save_all = ctx.bind(
            SaveAllFiles,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.save_all_files();
                    s.active_menu = None;
                })
            ),
        );

        let close_tab_action = ctx.bind(
            CloseTab(0),
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    let idx = s.active_tab;
                    s.close_tab(idx);
                    s.active_menu = None;
                })
            ),
        );

        let toggle_find = ctx.bind(
            ToggleFindReplace,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.show_find_replace = !s.show_find_replace;
                    s.active_menu = None;
                })
            ),
        );

        let toggle_sidebar = ctx.bind(
            ToggleSidebar,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.sidebar_visible = !s.sidebar_visible;
                    s.active_menu = None;
                })
            ),
        );

        let toggle_terminal = ctx.bind(
            ToggleTerminal,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.terminal_visible = !s.terminal_visible;
                    if s.terminal_visible {
                        s.bottom_panel_tab = crate::model::BottomPanelTab::Terminal;
                        s.ensure_terminal_session();
                    }
                    s.active_menu = None;
                })
            ),
        );

        let cmd_palette = ctx.bind(
            ToggleCommandPalette,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.show_command_palette = !s.show_command_palette;
                    s.active_menu = None;
                })
            ),
        );

        let about_action = ctx.bind(
            ShowMenuStatus("Fission Editor v0.1.0".into()),
            reduce_with!(
                (|s: &mut EditorState, a: ShowMenuStatus, _| {
                    s.status_message = Some(a.0);
                    s.active_menu = None;
                })
            ),
        );

        let new_file_action = ctx.bind(
            ShowMenuStatus("New File (use file tree context menu)".into()),
            reduce_with!(
                (|s: &mut EditorState, a: ShowMenuStatus, _| {
                    s.status_message = Some(a.0);
                    s.active_menu = None;
                })
            ),
        );

        let new_folder_action = ctx.bind(
            ShowMenuStatus("New Folder (use file tree context menu)".into()),
            reduce_with!(
                (|s: &mut EditorState, a: ShowMenuStatus, _| {
                    s.status_message = Some(a.0);
                    s.active_menu = None;
                })
            ),
        );

        let go_to_def_action = ctx.bind(
            GoToDefinition,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.status_message = Some("Go to Definition: LSP not connected".into());
                    s.active_menu = None;
                })
            ),
        );

        let go_to_line_action = ctx.bind(
            ToggleCommandPalette,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.show_command_palette = true;
                    s.command_query = "Go to Line:".into();
                    s.active_menu = None;
                })
            ),
        );

        let undo_action = ctx.bind(
            Undo,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.undo_active();
                    s.active_menu = None;
                })
            ),
        );

        let redo_action = ctx.bind(
            Redo,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.redo_active();
                    s.active_menu = None;
                })
            ),
        );

        let copy_action = ctx.bind(
            CopySelection,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.copy_line();
                    s.active_menu = None;
                })
            ),
        );

        let cut_action = ctx.bind(
            CutSelection,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.cut_line();
                    s.active_menu = None;
                })
            ),
        );

        let paste_action = ctx.bind(
            PasteClipboard,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.paste();
                    s.active_menu = None;
                })
            ),
        );

        // ── Top-level buttons ──

        let labels = ["File", "Edit", "View", "Go", "Help"];
        let mut buttons: Vec<Widget> = labels
            .iter()
            .map(|label| {
                EditorMenuButton {
                    label: label.to_string(),
                    set_menu_id,
                }
                .into()
            })
            .collect();
        buttons.push(
            Spacer {
                flex_grow: 1.0,
                ..Default::default()
            }
            .into(),
        );

        let bar = Container::new(Row {
            children: buttons,
            align_items: fission::op::AlignItems::Center,
            ..Default::default()
        })
        .height(MENU_BAR_HEIGHT)
        .bg(MENU_BAR_BG)
        .flex_shrink(0.0)
        .into();

        // ── Flyout dropdown (portal) ──

        if let Some(ref active) = view.state().active_menu {
            let items: Vec<Widget> = match active.as_str() {
                "File" => vec![
                    EditorMenuItem::new("New File", new_file_action.clone()).into(),
                    EditorMenuItem::new("New Folder", new_folder_action.clone()).into(),
                    EditorMenuItem::new("Save", save_file.clone()).into(),
                    EditorMenuItem::new("Save All", save_all.clone()).into(),
                    EditorMenuItem::new("Close Tab", close_tab_action.clone()).into(),
                ],
                "Edit" => vec![
                    EditorMenuItem::new("Undo", undo_action.clone()).into(),
                    EditorMenuItem::new("Redo", redo_action.clone()).into(),
                    EditorMenuItem::new("Cut", cut_action.clone()).into(),
                    EditorMenuItem::new("Copy", copy_action.clone()).into(),
                    EditorMenuItem::new("Paste", paste_action.clone()).into(),
                    EditorMenuItem::new("Find/Replace", toggle_find.clone()).into(),
                ],
                "View" => vec![
                    EditorMenuItem::new("Toggle Sidebar", toggle_sidebar.clone()).into(),
                    EditorMenuItem::new("Toggle Terminal", toggle_terminal.clone()).into(),
                    EditorMenuItem::new("Command Palette", cmd_palette.clone()).into(),
                ],
                "Go" => vec![
                    EditorMenuItem::new("Go to Line", go_to_line_action.clone()).into(),
                    EditorMenuItem::new("Go to Definition", go_to_def_action.clone()).into(),
                ],
                "Help" => vec![EditorMenuItem::new("About", about_action.clone()).into()],
                _ => vec![],
            };

            // Compute left offset based on which menu is active
            let menu_index = match active.as_str() {
                "File" => 0,
                "Edit" => 1,
                "View" => 2,
                "Go" => 3,
                "Help" => 4,
                _ => 0,
            };
            let flyout_left = (menu_index as f32 * MENU_BUTTON_WIDTH)
                .min((viewport.width - flyout_width - OVERLAY_EDGE_GUTTER).max(OVERLAY_MIN_INSET));

            let flyout = Container::new(Column {
                children: items,
                gap: Some(tokens.spacing.none),
                flex_grow: 0.0,
                justify_content: fission::core::op::JustifyContent::Start,
                ..Default::default()
            })
            .width(flyout_width)
            .bg(FLYOUT_BG)
            .border(FLYOUT_BORDER, 1.0)
            .border_radius(tokens.radii.small)
            .into();

            // Dismiss backdrop
            let backdrop = GestureDetector {
                on_tap: Some(dismiss_menu.clone()),
                child: Container::new(Spacer::default())
                    .bg(INTERACTION_BACKDROP)
                    .flex_grow(1.0)
                    .into(),
                ..Default::default()
            }
            .into();

            let overlay = ZStack {
                children: vec![
                    // Full-screen dismiss target
                    Positioned {
                        left: Some(0.0),
                        right: Some(0.0),
                        top: Some(0.0),
                        bottom: Some(0.0),
                        child: Some(backdrop),
                        ..Default::default()
                    }
                    .into(),
                    // The flyout itself, positioned under the menu bar
                    Positioned {
                        left: Some(flyout_left), // offset by activity bar width
                        top: Some(28.0),
                        child: Some(flyout),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into();

            let positioned_root = Positioned {
                left: Some(0.0),
                right: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                child: Some(overlay),
                ..Default::default()
            }
            .into();

            ctx.register_portal_with_layer(
                fission::core::registry::PortalLayer::Modal,
                Some(WidgetId::explicit("menu_bar_flyout")),
                positioned_root,
            );
        }

        bar
    }
}
