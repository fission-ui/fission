use crate::command_palette_item::CommandPaletteItem;
use crate::layout::{
    COMMAND_PALETTE_MAX_WIDTH, COMMAND_PALETTE_MIN_WIDTH, COMMAND_PALETTE_VERTICAL_RESERVE,
    COMMAND_RESULTS_MAX_HEIGHT, COMMAND_RESULTS_MIN_HEIGHT, OVERLAY_HORIZONTAL_RESERVE,
};
use crate::model::{
    EditorState, RefreshGitStatus, SaveAllFiles, SaveFile, SetSidebarSection, SidebarSection,
    ToggleCommandPalette, ToggleSidebar, ToggleTerminal, UpdateCommandQuery,
};
use crate::palette::{FLYOUT_BG, FLYOUT_BORDER, MODAL_BACKDROP};
use fission::core::ui::{Container, GestureDetector, Positioned, TextInput, Widget, ZStack};
use fission::core::{reduce_with, ReducerContext, WidgetId};
use fission::widgets::{Spacer, VStack};

pub struct CommandPalette;

struct Command {
    label: &'static str,
    description: &'static str,
}

impl From<CommandPalette> for Widget {
    fn from(_component: CommandPalette) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        if !view.state().show_command_palette {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into();
        }

        let dismiss = ctx.bind(
            ToggleCommandPalette,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.show_command_palette = false;
                    s.command_query.clear();
                })
            ),
        );

        let update_query = ctx.bind(
            UpdateCommandQuery,
            reduce_with!(
                (|s: &mut EditorState,
                  _a: UpdateCommandQuery,
                  ctx: &mut ReducerContext<EditorState>| {
                    if let Some(change) = ctx.input.text_change() {
                        s.command_query = change.new_text.clone();
                    }
                })
            ),
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

        let query = view.state().command_query.to_lowercase();
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
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.save_active_file();
                    s.show_command_palette = false;
                })
            ),
        );
        let save_all = ctx.bind(
            SaveAllFiles,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.save_all_files();
                    s.show_command_palette = false;
                })
            ),
        );
        let toggle_sidebar = ctx.bind(
            ToggleSidebar,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.sidebar_visible = !s.sidebar_visible;
                    s.show_command_palette = false;
                })
            ),
        );
        let toggle_terminal = ctx.bind(
            ToggleTerminal,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.terminal_visible = !s.terminal_visible;
                    s.show_command_palette = false;
                })
            ),
        );
        let show_explorer = ctx.bind(
            SetSidebarSection(SidebarSection::Explorer),
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.sidebar_section = SidebarSection::Explorer;
                    s.sidebar_visible = true;
                    s.show_command_palette = false;
                })
            ),
        );
        let show_search = ctx.bind(
            SetSidebarSection(SidebarSection::Search),
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.sidebar_section = SidebarSection::Search;
                    s.sidebar_visible = true;
                    s.show_command_palette = false;
                })
            ),
        );
        let show_git = ctx.bind(
            SetSidebarSection(SidebarSection::Git),
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.sidebar_section = SidebarSection::Git;
                    s.sidebar_visible = true;
                    s.show_command_palette = false;
                })
            ),
        );
        let refresh_git = ctx.bind(
            RefreshGitStatus,
            reduce_with!(
                (|s: &mut EditorState, _, _| {
                    s.refresh_git_status();
                    s.show_command_palette = false;
                })
            ),
        );

        let action_for = |label: &str| -> fission::core::ActionEnvelope {
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
            result_items.push(
                CommandPaletteItem {
                    label: cmd.label,
                    description: cmd.description,
                    action: action_for(cmd.label),
                }
                .into(),
            );
        }

        let viewport = view.viewport_size();
        let palette_width = (viewport.width - OVERLAY_HORIZONTAL_RESERVE)
            .clamp(COMMAND_PALETTE_MIN_WIDTH, COMMAND_PALETTE_MAX_WIDTH);
        let results_height = (viewport.height - COMMAND_PALETTE_VERTICAL_RESERVE)
            .clamp(COMMAND_RESULTS_MIN_HEIGHT, COMMAND_RESULTS_MAX_HEIGHT);

        // VS Code-style dropdown from top center
        let dropdown = Container::new(VStack {
            spacing: Some(tokens.spacing.none),
            children: vec![
                Container::new(TextInput {
                    id: Some(fission::WidgetId::explicit("editor_command_palette_input")),
                    value: view.state().command_query.clone(),
                    placeholder: Some("Type a command...".into()),
                    on_input: Some(update_query),
                    ..Default::default()
                })
                .padding_all(tokens.spacing.s)
                .into(),
                Container::new(fission::core::ui::widgets::scroll::Scroll {
                    direction: fission::core::op::FlexDirection::Column,
                    child: Some(
                        VStack {
                            spacing: Some(tokens.spacing.none),
                            children: result_items,
                        }
                        .into(),
                    ),
                    height: Some(results_height),
                    show_scrollbar: true,
                    ..Default::default()
                })
                .padding_all(tokens.spacing.xs)
                .into(),
            ],
        })
        .width(palette_width)
        .bg(FLYOUT_BG)
        .border(FLYOUT_BORDER, 1.0)
        .border_radius(tokens.radii.small);
        let dropdown: Widget = if let Some(shadow) = tokens.elevations.level3 {
            dropdown.shadow(shadow).flex_shrink(1.0).into()
        } else {
            dropdown.flex_shrink(1.0).into()
        };

        // Backdrop + dropdown positioned at top center
        let backdrop = GestureDetector {
            on_tap: Some(dismiss.clone()),
            child: Container::new(Spacer::default())
                .bg(MODAL_BACKDROP)
                .flex_grow(1.0)
                .into(),
            ..Default::default()
        }
        .into();

        let overlay = Container::new(ZStack {
            children: vec![
                // Full-screen backdrop
                Positioned {
                    left: Some(0.0),
                    right: Some(0.0),
                    top: Some(0.0),
                    bottom: Some(0.0),
                    child: Some(backdrop),
                    ..Default::default()
                }
                .into(),
                // Dropdown at top center
                Positioned {
                    top: Some(tokens.spacing.xl + tokens.spacing.s),
                    left: Some(tokens.spacing.none),
                    right: Some(tokens.spacing.none),
                    child: Some(fission::core::ui::Align::new(dropdown).into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        })
        .flex_grow(1.0)
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
            fission::core::PortalLayer::Modal,
            Some(WidgetId::explicit("command_palette")),
            positioned_root,
        );

        Spacer {
            height: Some(0.0),
            ..Default::default()
        }
        .into()
    }
}
