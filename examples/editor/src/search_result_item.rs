use crate::layout::SEARCH_CONTEXT_PREVIEW_CHARS;
use crate::model::{on_open_file, EditorState, OpenFile, SearchResult};
use crate::palette::EditorPalette;
use fission::prelude::*;
use fission::widgets::VStack;

pub(crate) struct SearchResultItem {
    pub result: SearchResult,
}

impl From<SearchResultItem> for Widget {
    fn from(item: SearchResultItem) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        let filename = item
            .result
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&item.result.path);
        let label = format!("{}:{}", filename, item.result.line);

        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(
                VStack {
                    spacing: Some(tokens.spacing.xs),
                    children: widgets![
                        Text::new(label)
                            .size(tokens.typography.font_size_sm)
                            .color(palette.panel_text),
                        Text::new(
                            item.result
                                .context
                                .chars()
                                .take(SEARCH_CONTEXT_PREVIEW_CHARS)
                                .collect::<String>(),
                        )
                        .size(tokens.typography.font_size_xs)
                        .color(palette.dim_text),
                    ],
                }
                .into(),
            ),
            on_press: Some(ctx.bind(OpenFile(item.result.path), reduce_with!(on_open_file))),
            padding: Some([
                tokens.spacing.xs,
                tokens.spacing.xs,
                tokens.spacing.none,
                tokens.spacing.none,
            ]),
            ..Default::default()
        }
        .into()
    }
}
