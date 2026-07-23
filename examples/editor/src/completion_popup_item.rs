use crate::layout::{COMPLETION_ITEM_HEIGHT, COMPLETION_KIND_WIDTH};
use crate::model::{EditorState, SelectCompletion};
use crate::palette::{
    BRIGHT_TEXT, COMPLETION_FALLBACK, COMPLETION_FUNCTION, COMPLETION_KEYWORD, COMPLETION_MODULE,
    COMPLETION_SELECTED_BG, COMPLETION_TYPE, COMPLETION_VARIABLE, DIM_TEXT,
};
use fission::prelude::*;
use fission::widgets::{HStack, Spacer};

pub(crate) struct CompletionPopupItem {
    pub index: usize,
    pub label: String,
    pub detail: Option<String>,
    pub kind: String,
    pub selected: bool,
    pub select_id: ActionId,
}

impl From<CompletionPopupItem> for Widget {
    fn from(item: CompletionPopupItem) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let detail = item
            .detail
            .map(|value| value.chars().take(30).collect::<String>());

        let button: Widget = Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: widgets![
                        Container::new(
                            Text::new(kind_label(&item.kind))
                                .size(tokens.typography.font_size_xs)
                                .color(kind_color(&item.kind)),
                        )
                        .width(COMPLETION_KIND_WIDTH),
                        Text::new(item.label)
                            .size(tokens.typography.font_size_xs)
                            .color(BRIGHT_TEXT),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        if let Some(detail) = detail {
                            Widget::from(
                                Text::new(detail)
                                    .size(tokens.typography.font_size_xs)
                                    .color(DIM_TEXT),
                            )
                        } else {
                            Widget::from(Spacer {
                                width: Some(tokens.spacing.none),
                                ..Default::default()
                            })
                        },
                    ],
                }
                .into(),
            ),
            on_press: Some(ActionEnvelope {
                id: item.select_id,
                payload: serde_json::to_vec(&SelectCompletion(item.index)).unwrap(),
            }),
            height: Some(COMPLETION_ITEM_HEIGHT),
            padding: Some([
                tokens.spacing.xs,
                tokens.spacing.xs,
                tokens.spacing.xs,
                tokens.spacing.xs,
            ]),
            ..Default::default()
        }
        .semantics_identifier(format!("editor.completion.{}", item.index))
        .into();

        if item.selected {
            Container::new(button).bg(COMPLETION_SELECTED_BG).into()
        } else {
            button
        }
    }
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "function" | "method" => "fn",
        "variable" | "field" => "ab",
        "keyword" => "kw",
        "struct" | "class" => "St",
        "enum" => "En",
        "module" => "Md",
        "property" => "Pr",
        "constant" => "Co",
        "interface" | "trait" => "Tr",
        "type" => "Ty",
        "snippet" => "Sn",
        _ => "  ",
    }
}

fn kind_color(kind: &str) -> Color {
    match kind {
        "function" | "method" => COMPLETION_FUNCTION,
        "variable" | "field" => COMPLETION_VARIABLE,
        "keyword" => COMPLETION_KEYWORD,
        "struct" | "class" | "enum" => COMPLETION_TYPE,
        "module" => COMPLETION_MODULE,
        _ => COMPLETION_FALLBACK,
    }
}
