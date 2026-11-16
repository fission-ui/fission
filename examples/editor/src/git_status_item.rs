use crate::layout::PANEL_ACTION_HEIGHT;
use crate::model::{on_open_file, EditorState, GitStatusEntry, OpenFile};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) GitStatusItem<EditorState> { entry: GitStatusEntry } |item, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let status_color = match item.entry.status.as_str() {
            "M" => palette.git_modified,
            "A" => palette.git_added,
            "D" => palette.git_deleted,
            "?" | "??" => palette.dim_text,
            _ => palette.panel_text,
        };
        let filename = item
            .entry
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&item.entry.path);
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: HStack(
                    spacing: tokens.spacing.s,
                    children: [
                        Text(item.entry.status.clone())
                            .size(tokens.typography.font_size_sm)
                            .color(status_color),
                        Text(filename)
                            .size(tokens.typography.font_size_sm)
                            .color(palette.panel_text)
                            .flex_grow(1.0),
                    ],
            ),
            on_press: cx.bind(OpenFile(item.entry.path.clone()), reduce_with!(on_open_file)),
            height: PANEL_ACTION_HEIGHT,
            padding: [
                tokens.spacing.xs,
                tokens.spacing.xs,
                tokens.spacing.none,
                tokens.spacing.none,
            ],
        )
}
