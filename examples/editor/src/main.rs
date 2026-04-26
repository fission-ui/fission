use fission_core::op::Color;
use fission_core::ui::{
    Align, Button, ButtonContentAlign, ButtonVariant, Column, Container, GestureDetector, Icon, Node,
    Positioned, Row, Text, TextInput, ZStack,
};
use fission_core::{ActionEnvelope, AppState, BuildCtx, Handler, PortalLayer, View, Widget, WidgetNodeId};
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use fission_widgets::{HStack, Spacer, SplitDirection, SplitView, VStack};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod model;
mod file_tree;
mod editor_render_node;
mod editor_surface;
mod tab_bar;
mod status_bar;
mod terminal_panel;
mod command_palette;
mod syntax;
mod lsp;
mod plugin;
mod search_panel;
mod git_panel;
mod diagnostics_panel;
mod completion_popup;
mod minimap;

use model::*;
use file_tree::FileTree;
use editor_surface::EditorSurface;
use tab_bar::TabBar;
use status_bar::StatusBar;
use search_panel::SearchPanel;
use git_panel::GitPanel;
use terminal_panel::TerminalPanel;
use command_palette::CommandPalette;

// ── Colours ──────────────────────────────────────────────────────────────────

const MENU_BAR_BG: Color = Color { r: 51, g: 51, b: 51, a: 255 };
const SURFACE_BG: Color = Color { r: 37, g: 37, b: 38, a: 255 };
const DARK_BG: Color = Color { r: 30, g: 30, b: 30, a: 255 };
const BORDER_COLOR: Color = Color { r: 48, g: 48, b: 49, a: 255 };
const DIM_TEXT: Color = Color { r: 140, g: 140, b: 140, a: 255 };
const BRIGHT_TEXT: Color = Color { r: 220, g: 220, b: 220, a: 255 };
const FLYOUT_BG: Color = Color { r: 37, g: 37, b: 38, a: 255 };
const FLYOUT_BORDER: Color = Color { r: 60, g: 60, b: 60, a: 255 };
const FIND_BAR_BG: Color = Color { r: 37, g: 37, b: 38, a: 255 };

// ── Activity bar (left icon strip, like VS Code) ─────────────────────────────

struct ActivityBar;

impl Widget<EditorState> for ActivityBar {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let bg = Color { r: 44, g: 44, b: 44, a: 255 };
        let active_color = Color { r: 255, g: 255, b: 255, a: 255 };
        let inactive_color = Color { r: 140, g: 140, b: 140, a: 255 };

        let section_icons = vec![
            (fission_icons::material::action::description::round(), SidebarSection::Explorer, "Explorer"),
            (fission_icons::material::action::search::round(), SidebarSection::Search, "Search"),
            (fission_icons::material::action::commit::round(), SidebarSection::Git, "Source Control"),
            (fission_icons::material::action::extension::round(), SidebarSection::Extensions, "Extensions"),
        ];

        let set_section_id = ctx.bind(
            SetSidebarSection(SidebarSection::Explorer),
            (|s: &mut EditorState, a: SetSidebarSection, _| {
                if s.sidebar_visible && s.sidebar_section == a.0 {
                    s.sidebar_visible = false;
                } else {
                    s.sidebar_section = a.0;
                    s.sidebar_visible = true;
                }
            }) as Handler<EditorState, SetSidebarSection>,
        ).id;

        let mut icons = Vec::new();
        for (icon_svg, section, _label) in &section_icons {
            let is_active = view.state.sidebar_visible && view.state.sidebar_section == *section;
            let color = if is_active { active_color } else { inactive_color };

            let indicator_color = if is_active {
                active_color
            } else {
                Color { r: 0, g: 0, b: 0, a: 0 }
            };

            icons.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(Box::new(
                        Container::new(
                            Align::new(
                                fission_widgets::Icon::svg(*icon_svg)
                                    .size(24.0)
                                    .color(color)
                                    .into_node(),
                            ).into_node(),
                        )
                        .border(indicator_color, 0.0)
                        .into_node(),
                    )),
                    on_press: Some(ActionEnvelope {
                        id: set_section_id,
                        payload: serde_json::to_vec(&SetSidebarSection(*section)).unwrap(),
                    }),
                    width: Some(48.0),
                    height: Some(48.0),
                    padding: Some([0.0; 4]),
                    ..Default::default()
                }
                .into_node(),
            );
        }

        Container::new(
            Column {
                children: icons,
                ..Default::default()
            }
            .into_node(),
        )
        .width(48.0)
        .bg(bg)
        .flex_shrink(0.0)
        .into_node()
    }
}

// ── Menu bar ─────────────────────────────────────────────────────────────────

struct MenuBar;

impl MenuBar {
    /// Build a single top-level menu button.
    fn menu_button(
        label: &str,
        set_menu_id: fission_core::ActionId,
    ) -> Node {
        let label_owned = label.to_string();
        Button {
            variant: ButtonVariant::Ghost,
            child: Some(Box::new(
                Text::new(label)
                    .size(12.0)
                    .color(BRIGHT_TEXT)
                    .into_node(),
            )),
            on_press: Some(ActionEnvelope {
                id: set_menu_id,
                payload: serde_json::to_vec(&SetActiveMenu(Some(label_owned))).unwrap(),
            }),
            height: Some(28.0),
            padding: Some([0.0, 8.0, 0.0, 8.0]),
            ..Default::default()
        }
        .into_node()
    }

    /// Build a single command row inside a dropdown flyout.
    fn flyout_item(label: &str, action: ActionEnvelope) -> Node {
        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(Box::new(
                Text::new(label)
                    .size(12.0)
                    .color(BRIGHT_TEXT)
                    .into_node(),
            )),
            on_press: Some(action),
            height: Some(26.0),
            padding: Some([4.0, 12.0, 4.0, 12.0]),
            ..Default::default()
        }
        .into_node()
    }
}

impl Widget<EditorState> for MenuBar {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        // Handler: set active_menu (toggle logic)
        let set_menu = ctx.bind(
            SetActiveMenu(None),
            (|s: &mut EditorState, a: SetActiveMenu, _| {
                if s.active_menu == a.0 {
                    s.active_menu = None;
                } else {
                    s.active_menu = a.0;
                }
            }) as Handler<EditorState, SetActiveMenu>,
        );
        let set_menu_id = set_menu.id;

        // Action for dismissing menu (click outside)
        let dismiss_menu = ActionEnvelope {
            id: set_menu_id,
            payload: serde_json::to_vec(&SetActiveMenu(None)).unwrap(),
        };

        // ── Shared action handlers for flyout commands ──

        let noop = ctx.bind(
            Noop,
            (|s: &mut EditorState, _, _| {
                s.active_menu = None;
            }) as Handler<EditorState, Noop>,
        );

        let save_file = ctx.bind(
            SaveFile,
            (|s: &mut EditorState, _, _| {
                s.save_active_file();
                s.active_menu = None;
            }) as Handler<EditorState, SaveFile>,
        );

        let save_all = ctx.bind(
            SaveAllFiles,
            (|s: &mut EditorState, _, _| {
                s.save_all_files();
                s.active_menu = None;
            }) as Handler<EditorState, SaveAllFiles>,
        );

        let close_tab_action = ctx.bind(
            CloseTab(0),
            (|s: &mut EditorState, _, _| {
                let idx = s.active_tab;
                s.close_tab(idx);
                s.active_menu = None;
            }) as Handler<EditorState, CloseTab>,
        );

        let toggle_find = ctx.bind(
            ToggleFindReplace,
            (|s: &mut EditorState, _, _| {
                s.show_find_replace = !s.show_find_replace;
                s.active_menu = None;
            }) as Handler<EditorState, ToggleFindReplace>,
        );

        let toggle_sidebar = ctx.bind(
            ToggleSidebar,
            (|s: &mut EditorState, _, _| {
                s.sidebar_visible = !s.sidebar_visible;
                s.active_menu = None;
            }) as Handler<EditorState, ToggleSidebar>,
        );

        let toggle_terminal = ctx.bind(
            ToggleTerminal,
            (|s: &mut EditorState, _, _| {
                s.terminal_visible = !s.terminal_visible;
                s.active_menu = None;
            }) as Handler<EditorState, ToggleTerminal>,
        );

        let cmd_palette = ctx.bind(
            ToggleCommandPalette,
            (|s: &mut EditorState, _, _| {
                s.show_command_palette = !s.show_command_palette;
                s.active_menu = None;
            }) as Handler<EditorState, ToggleCommandPalette>,
        );

        let about_action = ctx.bind(
            Noop,
            (|s: &mut EditorState, _, _| {
                s.status_message = Some("Fission Editor v0.1.0".into());
                s.active_menu = None;
            }) as Handler<EditorState, Noop>,
        );

        let new_file_action = ctx.bind(
            Noop,
            (|s: &mut EditorState, _, _| {
                s.status_message = Some("New File (use file tree context menu)".into());
                s.active_menu = None;
            }) as Handler<EditorState, Noop>,
        );

        let new_folder_action = ctx.bind(
            Noop,
            (|s: &mut EditorState, _, _| {
                s.status_message = Some("New Folder (use file tree context menu)".into());
                s.active_menu = None;
            }) as Handler<EditorState, Noop>,
        );

        let go_to_line_action = ctx.bind(
            ToggleCommandPalette,
            (|s: &mut EditorState, _, _| {
                s.show_command_palette = true;
                s.command_query = "Go to Line:".into();
                s.active_menu = None;
            }) as Handler<EditorState, ToggleCommandPalette>,
        );

        let undo_action = ctx.bind(
            Undo,
            (|s: &mut EditorState, _, _| {
                s.undo_active();
                s.active_menu = None;
            }) as Handler<EditorState, Undo>,
        );

        let redo_action = ctx.bind(
            Redo,
            (|s: &mut EditorState, _, _| {
                s.redo_active();
                s.active_menu = None;
            }) as Handler<EditorState, Redo>,
        );

        let copy_action = ctx.bind(
            CopySelection,
            (|s: &mut EditorState, _, _| {
                s.copy_line();
                s.active_menu = None;
            }) as Handler<EditorState, CopySelection>,
        );

        let cut_action = ctx.bind(
            CutSelection,
            (|s: &mut EditorState, _, _| {
                s.cut_line();
                s.active_menu = None;
            }) as Handler<EditorState, CutSelection>,
        );

        let paste_action = ctx.bind(
            PasteClipboard,
            (|s: &mut EditorState, _, _| {
                s.paste();
                s.active_menu = None;
            }) as Handler<EditorState, PasteClipboard>,
        );

        // ── Top-level buttons ──

        let labels = ["File", "Edit", "View", "Go", "Help"];
        let mut buttons: Vec<Node> = labels.iter()
            .map(|l| Self::menu_button(l, set_menu_id))
            .collect();
        buttons.push(Spacer { flex_grow: 1.0, ..Default::default() }.into_node());

        let bar = Container::new(
            Row {
                children: buttons,
                align_items: fission_ir::op::AlignItems::Center,
                ..Default::default()
            }
            .into_node(),
        )
        .height(28.0)
        .bg(MENU_BAR_BG)
        .flex_shrink(0.0)
        .into_node();

        // ── Flyout dropdown (portal) ──

        if let Some(ref active) = view.state.active_menu {
            let items: Vec<Node> = match active.as_str() {
                "File" => vec![
                    Self::flyout_item("New File", new_file_action.clone()),
                    Self::flyout_item("New Folder", new_folder_action.clone()),
                    Self::flyout_item("Save", save_file.clone()),
                    Self::flyout_item("Save All", save_all.clone()),
                    Self::flyout_item("Close Tab", close_tab_action.clone()),
                ],
                "Edit" => vec![
                    Self::flyout_item("Undo", undo_action.clone()),
                    Self::flyout_item("Redo", redo_action.clone()),
                    Self::flyout_item("Cut", cut_action.clone()),
                    Self::flyout_item("Copy", copy_action.clone()),
                    Self::flyout_item("Paste", paste_action.clone()),
                    Self::flyout_item("Find/Replace", toggle_find.clone()),
                ],
                "View" => vec![
                    Self::flyout_item("Toggle Sidebar", toggle_sidebar.clone()),
                    Self::flyout_item("Toggle Terminal", toggle_terminal.clone()),
                    Self::flyout_item("Command Palette", cmd_palette.clone()),
                ],
                "Go" => vec![
                    Self::flyout_item("Go to Line", go_to_line_action.clone()),
                    Self::flyout_item("Go to Definition", noop.clone()),
                ],
                "Help" => vec![
                    Self::flyout_item("About", about_action.clone()),
                ],
                _ => vec![],
            };

            // Compute left offset based on which menu is active
            let left_px: f32 = match active.as_str() {
                "File" => 0.0,
                "Edit" => 48.0,
                "View" => 96.0,
                "Go" => 144.0,
                "Help" => 180.0,
                _ => 0.0,
            };

            let flyout = Container::new(
                Column {
                    children: items,
                    gap: Some(0.0),
                    flex_grow: 0.0,
                    justify_content: fission_core::op::JustifyContent::Start,
                    ..Default::default()
                }.into_node(),
            )
            .width(200.0)
            .bg(FLYOUT_BG)
            .border(FLYOUT_BORDER, 1.0)
            .border_radius(4.0)
            .into_node();

            // Dismiss backdrop
            let backdrop = GestureDetector {
                on_tap: Some(dismiss_menu.clone()),
                child: Box::new(
                    Container::new(Spacer::default().into_node())
                        .bg(Color { r: 0, g: 0, b: 0, a: 1 }) // Nearly transparent
                        .flex_grow(1.0)
                        .into_node(),
                ),
                ..Default::default()
            }.into_node();

            let overlay = ZStack {
                children: vec![
                    // Full-screen dismiss target
                    Positioned {
                        left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
                        child: Some(Box::new(backdrop)),
                        ..Default::default()
                    }.into_node(),
                    // The flyout itself, positioned under the menu bar
                    Positioned {
                        left: Some(left_px + 48.0), // offset by activity bar width
                        top: Some(28.0),
                        child: Some(Box::new(flyout)),
                        ..Default::default()
                    }.into_node(),
                ],
                ..Default::default()
            }.into_node();

            let positioned_root = Positioned {
                left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
                child: Some(Box::new(overlay)),
                ..Default::default()
            }.into_node();

            ctx.register_portal_with_layer(
                fission_core::registry::PortalLayer::Modal,
                Some(WidgetNodeId::explicit("menu_bar_flyout")),
                positioned_root,
            );
        }

        bar
    }
}

// ── Find / Replace bar ───────────────────────────────────────────────────────

struct FindReplaceBar;

impl Widget<EditorState> for FindReplaceBar {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        if !view.state.show_find_replace {
            return Spacer { height: Some(0.0), ..Default::default() }.into_node();
        }

        let update_find = ctx.bind(
            UpdateFindQuery(String::new()),
            (|s: &mut EditorState, a: UpdateFindQuery, _| {
                s.find_query = a.0;
                s.find_next(); // Auto-search as you type
            }) as Handler<EditorState, UpdateFindQuery>,
        );

        let update_replace = ctx.bind(
            UpdateReplaceQuery(String::new()),
            (|s: &mut EditorState, a: UpdateReplaceQuery, _| s.replace_query = a.0)
                as Handler<EditorState, UpdateReplaceQuery>,
        );

        let close_find = ctx.bind(
            ToggleFindReplace,
            (|s: &mut EditorState, _, _| {
                s.show_find_replace = false;
            }) as Handler<EditorState, ToggleFindReplace>,
        );

        let find_next = ctx.bind(
            FindNext,
            (|s: &mut EditorState, _, _| {
                s.find_next();
            }) as Handler<EditorState, FindNext>,
        );

        let find_prev = ctx.bind(
            FindPrevious,
            (|s: &mut EditorState, _, _| {
                s.find_previous();
            }) as Handler<EditorState, FindPrevious>,
        );

        let replace_one = ctx.bind(
            ReplaceOne,
            (|s: &mut EditorState, _, _| {
                s.replace_one();
            }) as Handler<EditorState, ReplaceOne>,
        );

        let replace_all_action = ctx.bind(
            ReplaceAll,
            (|s: &mut EditorState, _, _| {
                s.replace_all();
            }) as Handler<EditorState, ReplaceAll>,
        );

        // Match count display
        let total = view.state.find_matches.len();
        let current = if total > 0 { view.state.find_match_index + 1 } else { 0 };
        let match_label = if view.state.find_query.is_empty() {
            "No results".to_string()
        } else if total == 0 {
            "No results".to_string()
        } else {
            format!("{} of {}", current, total)
        };

        let find_input = Container::new(
            TextInput {
                id: Some(fission_ir::NodeId::explicit("find_input")),
                value: view.state.find_query.clone(),
                placeholder: Some("Find".into()),
                on_change: Some(update_find),
                ..Default::default()
            }.into_node(),
        )
        .flex_grow(1.0)
        .into_node();

        let replace_input = Container::new(
            TextInput {
                value: view.state.replace_query.clone(),
                placeholder: Some("Replace".into()),
                on_change: Some(update_replace),
                ..Default::default()
            }.into_node(),
        )
        .flex_grow(1.0)
        .into_node();

        let match_text = Text::new(match_label.clone())
            .size(11.0)
            .color(DIM_TEXT)
            .into_node();

        use fission_icons::material;

        let btn_prev = Button {
            variant: ButtonVariant::Ghost,
            child: Some(Box::new(Icon::svg(material::navigation::chevron_left::round()).size(18.0).color(BRIGHT_TEXT).into_node())),
            on_press: Some(find_prev),
            height: Some(24.0),
            width: Some(24.0),
            padding: Some([0.0; 4]),
            ..Default::default()
        }.into_node();

        let btn_next = Button {
            variant: ButtonVariant::Ghost,
            child: Some(Box::new(Icon::svg(material::navigation::chevron_right::round()).size(18.0).color(BRIGHT_TEXT).into_node())),
            on_press: Some(find_next),
            height: Some(24.0),
            width: Some(24.0),
            padding: Some([0.0; 4]),
            ..Default::default()
        }.into_node();

        let btn_replace = Button {
            variant: ButtonVariant::Ghost,
            child: Some(Box::new(Text::new("Replace").size(11.0).color(BRIGHT_TEXT).into_node())),
            on_press: Some(replace_one),
            height: Some(24.0),
            padding: Some([0.0, 6.0, 0.0, 6.0]),
            ..Default::default()
        }.into_node();

        let btn_replace_all = Button {
            variant: ButtonVariant::Ghost,
            child: Some(Box::new(Text::new("Replace All").size(11.0).color(BRIGHT_TEXT).into_node())),
            on_press: Some(replace_all_action),
            height: Some(24.0),
            padding: Some([0.0, 6.0, 0.0, 6.0]),
            ..Default::default()
        }.into_node();

        let btn_close = Button {
            variant: ButtonVariant::Ghost,
            child: Some(Box::new(Icon::svg(material::navigation::close::round()).size(16.0).color(BRIGHT_TEXT).into_node())),
            on_press: Some(close_find),
            height: Some(24.0),
            width: Some(24.0),
            padding: Some([0.0; 4]),
            ..Default::default()
        }.into_node();

        Container::new(
            Row {
                children: vec![
                    Container::new(
                        Row {
                            children: vec![find_input, replace_input],
                            align_items: fission_ir::op::AlignItems::Center,
                            flex_grow: 1.0,
                            ..Default::default()
                        }.into_node(),
                    )
                    .border(FLYOUT_BORDER, 1.0)
                    .border_radius(3.0)
                    .flex_grow(1.0)
                    .into_node(),
                    Container::new(match_text).padding_all(4.0).into_node(),
                    btn_prev,
                    btn_next,
                    btn_replace,
                    btn_replace_all,
                    btn_close,
                ],
                align_items: fission_ir::op::AlignItems::Center,
                ..Default::default()
            }.into_node(),
        )
        .height(32.0)
        .bg(FIND_BAR_BG)
        .padding_all(4.0)
        .flex_shrink(0.0)
        .into_node()
    }
}

// ── Breadcrumb ───────────────────────────────────────────────────────────────

struct Breadcrumb;

impl Widget<EditorState> for Breadcrumb {
    fn build(&self, _ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        // Only shown when a file is open
        if view.state.open_tabs.is_empty() || view.state.breadcrumb_path.is_empty() {
            return Spacer { height: Some(0.0), ..Default::default() }.into_node();
        }

        let segments = &view.state.breadcrumb_path;
        let mut children: Vec<Node> = Vec::new();

        for (i, seg) in segments.iter().enumerate() {
            if i > 0 {
                children.push(
                    Text::new(" > ")
                        .size(11.0)
                        .color(DIM_TEXT)
                        .into_node(),
                );
            }
            children.push(
                Text::new(seg.as_str())
                    .size(11.0)
                    .color(DIM_TEXT)
                    .into_node(),
            );
        }

        Container::new(
            Row {
                children,
                align_items: fission_ir::op::AlignItems::Center,
                ..Default::default()
            }.into_node(),
        )
        .height(22.0)
        .padding_all(4.0)
        .bg(SURFACE_BG)
        .flex_shrink(0.0)
        .into_node()
    }
}

// ── Context menu (portal) ────────────────────────────────────────────────────

struct ContextMenu;

impl ContextMenu {
    fn item(label: &str, action: ActionEnvelope) -> Node {
        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(Box::new(
                Text::new(label)
                    .size(12.0)
                    .color(BRIGHT_TEXT)
                    .into_node(),
            )),
            on_press: Some(action),
            height: Some(26.0),
            padding: Some([4.0, 12.0, 4.0, 12.0]),
            ..Default::default()
        }
        .into_node()
    }
}

impl Widget<EditorState> for ContextMenu {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        if !view.state.context_menu_visible {
            return Spacer { height: Some(0.0), ..Default::default() }.into_node();
        }

        let dismiss = ctx.bind(
            DismissContextMenu,
            (|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
            }) as Handler<EditorState, DismissContextMenu>,
        );

        let noop = ctx.bind(
            Noop,
            (|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
                s.status_message = Some("Action (placeholder)".into());
            }) as Handler<EditorState, Noop>,
        );

        let toggle_find = ctx.bind(
            ToggleFindReplace,
            (|s: &mut EditorState, _, _| {
                s.show_find_replace = true;
                s.context_menu_visible = false;
            }) as Handler<EditorState, ToggleFindReplace>,
        );

        let new_file_ctx = ctx.bind(
            Noop,
            (|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
                s.status_message = Some("New File (placeholder)".into());
            }) as Handler<EditorState, Noop>,
        );

        let new_folder_ctx = ctx.bind(
            Noop,
            (|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
                s.status_message = Some("New Folder (placeholder)".into());
            }) as Handler<EditorState, Noop>,
        );

        let rename_action = ctx.bind(
            Noop,
            (|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
                if let Some(target) = s.context_menu_target.clone() {
                    s.start_rename(target);
                } else {
                    s.status_message = Some("Nothing selected to rename".into());
                }
            }) as Handler<EditorState, Noop>,
        );

        let delete_action = ctx.bind(
            Noop,
            (|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
                if let Some(target) = s.context_menu_target.clone() {
                    let path = std::path::Path::new(&target);
                    let result = if path.is_dir() {
                        std::fs::remove_dir_all(&target)
                    } else {
                        std::fs::remove_file(&target)
                    };
                    match result {
                        Ok(()) => {
                            s.tree_cache_dirty = true;
                            s.status_message = Some(format!("Deleted '{}'", target));
                        }
                        Err(e) => {
                            s.status_message = Some(format!("Delete failed: {}", e));
                        }
                    }
                }
            }) as Handler<EditorState, Noop>,
        );

        let go_to_def = ctx.bind(
            GoToDefinition,
            (|s: &mut EditorState, _, _| {
                s.context_menu_visible = false;
                s.status_message = Some("Go to Definition (placeholder)".into());
            }) as Handler<EditorState, GoToDefinition>,
        );

        let ctx_undo = ctx.bind(
            Undo,
            (|s: &mut EditorState, _, _| {
                s.undo_active();
                s.context_menu_visible = false;
            }) as Handler<EditorState, Undo>,
        );

        let ctx_redo = ctx.bind(
            Redo,
            (|s: &mut EditorState, _, _| {
                s.redo_active();
                s.context_menu_visible = false;
            }) as Handler<EditorState, Redo>,
        );

        let ctx_copy = ctx.bind(
            CopySelection,
            (|s: &mut EditorState, _, _| {
                s.copy_line();
                s.context_menu_visible = false;
            }) as Handler<EditorState, CopySelection>,
        );

        let ctx_cut = ctx.bind(
            CutSelection,
            (|s: &mut EditorState, _, _| {
                s.cut_line();
                s.context_menu_visible = false;
            }) as Handler<EditorState, CutSelection>,
        );

        let ctx_paste = ctx.bind(
            PasteClipboard,
            (|s: &mut EditorState, _, _| {
                s.paste();
                s.context_menu_visible = false;
            }) as Handler<EditorState, PasteClipboard>,
        );

        let items: Vec<Node> = if view.state.context_menu_target.is_some() {
            // File tree context menu
            vec![
                Self::item("New File", new_file_ctx.clone()),
                Self::item("New Folder", new_folder_ctx.clone()),
                Self::item("Rename", rename_action.clone()),
                Self::item("Delete", delete_action.clone()),
            ]
        } else {
            // Editor context menu
            vec![
                Self::item("Undo", ctx_undo.clone()),
                Self::item("Redo", ctx_redo.clone()),
                Self::item("Copy", ctx_copy.clone()),
                Self::item("Cut", ctx_cut.clone()),
                Self::item("Paste", ctx_paste.clone()),
                Self::item("Find/Replace", toggle_find.clone()),
                Self::item("Go to Definition", go_to_def.clone()),
            ]
        };

        let (cx, cy) = view.state.context_menu_position;

        let card = Container::new(
            VStack {
                spacing: Some(0.0),
                children: items,
            }.into_node(),
        )
        .width(180.0)
        .bg(FLYOUT_BG)
        .border(FLYOUT_BORDER, 1.0)
        .border_radius(4.0)
        .into_node();

        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child: Box::new(
                Container::new(Spacer::default().into_node())
                    .bg(Color { r: 0, g: 0, b: 0, a: 1 })
                    .flex_grow(1.0)
                    .into_node(),
            ),
            ..Default::default()
        }.into_node();

        let overlay = ZStack {
            children: vec![
                Positioned {
                    left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
                    child: Some(Box::new(backdrop)),
                    ..Default::default()
                }.into_node(),
                Positioned {
                    left: Some(cx),
                    top: Some(cy),
                    child: Some(Box::new(card)),
                    ..Default::default()
                }.into_node(),
            ],
            ..Default::default()
        }.into_node();

        let positioned_root = Positioned {
            left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
            child: Some(Box::new(overlay)),
            ..Default::default()
        }.into_node();

        ctx.register_portal_with_layer(
            PortalLayer::Flyout,
            Some(WidgetNodeId::explicit("context_menu")),
            positioned_root,
        );

        Spacer { height: Some(0.0), ..Default::default() }.into_node()
    }
}

// ── Main app ─────────────────────────────────────────────────────────────────

struct EditorApp;

impl Widget<EditorState> for EditorApp {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        // ── Menu bar (topmost) ──
        let menu_bar = MenuBar.build(ctx, view);

        // ── Activity bar (leftmost strip) ──
        let activity_bar = ActivityBar.build(ctx, view);

        // ── Sidebar (content depends on active section) ──
        let sidebar = if view.state.sidebar_visible {
            let (header_text, panel_content) = match view.state.sidebar_section {
                SidebarSection::Explorer => ("EXPLORER", FileTree.build(ctx, view)),
                SidebarSection::Search => ("SEARCH", SearchPanel.build(ctx, view)),
                SidebarSection::Git => ("SOURCE CONTROL", GitPanel.build(ctx, view)),
                SidebarSection::Extensions => ("EXTENSIONS", Container::new(
                    Text::new("No extensions installed")
                        .size(12.0)
                        .color(DIM_TEXT)
                        .into_node(),
                ).padding_all(8.0).flex_grow(1.0).into_node()),
            };

            let header = Container::new(
                Text::new(header_text)
                    .size(11.0)
                    .color(Color { r: 187, g: 187, b: 187, a: 255 })
                    .into_node(),
            )
            .bg(SURFACE_BG)
            .height(28.0)
            .padding_all(8.0)
            .flex_shrink(0.0)
            .into_node();

            Container::new(
                Column {
                    children: vec![header, panel_content],
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
            )
            .width(view.state.sidebar_width)
            .bg(SURFACE_BG)
            .flex_shrink(0.0)
            .into_node()
        } else {
            Spacer { width: Some(0.0), ..Default::default() }.into_node()
        };

        // 1px vertical divider between sidebar and editor
        let sidebar_divider = if view.state.sidebar_visible {
            Container::new(Spacer::default().into_node())
                .width(1.0)
                .bg(BORDER_COLOR)
                .flex_shrink(0.0)
                .into_node()
        } else {
            Spacer { width: Some(0.0), ..Default::default() }.into_node()
        };

        // ── Editor area: tabs + breadcrumb + find/replace + surface ──
        let tab_bar_node = TabBar.build(ctx, view);
        let breadcrumb_node = Breadcrumb.build(ctx, view);
        let find_replace_node = FindReplaceBar.build(ctx, view);
        let editor_surface_node = EditorSurface.build(ctx, view);

        let editor_area = Column {
            children: vec![
                tab_bar_node,
                breadcrumb_node,
                find_replace_node,
                editor_surface_node,
            ],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        // 1px horizontal divider above terminal
        let terminal_divider = Container::new(Spacer::default().into_node())
            .height(1.0)
            .bg(BORDER_COLOR)
            .flex_shrink(0.0)
            .into_node();

        // Center: editor area + terminal
        let center = Column {
            children: if view.state.terminal_visible {
                vec![
                    Container::new(editor_area).flex_grow(1.0).into_node(),
                    terminal_divider,
                    TerminalPanel.build(ctx, view),
                ]
            } else {
                vec![Container::new(editor_area).flex_grow(1.0).into_node()]
            },
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        // Main layout: activity bar | sidebar | divider | center
        let main_layout = Row {
            children: vec![activity_bar, sidebar, sidebar_divider, center],
            align_items: fission_ir::op::AlignItems::Stretch,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        // Root: menu bar + main + status bar
        let root = Column {
            children: vec![
                menu_bar,
                Container::new(main_layout).flex_grow(1.0).into_node(),
                StatusBar.build(ctx, view),
            ],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        // ── Overlays (portals) ──
        CommandPalette.build(ctx, view);
        ContextMenu.build(ctx, view);
        completion_popup::CompletionPopup.build(ctx, view);

        root
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let root_for_init = root.clone();
    let root_for_sync = root.clone();
    let app = DesktopApp::new(EditorApp)
        .with_title("Fission Editor")
        .with_state_init(move |state: &mut EditorState| {
            state.root_path = root_for_init.clone();
            state.tree_cache_dirty = true;
            state.refresh_git_status(); // Initial git status scan
        })
        .with_sync_env(move |_state: &EditorState, env: &mut fission_core::Env| {
            env.theme = fission_theme::Theme::dark();
        })
        // Poll LSP for diagnostics/completions on every frame so results
        // appear even when the user is not typing.
        .with_frame_hook({
            let is_test_mode = std::env::var("FISSION_TEST_CONTROL_PORT").is_ok();
            let last_lsp_poll = std::sync::Mutex::new(std::time::Instant::now());
            move |state: &mut EditorState| -> bool {
                let mut changed = false;

                // ── Poll background git-status result ──
                if let Ok(mut guard) = state.git_status_pending.try_lock() {
                    if let Some(entries) = guard.take() {
                        state.git_status_lines = entries;
                        changed = true;
                    }
                }

                // ── Kick off / poll background tree scan ──
                if state.tree_cache_dirty {
                    // Only spawn if no scan is already in flight
                    let can_spawn = state
                        .tree_scan_pending
                        .try_lock()
                        .map(|g| g.is_none())
                        .unwrap_or(false);
                    if can_spawn {
                        state.tree_cache_dirty = false;
                        let root = state.root_path.clone();
                        let pending = state.tree_scan_pending.clone();
                        std::thread::spawn(move || {
                            let entries = crate::model::scan_directory(&root, 0);
                            if let Ok(mut guard) = pending.lock() {
                                *guard = Some(entries);
                            }
                        });
                    }
                }
                if let Ok(mut guard) = state.tree_scan_pending.try_lock() {
                    if let Some(entries) = guard.take() {
                        state.cached_tree_entries = entries;
                        changed = true;
                    }
                }

                // Skip LSP entirely in test mode
                if is_test_mode {
                    return changed;
                }

                // Lazily initialize LSP on first frame
                if !state.lsp_initialized {
                    state.lsp_initialized = true;
                    state.lsp_handle = Some(LspHandle::new(&state.root_path));
                }

                // Throttle LSP polling to at most once per second to avoid
                // hot-looping when rust-analyzer is streaming diagnostics
                let now = std::time::Instant::now();
                if let Ok(mut last) = last_lsp_poll.lock() {
                    if now.duration_since(*last).as_millis() < 1000 {
                        return changed;
                    }
                    *last = now;
                }

                if let Some(ref handle) = state.lsp_handle {
                    let (diags, completions) = handle.poll_diagnostics();
                    if !diags.is_empty() {
                        for (path, file_diags) in diags {
                            state.diagnostics.insert(path, file_diags);
                        }
                        changed = true;
                    }
                    if !completions.is_empty() {
                        state.completions = completions;
                        state.show_completions = true;
                        state.selected_completion = 0;
                        changed = true;
                    }
                }
                changed
            }
        })
        .with_key_handler(move |state: &mut EditorState, key: &fission_core::KeyCode, mods: u8| -> bool {
            // Initialize root path on first call
            if state.root_path == PathBuf::from(".") {
                state.root_path = root_for_sync.clone();
            }

            // Tree scanning and external-change checking are handled
            // asynchronously in the frame hook -- no I/O in the key handler.

            let ctrl = (mods & 4) != 0 || (mods & 8) != 0; // Ctrl or Cmd
            let shift = (mods & 1) != 0;

            // Dismiss context menu on any keystroke (except Escape which handles it explicitly)
            if !matches!(key, fission_core::KeyCode::Escape) {
                state.context_menu_visible = false;
            }

            // Enter confirms rename if one is in progress
            if matches!(key, fission_core::KeyCode::Enter) && !ctrl {
                if state.renaming_path.is_some() {
                    state.confirm_rename();
                    return true;
                }
                if state.terminal_visible && !state.terminal_input.is_empty() {
                    state.run_terminal_command();
                    return true;
                }
                return false;
            }

            // Escape dismisses menus / context menus / find bar / command palette / rename
            if matches!(key, fission_core::KeyCode::Escape) {
                let mut handled = false;
                if state.renaming_path.is_some() {
                    state.cancel_rename();
                    handled = true;
                }
                if state.active_menu.is_some() {
                    state.active_menu = None;
                    handled = true;
                }
                if state.context_menu_visible {
                    state.context_menu_visible = false;
                    handled = true;
                }
                if state.show_find_replace {
                    state.show_find_replace = false;
                    handled = true;
                }
                if state.show_command_palette {
                    state.show_command_palette = false;
                    state.command_query.clear();
                    handled = true;
                }
                return handled;
            }

            if !ctrl { return false; }

            match key {
                fission_core::KeyCode::Char('s') | fission_core::KeyCode::Char('S') => {
                    if shift {
                        state.save_all_files();
                    } else {
                        state.save_active_file();
                    }
                    true
                }
                fission_core::KeyCode::Char('p') | fission_core::KeyCode::Char('P') if shift => {
                    state.show_command_palette = !state.show_command_palette;
                    if !state.show_command_palette {
                        state.command_query.clear();
                    }
                    true
                }
                fission_core::KeyCode::Char('b') | fission_core::KeyCode::Char('B') => {
                    state.sidebar_visible = !state.sidebar_visible;
                    true
                }
                fission_core::KeyCode::Char('`') => {
                    state.terminal_visible = !state.terminal_visible;
                    true
                }
                // Ctrl+F: toggle find/replace
                fission_core::KeyCode::Char('f') | fission_core::KeyCode::Char('F') => {
                    state.context_menu_visible = false;
                    state.show_find_replace = !state.show_find_replace;
                    true
                }
                // Ctrl+G: go to line (toggle command palette with prompt)
                fission_core::KeyCode::Char('g') | fission_core::KeyCode::Char('G') => {
                    state.show_command_palette = !state.show_command_palette;
                    if state.show_command_palette {
                        state.command_query = "Go to Line:".into();
                    } else {
                        state.command_query.clear();
                    }
                    true
                }
                // Ctrl+W: close active tab
                fission_core::KeyCode::Char('w') | fission_core::KeyCode::Char('W') => {
                    let idx = state.active_tab;
                    state.close_tab(idx);
                    true
                }
                // Ctrl+Z: undo, Ctrl+Shift+Z: redo
                fission_core::KeyCode::Char('z') | fission_core::KeyCode::Char('Z') => {
                    if shift {
                        state.redo_active();
                    } else {
                        state.undo_active();
                    }
                    true
                }
                // Ctrl+Y: redo (alternative)
                fission_core::KeyCode::Char('y') | fission_core::KeyCode::Char('Y') => {
                    state.redo_active();
                    true
                }
                // Ctrl+C: copy current line
                fission_core::KeyCode::Char('c') | fission_core::KeyCode::Char('C') => {
                    state.copy_line();
                    true
                }
                // Ctrl+X: cut current line
                fission_core::KeyCode::Char('x') | fission_core::KeyCode::Char('X') => {
                    state.cut_line();
                    true
                }
                // Ctrl+V: paste clipboard
                fission_core::KeyCode::Char('v') | fission_core::KeyCode::Char('V') => {
                    state.paste();
                    true
                }
                _ => false,
            }
        });

    app.run()
}
