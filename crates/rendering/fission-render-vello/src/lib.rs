pub mod text;
pub use text::VelloTextMeasurer;
pub use parley;

use anyhow::Result;
use fission_render::{DisplayList, DisplayOp, Renderer, Color as RenderColor, TextStyle as RenderTextStyle};
use vello::kurbo::{Affine, Rect, RoundedRect, Stroke};
// Minimal imports from peniko
use vello::peniko::{Color, Fill, Mix, Blob};
use vello::{Scene, Glyph};
use std::sync::{Arc, Mutex};
use parley::{FontContext, LayoutContext};
use parley::layout::PositionedLayoutItem;
use std::borrow::Cow;
use parley::style::{FontStack, StyleProperty};
use crate::text::ParleyBrush;

pub struct VelloRenderer<'a> {
    scene: &'a mut Scene,
    font_cx: Arc<Mutex<FontContext>>,
    transform_stack: Vec<Affine>,
    current_transform: Affine,
    layer_count_stack: Vec<usize>,
    current_layer_count: usize,
}

impl<'a> VelloRenderer<'a> {
    pub fn new(scene: &'a mut Scene, font_cx: Arc<Mutex<FontContext>>, scale_factor: f64) -> Self {
        Self {
            scene,
            font_cx,
            transform_stack: Vec::new(),
            current_transform: Affine::scale(scale_factor),
            layer_count_stack: Vec::new(),
            current_layer_count: 0,
        }
    }

    // Stubbed image loading
    // fn get_image(&self, _path: &str) -> Option<(Blob, u32, u32)> { None }

    fn render_text(
        &mut self, 
        text: &str, 
        base_size: f32, 
        base_color: RenderColor, 
        bounds: fission_render::LayoutRect, 
        caret_index: Option<usize>,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)]
    ) {
        let mut font_cx = self.font_cx.lock().unwrap();
        let mut layout_cx = LayoutContext::new(); 
        
        let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, false);
        builder.push_default(StyleProperty::FontSize(base_size));
        builder.push_default(StyleProperty::FontStack(FontStack::Source(Cow::Borrowed("system-ui"))));
        let brush = ParleyBrush([base_color.r, base_color.g, base_color.b, base_color.a]);
        builder.push_default(StyleProperty::Brush(brush));
        
        for (range, style) in styles {
            let brush = ParleyBrush([style.color.r, style.color.g, style.color.b, style.color.a]);
            builder.push(StyleProperty::Brush(brush), range.clone());
            builder.push(StyleProperty::FontSize(style.font_size), range.clone());
            if style.underline {
                builder.push(StyleProperty::Underline(true), range.clone());
            }
        }
        
        let mut layout = builder.build(text);
        layout.break_all_lines(if bounds.width() > 0.0 { Some(bounds.width() as f32) } else { None });
        
        // Draw Glyphs
        for line in layout.lines() {
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                    let style = glyph_run.style();
                    let run = glyph_run.run();
                    let font = run.font();
                    let font_size = run.font_size();
                    let brush_data = style.brush.clone();
                    // Use from_rgba8 as per error message suggestion
                    let color = Color::from_rgba8(brush_data.0[0], brush_data.0[1], brush_data.0[2], brush_data.0[3]);
                    
                    let mut x = glyph_run.offset();
                    let y = glyph_run.baseline();

                    let glyphs = glyph_run.glyphs().map(|g| {
                        let gx = x + g.x;
                        let gy = y - g.y;
                        x += g.advance;
                        Glyph {
                            id: g.id as u32,
                            x: gx,
                            y: gy,
                        }
                    });
                    
                    self.scene.draw_glyphs(font)
                        .font_size(font_size)
                        .transform(self.current_transform * Affine::translate((bounds.origin.x as f64, bounds.origin.y as f64)))
                        .brush(color)
                        .draw(Fill::NonZero, glyphs);
                }
            }
        }

        // Draw Caret
        if let Some(idx) = caret_index {
            let mut caret_drawn = false;
            let lines_count = layout.lines().count();
            
            for (i, line) in layout.lines().enumerate() {
                let range = line.text_range();
                let is_last_line = i == lines_count - 1;
                
                if (idx >= range.start && idx < range.end) || (is_last_line && idx == range.end) {
                    let mut x_pos = 0.0;
                    for item in line.items() {
                        if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                            let style_run_range = glyph_run.run().text_range();
                            let line_range = line.text_range();
                            let start = style_run_range.start.max(line_range.start);
                            let end = style_run_range.end.min(line_range.end);
                            let run_range = start..end;

                            if idx >= run_range.start && idx <= run_range.end {
                                let mut local_x = glyph_run.offset();
                                if idx == run_range.start {
                                    x_pos = local_x;
                                    break;
                                }
                                let mut current_char_idx = run_range.start;
                                for glyph in glyph_run.glyphs() {
                                    if current_char_idx >= idx { break; }
                                    local_x += glyph.advance;
                                    current_char_idx += 1;
                                }
                                x_pos = local_x;
                            } else if idx > run_range.end {
                                x_pos = glyph_run.offset() + glyph_run.advance();
                            }
                        }
                    }
                    
                    let metrics = line.metrics();
                    let line_height = metrics.ascent + metrics.descent;
                    
                    let baseline_y = if let Some(item) = line.items().next() {
                        match item {
                            PositionedLayoutItem::GlyphRun(gr) => gr.baseline(),
                            _ => 0.0, 
                        }
                    } else {
                        0.0
                    };

                    let top_y = baseline_y - metrics.ascent;
                    
                    let caret_rect = Rect::new(
                        bounds.origin.x as f64 + x_pos as f64,
                        bounds.origin.y as f64 + top_y as f64,
                        bounds.origin.x as f64 + x_pos as f64 + 2.0,
                        bounds.origin.y as f64 + top_y as f64 + line_height as f64
                    );
                    
                    self.scene.fill(
                        Fill::NonZero,
                        self.current_transform,
                        Color::BLACK,
                        None,
                        &caret_rect
                    );
                    caret_drawn = true;
                    break;
                }
            }
            if !caret_drawn && idx == 0 && text.is_empty() {
                 let caret_rect = Rect::new(
                    bounds.origin.x as f64,
                    bounds.origin.y as f64,
                    bounds.origin.x as f64 + 2.0,
                    bounds.origin.y as f64 + base_size as f64 * 1.2
                );
                self.scene.fill(Fill::NonZero, self.current_transform, Color::BLACK, None, &caret_rect);
            }
        }
    }
}

impl<'a> Renderer for VelloRenderer<'a> {
    fn render(&mut self, list: &DisplayList) -> Result<()> {
        for op in &list.ops {
            match op {
                DisplayOp::Save => {
                    self.transform_stack.push(self.current_transform);
                    self.layer_count_stack.push(self.current_layer_count);
                    self.current_layer_count = 0;
                }
                DisplayOp::Restore => {
                    for _ in 0..self.current_layer_count {
                        self.scene.pop_layer();
                    }
                    if let Some(t) = self.transform_stack.pop() {
                        self.current_transform = t;
                    }
                    if let Some(c) = self.layer_count_stack.pop() {
                        self.current_layer_count = c;
                    }
                }
                DisplayOp::Translate(pt) => {
                    let translation = Affine::translate((pt.x as f64, pt.y as f64));
                    self.current_transform = self.current_transform * translation;
                }
                DisplayOp::ClipRect(rect) => {
                    let r = Rect::new(
                        rect.origin.x as f64,
                        rect.origin.y as f64,
                        (rect.origin.x + rect.size.width) as f64,
                        (rect.origin.y + rect.size.height) as f64,
                    );
                    self.scene.push_layer(Mix::Normal, 1.0, self.current_transform, &r);
                    self.current_layer_count += 1;
                }
                DisplayOp::DrawRect {
                    rect,
                    fill,
                    stroke,
                    corner_radius,
                    ..
                } => {
                    let rect = Rect::new(
                        rect.origin.x as f64,
                        rect.origin.y as f64,
                        (rect.origin.x + rect.size.width) as f64,
                        (rect.origin.y + rect.size.height) as f64,
                    );
                    
                    let shape = RoundedRect::from_rect(rect, *corner_radius as f64);

                    if let Some(f) = fill {
                        let c = Color::from_rgba8(f.color.r, f.color.g, f.color.b, f.color.a);
                        self.scene.fill(Fill::NonZero, self.current_transform, c, None, &shape);
                    }
                    if let Some(s) = stroke {
                        let c = Color::from_rgba8(s.color.r, s.color.g, s.color.b, s.color.a);
                        self.scene.stroke(&Stroke::new(s.width as f64), self.current_transform, c, None, &shape);
                    }
                }
                DisplayOp::DrawText { text, size, color, bounds, caret_index, .. } => {
                    self.render_text(text, *size, *color, *bounds, *caret_index, &[]);
                }
                DisplayOp::DrawRichText { runs, bounds, caret_index, .. } => {
                    let mut full_text = String::new();
                    let mut styles = Vec::new();
                    let mut start = 0;
                    for run in runs {
                        full_text.push_str(&run.text);
                        let end = start + run.text.len();
                        styles.push((start..end, run.style.clone()));
                        start = end;
                    }
                    let (base_size, base_color) = if let Some(first) = runs.first() {
                        (first.style.font_size, first.style.color)
                    } else {
                        (14.0, RenderColor { r: 0, g: 0, b: 0, a: 255 })
                    };
                    
                    self.render_text(&full_text, base_size, base_color, *bounds, *caret_index, &styles);
                }
                DisplayOp::DrawImage { .. } => {
                    // TODO: Implement DrawImage once peniko::Image import is resolved
                }
                _ => {}
            }
        }
        Ok(())
    }
}