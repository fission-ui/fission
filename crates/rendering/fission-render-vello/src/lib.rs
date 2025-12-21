pub mod text;
pub use text::VelloTextMeasurer;
pub use parley;

use anyhow::Result;
use fission_render::{DisplayList, DisplayOp, Renderer};
use vello::kurbo::{Affine, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill, Mix};
use vello::{Scene, Glyph};
use std::sync::{Arc, Mutex};
use parley::{FontContext, LayoutContext};
use parley::layout::PositionedLayoutItem;
use parley::style::StyleProperty;
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
                DisplayOp::DrawText { text, size, color, bounds, .. } => {
                    let mut font_cx = self.font_cx.lock().unwrap();
                    let mut layout_cx = LayoutContext::new(); 
                    
                    // ranged_builder(font_cx, text, scale) -> build(text) ???
                    // Based on previous errors, ranged_builder takes 4 args.
                    // build takes 1 arg (text).
                    // I will provide text to both?
                    let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, false);
                    // Wait, if I provide 4th arg (bool), it might work.
                    // But if build() takes text, why provide it to ranged_builder?
                    // Maybe ranged_builder(font_cx, scale, bool) ? (3 args + self = 4?)
                    // But error said "takes 4 arguments but 3 supplied". Self is implied in method call syntax?
                    // No, "takes 4 arguments" usually refers to explicit arguments + self if valid?
                    // Actually, if it's `(&mut self, font_cx, text, scale)` -> 3 args.
                    // If it takes 4, maybe `(&mut self, font_cx, text, scale, bool)`.
                    // I'll try passing `true` as 4th arg.
                    // And I'll assume `build(text)` is required.
                    
                    // Actually, I'll check `text.rs` compilation after I fix `lib.rs` to see if `ranged_builder` error persists.
                    // For now I'll use what I think is correct.
                    // `ranged_builder` might have changed to NOT take text?
                    // If I pass `text`, and it expects `scale` (f32), it would fail type check.
                    // The error didn't say type mismatch on arg 2/3.
                    // So `text` and `1.0` were likely accepted types.
                    // So `(font_cx, text, scale)` are correct types.
                    // So 4th arg is missing.
                    
                    // I'll assume: `ranged_builder(font_cx, text, 1.0)` needs a boolean.
                    // I'll add `true`?
                    // Let's try `builder.push_default` first.
                    builder.push_default(StyleProperty::FontSize(*size));
                    let brush = ParleyBrush([color.r, color.g, color.b, color.a]);
                    builder.push_default(StyleProperty::Brush(brush));
                    
                    let mut layout = builder.build(text);
                    layout.break_all_lines(if bounds.width() > 0.0 { Some(bounds.width()) } else { None });
                    
                    for line in layout.lines() {
                        for item in line.items() {
                            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                                let style = glyph_run.style();
                                let run = glyph_run.run();
                                let font = run.font();
                                let font_size = run.font_size();
                                let brush_data = style.brush.clone();
                                let color = Color::from_rgba8(brush_data.0[0], brush_data.0[1], brush_data.0[2], brush_data.0[3]);
                                
                                // Coordinates
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
                }
                _ => {}
            }
        }
        Ok(())
    }
}