use crate::layout::SEARCH_CONTEXT_PREVIEW_CHARS;
use crate::model::{EditorState, OpenFile, SearchResult};
use crate::palette::{DIM_TEXT, PANEL_TEXT};
use fission::prelude::*;
use fission::widgets::VStack;

pub(crate) struct SearchResultItem {
    pub result: SearchResult,
    pub open_id: ActionId,
}

impl From<SearchResultItem> for Widget {
    fn from(item: SearchResultItem) -> Self {
        let (_ctx, view) = fission::build::current::<EditorState>();
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
                            .color(PANEL_TEXT),
                        Text::new(
                            item.result
                                .context
                                .chars()
                                .take(SEARCH_CONTEXT_PREVIEW_CHARS)
                                .collect::<String>(),
                        )
                        .size(tokens.typography.font_size_xs)
                        .color(DIM_TEXT),
                    ],
                }
                .into(),
            ),
            on_press: Some(ActionEnvelope {
                id: item.open_id,
                payload: serde_json::to_vec(&OpenFile(item.result.path)).unwrap(),
            }),
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
