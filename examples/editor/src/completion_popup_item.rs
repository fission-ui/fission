use crate::layout::{COMPLETION_ITEM_HEIGHT, COMPLETION_KIND_WIDTH};
use crate::model::{on_select_completion, EditorState, SelectCompletion};
use crate::palette::EditorPalette;
use fission::prelude::*;
use fission::widgets::{HStack, Spacer};

pub(crate) struct CompletionPopupItem {
    pub index: usize,
    pub label: String,
    pub detail: Option<String>,
    pub kind: String,
    pub selected: bool,
}

impl From<CompletionPopupItem> for Widget {
    fn from(item: CompletionPopupItem) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
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
                                .color(kind_color(&item.kind, &palette)),
                        )
                        .width(COMPLETION_KIND_WIDTH),
                        Text::new(item.label)
                            .size(tokens.typography.font_size_xs)
                            .color(palette.bright_text),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        if let Some(detail) = detail {
                            Widget::from(
                                Text::new(detail)
                                    .size(tokens.typography.font_size_xs)
                                    .color(palette.dim_text),
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
            on_press: Some(ctx.bind(
                SelectCompletion(item.index),
                reduce_with!(on_select_completion),
            )),
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
            Container::new(button)
                .bg(palette.completion_selected_bg)
                .into()
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

fn kind_color(kind: &str, palette: &EditorPalette) -> Color {
    match kind {
        "function" | "method" => palette.completion_function,
        "variable" | "field" => palette.completion_variable,
        "keyword" => palette.completion_keyword,
        "struct" | "class" | "enum" => palette.completion_type,
        "module" => palette.completion_module,
        _ => palette.completion_fallback,
    }
}
