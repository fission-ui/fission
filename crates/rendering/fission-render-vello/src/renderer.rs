use std::sync::{atomic::Ordering, Arc};

use anyhow::{anyhow, Result};
use fission_ir::op::{decode_text_paragraph_style, ImageRequest, TextOverflow, TextParagraphStyle};
use fission_render::capabilities::is_2d_affine_transform;
use fission_render::{
    surface_placeholder_color, Color as RenderColor, DisplayList, DisplayOp, LayerClip,
    RenderLayer, RenderNode, RenderScene, Renderer, TextStyle as RenderTextStyle,
};
#[cfg(test)]
use parley::layout::Alignment as ParleyAlignment;
use parley::layout::PositionedLayoutItem;
use vello::kurbo::{Affine, BezPath, Point, Rect, RoundedRect, Shape, Vec2};
use vello::peniko::{Color, Fill, ImageBrush, ImageData, ImageSampler, Mix};
use vello::{Glyph, Scene};

use crate::image::{
    aligned_offset, image_request_with_default_cache_size, spawn_image_load, ImageCacheEntry,
    IMAGE_CACHE, IMAGE_CACHE_HITS, IMAGE_CACHE_MISSES, IMAGE_LOADS_STARTED, IMAGE_OFFSCREEN_SKIPS,
};
use crate::paint::{map_fill_to_brush, map_stroke, text_style_requires_rich_layout};
use crate::paragraph::{
    paragraph_alignment, paragraph_alignment_options, paragraph_alignment_width, paragraph_fade,
    paragraph_line_trim, paragraph_line_visual_bounds, paragraph_y_offset,
    prepare_paragraph_layout, text_background_segments_for_cluster_ranges, ParagraphFade,
    ParagraphLineVisualBounds, PreparedParagraphLayout, TextBackgroundSegment, TextClip,
    PARAGRAPH_FADE_SLICE_COUNT,
};
use crate::scene_cache::RetainedSceneCache;
use crate::svg::{parse_svg_entry, svg_cache_entry, SvgShape};
use crate::text::{ParleyBrush, VelloTextMeasurer};
#[cfg(test)]
use crate::workload::{workload_profile_for_encoded_scene, workload_profile_for_scene};

const TEXT_CULL_PADDING: f32 = 8.0;

mod text_rendering;

pub struct VelloRenderer<'a> {
    scene: &'a mut Scene,
    measurer: Arc<VelloTextMeasurer>,
    scene_cache: &'a mut RetainedSceneCache,
    transform_stack: Vec<Affine>,
    current_transform: Affine,
    layer_count_stack: Vec<usize>,
    current_layer_count: usize,
    clip_stack: Vec<Rect>,
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
            clip_stack: Vec::new(),
        }
    }

    fn layout_rect_to_rect(rect: fission_render::LayoutRect) -> Rect {
        Rect::new(
            rect.origin.x as f64,
            rect.origin.y as f64,
            (rect.origin.x + rect.size.width) as f64,
            (rect.origin.y + rect.size.height) as f64,
        )
    }

    pub(crate) fn transform_rect_bounds(transform: Affine, rect: Rect) -> Rect {
        let points = [
            Point::new(rect.x0, rect.y0),
            Point::new(rect.x1, rect.y0),
            Point::new(rect.x0, rect.y1),
            Point::new(rect.x1, rect.y1),
        ];
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for point in points {
            let point = transform * point;
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }
        Rect::new(min_x, min_y, max_x, max_y)
    }

    fn rects_intersect(a: Rect, b: Rect) -> bool {
        a.width() > 0.0
            && a.height() > 0.0
            && b.width() > 0.0
            && b.height() > 0.0
            && a.x1 >= b.x0
            && a.x0 <= b.x1
            && a.y1 >= b.y0
            && a.y0 <= b.y1
    }

    fn intersect_rects(a: Rect, b: Rect) -> Rect {
        Rect::new(
            a.x0.max(b.x0),
            a.y0.max(b.y0),
            a.x1.min(b.x1),
            a.y1.min(b.y1),
        )
    }

    fn local_rect_visible(&self, rect: Rect) -> bool {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return false;
        }
        let Some(active_clip) = self.clip_stack.last().copied() else {
            return true;
        };
        let transformed = Self::transform_rect_bounds(self.current_transform, rect);
        Self::rects_intersect(transformed, active_clip)
    }

    fn image_request_for_rect(
        &self,
        request: &ImageRequest,
        rect: fission_render::LayoutRect,
    ) -> ImageRequest {
        image_request_with_default_cache_size(
            request,
            Self::layout_rect_to_rect(rect),
            self.current_transform,
        )
    }

    fn push_clip_bounds(&mut self, rect: Rect) {
        let transformed = Self::transform_rect_bounds(self.current_transform, rect);
        let clipped = if let Some(active_clip) = self.clip_stack.last().copied() {
            Self::intersect_rects(active_clip, transformed)
        } else {
            transformed
        };
        self.clip_stack.push(clipped);
    }

    fn pop_clip_bounds(&mut self) {
        let _ = self.clip_stack.pop();
    }

    fn text_clip(
        &self,
        position: fission_render::LayoutPoint,
        bounds: fission_render::LayoutRect,
    ) -> Option<TextClip> {
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return None;
        }
        let mut visible = Self::layout_rect_to_rect(bounds);
        if let Some(active_clip) = self.clip_stack.last().copied() {
            let local_clip =
                Self::transform_rect_bounds(self.current_transform.inverse(), active_clip);
            visible = Self::intersect_rects(visible, local_clip);
        }
        Some(TextClip {
            left: visible.x0 as f32 - position.x - TEXT_CULL_PADDING,
            right: visible.x1 as f32 - position.x + TEXT_CULL_PADDING,
            top: visible.y0 as f32 - position.y - TEXT_CULL_PADDING,
            bottom: visible.y1 as f32 - position.y + TEXT_CULL_PADDING,
        })
    }

    fn get_image(&self, request: &ImageRequest) -> Option<Arc<ImageData>> {
        let key = request.stable_cache_key();
        if let Some(entry) = IMAGE_CACHE.get(&key) {
            return match entry {
                ImageCacheEntry::Ready(img) => {
                    IMAGE_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                    Some(Arc::clone(&img))
                }
                ImageCacheEntry::Loading | ImageCacheEntry::Failed => None,
            };
        }

        IMAGE_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        IMAGE_LOADS_STARTED.fetch_add(1, Ordering::Relaxed);
        IMAGE_CACHE.insert(key.clone(), ImageCacheEntry::Loading);
        spawn_image_load(key, request.clone());
        None
    }

    fn affine_from_mat4(matrix: &[f32; 16]) -> Result<Affine> {
        if matrix.iter().any(|value| !value.is_finite()) {
            return Err(anyhow!(
                "Vello renderer requires finite transform matrix values"
            ));
        }
        if !is_2d_affine_transform(matrix) {
            return Err(anyhow!(
                "Vello renderer supports only two-dimensional affine transforms"
            ));
        }
        let m00 = matrix[0] as f64;
        let m10 = matrix[1] as f64;
        let m01 = matrix[4] as f64;
        let m11 = matrix[5] as f64;
        let dx = matrix[12] as f64;
        let dy = matrix[13] as f64;
        Ok(Affine::new([m00, m10, m01, m11, dx, dy]))
    }

    fn with_clip_rect<F>(&mut self, rect: Rect, draw: F)
    where
        F: FnOnce(&mut Self),
    {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }

        self.scene
            .push_layer(Mix::Normal, 1.0, self.current_transform, &rect);
        draw(self);
        self.scene.pop_layer();
    }

    fn with_alpha_clip_rect<F>(&mut self, rect: Rect, alpha: f32, draw: F)
    where
        F: FnOnce(&mut Self),
    {
        if alpha <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }

        self.scene
            .push_layer(Mix::Normal, alpha, self.current_transform, &rect);
        draw(self);
        self.scene.pop_layer();
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
                        self.pop_clip_bounds();
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
                    let affine = Self::affine_from_mat4(matrix)?;
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
                    let r = Self::layout_rect_to_rect(*rect);
                    self.scene
                        .push_layer(Mix::Normal, 1.0, self.current_transform, &r);
                    self.push_clip_bounds(r);
                    self.current_layer_count += 1;
                }
                DisplayOp::ClipRoundedRect { rect, radius } => {
                    let r = Self::layout_rect_to_rect(*rect);
                    let shape = RoundedRect::from_rect(r, *radius as f64);
                    self.scene
                        .push_layer(Mix::Normal, 1.0, self.current_transform, &shape);
                    self.push_clip_bounds(r);
                    self.current_layer_count += 1;
                }
                DisplayOp::OpacityLayer { alpha, bounds } => {
                    let r = Self::layout_rect_to_rect(*bounds);
                    self.scene
                        .push_layer(Mix::Normal, *alpha, self.current_transform, &r);
                    self.push_clip_bounds(r);
                    self.current_layer_count += 1;
                }
                DisplayOp::BackdropFilter {
                    rect,
                    filter,
                    corner_radius,
                    ..
                } => {
                    // Vello does not yet expose a framebuffer backdrop filter.
                    // Preserve the clipped filter primitive in the display list;
                    // GPU hosts can promote it to a compositor pass, while the
                    // software and site renderers execute the blur directly.
                    let _ = (rect, filter, corner_radius);
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

                    if let Some(shadow) = shadow.filter(|shadow| !shadow.inset) {
                        let shadow_origin_x = rect.x0 + shadow.offset.0 as f64;
                        let shadow_origin_y = rect.y0 + shadow.offset.1 as f64;
                        let shadow_rect = Rect::new(
                            shadow_origin_x,
                            shadow_origin_y,
                            shadow_origin_x + rect.width(),
                            shadow_origin_y + rect.height(),
                        )
                        .inflate(shadow.spread_radius as f64, shadow.spread_radius as f64);
                        let shadow_color = Color::from_rgba8(
                            shadow.color.r,
                            shadow.color.g,
                            shadow.color.b,
                            shadow.color.a,
                        );

                        self.scene.draw_blurred_rounded_rect(
                            self.current_transform,
                            shadow_rect,
                            shadow_color,
                            (*corner_radius + shadow.spread_radius).max(0.0) as f64,
                            (shadow.blur_radius.max(0.0) * 0.5) as f64,
                        );
                    }

                    if let Some(f) = fill {
                        let brush = map_fill_to_brush(f, rect);
                        self.scene.fill(
                            Fill::NonZero,
                            self.current_transform,
                            &brush,
                            None,
                            &shape,
                        );
                    }
                    if let Some(s) = stroke {
                        let (stroke_style, brush) = map_stroke(s, rect);
                        self.scene.stroke(
                            &stroke_style,
                            self.current_transform,
                            &brush,
                            None,
                            &shape,
                        );
                    }

                    if let Some(shadow) = shadow.filter(|shadow| shadow.inset) {
                        let std_dev = (shadow.blur_radius.max(0.0) * 0.5) as f64;
                        let band = (shadow.spread_radius.max(0.0) as f64 + 2.5 * std_dev).max(1.0);
                        let inner = rect.inset(-band)
                            + Vec2::new(shadow.offset.0 as f64, shadow.offset.1 as f64);
                        let clip_shape = RoundedRect::from_rect(rect, *corner_radius as f64);
                        let inner_shape =
                            RoundedRect::from_rect(inner, (*corner_radius as f64 - band).max(0.0));
                        let ring = BezPath::from_iter(
                            clip_shape
                                .path_elements(0.1)
                                .chain(inner_shape.to_path(0.1).reverse_subpaths()),
                        );
                        let shadow_color = Color::from_rgba8(
                            shadow.color.r,
                            shadow.color.g,
                            shadow.color.b,
                            shadow.color.a,
                        );
                        self.scene.draw_blurred_rounded_rect_in(
                            &ring,
                            self.current_transform,
                            rect,
                            shadow_color,
                            *corner_radius as f64,
                            std_dev,
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
                    caret_color,
                    caret_width,
                    caret_height,
                    caret_radius,
                    paragraph_style,
                    ..
                } => {
                    if !self.local_rect_visible(Self::layout_rect_to_rect(*bounds)) {
                        continue;
                    }
                    self.render_text(
                        text,
                        *size,
                        *color,
                        *underline,
                        *wrap,
                        *position,
                        *bounds,
                        *caret_index,
                        *caret_color,
                        *caret_width,
                        *caret_height,
                        *caret_radius,
                        *paragraph_style,
                        &[],
                        &[],
                    );
                }
                DisplayOp::DrawRichText {
                    runs,
                    position,
                    bounds,
                    wrap,
                    caret_index,
                    caret_color,
                    caret_width,
                    caret_height,
                    caret_radius,
                    paragraph_style,
                    ..
                } => {
                    if !self.local_rect_visible(Self::layout_rect_to_rect(*bounds)) {
                        continue;
                    }
                    let rich =
                        crate::text::VelloTextMeasurer::rich_layout_input_from_render_runs(runs);
                    if let Some(first) = runs.first() {
                        if runs.iter().all(|run| run.style == first.style)
                            && rich.inline_boxes.is_empty()
                            && !text_style_requires_rich_layout(&first.style)
                        {
                            self.render_text(
                                &rich.text,
                                first.style.font_size,
                                first.style.color,
                                first.style.underline,
                                *wrap,
                                *position,
                                *bounds,
                                *caret_index,
                                *caret_color,
                                *caret_width,
                                *caret_height,
                                *caret_radius,
                                *paragraph_style,
                                &[],
                                &[],
                            );
                            continue;
                        }
                    }

                    self.render_text(
                        &rich.text,
                        rich.base_size,
                        rich.base_color,
                        false,
                        *wrap,
                        *position,
                        *bounds,
                        *caret_index,
                        *caret_color,
                        *caret_width,
                        *caret_height,
                        *caret_radius,
                        *paragraph_style,
                        &rich.inline_boxes,
                        &rich.styles,
                    );
                }
                DisplayOp::DrawImage {
                    request,
                    rect,
                    fit,
                    alignment,
                    ..
                } => {
                    if !self.local_rect_visible(Self::layout_rect_to_rect(*rect)) {
                        IMAGE_OFFSCREEN_SKIPS.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    let request = self.image_request_for_rect(request, *rect);
                    if let Some(image_data) = self.get_image(&request) {
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
                                let (offset_x, offset_y) =
                                    aligned_offset(rect_w - w, rect_h - h, *alignment);
                                (
                                    scale,
                                    scale,
                                    rect.origin.x as f64 + offset_x,
                                    rect.origin.y as f64 + offset_y,
                                )
                            }
                            fission_render::ImageFit::Cover => {
                                let scale = (rect_w / img_w).max(rect_h / img_h);
                                let w = img_w * scale;
                                let h = img_h * scale;
                                let (offset_x, offset_y) =
                                    aligned_offset(rect_w - w, rect_h - h, *alignment);
                                (
                                    scale,
                                    scale,
                                    rect.origin.x as f64 + offset_x,
                                    rect.origin.y as f64 + offset_y,
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
                        let clip_rect = Rect::new(
                            rect.origin.x as f64,
                            rect.origin.y as f64,
                            (rect.origin.x + rect.size.width) as f64,
                            (rect.origin.y + rect.size.height) as f64,
                        );
                        self.with_clip_rect(clip_rect, |this| {
                            this.scene.draw_image(brush, transform);
                        });
                    }
                }
                DisplayOp::DrawPath {
                    path,
                    fill,
                    stroke,
                    bounds,
                    node_id,
                } => {
                    let provenance = node_id
                        .map(|node_id| format!(" for node {node_id}"))
                        .unwrap_or_default();
                    let bez_path = BezPath::from_svg(path).map_err(|error| {
                        anyhow!("Vello renderer rejected path{provenance}: {error}")
                    })?;
                    let transform = self.current_transform
                        * Affine::translate((bounds.origin.x as f64, bounds.origin.y as f64));
                    let paint_bounds = Rect::new(
                        0.0,
                        0.0,
                        bounds.size.width as f64,
                        bounds.size.height as f64,
                    );

                    if let Some(f) = fill {
                        let brush = map_fill_to_brush(f, paint_bounds);
                        self.scene
                            .fill(Fill::NonZero, transform, &brush, None, &bez_path);
                    }
                    if let Some(s) = stroke {
                        let (stroke_style, brush) = map_stroke(s, paint_bounds);
                        self.scene
                            .stroke(&stroke_style, transform, &brush, None, &bez_path);
                    }
                }
                DisplayOp::DrawSvg {
                    content,
                    fill,
                    stroke,
                    bounds,
                    node_id,
                } => {
                    let entry = svg_cache_entry(content);
                    if let Some(parse_error) = &entry.error {
                        let provenance = node_id
                            .map(|node_id| format!(" for node {node_id}"))
                            .unwrap_or_default();
                        return Err(anyhow!(
                            "Vello renderer rejected SVG{provenance}: {parse_error}"
                        ));
                    }
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
                    let paint_bounds = Rect::new(vb_x, vb_y, vb_x + vb_w, vb_y + vb_h);

                    for shape in &entry.shapes {
                        match shape {
                            SvgShape::Path(path) => {
                                if let Some(f) = fill {
                                    let brush = map_fill_to_brush(f, paint_bounds);
                                    self.scene.fill(
                                        Fill::NonZero,
                                        svg_transform,
                                        &brush,
                                        None,
                                        path,
                                    );
                                }
                                if let Some(s) = stroke {
                                    let (stroke_style, brush) = map_stroke(s, paint_bounds);
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
                                    let brush = map_fill_to_brush(f, paint_bounds);
                                    self.scene.fill(
                                        Fill::NonZero,
                                        svg_transform,
                                        &brush,
                                        None,
                                        rect,
                                    );
                                }
                                if let Some(s) = stroke {
                                    let (stroke_style, brush) = map_stroke(s, paint_bounds);
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
                DisplayOp::DrawSurface {
                    rect,
                    surface_id,
                    position,
                    ..
                } => {
                    let color = surface_placeholder_color(*surface_id, *position);
                    let shape = Rect::new(
                        rect.origin.x as f64,
                        rect.origin.y as f64,
                        (rect.origin.x + rect.size.width) as f64,
                        (rect.origin.y + rect.size.height) as f64,
                    );
                    self.scene.fill(
                        Fill::NonZero,
                        self.current_transform,
                        Color::from_rgba8(color.r, color.g, color.b, color.a),
                        None,
                        &shape,
                    );
                }
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
        let saved_clip_count = self.clip_stack.len();
        let layer_transform = layer
            .style
            .transform
            .as_ref()
            .map(Self::affine_from_mat4)
            .transpose()?;

        if let Some(clip) = &layer.style.clip {
            match clip {
                LayerClip::Rect(rect) => {
                    let r = Self::layout_rect_to_rect(*rect);
                    self.scene
                        .push_layer(Mix::Normal, 1.0, self.current_transform, &r);
                    self.push_clip_bounds(r);
                    self.current_layer_count += 1;
                }
                LayerClip::RoundedRect { rect, radius } => {
                    let r = Self::layout_rect_to_rect(*rect);
                    let shape = RoundedRect::from_rect(r, *radius as f64);
                    self.scene
                        .push_layer(Mix::Normal, 1.0, self.current_transform, &shape);
                    self.push_clip_bounds(r);
                    self.current_layer_count += 1;
                }
            }
        }

        if (layer.style.opacity - 1.0).abs() > 0.001 {
            let r = Self::layout_rect_to_rect(layer.bounds);
            self.scene
                .push_layer(Mix::Normal, layer.style.opacity, self.current_transform, &r);
            self.push_clip_bounds(r);
            self.current_layer_count += 1;
        }

        if let Some(affine) = layer_transform {
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
        self.clip_stack.truncate(saved_clip_count);
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

#[cfg(test)]
mod tests;
