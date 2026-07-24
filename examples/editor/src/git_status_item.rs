use crate::layout::PANEL_ACTION_HEIGHT;
use crate::model::{EditorState, GitStatusEntry, OpenFile};
use crate::palette::{DIM_TEXT, GIT_ADDED, GIT_DELETED, GIT_MODIFIED, PANEL_TEXT};
use fission::prelude::*;
use fission::widgets::HStack;

pub(crate) struct GitStatusItem {
    pub entry: GitStatusEntry,
    pub open_id: ActionId,
}

impl From<GitStatusItem> for Widget {
    fn from(item: GitStatusItem) -> Self {
        let (_ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let status_color = match item.entry.status.as_str() {
            "M" => GIT_MODIFIED,
            "A" => GIT_ADDED,
            "D" => GIT_DELETED,
            "?" | "??" => DIM_TEXT,
            _ => PANEL_TEXT,
        };
        let filename = item
            .entry
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&item.entry.path);

        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: widgets![
                        Text::new(item.entry.status)
                            .size(tokens.typography.font_size_sm)
                            .color(status_color),
                        Text::new(filename)
                            .size(tokens.typography.font_size_sm)
                            .color(PANEL_TEXT)
                            .flex_grow(1.0),
                    ],
                }
                .into(),
            ),
            on_press: Some(ActionEnvelope {
                id: item.open_id,
                payload: serde_json::to_vec(&OpenFile(item.entry.path)).unwrap(),
            }),
            height: Some(PANEL_ACTION_HEIGHT),
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
