use crate::layout::{COMPLETION_ITEM_HEIGHT, COMPLETION_KIND_WIDTH};
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) CompletionPopupItem<EditorState> {
        index: usize,
        label: String,
        detail: Option<String>,
        kind: String,
        selected: bool,
    } |item, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let detail = item
            .detail
            .as_ref()
            .map(|value| value.chars().take(30).collect::<String>());
    } =>
        Container(
            Button(
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: HStack(
                    spacing: tokens.spacing.s,
                    children: [
                        Container(
                            Text::new(kind_label(&item.kind))
                                .size(tokens.typography.font_size_xs)
                                .color(kind_color(&item.kind, &palette)),
                        )
                        .width(COMPLETION_KIND_WIDTH),
                        Text(item.label.clone())
                            .size(tokens.typography.font_size_xs)
                            .color(palette.bright_text),
                        Spacer(flex_grow: 1.0),
                        if let Some(detail) = detail {
                            Text(detail)
                                    .size(tokens.typography.font_size_xs)
                                    .color(palette.dim_text)
                        },
                    ],
            ),
            on_press: |item, state, _cx| state.select_completion(item.index),
            height: COMPLETION_ITEM_HEIGHT,
            padding: [
                tokens.spacing.xs,
                tokens.spacing.xs,
                tokens.spacing.xs,
                tokens.spacing.xs,
            ],
            ).semantics_identifier(format!("editor.completion.{}", item.index)),
        )
        .bg(if item.selected {
            palette.completion_selected_bg
        } else {
            palette.transparent
        })
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
