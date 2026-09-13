use crate::layout::{
    MINIMAP_BAR_HEIGHT, MINIMAP_CHARACTER_WIDTH_FACTOR, MINIMAP_MAX_BAR_COUNT,
    MINIMAP_MAX_BAR_WIDTH, MINIMAP_MAX_CONTENT_HEIGHT, MINIMAP_MIN_BAR_WIDTH,
    MINIMAP_VISIBLE_LINE_COUNT, MINIMAP_WIDTH,
};
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;
use fission::widgets::{Spacer, VStack};

/// A minimap widget that renders a narrow, scaled-down overview of the file
/// content on the right side of the editor. Each source line is represented
/// as a thin coloured bar whose hue hints at the kind of content (comment,
/// string literal, blank, or plain code) and whose width is proportional to
/// the trimmed line length.
pub struct Minimap;

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

impl From<Minimap> for Widget {
    fn from(_component: Minimap) -> Self {
        let (_ctx, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        // If there is no active buffer we collapse to nothing.
        let Some((_tab, buffer)) = view.state().active_buffer() else {
            return Spacer::default().into();
        };

        let content_str = buffer.content();
        let lines: Vec<&str> = content_str.lines().collect();
        let line_count = lines.len();
        if line_count == 0 {
            return Spacer::default().into();
        }

        let scale = if line_count as f32 * MINIMAP_BAR_HEIGHT > MINIMAP_MAX_CONTENT_HEIGHT {
            MINIMAP_MAX_CONTENT_HEIGHT / line_count as f32
        } else {
            MINIMAP_BAR_HEIGHT
        };

        let cursor = buffer.cursor_line;
        let vis_start = cursor.saturating_sub(MINIMAP_VISIBLE_LINE_COUNT / 2);
        let vis_end = (cursor + MINIMAP_VISIBLE_LINE_COUNT / 2).min(line_count);

        let step = if line_count > MINIMAP_MAX_BAR_COUNT {
            line_count / MINIMAP_MAX_BAR_COUNT
        } else {
            1
        };
        let bar_count = (line_count + step - 1) / step;

        let mut bars: Vec<Widget> = Vec::with_capacity(bar_count);
        for (i, line) in lines.iter().enumerate() {
            if step > 1 && i % step != 0 {
                continue;
            }
            let trimmed = line.trim();
            let color = line_color(trimmed, &palette);

            let width = (trimmed.len() as f32 * MINIMAP_CHARACTER_WIDTH_FACTOR)
                .clamp(MINIMAP_MIN_BAR_WIDTH, MINIMAP_MAX_BAR_WIDTH);

            let in_viewport = i >= vis_start && i < vis_end;

            let bar = Container::new(Spacer::default())
                .width(width)
                .height(scale)
                .bg(color)
                .into();

            if in_viewport {
                bars.push(
                    Container::new(bar)
                        .height(scale)
                        .bg(palette.minimap_viewport)
                        .into(),
                );
            } else {
                bars.push(bar);
            }
        }

        Container::new(VStack {
            spacing: Some(tokens.spacing.none),
            children: bars,
        })
        .width(MINIMAP_WIDTH)
        .bg(palette.minimap_bg)
        .padding_all(tokens.spacing.xs)
        .flex_shrink(0.0)
        .into()
    }
}
