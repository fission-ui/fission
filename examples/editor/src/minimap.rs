use crate::layout::{
    MINIMAP_BAR_HEIGHT, MINIMAP_CHARACTER_WIDTH_FACTOR, MINIMAP_MAX_BAR_COUNT,
    MINIMAP_MAX_BAR_WIDTH, MINIMAP_MAX_CONTENT_HEIGHT, MINIMAP_MIN_BAR_WIDTH,
    MINIMAP_VISIBLE_LINE_COUNT, MINIMAP_WIDTH,
};
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

/// A minimap widget that renders a narrow, scaled-down overview of the file
/// content on the right side of the editor. Each source line is represented
/// as a thin coloured bar whose hue hints at the kind of content (comment,
/// string literal, blank, or plain code) and whose width is proportional to
/// the trimmed line length.
/// Classify a single trimmed source line into a colour.
fn line_color(trimmed: &str, palette: &EditorPalette) -> Color {
    if trimmed.is_empty() {
        palette.minimap_empty
    } else if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
        palette.minimap_comment
    } else if trimmed.contains('"') {
        palette.minimap_string
    } else {
        palette.minimap_code
    }
}

fission_component! {
    pub Minimap<EditorState> |_minimap, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let buffer = state.active_buffer().map(|(_, buffer)| buffer);
        let lines: Vec<String> = buffer
            .map(|buffer| buffer.content().lines().map(str::to_owned).collect())
            .unwrap_or_default();
        let line_count = lines.len();
        let scale = if line_count as f32 * MINIMAP_BAR_HEIGHT > MINIMAP_MAX_CONTENT_HEIGHT {
            MINIMAP_MAX_CONTENT_HEIGHT / line_count as f32
        } else {
            MINIMAP_BAR_HEIGHT
        };

        let cursor = buffer.map_or(0, |buffer| buffer.cursor_line);
        let vis_start = cursor.saturating_sub(MINIMAP_VISIBLE_LINE_COUNT / 2);
        let vis_end = (cursor + MINIMAP_VISIBLE_LINE_COUNT / 2).min(line_count);

        let step = if line_count > MINIMAP_MAX_BAR_COUNT {
            line_count / MINIMAP_MAX_BAR_COUNT
        } else {
            1
        };
    } =>
    if lines.is_empty() {
        Spacer()
    } else {
        Container(
            VStack(
                spacing: tokens.spacing.none,
                children: [
                    for (index, line) in lines.iter().enumerate().filter(|(index, _)| step == 1 || index % step == 0) {
                        MinimapBar(
                            line.clone(),
                            scale,
                            index >= vis_start && index < vis_end,
                            key: index,
                        )
                    },
                ],
            ),
        )
        .width(MINIMAP_WIDTH)
        .bg(palette.minimap_bg)
        .padding_all(tokens.spacing.xs)
        .flex_shrink(0.0)
    }
}

fission_component! {
    MinimapBar<EditorState> {
        line: String,
        height: f32,
        visible: bool,
    } |bar, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let trimmed = bar.line.trim();
        let width = (trimmed.len() as f32 * MINIMAP_CHARACTER_WIDTH_FACTOR)
            .clamp(MINIMAP_MIN_BAR_WIDTH, MINIMAP_MAX_BAR_WIDTH);
    } =>
    Container(
        Container(Spacer())
            .width(width)
            .height(bar.height)
            .bg(line_color(trimmed, &palette)),
    )
    .height(bar.height)
    .bg(if bar.visible { palette.minimap_viewport } else { palette.transparent })
}
