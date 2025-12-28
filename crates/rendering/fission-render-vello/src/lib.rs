use anyhow::Result;
use fission_render::{DisplayList, DisplayOp, Renderer};
use vello::kurbo::{Affine, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill, Mix, Image, Format, Blob};
use vello::{Scene, Glyph};
use std::sync::{Arc, Mutex};
use parley::{FontContext, LayoutContext};
use parley::layout::PositionedLayoutItem;
use std::borrow::Cow;
use parley::style::{FontStack, StyleProperty};
use crate::text::ParleyBrush;
use image::GenericImageView;
use std::path::Path;

pub struct VelloRenderer<'a> {
    scene: &'a mut Scene,
    font_cx: Arc<Mutex<FontContext>>,
    transform_stack: Vec<Affine>,
    current_transform: Affine,
    layer_count_stack: Vec<usize>,
    current_layer_count: usize,
    // Simple in-memory cache to avoid re-loading/decoding every frame
    // In a real system this belongs in an AssetManager
    image_cache: Arc<Mutex<std::collections::HashMap<String, Option<(Image, u32, u32)>>>>, 
}

// Global cache to persist across frames (since VelloRenderer is recreated per frame in shell)
lazy_static::lazy_static! {
    static ref GLOBAL_IMAGE_CACHE: Mutex<std::collections::HashMap<String, Option<(Image, u32, u32)>>> = Mutex::new(std::collections::HashMap::new());
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
            image_cache: Arc::new(Mutex::new(std::collections::HashMap::new())), // Unused now using global
        }
    }

    fn get_image(&self, path: &str) -> Option<(Image, u32, u32)> {
        let mut cache = GLOBAL_IMAGE_CACHE.lock().unwrap();
        if let Some(entry) = cache.get(path) {
            return entry.clone();
        }

        // Load image
        let result = (|| {
            let img = image::open(path).ok()?;
            let (w, h) = img.dimensions();
            let rgba = img.into_rgba8();
            let data = rgba.into_raw();
            let blob = Blob::new(Arc::new(data));
            let image = Image::new(blob, Format::Rgba8, w, h);
            Some((image, w, h))
        })();

        cache.insert(path.to_string(), result.clone());
        result
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
                    
                    let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, false);
                    builder.push_default(StyleProperty::FontSize(*size));
                    builder.push_default(StyleProperty::FontStack(FontStack::Source(Cow::Borrowed("system-ui"))));
                    let brush = ParleyBrush([color.r, color.g, color.b, color.a]);
                    builder.push_default(StyleProperty::Brush(brush));
                    
                    let mut layout = builder.build(text);
                    layout.break_all_lines(if bounds.width() > 0.0 { Some(bounds.width() + 1.0) } else { None });
                    
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
                DisplayOp::DrawImage { rect, source, fit, .. } => {
                    if let Some((image, w, h)) = self.get_image(source) {
                        let target_rect = Rect::new(
                            rect.origin.x as f64,
                            rect.origin.y as f64,
                            (rect.origin.x + rect.size.width) as f64,
                            (rect.origin.y + rect.size.height) as f64,
                        );

                        // Calculate fit transform
                        let img_w = w as f64;
                        let img_h = h as f64;
                        let target_w = target_rect.width();
                        let target_h = target_rect.height();

                        // Default to Contain
                        let scale_x = target_w / img_w;
                        let scale_y = target_h / img_h;
                        let scale = scale_x.min(scale_y);
                        
                        let draw_w = img_w * scale;
                        let draw_h = img_h * scale;
                        
                        let offset_x = (target_w - draw_w) / 2.0;
                        let offset_y = (target_h - draw_h) / 2.0;

                        let transform = self.current_transform 
                            * Affine::translate((target_rect.x0 + offset_x, target_rect.y0 + offset_y))
                            * Affine::scale(scale);

                        self.scene.draw_image(&image, transform);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}