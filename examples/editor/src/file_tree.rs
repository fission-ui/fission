use crate::model::{
    CancelRename, CreateFile, CreateFolder, EditorState, FileEntry, OpenFile, RefreshTree,
    SelectTreeNode, ShowContextMenu, ToggleTreeNode, UpdateRenameInput,
};
use fission::core::op::Color;
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, GestureDetector, Node, Text, TextInput,
};
use fission::core::{reduce_with, ActionEnvelope, BuildCtx, View, Widget};
use fission::widgets::{HStack, Icon, Spacer, VStack};
use serde_json;

pub struct FileTree;

impl Widget<EditorState> for FileTree {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let entries = &view.state.cached_tree_entries;

        // --- Bind actions ---

        let toggle_id = ctx
            .bind(
                ToggleTreeNode(String::new()),
                reduce_with!(
                    (|s: &mut EditorState, a: ToggleTreeNode, _| {
                        if !s.tree_expanded.remove(&a.0) {
                            s.tree_expanded.insert(a.0);
                        }
                    })
                ),
            )
            .id;

        let open_id = ctx
            .bind(
                OpenFile(String::new()),
                reduce_with!(
                    (|s: &mut EditorState, a: OpenFile, _| {
                        s.open_file(a.0);
                    })
                ),
            )
            .id;

        let select_id = ctx
            .bind(
                SelectTreeNode(String::new()),
                reduce_with!(
                    (|s: &mut EditorState, a: SelectTreeNode, _| {
                        s.tree_selected = Some(a.0);
                    })
                ),
            )
            .id;

        let context_menu_id = ctx
            .bind(
                ShowContextMenu {
                    x: 0.0,
                    y: 0.0,
                    target: None,
                },
                reduce_with!(
                    (|s: &mut EditorState,
                      a: ShowContextMenu,
                      rctx: &mut fission::core::ReducerContext<EditorState>| {
                        let (px, py) = match rctx.input {
                            fission::core::ActionInput::Pointer { x, y, .. } => (*x, *y),
                            _ => (a.x, a.y),
                        };
                        let final_x = if px < 10.0 { 100.0 } else { px };
                        let final_y = if py < 10.0 { 100.0 } else { py };
                        s.context_menu_visible = true;
                        s.context_menu_position = (final_x, final_y);
                        s.context_menu_target = a.target;
                    })
                ),
            )
            .id;

        let create_file_id = ctx
            .bind(
                CreateFile(String::new()),
                reduce_with!(
                    (|s: &mut EditorState, a: CreateFile, _| {
                        // Generate a unique path so multiple "New File" clicks
                        // each create a distinct file.
                        let mut path = a.0.clone();
                        while std::path::Path::new(&path).exists()
                            || s.open_tabs.iter().any(|t| t.path == path)
                        {
                            s.untitled_counter += 1;
                            path = format!("{}-{}", a.0, s.untitled_counter);
                        }
                        let _ = std::fs::write(&path, "");
                        s.request_tree_refresh();
                        s.open_file(path);
                    })
                ),
            )
            .id;

        let create_folder_id = ctx
            .bind(
                CreateFolder(String::new()),
                reduce_with!(
                    (|s: &mut EditorState, a: CreateFolder, _| {
                        // Generate a unique folder name
                        let mut path = a.0.clone();
                        let mut counter = 0u32;
                        while std::path::Path::new(&path).exists() {
                            counter += 1;
                            path = format!("{}-{}", a.0, counter);
                        }
                        let _ = std::fs::create_dir_all(&path);
                        s.request_tree_refresh();
                        // Expand the parent so the new folder is visible
                        if let Some(parent) = std::path::Path::new(&path).parent() {
                            s.tree_expanded.insert(parent.to_string_lossy().to_string());
                        }
                        // Start inline rename so user can give it a proper name
                        s.start_rename(path);
                    })
                ),
            )
            .id;

        let refresh_id = ctx
            .bind(
                RefreshTree,
                reduce_with!(
                    (|s: &mut EditorState, _a: RefreshTree, _| {
                        // Collapse all expanded nodes to force a fresh view
                        s.tree_expanded.clear();
                        s.request_tree_refresh();
                    })
                ),
            )
            .id;

        let rename_input_id = ctx.bind(
            UpdateRenameInput(String::new()),
            reduce_with!(
                (|s: &mut EditorState, a: UpdateRenameInput, _| {
                    s.rename_input = a.0;
                })
            ),
        );

        let _cancel_rename_id = ctx.bind(
            CancelRename,
            reduce_with!(
                (|s: &mut EditorState, _a: CancelRename, _| {
                    s.cancel_rename();
                })
            ),
        );

        // --- Toolbar row ---

        let root_path_str = view.state.root_path.to_string_lossy().to_string();

        let new_file_action = ActionEnvelope {
            id: create_file_id,
            payload: serde_json::to_vec(&CreateFile(format!("{}/untitled", root_path_str)))
                .unwrap(),
        };

        let new_folder_action = ActionEnvelope {
            id: create_folder_id,
            payload: serde_json::to_vec(&CreateFolder(format!("{}/new_folder", root_path_str)))
                .unwrap(),
        };

        let refresh_action = ActionEnvelope {
            id: refresh_id,
            payload: serde_json::to_vec(&RefreshTree).unwrap(),
        };

        let icon_color = tokens.colors.text_secondary;

        let toolbar = Container::new(
            HStack {
                spacing: Some(2.0),
                children: vec![
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into_node(),
                    Button {
                        variant: ButtonVariant::Ghost,
                        on_press: Some(new_file_action),
                        child: Some(Box::new(
                            Icon::svg(fission::icons::material::content::add::round())
                                .size(18.0)
                                .color(icon_color)
                                .into_node(),
                        )),
                        width: Some(24.0),
                        height: Some(24.0),
                        padding: Some([0.0; 4]),
                        ..Default::default()
                    }
                    .into_node(),
                    Button {
                        variant: ButtonVariant::Ghost,
                        on_press: Some(new_folder_action),
                        child: Some(Box::new(
                            Icon::svg(fission::icons::material::file::create_new_folder::round())
                                .size(18.0)
                                .color(icon_color)
                                .into_node(),
                        )),
                        width: Some(24.0),
                        height: Some(24.0),
                        padding: Some([0.0; 4]),
                        ..Default::default()
                    }
                    .into_node(),
                    Button {
                        variant: ButtonVariant::Ghost,
                        on_press: Some(refresh_action),
                        child: Some(Box::new(
                            Icon::svg(fission::icons::material::navigation::refresh::round())
                                .size(18.0)
                                .color(icon_color)
                                .into_node(),
                        )),
                        width: Some(24.0),
                        height: Some(24.0),
                        padding: Some([0.0; 4]),
                        ..Default::default()
                    }
                    .into_node(),
                ],
            }
            .into_node(),
        )
        .padding_all(4.0)
        .into_node();

        // --- Tree rows ---

        let mut rows = Vec::new();
        for entry in entries {
            build_tree_rows(
                entry,
                0,
                &mut rows,
                view,
                toggle_id,
                open_id,
                select_id,
                context_menu_id,
                &rename_input_id,
            );
        }

        let tree_scroll = fission::core::ui::Scroll {
            id: Some(fission::ir::NodeId::explicit("file_tree_scroll")),
            direction: fission::ir::op::FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            child: Some(Box::new(
                VStack {
                    spacing: Some(0.0),
                    children: rows,
                }
                .into_node(),
            )),
            ..Default::default()
        }
        .into_node();

        // --- Compose toolbar + tree ---

        Container::new(
            VStack {
                spacing: Some(0.0),
                children: vec![toolbar, tree_scroll],
            }
            .into_node(),
        )
        .bg(tokens.colors.surface)
        .flex_grow(1.0)
        .into_node()
    }
}

fn build_tree_rows(
    entry: &FileEntry,
    depth: usize,
    rows: &mut Vec<Node>,
    view: &View<EditorState>,
    toggle_id: fission::core::ActionId,
    open_id: fission::core::ActionId,
    select_id: fission::core::ActionId,
    context_menu_id: fission::core::ActionId,
    rename_input_action: &ActionEnvelope,
) {
    let tokens = &view.env.theme.tokens;
    let is_expanded = view.state.tree_expanded.contains(&entry.path);
    let is_selected = view.state.tree_selected.as_deref() == Some(&entry.path);

    // IntelliJ-style: colored icons, compact rows
    let icon_color = if entry.is_dir {
        Color {
            r: 204,
            g: 166,
            b: 75,
            a: 255,
        } // Warm yellow for folders
    } else {
        file_icon_color(&entry.name)
    };

    let indent = depth as f32 * 16.0;

    // Fixed-width chevron container for alignment: ">" collapsed, "v" expanded, " " for files
    let chevron = if entry.is_dir {
        if is_expanded {
            "v"
        } else {
            ">"
        }
    } else {
        " "
    };

    let file_icon = if entry.is_dir {
        if is_expanded {
            fission::icons::material::file::folder_open::regular()
        } else {
            fission::icons::material::file::folder::regular()
        }
    } else {
        fission::icons::material::action::description::regular()
    };

    let bg = if is_selected {
        tokens.colors.primary.with_alpha(30)
    } else {
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    };

    // Primary tap action: toggle for dirs, open for files
    let tap_action = if entry.is_dir {
        ActionEnvelope {
            id: toggle_id,
            payload: serde_json::to_vec(&ToggleTreeNode(entry.path.clone())).unwrap(),
        }
    } else {
        ActionEnvelope {
            id: open_id,
            payload: serde_json::to_vec(&OpenFile(entry.path.clone())).unwrap(),
        }
    };

    // Right-click action: show context menu targeting this entry
    let long_press_action = ActionEnvelope {
        id: context_menu_id,
        payload: serde_json::to_vec(&ShowContextMenu {
            x: 0.0,
            y: 0.0,
            target: Some(entry.path.clone()),
        })
        .unwrap(),
    };

    // Check if this entry is being renamed
    let is_renaming = view.state.renaming_path.as_deref() == Some(&entry.path);

    // Build the name column: either a TextInput (renaming) or a Text label
    let name_node = if is_renaming {
        TextInput {
            id: Some(fission::ir::NodeId::explicit("rename_input")),
            value: view.state.rename_input.clone(),
            placeholder: Some("New name".into()),
            on_change: Some(rename_input_action.clone()),
            ..Default::default()
        }
        .into_node()
    } else {
        Text::new(entry.name.clone())
            .size(13.0)
            .color(tokens.colors.text_primary)
            .flex_grow(1.0)
            .into_node()
    };

    // Build the row content
    let row_content = Container::new(
        HStack {
            spacing: Some(4.0),
            children: vec![
                // Indentation spacer
                Spacer {
                    width: Some(indent),
                    ..Default::default()
                }
                .into_node(),
                // Fixed-width chevron container (12px) for consistent alignment
                Container::new(
                    Text::new(chevron)
                        .size(11.0)
                        .color(tokens.colors.text_secondary)
                        .into_node(),
                )
                .width(12.0)
                .into_node(),
                // File/folder icon
                Icon::svg(file_icon)
                    .size(16.0)
                    .color(icon_color)
                    .into_node(),
                // File/folder name (or rename TextInput)
                name_node,
            ],
        }
        .into_node(),
    )
    .bg(bg)
    .padding_all(2.0)
    .into_node();

    let row = if is_renaming {
        Container::new(row_content).height(24.0).into_node()
    } else {
        // Wrap in a Button for tap handling, then wrap that in GestureDetector for long-press
        let button_row = Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            on_press: Some(tap_action),
            child: Some(Box::new(row_content)),
            height: Some(24.0),
            padding: Some([0.0; 4]),
            ..Default::default()
        }
        .into_node();

        // GestureDetector wraps the entire row to capture right-click for context menu
        GestureDetector {
            on_secondary_click: Some(long_press_action),
            child: Box::new(button_row),
            ..Default::default()
        }
        .into_node()
    };

    rows.push(row);

    if entry.is_dir && is_expanded {
        for child in &entry.children {
            build_tree_rows(
                child,
                depth + 1,
                rows,
                view,
                toggle_id,
                open_id,
                select_id,
                context_menu_id,
                rename_input_action,
            );
        }
    }
}

fn file_icon_color(name: &str) -> Color {
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => Color {
            r: 222,
            g: 120,
            b: 50,
            a: 255,
        }, // Rust orange
        "toml" => Color {
            r: 140,
            g: 180,
            b: 100,
            a: 255,
        }, // Green
        "md" => Color {
            r: 66,
            g: 133,
            b: 244,
            a: 255,
        }, // Blue
        "json" => Color {
            r: 255,
            g: 193,
            b: 7,
            a: 255,
        }, // Amber
        "lock" => Color {
            r: 130,
            g: 130,
            b: 130,
            a: 255,
        }, // Gray
        "html" | "htm" => Color {
            r: 227,
            g: 134,
            b: 43,
            a: 255,
        }, // Orange
        "css" | "scss" | "sass" | "less" => Color {
            r: 66,
            g: 133,
            b: 244,
            a: 255,
        }, // Blue
        "js" | "jsx" | "ts" | "tsx" | "mjs" => Color {
            r: 241,
            g: 196,
            b: 15,
            a: 255,
        }, // Yellow
        "py" | "pyi" => Color {
            r: 80,
            g: 175,
            b: 76,
            a: 255,
        }, // Green
        "sh" | "bash" | "zsh" | "fish" => Color {
            r: 150,
            g: 150,
            b: 150,
            a: 255,
        }, // Gray
        "yaml" | "yml" => Color {
            r: 200,
            g: 100,
            b: 100,
            a: 255,
        }, // Reddish
        "xml" | "svg" => Color {
            r: 200,
            g: 120,
            b: 50,
            a: 255,
        }, // Burnt orange
        "go" => Color {
            r: 0,
            g: 173,
            b: 216,
            a: 255,
        }, // Cyan
        "rb" => Color {
            r: 204,
            g: 52,
            b: 45,
            a: 255,
        }, // Ruby red
        _ => Color {
            r: 160,
            g: 160,
            b: 160,
            a: 255,
        }, // Default gray
    }
}
