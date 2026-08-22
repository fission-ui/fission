use crate::model::{EditorState, FileEntry, OpenFile, ShowContextMenu, ToggleTreeNode};
use crate::palette::{
    FILE_CONFIG, FILE_DATA, FILE_FOLDER, FILE_GO, FILE_HTML, FILE_MARKUP, FILE_MUTED, FILE_NEUTRAL,
    FILE_PYTHON, FILE_RUBY, FILE_RUST, FILE_SCRIPT, FILE_WEB_STYLE, FILE_YAML,
};
use fission::core::op::Color;
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, GestureDetector, Text, TextInput, Widget,
};
use fission::core::{ActionEnvelope, ActionId};
use fission::widgets::{HStack, Icon, Spacer, VStack};

const TREE_INDENT: f32 = 16.0;
const ROW_HEIGHT: f32 = 24.0;
const ROW_PADDING: f32 = 2.0;
const ROW_GAP: f32 = 4.0;
const CHEVRON_WIDTH: f32 = 12.0;
const CHEVRON_SIZE: f32 = 11.0;
const LABEL_SIZE: f32 = 13.0;
const ICON_SIZE: f32 = 16.0;

pub(crate) struct FileTreeEntry {
    pub entry: FileEntry,
    pub depth: usize,
    pub toggle_id: ActionId,
    pub open_id: ActionId,
    pub context_menu_id: ActionId,
    pub rename_input_action: ActionEnvelope,
}

impl From<FileTreeEntry> for Widget {
    fn from(component: FileTreeEntry) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let entry = component.entry;
        let is_expanded = view.state().tree_expanded.contains(&entry.path);
        let is_selected = view.state().tree_selected.as_deref() == Some(&entry.path);
        let is_renaming = view.state().renaming_path.as_deref() == Some(&entry.path);

        let icon_color = if entry.is_dir {
            FILE_FOLDER
        } else {
            file_icon_color(&entry.name)
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
            ActionEnvelope {
                id: component.toggle_id,
                payload: serde_json::to_vec(&ToggleTreeNode(entry.path.clone())).unwrap(),
            }
        } else {
            ActionEnvelope {
                id: component.open_id,
                payload: serde_json::to_vec(&OpenFile(entry.path.clone())).unwrap(),
            }
        };
        let context_menu_action = ActionEnvelope {
            id: component.context_menu_id,
            payload: serde_json::to_vec(&ShowContextMenu {
                x: 0.0,
                y: 0.0,
                target: Some(entry.path.clone()),
            })
            .unwrap(),
        };

        let name: Widget = if is_renaming {
            TextInput {
                id: Some(fission::WidgetId::explicit("rename_input")),
                value: view.state().rename_input.clone(),
                placeholder: Some("New name".into()),
                on_input: Some(component.rename_input_action.clone()),
                ..Default::default()
            }
            .into()
        } else {
            Text::new(entry.name.clone())
                .size(LABEL_SIZE)
                .color(tokens.colors.text_primary)
                .flex_grow(1.0)
                .into()
        };

        let content = Container::new(HStack {
            spacing: Some(ROW_GAP),
            children: vec![
                Spacer {
                    width: Some(component.depth as f32 * TREE_INDENT),
                    ..Default::default()
                }
                .into(),
                Container::new(
                    Text::new(chevron)
                        .size(CHEVRON_SIZE)
                        .color(tokens.colors.text_secondary),
                )
                .width(CHEVRON_WIDTH)
                .into(),
                Icon::svg(file_icon)
                    .size(ICON_SIZE)
                    .color(icon_color)
                    .into(),
                name,
            ],
        })
        .bg(background)
        .padding_all(ROW_PADDING)
        .into();

        let row = if is_renaming {
            Container::new(content).height(ROW_HEIGHT).into()
        } else {
            GestureDetector {
                on_secondary_click: Some(context_menu_action),
                child: Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    on_press: Some(tap_action),
                    child: Some(content),
                    height: Some(ROW_HEIGHT),
                    padding: Some([tokens.spacing.none; 4]),
                    ..Default::default()
                }
                .semantics_identifier(format!("editor.file-tree.{}", entry.path))
                .into(),
                ..Default::default()
            }
            .into()
        };

        let mut children = vec![row];
        if entry.is_dir && is_expanded {
            children.extend(entry.children.into_iter().map(|child| {
                FileTreeEntry {
                    entry: child,
                    depth: component.depth + 1,
                    toggle_id: component.toggle_id,
                    open_id: component.open_id,
                    context_menu_id: component.context_menu_id,
                    rename_input_action: component.rename_input_action.clone(),
                }
                .into()
            }));
        }

        VStack {
            spacing: Some(tokens.spacing.none),
            children,
        }
        .into()
    }
}

fn file_icon_color(name: &str) -> Color {
    match name.rsplit('.').next().unwrap_or("") {
        "rs" => FILE_RUST,
        "toml" => FILE_CONFIG,
        "md" | "css" | "scss" | "sass" | "less" => FILE_WEB_STYLE,
        "json" => FILE_DATA,
        "js" | "jsx" | "ts" | "tsx" | "mjs" => FILE_SCRIPT,
        "lock" => FILE_MUTED,
        "sh" | "bash" | "zsh" | "fish" => FILE_NEUTRAL,
        "html" | "htm" => FILE_HTML,
        "xml" | "svg" => FILE_MARKUP,
        "py" | "pyi" => FILE_PYTHON,
        "yaml" | "yml" => FILE_YAML,
        "rb" => FILE_RUBY,
        "go" => FILE_GO,
        _ => FILE_NEUTRAL,
    }
}
