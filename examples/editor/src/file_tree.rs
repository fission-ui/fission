use crate::file_tree_entry::FileTreeEntry;
use crate::layout::TOOLBAR_CONTROL_SIZE;
use crate::model::{
    CancelRename, CreateFile, CreateFolder, EditorState, OpenFile, RefreshTree, ShowContextMenu,
    ToggleTreeNode, UpdateRenameInput,
};
use fission::core::ui::{Button, ButtonVariant, Container, Widget};
use fission::core::{reduce_with, ActionEnvelope};
use fission::widgets::{HStack, Icon, Spacer, VStack};
use serde_json;

pub struct FileTree;

impl From<FileTree> for Widget {
    fn from(_component: FileTree) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let entries = &view.state().cached_tree_entries;

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

        let root_path_str = view.state().root_path.to_string_lossy().to_string();

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

        let toolbar = Container::new(HStack {
            spacing: Some(tokens.spacing.xs),
            children: vec![
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Button {
                    variant: ButtonVariant::Ghost,
                    on_press: Some(new_file_action),
                    child: Some(
                        Icon::svg(fission::icons::material::content::add::round())
                            .size(tokens.typography.font_size_lg)
                            .color(icon_color)
                            .into(),
                    ),
                    width: Some(TOOLBAR_CONTROL_SIZE),
                    height: Some(TOOLBAR_CONTROL_SIZE),
                    padding: Some([tokens.spacing.none; 4]),
                    ..Default::default()
                }
                .into(),
                Button {
                    variant: ButtonVariant::Ghost,
                    on_press: Some(new_folder_action),
                    child: Some(
                        Icon::svg(fission::icons::material::file::create_new_folder::round())
                            .size(tokens.typography.font_size_lg)
                            .color(icon_color)
                            .into(),
                    ),
                    width: Some(TOOLBAR_CONTROL_SIZE),
                    height: Some(TOOLBAR_CONTROL_SIZE),
                    padding: Some([tokens.spacing.none; 4]),
                    ..Default::default()
                }
                .into(),
                Button {
                    variant: ButtonVariant::Ghost,
                    on_press: Some(refresh_action),
                    child: Some(
                        Icon::svg(fission::icons::material::navigation::refresh::round())
                            .size(tokens.typography.font_size_lg)
                            .color(icon_color)
                            .into(),
                    ),
                    width: Some(TOOLBAR_CONTROL_SIZE),
                    height: Some(TOOLBAR_CONTROL_SIZE),
                    padding: Some([tokens.spacing.none; 4]),
                    ..Default::default()
                }
                .into(),
            ],
        })
        .padding_all(tokens.spacing.xs)
        .into();

        // --- Tree rows ---

        let rows = entries
            .iter()
            .cloned()
            .map(|entry| {
                FileTreeEntry {
                    entry,
                    depth: 0,
                    toggle_id,
                    open_id,
                    context_menu_id,
                    rename_input_action: rename_input_id.clone(),
                }
                .into()
            })
            .collect();

        let tree_scroll = fission::core::ui::Scroll {
            id: Some(fission::WidgetId::explicit("file_tree_scroll")),
            direction: fission::op::FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            child: Some(
                VStack {
                    spacing: Some(tokens.spacing.none),
                    children: rows,
                }
                .into(),
            ),
            ..Default::default()
        }
        .into();

        // --- Compose toolbar + tree ---

        Container::new(VStack {
            spacing: Some(tokens.spacing.none),
            children: vec![toolbar, tree_scroll],
        })
        .bg(tokens.colors.surface)
        .flex_grow(1.0)
        .into()
    }
}
