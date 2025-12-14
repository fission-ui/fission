use fission_render::{Renderer, DisplayList, DisplayOp, Color, LayoutRect, LayoutPoint, LayoutUnit, BoxShadow, TextMeasurer, ImageFit};
use skia_safe::{Canvas, Paint, Rect, Color as SkColor, FontMgr, MaskFilter, RRect, BlurStyle, Vector}; 
use skia_safe::font::Font;
use skia_safe::font_style::FontStyle;
use skia_safe::Typeface; 
use skia_safe::wrapper::NativeTransmutableWrapper; // For unwrap
use anyhow::Result;
use std::fs;

pub struct SkiaRenderer<'a> {
    canvas: &'a Canvas, 
}

impl<'a> SkiaRenderer<'a> {
    pub fn new(canvas: &'a Canvas) -> Self {
        Self { canvas }
    }
}

pub struct SkiaTextMeasurer;

impl TextMeasurer for SkiaTextMeasurer {
    fn measure(&self, text: &str, font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        let font_mgr = FontMgr::new();
        let typeface = load_typeface(&font_mgr);
        let font = Font::new(typeface, font_size);
        let (_width, bounds) = font.measure_str(text, None);
        (bounds.width(), bounds.height())
    }
}

// Helper to convert Fission types to Skia types
fn to_skia_rect(r: &LayoutRect) -> Rect {
    Rect::from_xywh(r.x(), r.y(), r.width(), r.height())
}

fn to_skia_color(c: &Color) -> SkColor {
    SkColor::from_argb(c.a, c.r, c.g, c.b)
}

fn load_typeface(font_mgr: &FontMgr) -> Typeface {
    let families = ["sans-serif", "Helvetica", "Arial", ".AppleSystemUIFont", "Segoe UI", "Roboto"];
    
    for family in families {
        if let Some(tf) = font_mgr.match_family_style(family, FontStyle::default()) {
            return tf;
        }
    }

    if let Some(tf) = font_mgr.match_family_style("", FontStyle::default()) {
        return tf;
    }

    panic!("Failed to load any system font (tried common families and default)");
}

impl<'a> Renderer for SkiaRenderer<'a> {
    fn render(&mut self, display_list: &DisplayList) -> Result<()> {
        self.canvas.clear(SkColor::WHITE); // Clear background

        for op in &display_list.ops {
            match op {
                DisplayOp::DrawRect { rect, fill, stroke, corner_radius, shadow, bounds, node_id } => {
                    let sk_rect = to_skia_rect(rect);
                    
                    let rrect = RRect::new_rect_xy(&sk_rect, *corner_radius, *corner_radius);

                    if let Some(s) = shadow {
                        let mut shadow_paint = Paint::default();
                        shadow_paint.set_color(to_skia_color(&s.color));
                        shadow_paint.set_anti_alias(true);
                        shadow_paint.set_mask_filter(MaskFilter::blur(
                            BlurStyle::Normal,
                            s.blur_radius,
                            false,
                        ));
                        
                        let shadow_rect: Rect = sk_rect.with_offset(s.offset);
                        let shadow_rrect = RRect::new_rect_xy(&shadow_rect, *corner_radius, *corner_radius);
                        self.canvas.draw_rrect(&shadow_rrect, &shadow_paint);
                    }

                    if let Some(f) = fill {
                        let mut paint = Paint::default();
                        paint.set_anti_alias(true);
                        paint.set_color(to_skia_color(&f.color));
                        paint.set_style(skia_safe::paint::Style::Fill);
                        self.canvas.draw_rrect(&rrect, &paint);
                    }

                    if let Some(s) = stroke {
                        let mut paint = Paint::default();
                        paint.set_anti_alias(true);
                        paint.set_color(to_skia_color(&s.color));
                        paint.set_style(skia_safe::paint::Style::Stroke);
                        paint.set_stroke_width(s.width);
                        self.canvas.draw_rrect(&rrect, &paint);
                    }
                }
                DisplayOp::DrawText { text, position, size, color, bounds, .. } => {
                    let mut paint = Paint::default();
                    paint.set_color(to_skia_color(color));
                    paint.set_anti_alias(true);

                    let font_manager = FontMgr::new();
                    let typeface = load_typeface(&font_manager);
                    let font = Font::new(typeface, *size);

                    let (_scale, text_metrics) = font.metrics(); 
                    let ascender = text_metrics.ascent.abs(); 
                    
                    let text_draw_y = position.y + ascender; 

                    self.canvas.draw_str(
                        text, 
                        (position.x, text_draw_y),
                        &font, 
                        &paint
                    );
                }
                DisplayOp::DrawImage { rect, source, fit: _, .. } => {
                    if let Ok(bytes) = fs::read(source) {
                        if let Some(image) = skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&bytes)) {
                            let mut paint = Paint::default();
                            paint.set_anti_alias(true);
                            
                            let src_rect = Rect::from_wh(image.width() as f32, image.height() as f32);
                            let dst_rect = to_skia_rect(rect);
                            
                            self.canvas.draw_image_rect(&image, Some((&src_rect, skia_safe::canvas::SrcRectConstraint::Strict)), dst_rect, &paint);
                        } else {
                            eprintln!("Failed to decode image: {}", source);
                        }
                    } else {
                        eprintln!("Failed to read image file: {}", source);
                    }
                }
                DisplayOp::Save => {
                    self.canvas.save();
                }
                DisplayOp::Restore => {
                    self.canvas.restore();
                }
                DisplayOp::Translate(pt) => {
                    self.canvas.translate((pt.x, pt.y));
                }
                DisplayOp::ClipRect(rect) => {
                    self.canvas.clip_rect(to_skia_rect(rect), skia_safe::ClipOp::Intersect, true);
                }
                _ => {
                    // Implement other ops as needed
                }
            }
        }
        Ok(())}
}
