use crate::frame::{TerminalColor, TerminalFrame, TerminalStyle};
use crate::text::TerminalTextMeasurer;
use anyhow::{anyhow, Result};
use fission_core::scrollbar::{scrollbar_geometry_for_node, ScrollbarAxis};
use fission_core::ScrollStateMap;
use fission_ir::op::{Color, Fill, LayoutOp, PaintOp, TextRun};
use fission_ir::{CoreIR, Op, Semantics, WidgetId};
use fission_layout::{LayoutPoint, LayoutRect, LayoutSnapshot};
use fission_theme::Theme;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
pub struct TerminalRenderer {
    pub background: TerminalColor,
    pub foreground: TerminalColor,
    pub border: TerminalColor,
    pub accent: TerminalColor,
}

impl TerminalRenderer {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            background: TerminalColor::from(theme.tokens.colors.background),
            foreground: TerminalColor::from(theme.tokens.colors.text_primary),
            border: TerminalColor::from(theme.tokens.colors.border),
            accent: TerminalColor::from(theme.tokens.colors.primary),
        }
    }

    pub fn render(
        &self,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        scroll: &ScrollStateMap,
        width: u16,
        height: u16,
    ) -> Result<TerminalFrame> {
        let base = TerminalStyle::new(self.foreground, self.background);
        let mut frame = TerminalFrame::new(width, height, base);
        let root = ir
            .root
            .ok_or_else(|| anyhow!("terminal render failed: Core IR has no root"))?;
        let ctx = RenderCtx::root(&frame);
        self.render_node(root, ir, snapshot, scroll, &mut frame, ctx)?;
        Ok(frame)
    }

    fn render_node(
        &self,
        node_id: WidgetId,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        scroll: &ScrollStateMap,
        frame: &mut TerminalFrame,
        ctx: RenderCtx,
    ) -> Result<()> {
        let Some(node) = ir.nodes.get(&node_id) else {
            return Ok(());
        };

        match &node.op {
            Op::Paint(op) => self.render_paint(node_id, op, snapshot, frame, ctx)?,
            Op::Semantics(semantics) if node.children.is_empty() => {
                self.render_semantic_fallback(node_id, semantics, snapshot, frame, ctx);
            }
            _ => {}
        }

        let child_ctx = self.child_render_ctx(node_id, &node.op, snapshot, scroll, ctx);
        for child in &node.children {
            self.render_node(*child, ir, snapshot, scroll, frame, child_ctx)?;
        }
        if matches!(node.op, Op::Layout(LayoutOp::Scroll { .. })) {
            self.render_scrollbar(node_id, ir, snapshot, scroll, frame, ctx);
        }
        Ok(())
    }

    fn child_render_ctx(
        &self,
        node_id: WidgetId,
        op: &Op,
        snapshot: &LayoutSnapshot,
        scroll: &ScrollStateMap,
        ctx: RenderCtx,
    ) -> RenderCtx {
        match op {
            Op::Layout(LayoutOp::Scroll { direction, .. }) => {
                let Some(rect) = snapshot.get_node_rect(node_id) else {
                    return ctx;
                };
                let visual_rect = translate_rect(rect, ctx.offset);
                let mut child_ctx = ctx;
                child_ctx.clip = ctx.clip.intersect_layout_rect(visual_rect);
                let offset = scroll.get_offset(node_id);
                match direction {
                    fission_ir::FlexDirection::Row => child_ctx.offset.x -= offset,
                    fission_ir::FlexDirection::Column => child_ctx.offset.y -= offset,
                }
                child_ctx
            }
            Op::Layout(LayoutOp::Clip { path: None }) => {
                let Some(rect) = snapshot.get_node_rect(node_id) else {
                    return ctx;
                };
                RenderCtx {
                    clip: ctx
                        .clip
                        .intersect_layout_rect(translate_rect(rect, ctx.offset)),
                    ..ctx
                }
            }
            _ => ctx,
        }
    }

    fn render_paint(
        &self,
        node_id: WidgetId,
        op: &PaintOp,
        snapshot: &LayoutSnapshot,
        frame: &mut TerminalFrame,
        ctx: RenderCtx,
    ) -> Result<()> {
        let Some(rect) = snapshot.get_node_rect(node_id) else {
            return Ok(());
        };
        let rect = translate_rect(rect, ctx.offset);
        match op {
            PaintOp::DrawRect { fill, stroke, .. } => {
                let bg = fill.as_ref().and_then(fill_color).map(TerminalColor::from);
                if let Some(bg) = bg {
                    fill_frame_rect(frame, rect, bg, ctx.clip);
                }
                if let Some(stroke) = stroke {
                    if let Some(color) = fill_color(&stroke.fill) {
                        draw_border(frame, rect, TerminalColor::from(color), ctx.clip);
                    }
                }
            }
            PaintOp::DrawText {
                text,
                color,
                underline,
                wrap,
                caret_index,
                caret_color,
                ..
            } => {
                let style = TerminalStyle {
                    fg: TerminalColor::from(*color),
                    bg: self.background,
                    bold: false,
                    underline: *underline,
                };
                draw_text(
                    frame,
                    rect,
                    text,
                    style,
                    *wrap,
                    *caret_index,
                    caret_color.map(TerminalColor::from),
                    ctx.clip,
                );
            }
            PaintOp::DrawRichText {
                runs,
                wrap,
                caret_index,
                caret_color,
                ..
            } => {
                draw_rich_text(
                    frame,
                    rect,
                    runs,
                    self.background,
                    *wrap,
                    *caret_index,
                    caret_color.map(TerminalColor::from),
                    ctx.clip,
                );
            }
            PaintOp::DrawImage { .. } | PaintOp::DrawPath { .. } | PaintOp::DrawSvg { .. } => {
                // Unsupported paint operations are rejected by verify_terminal_ir before render.
            }
        }
        Ok(())
    }

    fn render_semantic_fallback(
        &self,
        node_id: WidgetId,
        semantics: &Semantics,
        snapshot: &LayoutSnapshot,
        frame: &mut TerminalFrame,
        ctx: RenderCtx,
    ) {
        let Some(rect) = snapshot.get_node_rect(node_id) else {
            return;
        };
        let rect = translate_rect(rect, ctx.offset);
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
                None,
                None,
                ctx.clip,
            );
        }
    }

    fn render_scrollbar(
        &self,
        node_id: WidgetId,
        ir: &CoreIR,
        snapshot: &LayoutSnapshot,
        scroll: &ScrollStateMap,
        frame: &mut TerminalFrame,
        ctx: RenderCtx,
    ) {
        let Some(geometry) = scrollbar_geometry_for_node(ir, snapshot, scroll, node_id) else {
            return;
        };
        let rail = translate_rect(geometry.rail_rect, ctx.offset);
        let thumb = translate_rect(geometry.thumb_rect, ctx.offset);
        let rail_style = TerminalStyle::new(self.border, self.background);
        let thumb_style = TerminalStyle::new(self.accent, self.background);
        match geometry.axis {
            ScrollbarAxis::Vertical => {
                let (x, y, width, height) = rect_to_cells(rail);
                for row in y..y + height {
                    for col in x..x + width {
                        set_cell(frame, col, row, '.', rail_style, ctx.clip);
                    }
                }
                let (thumb_x, thumb_y, thumb_w, thumb_h) = rect_to_cells(thumb);
                for row in thumb_y..thumb_y + thumb_h.max(1) {
                    for col in thumb_x..thumb_x + thumb_w.max(1) {
                        set_cell(frame, col, row, '#', thumb_style, ctx.clip);
                    }
                }
            }
            ScrollbarAxis::Horizontal => {
                let (x, y, width, height) = rect_to_cells(rail);
                for col in x..x + width {
                    for row in y..y + height {
                        set_cell(frame, col, row, '.', rail_style, ctx.clip);
                    }
                }
                let (thumb_x, thumb_y, thumb_w, thumb_h) = rect_to_cells(thumb);
                for col in thumb_x..thumb_x + thumb_w.max(1) {
                    for row in thumb_y..thumb_y + thumb_h.max(1) {
                        set_cell(frame, col, row, '#', thumb_style, ctx.clip);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RenderCtx {
    offset: LayoutPoint,
    clip: CellClip,
}

impl RenderCtx {
    fn root(frame: &TerminalFrame) -> Self {
        Self {
            offset: LayoutPoint::ZERO,
            clip: CellClip {
                left: 0,
                top: 0,
                right: i32::from(frame.width),
                bottom: i32::from(frame.height),
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CellClip {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl CellClip {
    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    fn intersect_layout_rect(self, rect: LayoutRect) -> Self {
        let (x, y, width, height) = rect_to_cells(rect);
        Self {
            left: self.left.max(x),
            top: self.top.max(y),
            right: self.right.min(x + width),
            bottom: self.bottom.min(y + height),
        }
    }
}

fn translate_rect(rect: LayoutRect, offset: LayoutPoint) -> LayoutRect {
    LayoutRect::new(
        rect.origin.x + offset.x,
        rect.origin.y + offset.y,
        rect.size.width,
        rect.size.height,
    )
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

fn fill_frame_rect(
    frame: &mut TerminalFrame,
    rect: LayoutRect,
    color: TerminalColor,
    clip: CellClip,
) {
    let (x, y, width, height) = rect_to_cells(rect);
    if width <= 0 || height <= 0 {
        return;
    }
    let fg = frame
        .get(x.max(0) as u16, y.max(0) as u16)
        .map(|cell| cell.style.fg)
        .unwrap_or(TerminalColor::WHITE);
    let style = TerminalStyle::new(fg, color);
    for row in y.max(clip.top)..(y + height).min(clip.bottom) {
        for col in x.max(clip.left)..(x + width).min(clip.right) {
            frame.set(col, row, ' ', style);
        }
    }
}

fn draw_border(frame: &mut TerminalFrame, rect: LayoutRect, color: TerminalColor, clip: CellClip) {
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
        set_cell(frame, x, y, '+', style, clip);
        return;
    }
    if height == 1 {
        draw_hline(frame, x, y, width, '-', style, clip);
        return;
    }
    if width == 1 {
        draw_vline(frame, x, y, height, '|', style, clip);
        return;
    }
    set_cell(frame, x, y, '+', style, clip);
    set_cell(frame, x + width - 1, y, '+', style, clip);
    set_cell(frame, x, y + height - 1, '+', style, clip);
    set_cell(frame, x + width - 1, y + height - 1, '+', style, clip);
    draw_hline(frame, x + 1, y, width - 2, '-', style, clip);
    draw_hline(frame, x + 1, y + height - 1, width - 2, '-', style, clip);
    draw_vline(frame, x, y + 1, height - 2, '|', style, clip);
    draw_vline(frame, x + width - 1, y + 1, height - 2, '|', style, clip);
}

fn draw_text(
    frame: &mut TerminalFrame,
    rect: LayoutRect,
    text: &str,
    style: TerminalStyle,
    wrap: bool,
    caret_index: Option<usize>,
    caret_color: Option<TerminalColor>,
    clip: CellClip,
) {
    let (x, y, width, height) = rect_to_cells(rect);
    if width <= 0 || height <= 0 {
        return;
    }
    let lines = wrap_text(text, width as usize, wrap);
    for (row, line) in lines.into_iter().take(height as usize).enumerate() {
        draw_text_line(
            frame,
            x,
            y + row as i32,
            width as usize,
            &line,
            style,
            true,
            clip,
        );
    }
    if let Some(caret) = caret_index {
        draw_caret(
            frame,
            rect,
            text,
            wrap,
            caret,
            caret_color.unwrap_or(style.fg),
            clip,
        );
    }
}

fn draw_rich_text(
    frame: &mut TerminalFrame,
    rect: LayoutRect,
    runs: &[TextRun],
    default_bg: TerminalColor,
    wrap: bool,
    caret_index: Option<usize>,
    caret_color: Option<TerminalColor>,
    clip: CellClip,
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
                set_cell(frame, x + col, y + row, ch, style, clip);
                for extra in 1..w {
                    set_cell(frame, x + col + extra, y + row, ' ', style, clip);
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
    if let Some(caret) = caret_index {
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        draw_caret(
            frame,
            rect,
            &text,
            wrap,
            caret,
            caret_color.unwrap_or(TerminalColor::WHITE),
            clip,
        );
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
    clip: CellClip,
) {
    let mut col = 0i32;
    for grapheme in UnicodeSegmentation::graphemes(line, true) {
        let width = TerminalTextMeasurer::char_width(grapheme.chars().next().unwrap_or(' ')) as i32;
        if col + width > max_width as i32 {
            break;
        }
        let ch = grapheme.chars().next().unwrap_or(' ');
        let style = style_for_cell(frame, x + col, y, style, preserve_background);
        set_cell(frame, x + col, y, ch, style, clip);
        for extra in 1..width {
            set_cell(frame, x + col + extra, y, ' ', style, clip);
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

fn draw_caret(
    frame: &mut TerminalFrame,
    rect: LayoutRect,
    text: &str,
    wrap: bool,
    caret_index: usize,
    caret_color: TerminalColor,
    clip: CellClip,
) {
    let (x, y, width, height) = rect_to_cells(rect);
    if width <= 0 || height <= 0 {
        return;
    }
    let (row, col) = caret_position(text, width, wrap, caret_index);
    if row >= height {
        return;
    }
    let caret_x = x + col.clamp(0, width.saturating_sub(1));
    let caret_y = y + row;
    let bg = frame
        .get(caret_x.max(0) as u16, caret_y.max(0) as u16)
        .map(|cell| cell.style.bg)
        .unwrap_or(TerminalColor::BLACK);
    set_cell(
        frame,
        caret_x,
        caret_y,
        '|',
        TerminalStyle::new(caret_color, bg),
        clip,
    );
}

fn caret_position(text: &str, width: i32, wrap: bool, caret_index: usize) -> (i32, i32) {
    let mut row = 0i32;
    let mut col = 0i32;
    let caret_index = caret_index.min(text.len());
    for (idx, grapheme) in UnicodeSegmentation::grapheme_indices(text, true) {
        if idx >= caret_index {
            break;
        }
        if grapheme == "\n" {
            row += 1;
            col = 0;
            continue;
        }
        let w = TerminalTextMeasurer::char_width(grapheme.chars().next().unwrap_or(' ')) as i32;
        if wrap && col > 0 && col + w > width {
            row += 1;
            col = 0;
        }
        col += w.max(1);
    }
    (row, col)
}

fn set_cell(
    frame: &mut TerminalFrame,
    x: i32,
    y: i32,
    ch: char,
    style: TerminalStyle,
    clip: CellClip,
) {
    if clip.contains(x, y) {
        frame.set(x, y, ch, style);
    }
}

fn draw_hline(
    frame: &mut TerminalFrame,
    x: i32,
    y: i32,
    width: i32,
    ch: char,
    style: TerminalStyle,
    clip: CellClip,
) {
    for col in x..x + width {
        set_cell(frame, col, y, ch, style, clip);
    }
}

fn draw_vline(
    frame: &mut TerminalFrame,
    x: i32,
    y: i32,
    height: i32,
    ch: char,
    style: TerminalStyle,
    clip: CellClip,
) {
    for row in y..y + height {
        set_cell(frame, x, row, ch, style, clip);
    }
}
