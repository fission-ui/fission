use crate::layout::SEARCH_CONTEXT_PREVIEW_CHARS;
use crate::model::{on_open_file, EditorState, OpenFile, SearchResult};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) SearchResultItem<EditorState> { result: SearchResult } |item, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let filename = item
            .result
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&item.result.path);
        let label = format!("{}:{}", filename, item.result.line);
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: VStack(
                    spacing: tokens.spacing.xs,
                    children: [
                        Text(label)
                            .size(tokens.typography.font_size_sm)
                            .color(palette.panel_text),
                        Text(
                            item.result
                                .context
                                .chars()
                                .take(SEARCH_CONTEXT_PREVIEW_CHARS)
                                .collect::<String>(),
                        )
                        .size(tokens.typography.font_size_xs)
                        .color(palette.dim_text),
                    ],
            ),
            on_press: cx.bind(
                OpenFile(item.result.path.clone()),
                reduce_with!(on_open_file),
            ),
            padding: [
                tokens.spacing.xs,
                tokens.spacing.xs,
                tokens.spacing.none,
                tokens.spacing.none,
            ],
        )
}
