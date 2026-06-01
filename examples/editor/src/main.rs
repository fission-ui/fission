use fission::core::op::Color;
use fission::core::ui::{
    Align, Button, ButtonContentAlign, ButtonVariant, Column, Container, GestureDetector, Icon,
    Node, Positioned, Row, Text, TextInput, ZStack,
};
use fission::core::{
    reduce_with, ActionEnvelope, BuildCtx, JobResource, PortalLayer, ReducerContext, ResourceKey,
    TimerResource, View, Widget, WidgetNodeId,
};
use fission::prelude::DesktopApp;
use fission::widgets::{Spacer, VStack};
use std::path::PathBuf;
use std::time::Duration;

mod command_palette;
mod completion_popup;
mod diagnostics_panel;
mod editor_render_node;
mod editor_surface;
mod file_tree;
mod git_panel;
mod lsp;
mod minimap;
mod model;
mod plugin;
mod search_panel;
mod status_bar;
mod syntax;
mod tab_bar;
mod terminal_panel;

use command_palette::CommandPalette;
use editor_surface::EditorSurface;
use file_tree::FileTree;
use git_panel::GitPanel;
use model::*;
use search_panel::SearchPanel;
use status_bar::StatusBar;
use tab_bar::TabBar;
use terminal_panel::TerminalPanel;

// ── Colours ──────────────────────────────────────────────────────────────────

const MENU_BAR_BG: Color = Color {
    r: 51,
    g: 51,
    b: 51,
    a: 255,
};
const SURFACE_BG: Color = Color {
    r: 37,
    g: 37,
    b: 38,
    a: 255,
};
const BORDER_COLOR: Color = Color {
    r: 48,
    g: 48,
    b: 49,
    a: 255,
};
const DIM_TEXT: Color = Color {
    r: 140,
    g: 140,
    b: 140,
    a: 255,
};
const BRIGHT_TEXT: Color = Color {
    r: 220,
    g: 220,
    b: 220,
    a: 255,
};
const FLYOUT_BG: Color = Color {
    r: 37,
    g: 37,
    b: 38,
    a: 255,
};
const FLYOUT_BORDER: Color = Color {
    r: 60,
    g: 60,
    b: 60,
    a: 255,
};
const FIND_BAR_BG: Color = Color {
    r: 37,
    g: 37,
    b: 38,
    a: 255,
};

// ── Activity bar (left icon strip, like VS Code) ─────────────────────────────

struct ActivityBar;

impl Widget<EditorState> for ActivityBar {
    fn build(
        &self,
        ctx: &mut BuildCtx<EditorState>,
        view: &View<EditorState>,
    ) -> impl fission::IntoWidget<EditorState> {
        fission::AnyWidget::from_node({
            let bg = Color {
                r: 44,
                g: 44,
                b: 44,
                a: 255,
            };
            let active_color = Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            };
            let inactive_color = Color {
                r: 140,
                g: 140,
                b: 140,
                a: 255,
            };

            let section_icons = vec![
                (
                    fission::icons::material::action::description::round(),
                    SidebarSection::Explorer,
                    "Explorer",
                ),
                (
                    fission::icons::material::action::search::round(),
                    SidebarSection::Search,
                    "Search",
                ),
                (
                    fission::icons::material::action::commit::round(),
                    SidebarSection::Git,
                    "Source Control",
                ),
                (
                    fission::icons::material::action::extension::round(),
                    SidebarSection::Extensions,
                    "Extensions",
                ),
            ];

            let set_section_id = ctx
                .bind(
                    SetSidebarSection(SidebarSection::Explorer),
                    reduce_with!(
                        (|s: &mut EditorState, a: SetSidebarSection, _| {
                            if s.sidebar_visible && s.sidebar_section == a.0 {
                                s.sidebar_visible = false;
                            } else {
                                s.sidebar_section = a.0;
                                s.sidebar_visible = true;
                            }
                        })
                    ),
                )
                .id;

            let mut icons = Vec::new();
            for (icon_svg, section, _label) in &section_icons {
                let is_active =
                    view.state.sidebar_visible && view.state.sidebar_section == *section;
                let color = if is_active {
                    active_color
                } else {
                    inactive_color
                };

                let indicator_color = if is_active {
                    active_color
                } else {
                    Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    }
                };

                icons.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(
                            Container::new(
                                Align::new(
                                    fission::widgets::Icon::svg(*icon_svg)
                                        .size(24.0)
                                        .color(color)
                                        .into_node(),
                                )
                                .into_node(),
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
        })
    }
}

// ── Menu bar ─────────────────────────────────────────────────────────────────

struct MenuBar;

impl MenuBar {
    /// Build a single top-level menu button.
    fn menu_button(label: &str, set_menu_id: fission::core::ActionId) -> Node {
        let label_owned = label.to_string();
        Button {
            variant: ButtonVariant::Ghost,
            child: Some(Box::new(
                Text::new(label).size(12.0).color(BRIGHT_TEXT).into_node(),
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
                Text::new(label).size(12.0).color(BRIGHT_TEXT).into_node(),
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
    fn build(
        &self,
        ctx: &mut BuildCtx<EditorState>,
        view: &View<EditorState>,
    ) -> impl fission::IntoWidget<EditorState> {
        fission::AnyWidget::from_node({
            let viewport = view.viewport_size();
            let flyout_width = (viewport.width - 80.0).clamp(180.0, 240.0);

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
            let mut buttons: Vec<Node> = labels
                .iter()
                .map(|l| Self::menu_button(l, set_menu_id))
                .collect();
            buttons.push(
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
            );

            let bar = Container::new(
                Row {
                    children: buttons,
                    align_items: fission::ir::op::AlignItems::Center,
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
                        Self::flyout_item("Go to Definition", go_to_def_action.clone()),
                    ],
                    "Help" => vec![Self::flyout_item("About", about_action.clone())],
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
                let flyout_left =
                    (left_px + 48.0).min((viewport.width - flyout_width - 16.0).max(8.0));

                let flyout = Container::new(
                    Column {
                        children: items,
                        gap: Some(0.0),
                        flex_grow: 0.0,
                        justify_content: fission::core::op::JustifyContent::Start,
                        ..Default::default()
                    }
                    .into_node(),
                )
                .width(flyout_width)
                .bg(FLYOUT_BG)
                .border(FLYOUT_BORDER, 1.0)
                .border_radius(4.0)
                .into_node();

                // Dismiss backdrop
                let backdrop = GestureDetector {
                    on_tap: Some(dismiss_menu.clone()),
                    child: Box::new(
                        Container::new(Spacer::default().into_node())
                            .bg(Color {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 1,
                            }) // Nearly transparent
                            .flex_grow(1.0)
                            .into_node(),
                    ),
                    ..Default::default()
                }
                .into_node();

                let overlay = ZStack {
                    children: vec![
                        // Full-screen dismiss target
                        Positioned {
                            left: Some(0.0),
                            right: Some(0.0),
                            top: Some(0.0),
                            bottom: Some(0.0),
                            child: Some(Box::new(backdrop)),
                            ..Default::default()
                        }
                        .into_node(),
                        // The flyout itself, positioned under the menu bar
                        Positioned {
                            left: Some(flyout_left), // offset by activity bar width
                            top: Some(28.0),
                            child: Some(Box::new(flyout)),
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                    ..Default::default()
                }
                .into_node();

                let positioned_root = Positioned {
                    left: Some(0.0),
                    right: Some(0.0),
                    top: Some(0.0),
                    bottom: Some(0.0),
                    child: Some(Box::new(overlay)),
                    ..Default::default()
                }
                .into_node();

                ctx.register_portal_with_layer(
                    fission::core::registry::PortalLayer::Modal,
                    Some(WidgetNodeId::explicit("menu_bar_flyout")),
                    positioned_root,
                );
            }

            bar
        })
    }
}

// ── Find / Replace bar ───────────────────────────────────────────────────────

struct FindReplaceBar;

impl Widget<EditorState> for FindReplaceBar {
    fn build(
        &self,
        ctx: &mut BuildCtx<EditorState>,
        view: &View<EditorState>,
    ) -> impl fission::IntoWidget<EditorState> {
        fission::AnyWidget::from_node({
            if !view.state.show_find_replace {
                return fission::AnyWidget::from_node(
                    Spacer {
                        height: Some(0.0),
                        ..Default::default()
                    }
                    .into_node(),
                );
            }

            let update_find = ctx.bind(
                UpdateFindQuery(String::new()),
                reduce_with!(
                    (|s: &mut EditorState, a: UpdateFindQuery, _| {
                        s.find_query = a.0;
                        s.find_next(); // Auto-search as you type
                    })
                ),
            );

            let update_replace = ctx.bind(
                UpdateReplaceQuery(String::new()),
                reduce_with!(
                    (|s: &mut EditorState, a: UpdateReplaceQuery, _| s.replace_query = a.0)
                ),
            );

            let close_find = ctx.bind(
                ToggleFindReplace,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.show_find_replace = false;
                    })
                ),
            );

            let find_next = ctx.bind(
                FindNext,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.find_next();
                    })
                ),
            );

            let find_prev = ctx.bind(
                FindPrevious,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.find_previous();
                    })
                ),
            );

            let replace_one = ctx.bind(
                ReplaceOne,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.replace_one();
                    })
                ),
            );

            let replace_all_action = ctx.bind(
                ReplaceAll,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.replace_all();
                    })
                ),
            );

            // Match count display
            let total = view.state.find_matches.len();
            let current = if total > 0 {
                view.state.find_match_index + 1
            } else {
                0
            };
            let match_label = if view.state.find_query.is_empty() {
                "No results".to_string()
            } else if total == 0 {
                "No results".to_string()
            } else {
                format!("{} of {}", current, total)
            };

            let find_input = Container::new(
                TextInput {
                    id: Some(fission::ir::NodeId::explicit("find_input")),
                    value: view.state.find_query.clone(),
                    placeholder: Some("Find".into()),
                    on_change: Some(update_find),
                    ..Default::default()
                }
                .into_node(),
            )
            .flex_grow(1.0)
            .into_node();

            let replace_input = Container::new(
                TextInput {
                    id: Some(fission::ir::NodeId::explicit("replace_input")),
                    value: view.state.replace_query.clone(),
                    placeholder: Some("Replace".into()),
                    on_change: Some(update_replace),
                    ..Default::default()
                }
                .into_node(),
            )
            .flex_grow(1.0)
            .into_node();

            let match_text = Text::new(match_label.clone())
                .size(11.0)
                .color(DIM_TEXT)
                .into_node();

            use fission::icons::material;

            let btn_prev = Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    Icon::svg(material::navigation::chevron_left::round())
                        .size(18.0)
                        .color(BRIGHT_TEXT)
                        .into_node(),
                )),
                on_press: Some(find_prev),
                height: Some(24.0),
                width: Some(24.0),
                padding: Some([0.0; 4]),
                ..Default::default()
            }
            .into_node();

            let btn_next = Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    Icon::svg(material::navigation::chevron_right::round())
                        .size(18.0)
                        .color(BRIGHT_TEXT)
                        .into_node(),
                )),
                on_press: Some(find_next),
                height: Some(24.0),
                width: Some(24.0),
                padding: Some([0.0; 4]),
                ..Default::default()
            }
            .into_node();

            let btn_replace = Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    Text::new("Replace")
                        .size(11.0)
                        .color(BRIGHT_TEXT)
                        .into_node(),
                )),
                on_press: Some(replace_one),
                height: Some(24.0),
                padding: Some([0.0, 6.0, 0.0, 6.0]),
                ..Default::default()
            }
            .into_node();

            let btn_replace_all = Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    Text::new("Replace All")
                        .size(11.0)
                        .color(BRIGHT_TEXT)
                        .into_node(),
                )),
                on_press: Some(replace_all_action),
                height: Some(24.0),
                padding: Some([0.0, 6.0, 0.0, 6.0]),
                ..Default::default()
            }
            .into_node();

            let btn_close = Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    Icon::svg(material::navigation::close::round())
                        .size(16.0)
                        .color(BRIGHT_TEXT)
                        .into_node(),
                )),
                on_press: Some(close_find),
                height: Some(24.0),
                width: Some(24.0),
                padding: Some([0.0; 4]),
                ..Default::default()
            }
            .into_node();

            Container::new(
                Row {
                    children: vec![
                        Container::new(
                            Row {
                                children: vec![find_input, replace_input],
                                align_items: fission::ir::op::AlignItems::Center,
                                flex_grow: 1.0,
                                ..Default::default()
                            }
                            .into_node(),
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
                    align_items: fission::ir::op::AlignItems::Center,
                    ..Default::default()
                }
                .into_node(),
            )
            .height(32.0)
            .bg(FIND_BAR_BG)
            .padding_all(4.0)
            .flex_shrink(0.0)
            .into_node()
        })
    }
}

// ── Breadcrumb ───────────────────────────────────────────────────────────────

struct Breadcrumb;

impl Widget<EditorState> for Breadcrumb {
    fn build(
        &self,
        _ctx: &mut BuildCtx<EditorState>,
        view: &View<EditorState>,
    ) -> impl fission::IntoWidget<EditorState> {
        fission::AnyWidget::from_node({
            // Only shown when a file is open
            if view.state.open_tabs.is_empty() || view.state.breadcrumb_path.is_empty() {
                return fission::AnyWidget::from_node(
                    Spacer {
                        height: Some(0.0),
                        ..Default::default()
                    }
                    .into_node(),
                );
            }

            let segments = &view.state.breadcrumb_path;
            let mut children: Vec<Node> = Vec::new();

            for (i, seg) in segments.iter().enumerate() {
                if i > 0 {
                    children.push(Text::new(" > ").size(11.0).color(DIM_TEXT).into_node());
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
                    align_items: fission::ir::op::AlignItems::Center,
                    ..Default::default()
                }
                .into_node(),
            )
            .height(22.0)
            .padding_all(4.0)
            .bg(SURFACE_BG)
            .flex_shrink(0.0)
            .into_node()
        })
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
                Text::new(label).size(12.0).color(BRIGHT_TEXT).into_node(),
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
    fn build(
        &self,
        ctx: &mut BuildCtx<EditorState>,
        view: &View<EditorState>,
    ) -> impl fission::IntoWidget<EditorState> {
        fission::AnyWidget::from_node({
            if !view.state.context_menu_visible {
                return fission::AnyWidget::from_node(
                    Spacer {
                        height: Some(0.0),
                        ..Default::default()
                    }
                    .into_node(),
                );
            }

            let dismiss = ctx.bind(
                DismissContextMenu,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.context_menu_visible = false;
                    })
                ),
            );

            let toggle_find = ctx.bind(
                ToggleFindReplace,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.show_find_replace = true;
                        s.context_menu_visible = false;
                    })
                ),
            );

            let new_file_ctx = ctx.bind(
                CreateFile(String::new()),
                reduce_with!(
                    (|s: &mut EditorState, _: CreateFile, _| {
                        s.context_menu_visible = false;
                        if let Some(target) = s.context_menu_target.clone() {
                            let dir = if std::path::Path::new(&target).is_dir() {
                                target
                            } else {
                                std::path::Path::new(&target)
                                    .parent()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|| s.root_path.to_string_lossy().to_string())
                            };
                            s.create_file(format!("{}/untitled.rs", dir));
                        }
                        s.context_menu_target = None;
                    })
                ),
            );

            let new_folder_ctx = ctx.bind(
                CreateFolder(String::new()),
                reduce_with!(
                    (|s: &mut EditorState, _: CreateFolder, _| {
                        s.context_menu_visible = false;
                        if let Some(target) = s.context_menu_target.clone() {
                            let dir = if std::path::Path::new(&target).is_dir() {
                                target
                            } else {
                                std::path::Path::new(&target)
                                    .parent()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|| s.root_path.to_string_lossy().to_string())
                            };
                            s.create_folder(format!("{}/new_folder", dir));
                        }
                        s.context_menu_target = None;
                    })
                ),
            );

            let rename_action = ctx.bind(
                RenameContextTarget,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.context_menu_visible = false;
                        if let Some(target) = s.context_menu_target.clone() {
                            s.start_rename(target);
                        } else {
                            s.status_message = Some("Nothing selected to rename".into());
                        }
                    })
                ),
            );

            let delete_action = ctx.bind(
                DeleteContextTarget,
                reduce_with!(
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
                                    s.request_tree_refresh();
                                    s.status_message = Some(format!("Deleted '{}'", target));
                                }
                                Err(e) => {
                                    s.status_message = Some(format!("Delete failed: {}", e));
                                }
                            }
                        }
                    })
                ),
            );

            let go_to_def = ctx.bind(
                GoToDefinition,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.context_menu_visible = false;
                        s.status_message = Some("Go to Definition (placeholder)".into());
                    })
                ),
            );

            let ctx_undo = ctx.bind(
                Undo,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.undo_active();
                        s.context_menu_visible = false;
                    })
                ),
            );

            let ctx_redo = ctx.bind(
                Redo,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.redo_active();
                        s.context_menu_visible = false;
                    })
                ),
            );

            let ctx_copy = ctx.bind(
                CopySelection,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.copy_line();
                        s.context_menu_visible = false;
                    })
                ),
            );

            let ctx_cut = ctx.bind(
                CutSelection,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.cut_line();
                        s.context_menu_visible = false;
                    })
                ),
            );

            let ctx_paste = ctx.bind(
                PasteClipboard,
                reduce_with!(
                    (|s: &mut EditorState, _, _| {
                        s.paste();
                        s.context_menu_visible = false;
                    })
                ),
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
            let viewport = view.viewport_size();
            let card_width = (viewport.width - 80.0).clamp(160.0, 220.0);
            let clamped_left = cx.min((viewport.width - card_width - 16.0).max(8.0));
            let clamped_top = cy.min((viewport.height - 220.0).max(8.0));

            let card = Container::new(
                VStack {
                    spacing: Some(0.0),
                    children: items,
                }
                .into_node(),
            )
            .width(card_width)
            .bg(FLYOUT_BG)
            .border(FLYOUT_BORDER, 1.0)
            .border_radius(4.0)
            .into_node();

            let backdrop = GestureDetector {
                on_tap: Some(dismiss.clone()),
                child: Box::new(
                    Container::new(Spacer::default().into_node())
                        .bg(Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 1,
                        })
                        .flex_grow(1.0)
                        .into_node(),
                ),
                ..Default::default()
            }
            .into_node();

            let overlay = ZStack {
                children: vec![
                    Positioned {
                        left: Some(0.0),
                        right: Some(0.0),
                        top: Some(0.0),
                        bottom: Some(0.0),
                        child: Some(Box::new(backdrop)),
                        ..Default::default()
                    }
                    .into_node(),
                    Positioned {
                        left: Some(clamped_left),
                        top: Some(clamped_top),
                        child: Some(Box::new(card)),
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node();

            let positioned_root = Positioned {
                left: Some(0.0),
                right: Some(0.0),
                top: Some(0.0),
                bottom: Some(0.0),
                child: Some(Box::new(overlay)),
                ..Default::default()
            }
            .into_node();

            ctx.register_portal_with_layer(
                PortalLayer::Flyout,
                Some(WidgetNodeId::explicit("context_menu")),
                positioned_root,
            );

            Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into_node()
        })
    }
}

// ── Main app ─────────────────────────────────────────────────────────────────

struct EditorApp;

impl Widget<EditorState> for EditorApp {
    fn build(
        &self,
        ctx: &mut BuildCtx<EditorState>,
        view: &View<EditorState>,
    ) -> impl fission::IntoWidget<EditorState> {
        fission::AnyWidget::from_node({
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
                            state.status_message =
                                Some(format!("Tree refresh failed: {}", message));
                        }
                    })
                ),
            );
            if view.state.tree_scan_pending() {
                ctx.resources.job(
                    JobResource::new(
                        ResourceKey::new("editor-tree-scan"),
                        TREE_SCAN_JOB,
                        TreeScanRequest {
                            root_path: view.state.root_path.clone(),
                            generation: view.state.tree_scan_generation,
                        },
                    )
                    .deps((
                        view.state.root_path.clone(),
                        view.state.tree_scan_generation,
                    ))
                    .on_ok(tree_scan_loaded)
                    .on_err(tree_scan_failed),
                );
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
            if view.state.git_status_pending() {
                ctx.resources.job(
                    JobResource::new(
                        ResourceKey::new("editor-git-status"),
                        GIT_STATUS_JOB,
                        GitStatusRequest {
                            root_path: view.state.root_path.clone(),
                            generation: view.state.git_status_generation,
                        },
                    )
                    .deps((
                        view.state.root_path.clone(),
                        view.state.git_status_generation,
                    ))
                    .on_ok(git_status_loaded)
                    .on_err(git_status_failed),
                );
            }

            let poll_terminal = ctx.bind(
                PollTerminal,
                reduce_with!(
                    (|state: &mut EditorState,
                      _: PollTerminal,
                      reducer: &mut ReducerContext<EditorState>| {
                        let _tick: PollTerminalTick =
                            reducer.input.timer_tick().unwrap_or_default();
                        if let Some(session) = state.terminal_session.as_ref() {
                            if session.take_dirty() {
                                state.redraw_epoch = state.redraw_epoch.wrapping_add(1);
                            }
                        }
                    })
                ),
            );
            if view.state.terminal_visible
                && view.state.bottom_panel_tab == BottomPanelTab::Terminal
                && view.state.terminal_session.is_some()
            {
                ctx.resources.timer(
                    TimerResource::new(
                        ResourceKey::new("editor-terminal-poll"),
                        Duration::from_millis(16),
                        PollTerminalTick,
                    )
                    .on_tick(poll_terminal),
                );
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
            if view.state.lsp_enabled() {
                ctx.resources.timer(
                    TimerResource::new(
                        ResourceKey::new("editor-lsp-poll"),
                        Duration::from_secs(1),
                        PollLspTick,
                    )
                    .immediate()
                    .on_tick(poll_lsp),
                );
            }

            let viewport = view.viewport_size();
            let sidebar_width = view
                .state
                .sidebar_width
                .min((viewport.width - 160.0).clamp(180.0, 360.0));

            // ── Menu bar (topmost) ──
            let menu_bar = MenuBar.build_node(ctx, view);

            // ── Activity bar (leftmost strip) ──
            let activity_bar = ActivityBar.build_node(ctx, view);

            // ── Sidebar (content depends on active section) ──
            let sidebar = if view.state.sidebar_visible {
                let (header_text, panel_content) = match view.state.sidebar_section {
                    SidebarSection::Explorer => ("EXPLORER", FileTree.build_node(ctx, view)),
                    SidebarSection::Search => ("SEARCH", SearchPanel.build_node(ctx, view)),
                    SidebarSection::Git => ("SOURCE CONTROL", GitPanel.build_node(ctx, view)),
                    SidebarSection::Extensions => (
                        "EXTENSIONS",
                        Container::new(
                            Text::new("No extensions installed")
                                .size(12.0)
                                .color(DIM_TEXT)
                                .into_node(),
                        )
                        .padding_all(8.0)
                        .flex_grow(1.0)
                        .into_node(),
                    ),
                };

                let header = Container::new(
                    Text::new(header_text)
                        .size(11.0)
                        .color(Color {
                            r: 187,
                            g: 187,
                            b: 187,
                            a: 255,
                        })
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
                .width(sidebar_width)
                .bg(SURFACE_BG)
                .flex_shrink(0.0)
                .into_node()
            } else {
                Spacer {
                    width: Some(0.0),
                    ..Default::default()
                }
                .into_node()
            };

            // 1px vertical divider between sidebar and editor
            let sidebar_divider = if view.state.sidebar_visible {
                Container::new(Spacer::default().into_node())
                    .width(1.0)
                    .bg(BORDER_COLOR)
                    .flex_shrink(0.0)
                    .into_node()
            } else {
                Spacer {
                    width: Some(0.0),
                    ..Default::default()
                }
                .into_node()
            };

            // ── Editor area: tabs + breadcrumb + find/replace + surface ──
            let tab_bar_node = TabBar.build_node(ctx, view);
            let breadcrumb_node = Breadcrumb.build_node(ctx, view);
            let find_replace_node = FindReplaceBar.build_node(ctx, view);
            let editor_surface_node = EditorSurface.build_node(ctx, view);

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
                        TerminalPanel.build_node(ctx, view),
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
                align_items: fission::ir::op::AlignItems::Stretch,
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node();

            // Root: menu bar + main + status bar
            let root = Column {
                children: vec![
                    menu_bar,
                    Container::new(main_layout).flex_grow(1.0).into_node(),
                    StatusBar.build_node(ctx, view),
                ],
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node();

            // ── Overlays (portals) ──
            CommandPalette.build_node(ctx, view);
            ContextMenu.build_node(ctx, view);
            completion_popup::CompletionPopup.build_node(ctx, view);

            root
        })
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let root_for_startup = root.clone();
    let app = DesktopApp::new(EditorApp)
        .with_title("Fission Editor")
        .with_startup_action(EditorStarted {
            root_path: root_for_startup,
        })
        .with_async(|asyncs| {
            asyncs.register_job(TREE_SCAN_JOB, |request: TreeScanRequest, _| async move {
                run_tree_scan(request)
            });
            asyncs.register_job(GIT_STATUS_JOB, |request: GitStatusRequest, _| async move {
                run_git_status(request)
            });
        })
        .with_sync_env(move |_state: &EditorState, env: &mut fission::core::Env| {
            env.theme = fission::theme::Theme::dark();
        })
        .with_key_handler(
            move |state: &mut EditorState, key: &fission::core::KeyCode, mods: u8| -> bool {
                // Async resources handle background scanning and polling.

                let ctrl = (mods & 2) != 0 || (mods & 8) != 0; // Ctrl or Cmd
                let shift = (mods & 1) != 0;

                // Dismiss context menu on any keystroke (except Escape which handles it explicitly)
                if !matches!(key, fission::core::KeyCode::Escape) {
                    state.context_menu_visible = false;
                }

                // Enter confirms rename if one is in progress
                if matches!(key, fission::core::KeyCode::Enter) && !ctrl {
                    if state.renaming_path.is_some() {
                        state.confirm_rename();
                        return true;
                    }
                    return false;
                }

                if state.renaming_path.is_some() && !ctrl {
                    let should_replace_rename_text = state
                        .renaming_path
                        .as_ref()
                        .and_then(|path| std::path::Path::new(path).file_name())
                        .and_then(|value| value.to_str())
                        .map(|name| state.rename_input == name)
                        .unwrap_or(false);
                    match key {
                        fission::core::KeyCode::Backspace => {
                            if should_replace_rename_text {
                                state.rename_input.clear();
                            } else {
                                state.rename_input.pop();
                            }
                            return true;
                        }
                        fission::core::KeyCode::Space => {
                            if should_replace_rename_text {
                                state.rename_input.clear();
                            }
                            state.rename_input.push(' ');
                            return true;
                        }
                        fission::core::KeyCode::Char(ch) => {
                            if should_replace_rename_text {
                                state.rename_input.clear();
                            }
                            state.rename_input.push(*ch);
                            return true;
                        }
                        _ => {}
                    }
                }

                // Escape dismisses menus / context menus / find bar / command palette / rename
                if matches!(key, fission::core::KeyCode::Escape) {
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

                if !ctrl {
                    return false;
                }

                match key {
                    fission::core::KeyCode::Char('s') | fission::core::KeyCode::Char('S') => {
                        if shift {
                            state.save_all_files();
                        } else {
                            state.save_active_file();
                        }
                        true
                    }
                    fission::core::KeyCode::Char('p') | fission::core::KeyCode::Char('P')
                        if shift =>
                    {
                        state.show_command_palette = !state.show_command_palette;
                        if !state.show_command_palette {
                            state.command_query.clear();
                        }
                        true
                    }
                    fission::core::KeyCode::Char('b') | fission::core::KeyCode::Char('B') => {
                        state.sidebar_visible = !state.sidebar_visible;
                        true
                    }
                    fission::core::KeyCode::Char('`') => {
                        state.terminal_visible = !state.terminal_visible;
                        if state.terminal_visible {
                            state.bottom_panel_tab = BottomPanelTab::Terminal;
                            state.ensure_terminal_session();
                        }
                        true
                    }
                    // Ctrl+F: toggle find/replace
                    fission::core::KeyCode::Char('f') | fission::core::KeyCode::Char('F') => {
                        state.context_menu_visible = false;
                        state.show_find_replace = !state.show_find_replace;
                        true
                    }
                    // Ctrl+G: go to line (toggle command palette with prompt)
                    fission::core::KeyCode::Char('g') | fission::core::KeyCode::Char('G') => {
                        state.show_command_palette = !state.show_command_palette;
                        if state.show_command_palette {
                            state.command_query = "Go to Line:".into();
                        } else {
                            state.command_query.clear();
                        }
                        true
                    }
                    // Ctrl+W: close active tab
                    fission::core::KeyCode::Char('w') | fission::core::KeyCode::Char('W') => {
                        let idx = state.active_tab;
                        state.close_tab(idx);
                        true
                    }
                    // Ctrl+Z: undo, Ctrl+Shift+Z: redo
                    fission::core::KeyCode::Char('z') | fission::core::KeyCode::Char('Z') => {
                        if shift {
                            state.redo_active();
                        } else {
                            state.undo_active();
                        }
                        true
                    }
                    // Ctrl+Y: redo (alternative)
                    fission::core::KeyCode::Char('y') | fission::core::KeyCode::Char('Y') => {
                        state.redo_active();
                        true
                    }
                    // Ctrl+C: copy current line
                    fission::core::KeyCode::Char('c') | fission::core::KeyCode::Char('C') => {
                        state.copy_line();
                        true
                    }
                    // Ctrl+X: cut current line
                    fission::core::KeyCode::Char('x') | fission::core::KeyCode::Char('X') => {
                        state.cut_line();
                        true
                    }
                    // Ctrl+V: paste clipboard
                    fission::core::KeyCode::Char('v') | fission::core::KeyCode::Char('V') => {
                        state.paste();
                        true
                    }
                    _ => false,
                }
            },
        );

    app.run()
}
