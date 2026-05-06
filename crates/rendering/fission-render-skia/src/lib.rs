use anyhow::Result;
use fission_layout::TextMeasurer;
use fission_theme::fonts;
use fission_render::{
    BoxShadow, Color as RenderColor, DisplayList, DisplayOp, Fill, ImageFit, RenderScene,
    Renderer, Stroke,
};
use skia_safe::wrapper::NativeTransmutableWrapper;
use skia_safe::{
    BlurStyle, Canvas, Color as SkColor, Data, Font, FontArguments, FontMetrics, FontMgr,
    MaskFilter, Matrix, Paint, RRect, Rect, Typeface, Vector,
};
use skia_safe::textlayout::{ParagraphBuilder, ParagraphStyle, TextDecoration, TextStyle, FontCollection, TypefaceFontProvider};
use once_cell::sync::OnceCell;
use std::fs;

pub struct SkiaRenderer<'a> {
    canvas: &'a Canvas,
    font_mgr: FontMgr,
}

impl<'a> SkiaRenderer<'a> {
    pub fn new(canvas: &'a Canvas) -> Self {
        Self {
            canvas,
            font_mgr: FontMgr::new(),
        }
    }
}

pub struct SkiaTextMeasurer;

static DEFAULT_TYPEFACE: OnceCell<Typeface> = OnceCell::new();

fn default_typeface() -> &'static Typeface {
    DEFAULT_TYPEFACE.get_or_init(|| {
        let fm = FontMgr::new();
        fm.new_from_data(fission_theme::fonts::default_font_bytes(), None)
            .expect("Failed to load bundled UI font")
    })
}

impl SkiaTextMeasurer {
    // Private helper to build a paragraph
    fn build_paragraph_internal(&self, text: &str, font_size: f32, color: SkColor, max_width: Option<f32>) -> skia_safe::textlayout::Paragraph {
        let mut collection = FontCollection::new();
        let mut provider = TypefaceFontProvider::new();
        provider.register_typeface(default_typeface().clone(), Some("Default"));
        collection.set_asset_font_manager(Some(provider.into()));

        let mut style = ParagraphStyle::new();
        let mut ts = TextStyle::new();
        ts.set_font_families(&["Default"]);
        ts.set_font_size(font_size);
        ts.set_color(color);
        style.set_text_style(&ts);

        let mut builder = ParagraphBuilder::new(&style, collection);
        builder.add_text(text);
        
        let mut paragraph = builder.build();
        let width = max_width.unwrap_or(10000.0); 
        paragraph.layout(width);
        paragraph
    }
}

impl TextMeasurer for SkiaTextMeasurer {
    fn measure(&self, text: &str, font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let paragraph = self.build_paragraph_internal(text, font_size, SkColor::BLACK, available_width);
        (paragraph.max_width(), paragraph.height())
    }

    fn hit_test(&self, text: &str, font_size: f32, available_width: Option<f32>, x: f32, y: f32) -> usize {
        let paragraph = self.build_paragraph_internal(text, font_size, SkColor::BLACK, available_width);
        let pos = paragraph.get_glyph_position_at_coordinate((x, y));
        pos.position as usize
    }

    fn get_line_metrics(&self, text: &str, font_size: f32, available_width: Option<f32>) -> Vec<fission_layout::LineMetric> {
        let paragraph = self.build_paragraph_internal(text, font_size, SkColor::BLACK, available_width);
        paragraph.get_line_metrics().into_iter().map(|lm| fission_layout::LineMetric {
            start_index: lm.start_index as usize,
            end_index: lm.end_index as usize,
            baseline: lm.baseline as f32,
            height: lm.height as f32,
            width: lm.width as f32,
        }).collect()
    }

    fn get_caret_position(&self, text: &str, font_size: f32, available_width: Option<f32>, caret_index: usize) -> (f32, f32) {
        let paragraph = self.build_paragraph_internal(text, font_size, SkColor::BLACK, available_width);
        
        let line_metrics = paragraph.get_line_metrics();
        let mut caret_x = 0.0;
        let mut caret_y = 0.0;

        for lm in &line_metrics {
            if caret_index >= lm.start_index as usize && caret_index <= lm.end_index as usize {
                // Caret is on this line. Calculate X position within this line.
                let text_on_line = &text[lm.start_index as usize..caret_index];
                let (width_until_caret, _) = self.measure(text_on_line, font_size, Some(lm.width as f32)); // Measure sub-segment, constrained by line width
                caret_x = width_until_caret;
                caret_y = lm.baseline as f32; // Use baseline as Y for caret position.
                break;
            }
        }
        (caret_x, caret_y)
    }
}

impl<'r> Renderer for SkiaRenderer<'r> {
    fn render_scene(&mut self, scene: &RenderScene) -> Result<()> {
        self.canvas.clear(SkColor::WHITE);
        let display_list = scene.flatten();
        self.render_ops(&display_list)
    }
}

impl<'r> SkiaRenderer<'r> {
    fn render_ops(&mut self, display_list: &DisplayList) -> Result<()> {
        for op in &display_list.ops {
            match op {
                DisplayOp::Save => {
                    self.canvas.save();
                }
                DisplayOp::Restore => {
                    self.canvas.restore();
                }
                DisplayOp::ClipRect(rect) => {
                    self.canvas.clip_rect(
                        Rect::new(rect.x(), rect.y(), rect.right(), rect.bottom()),
                        skia_safe::ClipOp::Intersect,
                        true,
                    );
                }
                DisplayOp::ClipRoundedRect { rect, radius } => {
                    let rrect = RRect::new_rect_xy(
                        Rect::new(rect.x(), rect.y(), rect.right(), rect.bottom()),
                        *radius,
                        *radius,
                    );
                    self.canvas.clip_rrect(rrect, skia_safe::ClipOp::Intersect, true);
                }
                DisplayOp::OpacityLayer { alpha, bounds } => {
                    let rect = Rect::new(bounds.x(), bounds.y(), bounds.right(), bounds.bottom());
                    self.canvas.save_layer_alpha_f(Some(&rect), *alpha);
                }
                DisplayOp::Translate(point) => {
                    self.canvas.translate((point.x, point.y));
                }
                DisplayOp::Transform(matrix) => {
                    let m00 = matrix[0];
                    let m10 = matrix[1];
                    let m01 = matrix[4];
                    let m11 = matrix[5];
                    let m03 = matrix[12];
                    let m13 = matrix[13];
                    let m = Matrix::new_all(
                        m00, m01, m03,
                        m10, m11, m13,
                        0.0, 0.0, 1.0,
                    );
                    self.canvas.concat(&m);
                }
                DisplayOp::CachedScene { list, .. } => {
                    self.render_ops(list)?;
                }
                DisplayOp::DrawRect {
                    rect,
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                    bounds,
                    node_id,
                } => {
                    if let Some(shadow) = shadow {
                        let mut shadow_paint = Paint::default();
                        shadow_paint.set_color(SkColor::from_argb(
                            shadow.color.a,
                            shadow.color.r,
                            shadow.color.g,
                            shadow.color.b,
                        ));
                        shadow_paint.set_mask_filter(MaskFilter::blur(
                            BlurStyle::Normal,
                            shadow.blur_radius,
                            None,
                        ));

                        let shadow_rect = Rect::new(
                            rect.x() + shadow.offset.0,
                            rect.y() + shadow.offset.1,
                            rect.right() + shadow.offset.0,
                            rect.bottom() + shadow.offset.1,
                        );

                        if *corner_radius > 0.0 {
                            self.canvas.draw_rrect(
                                RRect::new_rect_xy(shadow_rect, *corner_radius, *corner_radius),
                                &shadow_paint,
                            );
                        } else {
                            self.canvas.draw_rect(shadow_rect, &shadow_paint);
                        }
                    }

                    if let Some(fill) = fill {
                        let mut paint = Paint::default();
                        paint.set_color(SkColor::from_argb(
                            fill.color.a,
                            fill.color.r,
                            fill.color.g,
                            fill.color.b,
                        ));

                        if *corner_radius > 0.0 {
                            self.canvas.draw_rrect(
                                RRect::new_rect_xy(
                                    Rect::new(rect.x(), rect.y(), rect.right(), rect.bottom()),
                                    *corner_radius,
                                    *corner_radius,
                                ),
                                &paint,
                            );
                        } else {
                            self.canvas.draw_rect(
                                Rect::new(rect.x(), rect.y(), rect.right(), rect.bottom()),
                                &paint,
                            );
                        }
                    }

                    if let Some(stroke) = stroke {
                        let mut paint = Paint::default();
                        paint.set_style(skia_safe::PaintStyle::Stroke);
                        paint.set_color(SkColor::from_argb(
                            stroke.color.a,
                            stroke.color.r,
                            stroke.color.g,
                            stroke.color.b,
                        ));
                        paint.set_stroke_width(stroke.width);

                        if *corner_radius > 0.0 {
                            self.canvas.draw_rrect(
                                RRect::new_rect_xy(
                                    Rect::new(rect.x(), rect.y(), rect.right(), rect.bottom()),
                                    *corner_radius,
                                    *corner_radius,
                                ),
                                &paint,
                            );
                        } else {
                            self.canvas.draw_rect(
                                Rect::new(rect.x(), rect.y(), rect.right(), rect.bottom()),
                                &paint,
                            );
                        }
                    }
                }
                DisplayOp::DrawText {
                    text,
                    position,
                    size,
                    color,
                    bounds,
                    underline,
                    wrap,
                    ..
                } => {
                    let sk_color = SkColor::from_argb(color.a, color.r, color.g, color.b);
                    // Use bounds width if available, otherwise unbounded
                    let max_width = if *wrap && bounds.width() > 0.0 {
                        Some(bounds.width())
                    } else {
                        None
                    };
                    
                    let mut collection = FontCollection::new();
                    let mut provider = TypefaceFontProvider::new();
                    provider.register_typeface(default_typeface().clone(), Some("Default"));
                    collection.set_asset_font_manager(Some(provider.into()));

                    let mut style = ParagraphStyle::new();
                    let mut ts = skia_safe::textlayout::TextStyle::new();
                    ts.set_font_families(&["Default"]);
                    ts.set_font_size(*size);
                    ts.set_color(sk_color);
                    style.set_text_style(&ts);

                    let mut builder = ParagraphBuilder::new(&style, collection);
                    builder.add_text(text);
                    
                    let mut paragraph = builder.build();
                    let width = max_width.unwrap_or(10000.0); 
                    paragraph.layout(width);

                    paragraph.paint(self.canvas, (position.x, position.y));
                }
                DisplayOp::DrawRichText {
                    runs,
                    position,
                    bounds,
                    wrap,
                    ..
                } => {
                    let mut collection = FontCollection::new();
                    let mut provider = TypefaceFontProvider::new();
                    provider.register_typeface(default_typeface().clone(), Some("Default"));
                    collection.set_asset_font_manager(Some(provider.into()));

                    let paragraph_style = ParagraphStyle::new();
                    let mut paragraph_builder = ParagraphBuilder::new(&paragraph_style, collection);

                    for run in runs {
                        let mut text_style = skia_safe::textlayout::TextStyle::new();
                        text_style.set_font_families(&["Default"]);
                        text_style.set_font_size(run.style.font_size);
                        text_style.set_color(SkColor::from_argb(run.style.color.a, run.style.color.r, run.style.color.g, run.style.color.b));
                        paragraph_builder.push_style(&text_style);
                        paragraph_builder.add_text(&run.text);
                    }
                    
                    let mut paragraph = paragraph_builder.build();
                    let max_width = if *wrap && bounds.width() > 0.0 {
                        Some(bounds.width())
                    } else {
                        None
                    };
                    let width = max_width.unwrap_or(10000.0); 
                    paragraph.layout(width);

                    paragraph.paint(self.canvas, (position.x, position.y));
                }
                DisplayOp::DrawImage {
                    rect,
                    source,
                    fit,
                    bounds,
                    node_id,
                } => {
                    if let Ok(data) = fs::read(source) {
                        if let Some(image) =
                            skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&data))
                        {
                            let src_rect =
                                Rect::from_wh(image.width() as f32, image.height() as f32);
                            let rect_w = rect.width();
                            let rect_h = rect.height();
                            let img_w = image.width() as f32;
                            let img_h = image.height() as f32;
                            if rect_w <= 0.0 || rect_h <= 0.0 || img_w <= 0.0 || img_h <= 0.0 {
                                continue;
                            }

                            let (dst_x, dst_y, dst_w, dst_h) = match fit {
                                ImageFit::Fill => (rect.x(), rect.y(), rect_w, rect_h),
                                ImageFit::Contain => {
                                    let scale = (rect_w / img_w).min(rect_h / img_h);
                                    let w = img_w * scale;
                                    let h = img_h * scale;
                                    (
                                        rect.x() + (rect_w - w) / 2.0,
                                        rect.y() + (rect_h - h) / 2.0,
                                        w,
                                        h,
                                    )
                                }
                                ImageFit::Cover => {
                                    let scale = (rect_w / img_w).max(rect_h / img_h);
                                    let w = img_w * scale;
                                    let h = img_h * scale;
                                    (
                                        rect.x() + (rect_w - w) / 2.0,
                                        rect.y() + (rect_h - h) / 2.0,
                                        w,
                                        h,
                                    )
                                }
                                ImageFit::None => (rect.x(), rect.y(), img_w, img_h),
                            };
                            let dst_rect = Rect::new(dst_x, dst_y, dst_x + dst_w, dst_y + dst_h);
                            self.canvas.draw_image_rect(
                                &image,
                                Some((&src_rect, skia_safe::canvas::SrcRectConstraint::Strict)),
                                dst_rect,
                                &Paint::default(),
                            );
                        }
                    }
                }
                DisplayOp::DrawSurface {
                    rect,
                    surface_id,
                    position,
                    ..
                } => {
                    let mut paint = Paint::default();
                    let r = ((surface_id * 50 + position / 20) % 255) as u8;
                    let g = ((surface_id * 30 + position / 30) % 255) as u8;
                    let b = ((surface_id * 70 + position / 40) % 255) as u8;
                    paint.set_color(SkColor::from_rgb(r, g, b));

                    self.canvas.draw_rect(
                        Rect::new(rect.x(), rect.y(), rect.right(), rect.bottom()),
                        &paint,
                    );
                }
                DisplayOp::DrawPath { .. } => {
                    eprintln!("Warning: DrawPath not implemented in Skia backend yet");
                }
                DisplayOp::DrawSvg { .. } => {
                    eprintln!("Warning: DrawSvg not implemented in Skia backend yet");
                }
            }
        }
        Ok(())
    }
}
