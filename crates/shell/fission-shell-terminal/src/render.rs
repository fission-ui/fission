use crate::frame::{TerminalColor, TerminalFrame, TerminalStyle};
use crate::text::TerminalTextMeasurer;
use anyhow::{anyhow, Result};
use fission_ir::op::{Color, Fill, PaintOp, TextRun};
use fission_ir::{CoreIR, NodeId, Op, Semantics};
use fission_layout::{LayoutRect, LayoutSnapshot};
use fission_theme::Theme;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
pub struct TerminalRenderer {
    pub background: TerminalColor,
    pub foreground: TerminalColor,
}

impl TerminalRenderer {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            background: TerminalColor::from(theme.tokens.colors.background),
            foreground: TerminalColor::from(theme.tokens.colors.text_primary),
        }
    }

    pub fn render(
        &self,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        width: u16,
        height: u16,
    ) -> Result<TerminalFrame> {
        let base = TerminalStyle::new(self.foreground, self.background);
        let mut frame = TerminalFrame::new(width, height, base);
        let root = ir
            .root
            .ok_or_else(|| anyhow!("terminal render failed: Core IR has no root"))?;
        self.render_node(root, ir, snapshot, &mut frame)?;
        Ok(frame)
    }

    fn render_node(
        &self,
        node_id: NodeId,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        frame: &mut TerminalFrame,
    ) -> Result<()> {
        let Some(node) = ir.nodes.get(&node_id) else {
            return Ok(());
        };

        match &node.op {
            Op::Paint(op) => self.render_paint(node_id, op, snapshot, frame)?,
            Op::Semantics(semantics) if node.children.is_empty() => {
                self.render_semantic_fallback(node_id, semantics, snapshot, frame);
            }
            _ => {}
        }

        for child in &node.children {
            self.render_node(*child, ir, snapshot, frame)?;
        }
        Ok(())
    }

    fn render_paint(
        &self,
        node_id: NodeId,
        op: &PaintOp,
        snapshot: &LayoutSnapshot,
        frame: &mut TerminalFrame,
    ) -> Result<()> {
        let Some(rect) = snapshot.get_node_rect(node_id) else {
            return Ok(());
        };
        match op {
            PaintOp::DrawRect { fill, stroke, .. } => {
                let bg = fill.as_ref().and_then(fill_color).map(TerminalColor::from);
                if let Some(bg) = bg {
                    fill_frame_rect(frame, rect, bg);
                }
                if let Some(stroke) = stroke {
                    if let Some(color) = fill_color(&stroke.fill) {
                        draw_border(frame, rect, TerminalColor::from(color));
                    }
                }
            }
            PaintOp::DrawText {
                text,
                color,
                underline,
                wrap,
                ..
            } => {
                let style = TerminalStyle {
                    fg: TerminalColor::from(*color),
                    bg: self.background,
                    bold: false,
                    underline: *underline,
                };
                draw_text(frame, rect, text, style, *wrap);
            }
            PaintOp::DrawRichText { runs, wrap, .. } => {
                draw_rich_text(frame, rect, runs, self.background, *wrap);
            }
            PaintOp::DrawImage { .. } | PaintOp::DrawPath { .. } | PaintOp::DrawSvg { .. } => {
                // Unsupported paint operations are rejected by verify_terminal_ir before render.
            }
        }
        Ok(())
    }

    fn render_semantic_fallback(
        &self,
        node_id: NodeId,
        semantics: &Semantics,
        snapshot: &LayoutSnapshot,
        frame: &mut TerminalFrame,
    ) {
        let Some(rect) = snapshot.get_node_rect(node_id) else {
            return;
        };
        let text = semantics
            .label
            .as_ref()
            .or(semantics.value.as_ref())
            .map(String::as_str);
        if let Some(text) = text {
            draw_text(
                frame,
                rect,
                text,
                TerminalStyle::new(self.foreground, self.background),
                true,
            );
        }
    }
}

fn fill_color(fill: &Fill) -> Option<Color> {
    match fill {
        Fill::Solid(color) => Some(*color),
        Fill::LinearGradient { .. } | Fill::RadialGradient { .. } => None,
    }
}

fn rect_to_cells(rect: LayoutRect) -> (i32, i32, i32, i32) {
    let x = rect.x().floor() as i32;
    let y = rect.y().floor() as i32;
    let width = rect.width().ceil().max(0.0) as i32;
    let height = rect.height().ceil().max(0.0) as i32;
    (x, y, width, height)
}

fn fill_frame_rect(frame: &mut TerminalFrame, rect: LayoutRect, color: TerminalColor) {
    let (x, y, width, height) = rect_to_cells(rect);
    if width <= 0 || height <= 0 {
        return;
    }
    let fg = frame
        .get(x.max(0) as u16, y.max(0) as u16)
        .map(|cell| cell.style.fg)
        .unwrap_or(TerminalColor::WHITE);
    frame.fill_rect(x, y, width, height, TerminalStyle::new(fg, color));
}

fn draw_border(frame: &mut TerminalFrame, rect: LayoutRect, color: TerminalColor) {
    let (x, y, width, height) = rect_to_cells(rect);
    if width <= 0 || height <= 0 {
        return;
    }
    let bg = frame
        .get(x.max(0) as u16, y.max(0) as u16)
        .map(|cell| cell.style.bg)
        .unwrap_or(TerminalColor::BLACK);
    let style = TerminalStyle::new(color, bg);
    if width == 1 && height == 1 {
        frame.set(x, y, '+', style);
        return;
    }
    if height == 1 {
        frame.draw_hline(x, y, width, '-', style);
        return;
    }
    if width == 1 {
        frame.draw_vline(x, y, height, '|', style);
        return;
    }
    frame.set(x, y, '+', style);
    frame.set(x + width - 1, y, '+', style);
    frame.set(x, y + height - 1, '+', style);
    frame.set(x + width - 1, y + height - 1, '+', style);
    frame.draw_hline(x + 1, y, width - 2, '-', style);
    frame.draw_hline(x + 1, y + height - 1, width - 2, '-', style);
    frame.draw_vline(x, y + 1, height - 2, '|', style);
    frame.draw_vline(x + width - 1, y + 1, height - 2, '|', style);
}

fn draw_text(
    frame: &mut TerminalFrame,
    rect: LayoutRect,
    text: &str,
    style: TerminalStyle,
    wrap: bool,
) {
    let (x, y, width, height) = rect_to_cells(rect);
    if width <= 0 || height <= 0 {
        return;
    }
    let lines = wrap_text(text, width as usize, wrap);
    for (row, line) in lines.into_iter().take(height as usize).enumerate() {
        draw_text_line(frame, x, y + row as i32, width as usize, &line, style, true);
    }
}

fn draw_rich_text(
    frame: &mut TerminalFrame,
    rect: LayoutRect,
    runs: &[TextRun],
    default_bg: TerminalColor,
    wrap: bool,
) {
    let (x, y, width, height) = rect_to_cells(rect);
    if width <= 0 || height <= 0 {
        return;
    }
    let mut row = 0i32;
    let mut col = 0i32;
    for run in runs {
        let has_explicit_background = run.style.background_color.is_some();
        let style = TerminalStyle {
            fg: TerminalColor::from(run.style.color),
            bg: run
                .style
                .background_color
                .map(TerminalColor::from)
                .unwrap_or(default_bg),
            bold: run.style.font_weight >= 600,
            underline: run.style.underline,
        };
        for line in wrap_text(&run.text, width as usize, wrap) {
            for grapheme in UnicodeSegmentation::graphemes(line.as_str(), true) {
                let w = UnicodeWidthStr::width(grapheme).max(1) as i32;
                if wrap && col > 0 && col + w > width {
                    row += 1;
                    col = 0;
                }
                if row >= height {
                    return;
                }
                let ch = grapheme.chars().next().unwrap_or(' ');
                let style =
                    style_for_cell(frame, x + col, y + row, style, !has_explicit_background);
                frame.set(x + col, y + row, ch, style);
                for extra in 1..w {
                    frame.set(x + col + extra, y + row, ' ', style);
                }
                col += w;
            }
            row += 1;
            col = 0;
            if row >= height {
                return;
            }
        }
    }
}

fn wrap_text(text: &str, width: usize, wrap: bool) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for raw_line in text.split('\n') {
        if !wrap {
            out.push(raw_line.to_string());
            continue;
        }
        let mut line = String::new();
        let mut line_width = 0usize;
        for grapheme in UnicodeSegmentation::graphemes(raw_line, true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme).max(1);
            if line_width > 0 && line_width + grapheme_width > width {
                out.push(std::mem::take(&mut line));
                line_width = 0;
            }
            line.push_str(grapheme);
            line_width += grapheme_width;
        }
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn draw_text_line(
    frame: &mut TerminalFrame,
    x: i32,
    y: i32,
    max_width: usize,
    line: &str,
    style: TerminalStyle,
    preserve_background: bool,
) {
    let mut col = 0i32;
    for grapheme in UnicodeSegmentation::graphemes(line, true) {
        let width = TerminalTextMeasurer::char_width(grapheme.chars().next().unwrap_or(' ')) as i32;
        if col + width > max_width as i32 {
            break;
        }
        let ch = grapheme.chars().next().unwrap_or(' ');
        let style = style_for_cell(frame, x + col, y, style, preserve_background);
        frame.set(x + col, y, ch, style);
        for extra in 1..width {
            frame.set(x + col + extra, y, ' ', style);
        }
        col += width;
    }
}

fn style_for_cell(
    frame: &TerminalFrame,
    x: i32,
    y: i32,
    mut style: TerminalStyle,
    preserve_background: bool,
) -> TerminalStyle {
    if preserve_background && x >= 0 && y >= 0 {
        if let Some(cell) = frame.get(x as u16, y as u16) {
            style.bg = cell.style.bg;
        }
    }
    style
}
