use fission_core::op::Color;
use fission_core::ui::{Column, Container, Node, Row, Text};
use fission_core::{AppState, BuildCtx, Handler, View, Widget, WidgetNodeId};
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use fission_widgets::{HStack, SplitDirection, SplitView, VStack, Spacer};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod model;
mod file_tree;
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

use model::*;
use file_tree::FileTree;
use editor_surface::EditorSurface;
use tab_bar::TabBar;
use status_bar::StatusBar;
use search_panel::SearchPanel;
use git_panel::GitPanel;
use terminal_panel::TerminalPanel;
use command_palette::CommandPalette;

// --- Activity bar (left icon strip, like VS Code) ---

struct ActivityBar;

impl Widget<EditorState> for ActivityBar {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let bg = Color { r: 51, g: 51, b: 51, a: 255 };
        let active_color = Color { r: 255, g: 255, b: 255, a: 255 };
        let inactive_color = Color { r: 120, g: 120, b: 120, a: 255 };

        let toggle_sidebar = ctx.bind(
            ToggleSidebar,
            (|s: &mut EditorState, _, _| s.sidebar_visible = !s.sidebar_visible)
                as Handler<EditorState, ToggleSidebar>,
        );

        let section_icons = vec![
            (fission_icons::material::file::folder::regular(), SidebarSection::Explorer, "Explorer"),
            (fission_icons::material::action::search::regular(), SidebarSection::Search, "Search"),
            (fission_icons::material::action::compare_arrows::regular(), SidebarSection::Git, "Source Control"),
            (fission_icons::material::action::extension::regular(), SidebarSection::Extensions, "Extensions"),
        ];

        let set_section_id = ctx.bind(
            SetSidebarSection(SidebarSection::Explorer),
            (|s: &mut EditorState, a: SetSidebarSection, _| {
                if s.sidebar_section == a.0 {
                    s.sidebar_visible = !s.sidebar_visible;
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

            icons.push(
                fission_core::ui::Button {
                    variant: fission_core::ui::ButtonVariant::Ghost,
                    child: Some(Box::new(
                        fission_widgets::Icon::svg(*icon_svg)
                            .size(22.0)
                            .color(color)
                            .into_node(),
                    )),
                    on_press: Some(fission_core::ActionEnvelope {
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

// --- Main app ---

struct EditorApp;

impl Widget<EditorState> for EditorApp {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let dark_bg = Color { r: 30, g: 30, b: 30, a: 255 };
        let surface_bg = Color { r: 37, g: 37, b: 38, a: 255 };

        // Activity bar (leftmost strip)
        let activity_bar = ActivityBar.build(ctx, view);

        // Sidebar (content depends on active section)
        let sidebar = if view.state.sidebar_visible {
            let (header_text, panel_content) = match view.state.sidebar_section {
                SidebarSection::Explorer => ("EXPLORER", FileTree.build(ctx, view)),
                SidebarSection::Search => ("SEARCH", SearchPanel.build(ctx, view)),
                SidebarSection::Git => ("SOURCE CONTROL", GitPanel.build(ctx, view)),
                SidebarSection::Extensions => ("EXTENSIONS", Container::new(
                    Text::new("No extensions installed")
                        .size(12.0)
                        .color(Color { r: 140, g: 140, b: 140, a: 255 })
                        .into_node(),
                ).padding_all(8.0).flex_grow(1.0).into_node()),
            };

            let header = Container::new(
                Text::new(header_text)
                    .size(11.0)
                    .color(Color { r: 187, g: 187, b: 187, a: 255 })
                    .into_node(),
            )
            .bg(surface_bg)
            .height(24.0)
            .padding_all(6.0)
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
            .bg(surface_bg)
            .flex_shrink(0.0)
            .into_node()
        } else {
            Spacer { width: Some(0.0), ..Default::default() }.into_node()
        };

        // Editor area (tabs + editor surface)
        let editor_area = Column {
            children: vec![
                TabBar.build(ctx, view),
                EditorSurface.build(ctx, view),
            ],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        // Center: editor area + terminal
        let center = Column {
            children: if view.state.terminal_visible {
                vec![
                    Container::new(editor_area).flex_grow(1.0).into_node(),
                    TerminalPanel.build(ctx, view),
                ]
            } else {
                vec![Container::new(editor_area).flex_grow(1.0).into_node()]
            },
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        // Main layout: activity bar | sidebar | center
        let main_layout = Row {
            children: vec![activity_bar, sidebar, center],
            align_items: fission_ir::op::AlignItems::Stretch,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        // Root: main + status bar
        let root = Column {
            children: vec![
                Container::new(main_layout).flex_grow(1.0).into_node(),
                StatusBar.build(ctx, view),
            ],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node();

        // Command palette overlay
        CommandPalette.build(ctx, view);

        root
    }
}

fn main() -> anyhow::Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let root_for_sync = root.clone();
    let app = DesktopApp::new(EditorApp)
        .with_title("Fission Editor")
        .with_sync_env(move |state: &EditorState, env: &mut fission_core::Env| {
            env.theme = fission_theme::Theme::dark();
        })
        .with_key_handler(move |state: &mut EditorState, key: &fission_core::KeyCode, mods: u8| -> bool {
            // Initialize root path on first call
            if state.root_path == PathBuf::from(".") {
                state.root_path = root_for_sync.clone();
            }

            let ctrl = (mods & 4) != 0 || (mods & 8) != 0; // Ctrl or Cmd
            let shift = (mods & 1) != 0;

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
                _ => false,
            }
        });

    app.run()
}
