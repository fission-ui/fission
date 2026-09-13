use crate::file_tree_entry::FileTreeEntry;
use crate::layout::TOOLBAR_CONTROL_SIZE;
use crate::model::{
    on_create_file, on_create_folder, on_refresh_tree, CreateFile, CreateFolder, EditorState,
    RefreshTree,
};
use fission::core::ui::{Button, ButtonVariant, Container, Scroll, Widget};
use fission::core::{reduce_with, ActionEnvelope, WidgetId};
use fission::icons::material;
use fission::op::FlexDirection;
use fission::widgets::{HStack, Icon, Spacer, VStack};

/// The explorer: a toolbar for new entries and refresh, above the scrolling tree.
pub struct FileTree;

impl From<FileTree> for Widget {
    fn from(_component: FileTree) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let rows = view
            .state()
            .cached_tree_entries
            .iter()
            .cloned()
            .map(|entry| FileTreeEntry { entry, depth: 0 }.into())
            .collect();

        Container::new(VStack {
            spacing: Some(tokens.spacing.none),
            children: vec![
                FileTreeToolbar.into(),
                Scroll {
                    id: Some(WidgetId::explicit("file_tree_scroll")),
                    direction: FlexDirection::Column,
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
                .into(),
            ],
        })
        .bg(tokens.colors.surface)
        .flex_grow(1.0)
        .into()
    }
}

struct FileTreeToolbar;

impl From<FileTreeToolbar> for Widget {
    fn from(_toolbar: FileTreeToolbar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let root = view.state().root_path.to_string_lossy().to_string();

        Container::new(HStack {
            spacing: Some(tokens.spacing.xs),
            children: vec![
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                ToolbarIconButton {
                    icon: material::content::add::round(),
                    identifier: "editor.file-tree.new-file",
                    action: ctx.bind(
                        CreateFile(format!("{}/untitled", root)),
                        reduce_with!(on_create_file),
                    ),
                }
                .into(),
                ToolbarIconButton {
                    icon: material::file::create_new_folder::round(),
                    identifier: "editor.file-tree.new-folder",
                    action: ctx.bind(
                        CreateFolder(format!("{}/new_folder", root)),
                        reduce_with!(on_create_folder),
                    ),
                }
                .into(),
                ToolbarIconButton {
                    icon: material::navigation::refresh::round(),
                    identifier: "editor.file-tree.refresh",
                    action: ctx.bind(RefreshTree, reduce_with!(on_refresh_tree)),
                }
                .into(),
            ],
        })
        .padding_all(tokens.spacing.xs)
        .into()
    }
}

struct ToolbarIconButton {
    icon: &'static str,
    identifier: &'static str,
    action: ActionEnvelope,
}

impl From<ToolbarIconButton> for Widget {
    fn from(button: ToolbarIconButton) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        Button {
            variant: ButtonVariant::Ghost,
            on_press: Some(button.action),
            child: Some(
                Icon::svg(button.icon)
                    .size(tokens.typography.font_size_lg)
                    .color(tokens.colors.text_secondary)
                    .into(),
            ),
            width: Some(TOOLBAR_CONTROL_SIZE),
            height: Some(TOOLBAR_CONTROL_SIZE),
            padding: Some([tokens.spacing.none; 4]),
            ..Default::default()
        }
        .semantics_identifier(button.identifier)
        .into()
    }
}
