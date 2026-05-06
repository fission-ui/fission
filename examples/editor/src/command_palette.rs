use crate::model::{
    EditorState, RefreshGitStatus, SaveAllFiles, SaveFile, SetSidebarSection, SidebarSection,
    ToggleCommandPalette, ToggleSidebar, ToggleTerminal, UpdateCommandQuery,
};
use fission_core::op::Color;
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, GestureDetector, Node, Positioned, Text,
    TextInput, ZStack,
};
use fission_core::{BuildCtx, Handler, View, Widget, WidgetNodeId};
use fission_widgets::{HStack, Spacer, VStack};

pub struct CommandPalette;

struct Command {
    label: &'static str,
    description: &'static str,
}

impl Widget<EditorState> for CommandPalette {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        if !view.state.show_command_palette {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into_node();
        }

        let dismiss = ctx.bind(
            ToggleCommandPalette,
            (|s: &mut EditorState, _, _| {
                s.show_command_palette = false;
                s.command_query.clear();
            }) as Handler<EditorState, ToggleCommandPalette>,
        );

        let update_query = ctx.bind(
            UpdateCommandQuery(String::new()),
            (|s: &mut EditorState, a: UpdateCommandQuery, _| s.command_query = a.0)
                as Handler<EditorState, UpdateCommandQuery>,
        );

        let commands = vec![
            Command {
                label: "Save",
                description: "Save the active file",
            },
            Command {
                label: "Save All",
                description: "Save all open files",
            },
            Command {
                label: "Toggle Sidebar",
                description: "Show or hide the side bar",
            },
            Command {
                label: "Toggle Terminal",
                description: "Show or hide the terminal panel",
            },
            Command {
                label: "Show Explorer",
                description: "Open the file explorer",
            },
            Command {
                label: "Show Search",
                description: "Open the search panel",
            },
            Command {
                label: "Show Source Control",
                description: "Open the git panel",
            },
            Command {
                label: "Refresh Git Status",
                description: "Fetch latest git status",
            },
        ];

        let query = view.state.command_query.to_lowercase();
        let filtered: Vec<&Command> = if query.is_empty() {
            commands.iter().collect()
        } else {
            commands
                .iter()
                .filter(|c| {
                    c.label.to_lowercase().contains(&query)
                        || c.description.to_lowercase().contains(&query)
                })
                .collect()
        };

        let save = ctx.bind(
            SaveFile,
            (|s: &mut EditorState, _, _| {
                s.save_active_file();
                s.show_command_palette = false;
            }) as Handler<EditorState, SaveFile>,
        );
        let save_all = ctx.bind(
            SaveAllFiles,
            (|s: &mut EditorState, _, _| {
                s.save_all_files();
                s.show_command_palette = false;
            }) as Handler<EditorState, SaveAllFiles>,
        );
        let toggle_sidebar = ctx.bind(
            ToggleSidebar,
            (|s: &mut EditorState, _, _| {
                s.sidebar_visible = !s.sidebar_visible;
                s.show_command_palette = false;
            }) as Handler<EditorState, ToggleSidebar>,
        );
        let toggle_terminal = ctx.bind(
            ToggleTerminal,
            (|s: &mut EditorState, _, _| {
                s.terminal_visible = !s.terminal_visible;
                s.show_command_palette = false;
            }) as Handler<EditorState, ToggleTerminal>,
        );
        let show_explorer = ctx.bind(
            SetSidebarSection(SidebarSection::Explorer),
            (|s: &mut EditorState, _, _| {
                s.sidebar_section = SidebarSection::Explorer;
                s.sidebar_visible = true;
                s.show_command_palette = false;
            }) as Handler<EditorState, SetSidebarSection>,
        );
        let show_search = ctx.bind(
            SetSidebarSection(SidebarSection::Search),
            (|s: &mut EditorState, _, _| {
                s.sidebar_section = SidebarSection::Search;
                s.sidebar_visible = true;
                s.show_command_palette = false;
            }) as Handler<EditorState, SetSidebarSection>,
        );
        let show_git = ctx.bind(
            SetSidebarSection(SidebarSection::Git),
            (|s: &mut EditorState, _, _| {
                s.sidebar_section = SidebarSection::Git;
                s.sidebar_visible = true;
                s.show_command_palette = false;
            }) as Handler<EditorState, SetSidebarSection>,
        );
        let refresh_git = ctx.bind(
            RefreshGitStatus,
            (|s: &mut EditorState, _, _| {
                s.refresh_git_status();
                s.show_command_palette = false;
            }) as Handler<EditorState, RefreshGitStatus>,
        );

        let action_for = |label: &str| -> fission_core::ActionEnvelope {
            match label {
                "Save" => save.clone(),
                "Save All" => save_all.clone(),
                "Toggle Sidebar" => toggle_sidebar.clone(),
                "Toggle Terminal" => toggle_terminal.clone(),
                "Show Explorer" => show_explorer.clone(),
                "Show Search" => show_search.clone(),
                "Show Source Control" => show_git.clone(),
                "Refresh Git Status" => refresh_git.clone(),
                _ => dismiss.clone(),
            }
        };

        let mut result_items = Vec::new();
        for cmd in &filtered {
            let dim = Color {
                r: 140,
                g: 140,
                b: 140,
                a: 255,
            };
            result_items.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Some(Box::new(
                        HStack {
                            spacing: Some(12.0),
                            children: vec![
                                Text::new(cmd.label)
                                    .size(13.0)
                                    .color(Color {
                                        r: 220,
                                        g: 220,
                                        b: 220,
                                        a: 255,
                                    })
                                    .into_node(),
                                Spacer {
                                    flex_grow: 1.0,
                                    ..Default::default()
                                }
                                .into_node(),
                                Text::new(cmd.description).size(11.0).color(dim).into_node(),
                            ],
                        }
                        .into_node(),
                    )),
                    on_press: Some(action_for(cmd.label)),
                    height: Some(28.0),
                    padding: Some([6.0, 6.0, 0.0, 0.0]),
                    ..Default::default()
                }
                .into_node(),
            );
        }

        let card_bg = Color {
            r: 37,
            g: 37,
            b: 38,
            a: 255,
        };
        let border = Color {
            r: 60,
            g: 60,
            b: 60,
            a: 255,
        };

        let shadow = fission_core::op::BoxShadow {
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 120,
            },
            blur_radius: 12.0,
            offset: (0.0, 4.0),
        };
        let viewport = view.viewport_size();
        let palette_width = (viewport.width - 80.0).clamp(280.0, 560.0);
        let results_height = (viewport.height - 140.0).clamp(160.0, 320.0);

        // VS Code-style dropdown from top center
        let dropdown = Container::new(
            VStack {
                spacing: Some(0.0),
                children: vec![
                    Container::new(
                        TextInput {
                            id: Some(fission_ir::NodeId::explicit("editor_command_palette_input")),
                            value: view.state.command_query.clone(),
                            placeholder: Some("Type a command...".into()),
                            on_change: Some(update_query),
                            ..Default::default()
                        }
                        .into_node(),
                    )
                    .padding_all(6.0)
                    .into_node(),
                    Container::new(
                        fission_core::ui::widgets::scroll::Scroll {
                            direction: fission_core::op::FlexDirection::Column,
                            child: Some(Box::new(
                                VStack {
                                    spacing: Some(0.0),
                                    children: result_items,
                                }
                                .into_node(),
                            )),
                            height: Some(results_height),
                            show_scrollbar: true,
                            ..Default::default()
                        }
                        .into_node(),
                    )
                    .padding_all(4.0)
                    .into_node(),
                ],
            }
            .into_node(),
        )
        .width(palette_width)
        .bg(card_bg)
        .border(border, 1.0)
        .border_radius(4.0)
        .shadow(shadow)
        .flex_shrink(1.0)
        .into_node();

        // Backdrop + dropdown positioned at top center
        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child: Box::new(
                Container::new(Spacer::default().into_node())
                    .bg(Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 80,
                    })
                    .flex_grow(1.0)
                    .into_node(),
            ),
            ..Default::default()
        }
        .into_node();

        let overlay = Container::new(
            ZStack {
                children: vec![
                    // Full-screen backdrop
                    Positioned {
                        left: Some(0.0),
                        right: Some(0.0),
                        top: Some(0.0),
                        bottom: Some(0.0),
                        child: Some(Box::new(backdrop)),
                        ..Default::default()
                    }
                    .into_node(),
                    // Dropdown at top center
                    Positioned {
                        top: Some(40.0),
                        left: Some(0.0),
                        right: Some(0.0),
                        child: Some(Box::new(fission_core::ui::Align::new(dropdown).into_node())),
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .flex_grow(1.0)
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
            fission_core::PortalLayer::Modal,
            Some(WidgetNodeId::explicit("command_palette")),
            positioned_root,
        );

        Spacer {
            height: Some(0.0),
            ..Default::default()
        }
        .into_node()
    }
}
