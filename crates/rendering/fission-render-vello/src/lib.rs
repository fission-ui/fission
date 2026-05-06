pub mod text;
pub use parley;
pub use text::VelloTextMeasurer;

use anyhow::Result;
use fission_render::{
    Color as RenderColor, DisplayList, DisplayOp, LayerClip, RenderLayer, RenderNode, RenderScene,
    Renderer, TextStyle as RenderTextStyle,
};
use vello::kurbo::{Affine, BezPath, Rect, RoundedRect};
// Minimal imports from peniko
use vello::peniko::{
    Blob, Brush, Color, Fill, ImageAlphaType, ImageBrush, ImageData, ImageFormat, ImageSampler, Mix,
};

fn map_color(c: &fission_render::Color) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a).into()
}

fn map_fill_to_brush(f: &fission_render::Fill) -> Brush {
    match f {
        fission_render::Fill::Solid(c) => Brush::Solid(map_color(c)),
        fission_render::Fill::LinearGradient { start, end, stops } => {
            let vello_stops: Vec<_> = stops
                .iter()
                .map(|(o, c)| vello::peniko::ColorStop {
                    offset: *o,
                    color: map_color(c).into(),
                })
                .collect();
            Brush::Gradient(
                vello::peniko::Gradient::new_linear(
                    vello::kurbo::Point::new(start.0 as f64, start.1 as f64),
                    vello::kurbo::Point::new(end.0 as f64, end.1 as f64),
                )
                .with_stops(vello_stops.as_slice()),
            )
        }
        fission_render::Fill::RadialGradient {
            center,
            radius,
            stops,
        } => {
            let vello_stops: Vec<_> = stops
                .iter()
                .map(|(o, c)| vello::peniko::ColorStop {
                    offset: *o,
                    color: map_color(c).into(),
                })
                .collect();
            Brush::Gradient(
                vello::peniko::Gradient::new_radial(
                    vello::kurbo::Point::new(center.0 as f64, center.1 as f64),
                    *radius as f32,
                )
                .with_stops(vello_stops.as_slice()),
            )
        }
    }
}

fn map_stroke(s: &fission_render::Stroke) -> (vello::kurbo::Stroke, Brush) {
    let cap = match s.line_cap {
        fission_render::LineCap::Butt => vello::kurbo::Cap::Butt,
        fission_render::LineCap::Round => vello::kurbo::Cap::Round,
        fission_render::LineCap::Square => vello::kurbo::Cap::Square,
    };
    let join = match s.line_join {
        fission_render::LineJoin::Miter => vello::kurbo::Join::Miter,
        fission_render::LineJoin::Round => vello::kurbo::Join::Round,
        fission_render::LineJoin::Bevel => vello::kurbo::Join::Bevel,
    };

    let mut stroke = vello::kurbo::Stroke::new(s.width as f64)
        .with_caps(cap)
        .with_join(join);
    if let Some(dash) = &s.dash_array {
        let dashes: Vec<f64> = dash.iter().map(|v| *v as f64).collect();
        stroke = stroke.with_dashes(0.0, dashes);
    }

    (stroke, map_fill_to_brush(&s.fill))
}
use crate::text::ParleyBrush;
use lazy_static::lazy_static;
use parley::layout::PositionedLayoutItem;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use vello::{Glyph, Scene};

lazy_static! {
    static ref IMAGE_CACHE: Mutex<HashMap<String, Arc<ImageData>>> = Mutex::new(HashMap::new());
    static ref SVG_CACHE: Mutex<HashMap<u64, Arc<SvgCacheEntry>>> = Mutex::new(HashMap::new());
}

#[derive(Debug)]
struct SvgCacheEntry {
    view_box: Option<(f64, f64, f64, f64)>,
    shapes: Vec<SvgShape>,
}

#[derive(Debug)]
enum SvgShape {
    Path(BezPath),
    Rect(RoundedRect),
}

fn svg_cache_key(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn parse_svg_entry(content: &str) -> SvgCacheEntry {
    let parse_view_box = |data: &str| -> Option<(f64, f64, f64, f64)> {
        let key = "viewBox=\"";
        let start = data.find(key)?;
        let rest = &data[start + key.len()..];
        let end = rest.find('\"')?;
        let nums: Vec<f64> = rest[..end]
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if nums.len() == 4 {
            Some((nums[0], nums[1], nums[2], nums[3]))
        } else {
            None
        }
    };

    let mut shapes = Vec::new();

    // Use regex-like manual parsing for robustness
    let path_regex = "d=\"";
    let _rect_regex = "<rect";
    let _poly_regex = "<polygon";

    // Re-implemented parsing using tag-based split for more reliability
    for tag in content.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or("");
        let tag_name = tag.split_whitespace().next().unwrap_or("");

        if tag_name == "path" {
            if let Some(d_start) = tag.find(path_regex) {
                let after_d = &tag[d_start + 3..];
                if let Some(d_end) = after_d.find('\"') {
                    let mut d = after_d[..d_end].to_string();
                    // Clean known bounding boxes
                    d = d.replace("M0 0h24v24H0z", "");
                    d = d.replace("M0 0h24v24H0V0z", "");
                    d = d.replace("M0,0h24v24H0V0z", "");
                    if !d.trim().is_empty() {
                        if let Ok(bez_path) = BezPath::from_svg(&d) {
                            shapes.push(SvgShape::Path(bez_path));
                        }
                    }
                }
            }
        } else if tag_name == "rect" {
            if tag.contains("fill=\"none\"") || tag.contains("fill='none'") {
                continue;
            }
            let parse_attr = |name: &str| -> f64 {
                if let Some(pos) = tag.find(&format!("{}=\"", name)) {
                    let after = &tag[pos + name.len() + 2..];
                    if let Some(end) = after.find('\"') {
                        return after[..end].parse().unwrap_or(0.0);
                    }
                }
                0.0
            };
            let x = parse_attr("x");
            let y = parse_attr("y");
            let w = parse_attr("width");
            let h = parse_attr("height");
            if w > 0.0 && h > 0.0 {
                shapes.push(SvgShape::Rect(RoundedRect::from_rect(
                    Rect::new(x, y, x + w, y + h),
                    0.0,
                )));
            }
        } else if tag_name == "polygon" {
            if let Some(p_start) = tag.find("points=\"") {
                let after = &tag[p_start + 8..];
                if let Some(end) = after.find('\"') {
                    let points_str = &after[..end];
                    let nums: Vec<f64> = points_str
                        .split(|c: char| c.is_whitespace() || c == ',')
                        .filter(|s| !s.is_empty())
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if nums.len() >= 4 {
                        let mut bez = BezPath::new();
                        bez.move_to((nums[0], nums[1]));
                        for i in (2..nums.len()).step_by(2) {
                            if i + 1 < nums.len() {
                                bez.line_to((nums[i], nums[i + 1]));
                            }
                        }
                        bez.close_path();
                        shapes.push(SvgShape::Path(bez));
                    }
                }
            }
        }
    }

    SvgCacheEntry {
        view_box: parse_view_box(content),
        shapes,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_svg_entry, SvgShape};

    #[test]
    fn svg_parser_skips_fill_none_rect_placeholders() {
        let svg = r#"<svg viewBox="0 0 24 24">
            <rect fill="none" width="24" height="24"/>
            <path d="M0 0h10v10H0z"/>
        </svg>"#;
        let entry = parse_svg_entry(svg);
        assert_eq!(entry.shapes.len(), 1);
        assert!(matches!(entry.shapes[0], SvgShape::Path(_)));
    }
}

fn svg_cache_entry(content: &str) -> Arc<SvgCacheEntry> {
    let key = svg_cache_key(content);
    if let Some(entry) = SVG_CACHE.lock().unwrap().get(&key) {
        return Arc::clone(entry);
    }

    let parsed = Arc::new(parse_svg_entry(content));
    let mut cache = SVG_CACHE.lock().unwrap();
    cache.entry(key).or_insert_with(|| Arc::clone(&parsed));
    parsed
}

pub struct VelloRenderer<'a> {
    scene: &'a mut Scene,
    measurer: Arc<VelloTextMeasurer>,
    scene_cache: &'a mut RetainedSceneCache,
    transform_stack: Vec<Affine>,
    current_transform: Affine,
    layer_count_stack: Vec<usize>,
    current_layer_count: usize,
}

pub struct RetainedSceneCache {
    entries: HashMap<u64, Scene>,
    order: VecDeque<u64>,
    max_entries: usize,
}

impl Default for RetainedSceneCache {
    fn default() -> Self {
        Self::new(256)
    }
}

impl RetainedSceneCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries: max_entries.max(1),
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    pub fn contains(&self, key: u64) -> bool {
        self.entries.contains_key(&key)
    }

    pub fn get(&self, key: u64) -> Option<&Scene> {
        self.entries.get(&key)
    }

    pub fn insert(&mut self, key: u64, scene: Scene) {
        if self.entries.contains_key(&key) {
            self.entries.insert(key, scene);
            return;
        }
        while self.entries.len() >= self.max_entries {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            } else {
                break;
            }
        }
        self.order.push_back(key);
        self.entries.insert(key, scene);
    }

    pub fn get_or_insert_with<F>(&mut self, key: u64, build: F) -> anyhow::Result<&Scene>
    where
        F: FnOnce(&mut RetainedSceneCache) -> anyhow::Result<Scene>,
    {
        if !self.entries.contains_key(&key) {
            let scene = build(self)?;
            self.insert(key, scene);
        }
        Ok(self
            .entries
            .get(&key)
            .expect("scene cache entry missing after insertion"))
    }
}

impl<'a> VelloRenderer<'a> {
    pub fn new(
        scene: &'a mut Scene,
        measurer: Arc<VelloTextMeasurer>,
        scene_cache: &'a mut RetainedSceneCache,
        scale_factor: f64,
    ) -> Self {
        Self {
            scene,
            measurer,
            scene_cache,
            transform_stack: Vec::new(),
            current_transform: Affine::scale(scale_factor),
            layer_count_stack: Vec::new(),
            current_layer_count: 0,
        }
    }

    fn get_image(&self, path: &str) -> Option<Arc<ImageData>> {
        let mut cache = IMAGE_CACHE.lock().unwrap();
        if let Some(img) = cache.get(path) {
            return Some(Arc::clone(img));
        }

        if let Ok(img) = image::open(path) {
            let img = img.to_rgba8();
            let (width, height) = img.dimensions();
            let data = img.into_raw();
            let image_data = Arc::new(ImageData {
                data: Blob::new(Arc::new(data)),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width,
                height,
            });
            cache.insert(path.to_string(), Arc::clone(&image_data));
            Some(image_data)
        } else {
            None
        }
    }

    fn affine_from_mat4(matrix: &[f32; 16]) -> Affine {
        let m00 = matrix[0] as f64;
        let m10 = matrix[1] as f64;
        let m01 = matrix[4] as f64;
        let m11 = matrix[5] as f64;
        let m03 = matrix[12] as f64;
        let m13 = matrix[13] as f64;
        Affine::new([m00, m10, m01, m11, m03, m13])
    }

    fn render_text(
        &mut self,
        text: &str,
        base_size: f32,
        base_color: RenderColor,
        underline: bool,
        wrap: bool,
        position: fission_render::LayoutPoint,
        bounds: fission_render::LayoutRect,
        caret_index: Option<usize>,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    ) {
        // Fast path for simple text using cache
        if styles.is_empty() {
            let layout = self.measurer.get_layout(
                text,
                base_size,
                if wrap && bounds.width() > 0.0 {
                    Some(bounds.width() as f32)
                } else {
                    None
                },
            );

            // Draw Glyphs (Reused layout logic)
            for line in layout.lines() {
                for item in line.items() {
                    if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                        let run = glyph_run.run();
                        let font = run.font();
                        let font_size = run.font_size();

                        // Override color from base_color since cached layout is color-agnostic
                        let color = Color::from_rgba8(
                            base_color.r,
                            base_color.g,
                            base_color.b,
                            base_color.a,
                        );

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

                        self.scene
                            .draw_glyphs(font)
                            .font_size(font_size)
                            .transform(
                                self.current_transform
                                    * Affine::translate((position.x as f64, position.y as f64)),
                            )
                            .brush(color)
                            .draw(Fill::NonZero, glyphs);

                        if underline {
                            let metrics = run.metrics();
                            let offset = metrics.underline_offset;
                            let size = metrics.underline_size.max(1.0);
                            let x0 = position.x as f64 + glyph_run.offset() as f64;
                            let x1 = x0 + glyph_run.advance() as f64;
                            let y0 = position.y as f64 + (glyph_run.baseline() + offset) as f64;
                            let rect = Rect::new(x0, y0, x1, y0 + size as f64);
                            self.scene.fill(
                                Fill::NonZero,
                                self.current_transform,
                                color,
                                None,
                                &rect,
                            );
                        }
                    }
                }
            }
            if let Some(idx) = caret_index {
                self.draw_caret(&layout, idx, position, text, base_size);
            }
            return;
        }

        // Slow path for rich text
        let layout = self.measurer.layout_rich(
            text,
            base_size,
            base_color,
            styles,
            if wrap && bounds.width() > 0.0 {
                Some(bounds.width() as f32)
            } else {
                None
            },
        );

        // Draw Glyphs for rich text (uses brushes from layout)
        for line in layout.lines() {
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                    let style = glyph_run.style();
                    let run = glyph_run.run();
                    let font = run.font();
                    let font_size = run.font_size();
                    let brush_data = style.brush.clone();
                    let color = Color::from_rgba8(
                        brush_data.0[0],
                        brush_data.0[1],
                        brush_data.0[2],
                        brush_data.0[3],
                    );

                    // Draw background highlight rect if any style in range has background_color
                    let run_text_range = run.text_range();
                    for (range, s) in styles.iter() {
                        if let Some(bg) = &s.background_color {
                            // Check overlap between glyph run range and style range
                            let overlap_start = range.start.max(run_text_range.start);
                            let overlap_end = range.end.min(run_text_range.end);
                            if overlap_start < overlap_end {
                                let metrics = line.metrics();
                                let line_height = metrics
                                    .line_height
                                    .max(metrics.ascent + metrics.descent)
                                    .max(1.0);
                                let top_y = metrics.baseline - metrics.ascent;
                                let bg_color = Color::from_rgba8(bg.r, bg.g, bg.b, bg.a);
                                let x0 = position.x as f64 + glyph_run.offset() as f64;
                                let x1 = x0 + glyph_run.advance() as f64;
                                let y0 = position.y as f64 + top_y as f64;
                                let bg_rect = Rect::new(x0, y0, x1, y0 + line_height as f64);
                                self.scene.fill(
                                    Fill::NonZero,
                                    self.current_transform,
                                    bg_color,
                                    None,
                                    &bg_rect,
                                );
                                break; // Only draw one background per glyph run
                            }
                        }
                    }

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

                    self.scene
                        .draw_glyphs(font)
                        .font_size(font_size)
                        .transform(
                            self.current_transform
                                * Affine::translate((position.x as f64, position.y as f64)),
                        )
                        .brush(color)
                        .draw(Fill::NonZero, glyphs);

                    if let Some(decoration) = &style.underline {
                        let metrics = run.metrics();
                        let offset = decoration.offset.unwrap_or(metrics.underline_offset);
                        let size = decoration.size.unwrap_or(metrics.underline_size).max(1.0);
                        let deco_brush = decoration.brush.clone();
                        let deco_color = Color::from_rgba8(
                            deco_brush.0[0],
                            deco_brush.0[1],
                            deco_brush.0[2],
                            deco_brush.0[3],
                        );

                        let x0 = position.x as f64 + glyph_run.offset() as f64;
                        let x1 = x0 + glyph_run.advance() as f64;
                        let y0 = position.y as f64 + (glyph_run.baseline() + offset) as f64;
                        let rect = Rect::new(x0, y0, x1, y0 + size as f64);
                        self.scene.fill(
                            Fill::NonZero,
                            self.current_transform,
                            deco_color,
                            None,
                            &rect,
                        );
                    }
                }
            }
        }

        if let Some(idx) = caret_index {
            self.draw_caret(&layout, idx, position, text, base_size);
        }
    }

    fn next_char_boundary(text: &str, idx: usize) -> usize {
        if idx >= text.len() {
            return text.len();
        }
        if !text.is_char_boundary(idx) {
            return text.len();
        }
        let mut it = text[idx..].char_indices();
        let _ = it.next();
        if let Some((off, _)) = it.next() {
            idx + off
        } else {
            text.len()
        }
    }

    fn draw_caret(
        &mut self,
        layout: &parley::layout::Layout<ParleyBrush>,
        idx: usize,
        position: fission_render::LayoutPoint,
        text: &str,
        base_size: f32,
    ) {
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
                                if current_char_idx >= idx {
                                    break;
                                }
                                local_x += glyph.advance;
                                current_char_idx = Self::next_char_boundary(text, current_char_idx)
                                    .min(run_range.end);
                            }
                            x_pos = local_x;
                        } else if idx > run_range.end {
                            x_pos = glyph_run.offset() + glyph_run.advance();
                        }
                    }
                }

                let metrics = line.metrics();
                let line_height = metrics
                    .line_height
                    .max(metrics.ascent + metrics.descent)
                    .max(1.0);
                let baseline_y = metrics.baseline;

                let top_y = baseline_y - metrics.ascent;

                let caret_rect = Rect::new(
                    position.x as f64 + x_pos as f64,
                    position.y as f64 + top_y as f64,
                    position.x as f64 + x_pos as f64 + 2.0,
                    position.y as f64 + top_y as f64 + line_height as f64,
                );

                self.scene.fill(
                    Fill::NonZero,
                    self.current_transform,
                    Color::BLACK,
                    None,
                    &caret_rect,
                );
                caret_drawn = true;
                break;
            }
        }
        if !caret_drawn && idx == 0 && text.is_empty() {
            let mut top_y = position.y as f64;
            let mut height = base_size as f64 * 1.2;
            if let Some(line) = layout.lines().next() {
                let metrics = line.metrics();
                top_y = position.y as f64 + (metrics.baseline - metrics.ascent) as f64;
                height = metrics
                    .line_height
                    .max(metrics.ascent + metrics.descent)
                    .max(1.0) as f64;
            }
            let caret_rect = Rect::new(
                position.x as f64,
                top_y,
                position.x as f64 + 2.0,
                top_y + height,
            );
            self.scene.fill(
                Fill::NonZero,
                self.current_transform,
                Color::BLACK,
                None,
                &caret_rect,
            );
        }
    }

    fn render_paint_list(&mut self, list: &DisplayList) -> Result<()> {
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
                DisplayOp::Transform(matrix) => {
                    let affine = Self::affine_from_mat4(matrix);
                    self.current_transform = self.current_transform * affine;
                }
                DisplayOp::CachedScene {
                    cache_key, list, ..
                } => {
                    if !self.scene_cache.contains(*cache_key) {
                        let mut cached_scene = Scene::new();
                        {
                            let mut cached_renderer = VelloRenderer::new(
                                &mut cached_scene,
                                Arc::clone(&self.measurer),
                                self.scene_cache,
                                1.0,
                            );
                            cached_renderer.render_paint_list(list)?;
                        }
                        self.scene_cache.insert(*cache_key, cached_scene);
                    }
                    if let Some(cached_scene) = self.scene_cache.get(*cache_key) {
                        self.scene
                            .append(cached_scene, Some(self.current_transform));
                    }
                }
                DisplayOp::ClipRect(rect) => {
                    let r = Rect::new(
                        rect.origin.x as f64,
                        rect.origin.y as f64,
                        (rect.origin.x + rect.size.width) as f64,
                        (rect.origin.y + rect.size.height) as f64,
                    );
                    self.scene
                        .push_layer(Mix::Normal, 1.0, self.current_transform, &r);
                    self.current_layer_count += 1;
                }
                DisplayOp::ClipRoundedRect { rect, radius } => {
                    let r = Rect::new(
                        rect.origin.x as f64,
                        rect.origin.y as f64,
                        (rect.origin.x + rect.size.width) as f64,
                        (rect.origin.y + rect.size.height) as f64,
                    );
                    let shape = RoundedRect::from_rect(r, *radius as f64);
                    self.scene
                        .push_layer(Mix::Normal, 1.0, self.current_transform, &shape);
                    self.current_layer_count += 1;
                }
                DisplayOp::OpacityLayer { alpha, bounds } => {
                    let r = Rect::new(
                        bounds.origin.x as f64,
                        bounds.origin.y as f64,
                        (bounds.origin.x + bounds.size.width) as f64,
                        (bounds.origin.y + bounds.size.height) as f64,
                    );
                    self.scene
                        .push_layer(Mix::Normal, *alpha, self.current_transform, &r);
                    self.current_layer_count += 1;
                }
                DisplayOp::DrawRect {
                    rect,
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                    ..
                } => {
                    let rect = Rect::new(
                        rect.origin.x as f64,
                        rect.origin.y as f64,
                        (rect.origin.x + rect.size.width) as f64,
                        (rect.origin.y + rect.size.height) as f64,
                    );

                    let shape = RoundedRect::from_rect(rect, *corner_radius as f64);

                    if let Some(shadow) = shadow {
                        let shadow_origin_x = rect.x0 + shadow.offset.0 as f64;
                        let shadow_origin_y = rect.y0 + shadow.offset.1 as f64;
                        let shadow_rect = Rect::new(
                            shadow_origin_x,
                            shadow_origin_y,
                            shadow_origin_x + rect.width(),
                            shadow_origin_y + rect.height(),
                        );
                        let shadow_shape =
                            RoundedRect::from_rect(shadow_rect, *corner_radius as f64);
                        let shadow_color = Color::from_rgba8(
                            shadow.color.r,
                            shadow.color.g,
                            shadow.color.b,
                            shadow.color.a,
                        );

                        self.scene.fill(
                            Fill::NonZero,
                            self.current_transform,
                            shadow_color,
                            None,
                            &shadow_shape,
                        );
                    }

                    if let Some(f) = fill {
                        let brush = map_fill_to_brush(f);
                        self.scene.fill(
                            Fill::NonZero,
                            self.current_transform,
                            &brush,
                            None,
                            &shape,
                        );
                    }
                    if let Some(s) = stroke {
                        let (stroke_style, brush) = map_stroke(s);
                        self.scene.stroke(
                            &stroke_style,
                            self.current_transform,
                            &brush,
                            None,
                            &shape,
                        );
                    }
                }
                DisplayOp::DrawText {
                    text,
                    size,
                    color,
                    underline,
                    wrap,
                    position,
                    bounds,
                    caret_index,
                    ..
                } => {
                    self.render_text(
                        text,
                        *size,
                        *color,
                        *underline,
                        *wrap,
                        *position,
                        *bounds,
                        *caret_index,
                        &[],
                    );
                }
                DisplayOp::DrawRichText {
                    runs,
                    position,
                    bounds,
                    wrap,
                    caret_index,
                    ..
                } => {
                    if let Some(first) = runs.first() {
                        if runs.iter().all(|run| run.style == first.style) {
                            let mut full_text = String::new();
                            for run in runs {
                                full_text.push_str(&run.text);
                            }
                            self.render_text(
                                &full_text,
                                first.style.font_size,
                                first.style.color,
                                first.style.underline,
                                *wrap,
                                *position,
                                *bounds,
                                *caret_index,
                                &[],
                            );
                            continue;
                        }
                    }

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
                        (
                            14.0,
                            RenderColor {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 255,
                            },
                        )
                    };

                    self.render_text(
                        &full_text,
                        base_size,
                        base_color,
                        false,
                        *wrap,
                        *position,
                        *bounds,
                        *caret_index,
                        &styles,
                    );
                }
                DisplayOp::DrawImage {
                    source, rect, fit, ..
                } => {
                    if let Some(image_data) = self.get_image(source) {
                        let rect_w = rect.size.width as f64;
                        let rect_h = rect.size.height as f64;
                        let img_w = image_data.width as f64;
                        let img_h = image_data.height as f64;

                        if rect_w <= 0.0 || rect_h <= 0.0 || img_w <= 0.0 || img_h <= 0.0 {
                            continue;
                        }

                        let (scale_x, scale_y, dx, dy) = match fit {
                            fission_render::ImageFit::Fill => (
                                rect_w / img_w,
                                rect_h / img_h,
                                rect.origin.x as f64,
                                rect.origin.y as f64,
                            ),
                            fission_render::ImageFit::Contain => {
                                let scale = (rect_w / img_w).min(rect_h / img_h);
                                let w = img_w * scale;
                                let h = img_h * scale;
                                (
                                    scale,
                                    scale,
                                    rect.origin.x as f64 + (rect_w - w) / 2.0,
                                    rect.origin.y as f64 + (rect_h - h) / 2.0,
                                )
                            }
                            fission_render::ImageFit::Cover => {
                                let scale = (rect_w / img_w).max(rect_h / img_h);
                                let w = img_w * scale;
                                let h = img_h * scale;
                                (
                                    scale,
                                    scale,
                                    rect.origin.x as f64 + (rect_w - w) / 2.0,
                                    rect.origin.y as f64 + (rect_h - h) / 2.0,
                                )
                            }
                            fission_render::ImageFit::None => {
                                (1.0, 1.0, rect.origin.x as f64, rect.origin.y as f64)
                            }
                        };

                        let transform = self.current_transform
                            * Affine::translate((dx, dy))
                            * Affine::scale_non_uniform(scale_x, scale_y);
                        let brush = ImageBrush {
                            image: &*image_data,
                            sampler: ImageSampler::default(),
                        };
                        self.scene.draw_image(brush, transform);
                    }
                }
                DisplayOp::DrawPath {
                    path,
                    fill,
                    stroke,
                    bounds,
                    ..
                } => {
                    if let Ok(bez_path) = BezPath::from_svg(path) {
                        let transform = self.current_transform
                            * Affine::translate((bounds.origin.x as f64, bounds.origin.y as f64));

                        if let Some(f) = fill {
                            let brush = map_fill_to_brush(f);
                            self.scene
                                .fill(Fill::NonZero, transform, &brush, None, &bez_path);
                        }
                        if let Some(s) = stroke {
                            let (stroke_style, brush) = map_stroke(s);
                            self.scene
                                .stroke(&stroke_style, transform, &brush, None, &bez_path);
                        }
                    }
                }
                DisplayOp::DrawSvg {
                    content,
                    fill,
                    stroke,
                    bounds,
                    ..
                } => {
                    let entry = svg_cache_entry(content);
                    let (vb_x, vb_y, vb_w, vb_h) = entry.view_box.unwrap_or((
                        0.0,
                        0.0,
                        bounds.size.width as f64,
                        bounds.size.height as f64,
                    ));
                    let rect_w = bounds.size.width as f64;
                    let rect_h = bounds.size.height as f64;
                    let (scale, dx, dy) =
                        if vb_w > 0.0 && vb_h > 0.0 && rect_w > 0.0 && rect_h > 0.0 {
                            let scale = (rect_w / vb_w).min(rect_h / vb_h);
                            let scaled_w = vb_w * scale;
                            let scaled_h = vb_h * scale;
                            (
                                scale,
                                bounds.origin.x as f64 + (rect_w - scaled_w) / 2.0 - vb_x * scale,
                                bounds.origin.y as f64 + (rect_h - scaled_h) / 2.0 - vb_y * scale,
                            )
                        } else {
                            (1.0, bounds.origin.x as f64, bounds.origin.y as f64)
                        };
                    let svg_transform =
                        self.current_transform * Affine::translate((dx, dy)) * Affine::scale(scale);

                    for shape in &entry.shapes {
                        match shape {
                            SvgShape::Path(path) => {
                                if let Some(f) = fill {
                                    let brush = map_fill_to_brush(f);
                                    self.scene.fill(
                                        Fill::NonZero,
                                        svg_transform,
                                        &brush,
                                        None,
                                        path,
                                    );
                                }
                                if let Some(s) = stroke {
                                    let (stroke_style, brush) = map_stroke(s);
                                    self.scene.stroke(
                                        &stroke_style,
                                        svg_transform,
                                        &brush,
                                        None,
                                        path,
                                    );
                                }
                            }
                            SvgShape::Rect(rect) => {
                                if let Some(f) = fill {
                                    let brush = map_fill_to_brush(f);
                                    self.scene.fill(
                                        Fill::NonZero,
                                        svg_transform,
                                        &brush,
                                        None,
                                        rect,
                                    );
                                }
                                if let Some(s) = stroke {
                                    let (stroke_style, brush) = map_stroke(s);
                                    self.scene.stroke(
                                        &stroke_style,
                                        svg_transform,
                                        &brush,
                                        None,
                                        rect,
                                    );
                                }
                            }
                        }
                    }
                }
                DisplayOp::DrawSurface { .. } => {}
            }
        }
        Ok(())
    }

    fn render_node(&mut self, node: &RenderNode) -> Result<()> {
        match node {
            RenderNode::Paint(list) => self.render_paint_list(list),
            RenderNode::Layer(layer) => self.render_layer(layer),
        }
    }

    fn render_layer(&mut self, layer: &RenderLayer) -> Result<()> {
        let enable_scene_cache = std::env::var("FISSION_ENABLE_VELLO_SCENE_CACHE")
            .ok()
            .as_deref()
            == Some("1");
        let can_cache_layer = enable_scene_cache
            && layer.style.clip.is_none()
            && layer.style.transform.is_none()
            && (layer.style.opacity - 1.0).abs() <= 0.001;

        if can_cache_layer {
            if let Some(cache_key) = layer.style.cache_key {
                if !self.scene_cache.contains(cache_key) {
                    let mut cached_scene = Scene::new();
                    {
                        let mut cached_renderer = VelloRenderer::new(
                            &mut cached_scene,
                            Arc::clone(&self.measurer),
                            self.scene_cache,
                            1.0,
                        );
                        cached_renderer.render_layer_uncached(layer)?;
                    }
                    self.scene_cache.insert(cache_key, cached_scene);
                }
                if let Some(cached_scene) = self.scene_cache.get(cache_key) {
                    self.scene
                        .append(cached_scene, Some(self.current_transform));
                }
                return Ok(());
            }
        }

        self.render_layer_uncached(layer)
    }

    fn render_layer_uncached(&mut self, layer: &RenderLayer) -> Result<()> {
        let saved_transform = self.current_transform;
        let saved_layer_count = self.current_layer_count;

        if let Some(clip) = &layer.style.clip {
            match clip {
                LayerClip::Rect(rect) => {
                    let r = Rect::new(
                        rect.origin.x as f64,
                        rect.origin.y as f64,
                        (rect.origin.x + rect.size.width) as f64,
                        (rect.origin.y + rect.size.height) as f64,
                    );
                    self.scene
                        .push_layer(Mix::Normal, 1.0, self.current_transform, &r);
                    self.current_layer_count += 1;
                }
                LayerClip::RoundedRect { rect, radius } => {
                    let r = Rect::new(
                        rect.origin.x as f64,
                        rect.origin.y as f64,
                        (rect.origin.x + rect.size.width) as f64,
                        (rect.origin.y + rect.size.height) as f64,
                    );
                    let shape = RoundedRect::from_rect(r, *radius as f64);
                    self.scene
                        .push_layer(Mix::Normal, 1.0, self.current_transform, &shape);
                    self.current_layer_count += 1;
                }
            }
        }

        if (layer.style.opacity - 1.0).abs() > 0.001 {
            let r = Rect::new(
                layer.bounds.origin.x as f64,
                layer.bounds.origin.y as f64,
                (layer.bounds.origin.x + layer.bounds.size.width) as f64,
                (layer.bounds.origin.y + layer.bounds.size.height) as f64,
            );
            self.scene
                .push_layer(Mix::Normal, layer.style.opacity, self.current_transform, &r);
            self.current_layer_count += 1;
        }

        if let Some(transform) = layer.style.transform {
            let affine = Self::affine_from_mat4(&transform);
            self.current_transform = self.current_transform * affine;
        }

        let enable_scene_cache = std::env::var("FISSION_ENABLE_VELLO_SCENE_CACHE")
            .ok()
            .as_deref()
            == Some("1");
        let can_cache_contents = enable_scene_cache
            && layer.style.clip.is_none()
            && layer.style.transform.is_none()
            && (layer.style.opacity - 1.0).abs() <= 0.001;

        if can_cache_contents {
            if let Some(cache_key) = layer.style.content_cache_key {
                if !self.scene_cache.contains(cache_key) {
                    let mut cached_scene = Scene::new();
                    {
                        let mut cached_renderer = VelloRenderer::new(
                            &mut cached_scene,
                            Arc::clone(&self.measurer),
                            self.scene_cache,
                            1.0,
                        );
                        cached_renderer.render_layer_contents(layer)?;
                    }
                    self.scene_cache.insert(cache_key, cached_scene);
                }
                if let Some(cached_scene) = self.scene_cache.get(cache_key) {
                    self.scene
                        .append(cached_scene, Some(self.current_transform));
                }
            } else {
                self.render_layer_contents(layer)?;
            }
        } else {
            self.render_layer_contents(layer)?;
        }

        while self.current_layer_count > saved_layer_count {
            self.scene.pop_layer();
            self.current_layer_count -= 1;
        }
        self.current_transform = saved_transform;
        Ok(())
    }

    fn render_layer_contents(&mut self, layer: &RenderLayer) -> Result<()> {
        for child in &layer.children {
            self.render_node(child)?;
        }
        Ok(())
    }
}

impl<'a> Renderer for VelloRenderer<'a> {
    fn render_scene(&mut self, scene: &RenderScene) -> Result<()> {
        for root in &scene.roots {
            self.render_node(root)?;
        }
        Ok(())
    }
}
