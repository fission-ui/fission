pub mod text;
pub use text::VelloTextMeasurer;
pub use parley;

use anyhow::Result;
use fission_render::{DisplayList, DisplayOp, Renderer};
use vello::kurbo::{Affine, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill, Mix};
use vello::Scene;
use std::sync::{Arc, Mutex};
use parley::{FontContext, LayoutContext};
use parley::layout::Alignment;
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
    pub fn new(scene: &'a mut Scene, font_cx: Arc<Mutex<FontContext>>) -> Self {
        Self {
            scene,
            font_cx,
            transform_stack: Vec::new(),
            current_transform: Affine::IDENTITY,
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
                        let c = Color::rgba8(f.color.r, f.color.g, f.color.b, f.color.a);
                        self.scene.fill(Fill::NonZero, self.current_transform, c, None, &shape);
                    }
                    if let Some(s) = stroke {
                        let c = Color::rgba8(s.color.r, s.color.g, s.color.b, s.color.a);
                        self.scene.stroke(&Stroke::new(s.width as f64), self.current_transform, c, None, &shape);
                    }
                }
                DisplayOp::DrawText { text, size, color, bounds, .. } => {
                    let mut font_cx = self.font_cx.lock().unwrap();
                    let mut layout_cx = LayoutContext::new(); 
                    
                    let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0);
                    builder.push_default(&parley::style::StyleProperty::FontSize(*size));
                    let brush = ParleyBrush([color.r, color.g, color.b, color.a]);
                    builder.push_default(&parley::style::StyleProperty::Brush(brush));
                    
                    let mut layout = builder.build();
                    layout.break_all_lines(if bounds.width() > 0.0 { Some(bounds.width()) } else { None }, Alignment::Start);
                    
                    // Render glyphs (placeholder)
                    // TODO: Iterate layout lines and items to render glyphs using scene.draw_glyphs()
                    // Requires matching parley::GlyphRun with vello::Glyph types.
                }
                _ => {}
            }
        }
        Ok(())
    }
}