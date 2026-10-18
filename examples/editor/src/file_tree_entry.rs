use crate::model::{
    on_open_file, on_show_context_menu, on_toggle_tree_node, on_update_rename_input, EditorState,
    FileEntry, OpenFile, ShowContextMenu, ToggleTreeNode, UpdateRenameInput,
};
use crate::palette::EditorPalette;
use fission::prelude::*;

const TREE_INDENT: f32 = 16.0;
const ROW_HEIGHT: f32 = 24.0;
const ROW_PADDING: f32 = 2.0;
const ROW_GAP: f32 = 4.0;
const CHEVRON_WIDTH: f32 = 12.0;
const CHEVRON_SIZE: f32 = 11.0;
const LABEL_SIZE: f32 = 13.0;
const ICON_SIZE: f32 = 16.0;

fission_component! {
    pub(crate) FileTreeEntry<EditorState> {
        entry: FileEntry,
        depth: usize,
    } |component, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let entry = &component.entry;
        let is_expanded = state.tree_expanded.contains(&entry.path);
        let is_selected = state.tree_selected.as_deref() == Some(&entry.path);
        let is_renaming = state.renaming_path.as_deref() == Some(&entry.path);
        let icon_color = if entry.is_dir {
            palette.file_folder
        } else {
            file_icon_color(&entry.name, &palette)
        };
        let chevron = match (entry.is_dir, is_expanded) {
            (true, true) => "v",
            (true, false) => ">",
            (false, _) => " ",
        };
        let file_icon = match (entry.is_dir, is_expanded) {
            (true, true) => fission::icons::material::file::folder_open::regular(),
            (true, false) => fission::icons::material::file::folder::regular(),
            (false, _) => fission::icons::material::action::description::regular(),
        };
        let background = if is_selected {
            tokens.colors.primary.with_alpha(30)
        } else {
            Color::TRANSPARENT
        };
        let tap_action = if entry.is_dir {
            cx.bind(
                ToggleTreeNode(entry.path.clone()),
                reduce_with!(on_toggle_tree_node),
            )
        } else {
            cx.bind(OpenFile(entry.path.clone()), reduce_with!(on_open_file))
        };
        let context_menu_action = cx.bind(
            ShowContextMenu {
                x: 0.0,
                y: 0.0,
                target: Some(entry.path.clone()),
            },
            reduce_with!(on_show_context_menu),
        );
    } =>
        VStack(
            spacing: tokens.spacing.none,
            children: [
                if is_renaming {
                    Container(
                        FileTreeRowContent(
                            component.entry.clone(),
                            component.depth,
                            chevron,
                            file_icon,
                            icon_color,
                            background,
                            true,
                        ),
                    ).height(ROW_HEIGHT)
                } else {
                    GestureDetector(
                        on_secondary_click: context_menu_action,
                        child: Button(
                            variant: ButtonVariant::Ghost,
                            content_align: ButtonContentAlign::Start,
                            on_press: tap_action,
                            child: FileTreeRowContent(
                                component.entry.clone(),
                                component.depth,
                                chevron,
                                file_icon,
                                icon_color,
                                background,
                                false,
                            ),
                            height: ROW_HEIGHT,
                            padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
                        ).semantics_identifier(format!("editor.file-tree.{}", entry.path)),
                    )
                },
                if entry.is_dir && is_expanded {
                    for child in entry.children.iter().cloned() {
                        FileTreeEntry(
                            child.clone(),
                            component.depth + 1,
                            key: child.path.clone(),
                        )
                    },
                },
            ],
        )
}

fission_component! {
    FileTreeRowContent<EditorState> {
        entry: FileEntry,
        depth: usize,
        chevron: &'static str,
        file_icon: &'static str,
        icon_color: Color,
        background: Color,
        is_renaming: bool,
    } |row, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            HStack(
                spacing: ROW_GAP,
                children: [
                    Spacer(width: row.depth as f32 * TREE_INDENT),
                    Container(
                        Text(row.chevron)
                            .size(CHEVRON_SIZE)
                            .color(tokens.colors.text_secondary),
                    ).width(CHEVRON_WIDTH),
                    Icon::svg(row.file_icon)
                        .size(ICON_SIZE)
                        .color(row.icon_color),
                    if row.is_renaming {
                        TextInput(
                            id: WidgetId::explicit("rename_input"),
                            value: state.rename_input.clone(),
                            placeholder: Some("New name".into()),
                            on_input: cx.bind(
                                UpdateRenameInput,
                                reduce_with!(on_update_rename_input),
                            ),
                        )
                    } else {
                        Text(row.entry.name.clone())
                            .size(LABEL_SIZE)
                            .color(tokens.colors.text_primary)
                            .flex_grow(1.0)
                    },
                ],
            ),
        )
        .bg(row.background)
        .padding_all(ROW_PADDING)
}

fn file_icon_color(name: &str, palette: &EditorPalette) -> Color {
    match name.rsplit('.').next().unwrap_or("") {
        "rs" => palette.file_rust,
        "toml" => palette.file_config,
        "md" | "css" | "scss" | "sass" | "less" => palette.file_web_style,
        "json" => palette.file_data,
        "js" | "jsx" | "ts" | "tsx" | "mjs" => palette.file_script,
        "lock" => palette.file_muted,
        "sh" | "bash" | "zsh" | "fish" => palette.file_neutral,
        "html" | "htm" => palette.file_html,
        "xml" | "svg" => palette.file_markup,
        "py" | "pyi" => palette.file_python,
        "yaml" | "yml" => palette.file_yaml,
        "rb" => palette.file_ruby,
        "go" => palette.file_go,
        _ => palette.file_neutral,
    }
}
