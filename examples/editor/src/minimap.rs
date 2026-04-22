use crate::model::EditorState;
use fission_core::op::Color;
use fission_core::ui::{Container, Node};
use fission_core::{BuildCtx, View, Widget};
use fission_widgets::{Spacer, VStack};

/// A minimap widget that renders a narrow, scaled-down overview of the file
/// content on the right side of the editor. Each source line is represented
/// as a thin coloured bar whose hue hints at the kind of content (comment,
/// string literal, blank, or plain code) and whose width is proportional to
/// the trimmed line length.
pub struct Minimap;

/// Background colour for the minimap column.
const MINIMAP_BG: Color = Color { r: 25, g: 25, b: 25, a: 255 };

/// Width of the minimap column in logical pixels.
const MINIMAP_WIDTH: f32 = 60.0;

/// Maximum total height (in logical pixels) the minimap bars may occupy.
/// Files longer than `MAX_HEIGHT / BAR_HEIGHT` lines are scaled down so that
/// the entire file fits inside this budget.
const MAX_HEIGHT: f32 = 400.0;

/// Default height of each per-line bar (before scaling).
const BAR_HEIGHT: f32 = 2.0;

/// Semi-transparent overlay that highlights the currently-visible region.
const VIEWPORT_OVERLAY: Color = Color { r: 255, g: 255, b: 255, a: 25 };

// Line-type colours -------------------------------------------------------

const COLOR_EMPTY: Color = Color { r: 30, g: 30, b: 30, a: 255 };
const COLOR_COMMENT: Color = Color { r: 60, g: 80, b: 50, a: 255 };
const COLOR_STRING: Color = Color { r: 80, g: 60, b: 50, a: 255 };
const COLOR_CODE: Color = Color { r: 70, g: 70, b: 70, a: 255 };

/// Classify a single trimmed source line into a colour.
fn line_color(trimmed: &str) -> Color {
    if trimmed.is_empty() {
        COLOR_EMPTY
    } else if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
        COLOR_COMMENT
    } else if trimmed.contains('"') {
        COLOR_STRING
    } else {
        COLOR_CODE
    }
}

impl Widget<EditorState> for Minimap {
    fn build(&self, _ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        // If there is no active buffer we collapse to nothing.
        let Some((_tab, buffer)) = view.state.active_buffer() else {
            return Spacer::default().into_node();
        };

        let lines: Vec<&str> = buffer.content.lines().collect();
        let line_count = lines.len();
        if line_count == 0 {
            return Spacer::default().into_node();
        }

        // Scale factor: if the file is long we shrink each bar so everything
        // fits within MAX_HEIGHT.
        let scale = if line_count as f32 * BAR_HEIGHT > MAX_HEIGHT {
            MAX_HEIGHT / line_count as f32
        } else {
            BAR_HEIGHT
        };

        // The cursor line determines which region is "visible".  We highlight
        // a window of lines centred on the cursor.
        let visible_window = 40_usize; // roughly how many editor lines fit on screen
        let cursor = buffer.cursor_line;
        let vis_start = cursor.saturating_sub(visible_window / 2);
        let vis_end = (cursor + visible_window / 2).min(line_count);

        // Sample every Nth line to cap widget count at ~MAX_MINIMAP_BARS.
        // For files shorter than the limit every line is shown.
        const MAX_MINIMAP_BARS: usize = 50;
        let step = if line_count > MAX_MINIMAP_BARS {
            line_count / MAX_MINIMAP_BARS
        } else {
            1
        };
        let bar_count = (line_count + step - 1) / step;

        let mut bars: Vec<Node> = Vec::with_capacity(bar_count);
        for (i, line) in lines.iter().enumerate() {
            if step > 1 && i % step != 0 {
                continue;
            }
            let trimmed = line.trim();
            let color = line_color(trimmed);

            // Width proportional to the trimmed length (capped to fit inside
            // the minimap column).
            let width = (trimmed.len() as f32 * 0.5).clamp(2.0, 50.0);

            let in_viewport = i >= vis_start && i < vis_end;

            let bar = Container::new(Spacer::default().into_node())
                .width(width)
                .height(scale)
                .bg(color)
                .into_node();

            if in_viewport {
                // Wrap in a container with the semi-transparent viewport
                // overlay so the visible region stands out.
                bars.push(
                    Container::new(bar)
                        .height(scale)
                        .bg(VIEWPORT_OVERLAY)
                        .into_node(),
                );
            } else {
                bars.push(bar);
            }
        }

        Container::new(
            VStack {
                spacing: Some(0.0),
                children: bars,
            }
            .into_node(),
        )
        .width(MINIMAP_WIDTH)
        .bg(MINIMAP_BG)
        .padding_all(4.0)
        .flex_shrink(0.0)
        .into_node()
    }
}
