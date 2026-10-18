use crate::file_tree_entry::FileTreeEntry;
use crate::layout::TOOLBAR_CONTROL_SIZE;
use crate::model::{
    on_create_file, on_create_folder, on_refresh_tree, CreateFile, CreateFolder, EditorState,
    RefreshTree,
};
use fission::prelude::*;

fission_component! {
/// The explorer: a toolbar for new entries and refresh, above the scrolling tree.
    pub FileTree<EditorState> |_tree, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            VStack(
                spacing: tokens.spacing.none,
                children: [
                    FileTreeToolbar(),
                    Scroll(
                        id: WidgetId::explicit("file_tree_scroll"),
                        direction: FlexDirection::Column,
                        show_scrollbar: true,
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        child: VStack(
                            spacing: tokens.spacing.none,
                            children: [
                                for entry in state.cached_tree_entries.iter().cloned() {
                                    FileTreeEntry(entry, 0, key: entry.path.clone())
                                },
                            ],
                        ),
                    ),
                ],
            ),
        )
        .bg(tokens.colors.surface)
        .flex_grow(1.0)
}

fission_component! {
    FileTreeToolbar<EditorState> |_toolbar, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let root = state.root_path.to_string_lossy().to_string();
    } =>
        Container(
            HStack(
                spacing: tokens.spacing.xs,
                children: [
                    Spacer(flex_grow: 1.0),
                    ToolbarIconButton(
                        fission::icons::material::content::add::round(),
                        "editor.file-tree.new-file",
                        cx.bind(
                            CreateFile(format!("{root}/untitled")),
                            reduce_with!(on_create_file),
                        ),
                    ),
                    ToolbarIconButton(
                        fission::icons::material::file::create_new_folder::round(),
                        "editor.file-tree.new-folder",
                        cx.bind(
                            CreateFolder(format!("{root}/new_folder")),
                            reduce_with!(on_create_folder),
                        ),
                    ),
                    ToolbarIconButton(
                        fission::icons::material::navigation::refresh::round(),
                        "editor.file-tree.refresh",
                        cx.bind(RefreshTree, reduce_with!(on_refresh_tree)),
                    ),
                ],
            ),
        ).padding_all(tokens.spacing.xs)
}

fission_component! {
    ToolbarIconButton<EditorState> {
        icon: &'static str,
        identifier: &'static str,
        action: ActionEnvelope,
    } |button, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            on_press: button.action,
            child: Some(Icon::svg(button.icon)
                .size(tokens.typography.font_size_lg)
                .color(tokens.colors.text_secondary).into()),
            width: TOOLBAR_CONTROL_SIZE,
            height: TOOLBAR_CONTROL_SIZE,
            padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
        ).semantics_identifier(button.identifier)
}
