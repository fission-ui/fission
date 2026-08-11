use std::borrow::Cow;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use fission_ir::op::{ImageAlignment, ImageRequest};
use fission_layout::TextMeasurer;
use fission_render::capabilities::is_2d_affine_transform;
use fission_render::{
    Color as RenderColor, DisplayList, DisplayOp, Fill, ImageFit, RenderScene, Stroke, TextRun,
};
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle as FontdueTextStyle};
use kurbo::BezPath;
use tiny_skia::{
    Color, FillRule as TinyFillRule, FilterQuality, Mask, Path, Pixmap, PixmapPaint, Transform,
};

use crate::fonts::{default_font, packaged_font};
use crate::resources::{
    aligned_offset, bez_to_tiny_path, cached_image, fill_paint,
    image_request_with_default_cache_size, insert_soft_wraps, normalized_scale_factor,
    pipeline_wrap_breaks, rect_path, rgba_to_premul, rounded_rect_path, stroke_style,
    svg_cache_entry, tiny_color, wrap_max_width, DrawState, ImageCacheLookup, SvgShape,
};
use crate::shadows::draw_software_box_shadow;

/// Rasterizes backend-neutral Fission scenes into premultiplied RGBA pixels.
pub struct SoftwareRenderer {
    width: u32,
    height: u32,
    scale_factor: f32,
    text_measurer: Option<Arc<dyn TextMeasurer>>,
    surfaces: Vec<Pixmap>,
    states: Vec<DrawState>,
}

impl SoftwareRenderer {
    fn new_with_scale(
        width: u32,
        height: u32,
        background: RenderColor,
        scale_factor: f32,
        text_measurer: Option<Arc<dyn TextMeasurer>>,
    ) -> Result<Self> {
        let mut root = Pixmap::new(width.max(1), height.max(1))
            .ok_or_else(|| anyhow!("failed to allocate software render target"))?;
        root.fill(tiny_color(background));
        Ok(Self {
            width: width.max(1),
            height: height.max(1),
            scale_factor: normalized_scale_factor(scale_factor),
            text_measurer,
            surfaces: vec![root],
            states: vec![DrawState {
                transform: Transform::identity(),
                clip: None,
                surface: 0,
                layer_alpha: None,
            }],
        })
    }

    /// Render a scene with the bundled fallback font.
    pub fn render(
        scene: &RenderScene,
        width: u32,
        height: u32,
        background: RenderColor,
        scale_factor: f32,
    ) -> Result<Vec<u8>> {
        Self::render_with_optional_text_measurer(
            scene,
            width,
            height,
            background,
            scale_factor,
            None,
        )
    }

    /// Render a scene while reusing line breaks from the active layout engine.
    pub fn render_with_text_measurer(
        scene: &RenderScene,
        width: u32,
        height: u32,
        background: RenderColor,
        scale_factor: f32,
        text_measurer: Arc<dyn TextMeasurer>,
    ) -> Result<Vec<u8>> {
        Self::render_with_optional_text_measurer(
            scene,
            width,
            height,
            background,
            scale_factor,
            Some(text_measurer),
        )
    }

    fn render_with_optional_text_measurer(
        scene: &RenderScene,
        width: u32,
        height: u32,
        background: RenderColor,
        scale_factor: f32,
        text_measurer: Option<Arc<dyn TextMeasurer>>,
    ) -> Result<Vec<u8>> {
        let mut renderer = Self::new_with_scale(
            width.max(1),
            height.max(1),
            background,
            scale_factor,
            text_measurer,
        )?;
        let display_list = scene.flatten();
        renderer.render_ops(&display_list)?;
        Ok(renderer.finish())
    }

    fn finish(self) -> Vec<u8> {
        self.finish_pixmap().take()
    }

    fn finish_pixmap(self) -> Pixmap {
        self.surfaces.into_iter().next().unwrap()
    }

    fn current_state(&self) -> &DrawState {
        self.states
            .last()
            .expect("software renderer state stack empty")
    }

    fn current_state_mut(&mut self) -> &mut DrawState {
        self.states
            .last_mut()
            .expect("software renderer state stack empty")
    }

    fn current_surface_mut(&mut self) -> &mut Pixmap {
        let surface = self.current_state().surface;
        &mut self.surfaces[surface]
    }

    fn current_clip(&self) -> Option<&Mask> {
        self.current_state().clip.as_ref()
    }

    fn device_transform(&self, logical: Transform) -> Transform {
        let scale = self.scale_factor;
        logical.post_scale(scale, scale)
    }

    fn current_device_transform(&self) -> Transform {
        self.device_transform(self.current_state().transform)
    }

    fn push_state(&mut self) {
        self.states.push(self.current_state().clone());
    }

    fn pop_state(&mut self) {
        if self.states.len() <= 1 {
            return;
        }
        let finished = self.states.pop().unwrap();
        let parent_surface = self.current_state().surface;
        if let Some(alpha) = finished.layer_alpha {
            if finished.surface != parent_surface {
                let clip = self.current_clip().cloned();
                let (low, high) = if parent_surface < finished.surface {
                    let (low, high) = self.surfaces.split_at_mut(finished.surface);
                    (&mut low[parent_surface], &mut high[0])
                } else {
                    let (low, high) = self.surfaces.split_at_mut(parent_surface);
                    (&mut high[0], &mut low[finished.surface])
                };
                let mut paint = PixmapPaint::default();
                paint.opacity = alpha;
                paint.quality = FilterQuality::Bilinear;
                low.draw_pixmap(
                    0,
                    0,
                    high.as_ref(),
                    &paint,
                    Transform::identity(),
                    clip.as_ref(),
                );
            }
        }
    }

    fn ensure_clip_path(&mut self, path: &Path) {
        let transform = self.current_device_transform();
        let width = self.width;
        let height = self.height;
        let state = self.current_state_mut();
        if let Some(mask) = state.clip.as_mut() {
            mask.intersect_path(path, TinyFillRule::Winding, true, transform);
        } else {
            let mut mask = Mask::new(width, height).unwrap();
            mask.fill_path(path, TinyFillRule::Winding, true, transform);
            state.clip = Some(mask);
        }
    }

    fn with_temporary_clip_rect<F>(
        &mut self,
        rect: fission_render::LayoutRect,
        draw: F,
    ) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        let Some(path) = rect_path(rect) else {
            return Ok(());
        };
        self.push_state();
        self.ensure_clip_path(&path);
        let result = draw(self);
        self.pop_state();
        result
    }

    fn start_opacity_layer(&mut self, alpha: f32) -> Result<()> {
        let mut layer = Pixmap::new(self.width, self.height)
            .ok_or_else(|| anyhow!("failed to allocate software layer"))?;
        layer.fill(Color::from_rgba8(0, 0, 0, 0));
        self.surfaces.push(layer);
        let surface = self.surfaces.len() - 1;
        let state = self.current_state_mut();
        state.surface = surface;
        state.layer_alpha = Some(alpha.clamp(0.0, 1.0));
        Ok(())
    }

    fn render_ops(&mut self, display_list: &DisplayList) -> Result<()> {
        for op in &display_list.ops {
            match op {
                DisplayOp::Save => self.push_state(),
                DisplayOp::Restore => self.pop_state(),
                DisplayOp::ClipRect(rect) => {
                    if let Some(path) = rect_path(*rect) {
                        self.ensure_clip_path(&path);
                    }
                }
                DisplayOp::ClipRoundedRect { rect, radius } => {
                    if let Some(path) = rounded_rect_path(*rect, *radius) {
                        self.ensure_clip_path(&path);
                    }
                }
                DisplayOp::OpacityLayer { alpha, .. } => {
                    self.start_opacity_layer(*alpha)?;
                }
                DisplayOp::BackdropFilter {
                    rect,
                    filter,
                    corner_radius,
                    ..
                } => self.draw_backdrop_filter(*rect, *filter, *corner_radius)?,
                DisplayOp::Translate(point) => {
                    let state = self.current_state_mut();
                    state.transform = state.transform.pre_translate(point.x, point.y);
                }
                DisplayOp::Transform(matrix) => {
                    let transform = software_affine_transform(*matrix)?;
                    let state = self.current_state_mut();
                    state.transform = state.transform.pre_concat(transform);
                }
                DisplayOp::CachedScene { list, .. } => self.render_ops(list)?,
                DisplayOp::DrawRect {
                    rect,
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                    ..
                } => {
                    self.draw_rect(
                        *rect,
                        fill.as_ref(),
                        stroke.as_ref(),
                        *corner_radius,
                        shadow.as_ref(),
                    )?;
                }
                DisplayOp::DrawText {
                    text,
                    position,
                    size,
                    color,
                    bounds,
                    underline,
                    wrap,
                    caret_index,
                    caret_color,
                    caret_width,
                    caret_height,
                    caret_radius,
                    paragraph_style,
                    node_id,
                } => {
                    validate_software_text_options(
                        *caret_index,
                        *caret_color,
                        *caret_width,
                        *caret_height,
                        *caret_radius,
                        *paragraph_style,
                        *node_id,
                    )?;
                    self.draw_text(text, *position, *size, *color, *bounds, *wrap, *underline)?;
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
                    node_id,
                    ..
                } => {
                    validate_software_text_options(
                        *caret_index,
                        *caret_color,
                        *caret_width,
                        *caret_height,
                        *caret_radius,
                        *paragraph_style,
                        *node_id,
                    )?;
                    validate_software_rich_text_styles(runs, *node_id)?;
                    self.draw_rich_text(runs, *position, *bounds, *wrap)?;
                }
                DisplayOp::DrawImage {
                    rect,
                    request,
                    fit,
                    alignment,
                    ..
                } => {
                    self.draw_image(*rect, request, *fit, *alignment)?;
                }
                DisplayOp::DrawPath {
                    path,
                    fill,
                    stroke,
                    bounds,
                    ..
                } => {
                    self.draw_path(path, fill.as_ref(), stroke.as_ref(), *bounds)?;
                }
                DisplayOp::DrawSvg {
                    content,
                    fill,
                    stroke,
                    bounds,
                    ..
                } => {
                    self.draw_svg(content, fill.as_ref(), stroke.as_ref(), *bounds)?;
                }
                DisplayOp::DrawSurface { surface_id, .. } => return Err(anyhow!(
                    "software renderer cannot compose external surface {surface_id}; the host must bind a supported external-surface transport"
                )),
            }
        }
        Ok(())
    }

    fn draw_backdrop_filter(
        &mut self,
        rect: fission_render::LayoutRect,
        filter: fission_ir::op::BackdropFilter,
        corner_radius: f32,
    ) -> Result<()> {
        let sigma = match filter {
            fission_ir::op::BackdropFilter::Blur(sigma) => sigma,
        };
        if sigma <= 0.0 {
            return Ok(());
        }

        let transform = self.current_device_transform();
        let scale_factor = self.scale_factor;
        let clip = self.current_clip().cloned();
        let path = if corner_radius > 0.0 {
            rounded_rect_path(rect, corner_radius)
        } else {
            rect_path(rect)
        }
        .ok_or_else(|| anyhow!("failed to build backdrop-filter path"))?;

        let surface = self.current_surface_mut();
        let width = surface.width();
        let height = surface.height();
        let original = surface.data().to_vec();
        let image = image::RgbaImage::from_raw(width, height, original.clone())
            .ok_or_else(|| anyhow!("invalid software-renderer backing surface"))?;
        let blurred = image::imageops::blur(&image, sigma * scale_factor);

        let mut filter_mask = Mask::new(width, height)
            .ok_or_else(|| anyhow!("failed to allocate backdrop-filter mask"))?;
        filter_mask.fill_path(&path, TinyFillRule::Winding, true, transform);
        let filter_mask = filter_mask.data();
        let clip_mask = clip.as_ref().map(Mask::data);
        let output = surface.data_mut();
        let blurred = blurred.as_raw();
        for pixel in 0..(width as usize * height as usize) {
            let mut coverage = u16::from(filter_mask[pixel]);
            if let Some(clip_mask) = clip_mask {
                coverage = coverage * u16::from(clip_mask[pixel]) / 255;
            }
            if coverage == 0 {
                continue;
            }
            let inverse = 255 - coverage;
            let byte = pixel * 4;
            for channel in 0..4 {
                output[byte + channel] = ((u16::from(original[byte + channel]) * inverse
                    + u16::from(blurred[byte + channel]) * coverage)
                    / 255) as u8;
            }
        }
        Ok(())
    }

    fn draw_rect(
        &mut self,
        rect: fission_render::LayoutRect,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
        corner_radius: f32,
        shadow: Option<&fission_render::BoxShadow>,
    ) -> Result<()> {
        let path = if corner_radius > 0.0 {
            rounded_rect_path(rect, corner_radius)
        } else {
            rect_path(rect)
        }
        .ok_or_else(|| anyhow!("failed to build rectangle path"))?;

        let transform = self.current_device_transform();
        let clip = self.current_clip().cloned();
        let scale_factor = self.scale_factor;
        let surface = self.current_surface_mut();

        if let Some(shadow) = shadow {
            draw_software_box_shadow(
                surface,
                rect,
                corner_radius,
                shadow,
                transform,
                clip.as_ref(),
                scale_factor,
            )?;
        }

        if let Some(fill) = fill {
            let paint = fill_paint(fill, rect);
            surface.fill_path(
                &path,
                &paint,
                TinyFillRule::Winding,
                transform,
                clip.as_ref(),
            );
        }
        if let Some(stroke) = stroke {
            let paint = fill_paint(&stroke.fill, rect);
            let style = stroke_style(stroke);
            surface.stroke_path(&path, &paint, &style, transform, clip.as_ref());
        }
        Ok(())
    }

    fn draw_text(
        &mut self,
        text: &str,
        position: fission_render::LayoutPoint,
        size: f32,
        color: RenderColor,
        bounds: fission_render::LayoutRect,
        wrap: bool,
        underline: bool,
    ) -> Result<()> {
        let font = default_font();
        let fonts = [font];
        let pipeline_breaks = pipeline_wrap_breaks(
            self.text_measurer.as_deref(),
            text,
            size,
            bounds.width(),
            wrap,
        );
        let layout_text = pipeline_breaks
            .as_deref()
            .map(|breaks| insert_soft_wraps(text, breaks))
            .unwrap_or(Cow::Borrowed(text));
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: position.x,
            y: position.y,
            max_width: if pipeline_breaks.is_some() {
                None
            } else {
                wrap_max_width(bounds.width(), size, wrap)
            },
            ..LayoutSettings::default()
        });
        layout.append(&fonts, &FontdueTextStyle::new(&layout_text, size, 0));
        self.draw_glyphs(&layout, &fonts, |_, _| color)?;
        if underline {
            self.draw_layout_underlines(&layout, color, size)?;
        }
        Ok(())
    }

    fn draw_rich_text(
        &mut self,
        runs: &[TextRun],
        position: fission_render::LayoutPoint,
        bounds: fission_render::LayoutRect,
        wrap: bool,
    ) -> Result<()> {
        let resolved_fonts = runs
            .iter()
            .map(|run| {
                packaged_font(
                    run.style.font_family.as_deref(),
                    run.style.font_weight,
                    run.style.font_style,
                )
            })
            .collect::<Vec<_>>();
        let fonts = resolved_fonts
            .iter()
            .map(|font| match font {
                Some(font) => font.as_ref(),
                None => default_font(),
            })
            .collect::<Vec<_>>();
        let full_text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        let base_size = runs.first().map(|run| run.style.font_size).unwrap_or(14.0);
        let pipeline_breaks = pipeline_wrap_breaks(
            self.text_measurer.as_deref(),
            &full_text,
            base_size,
            bounds.width(),
            wrap,
        );
        let mut break_cursor = 0;
        let mut text_cursor = 0;
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: position.x,
            y: position.y,
            max_width: if pipeline_breaks.is_some() {
                None
            } else {
                wrap_max_width(bounds.width(), base_size, wrap)
            },
            ..LayoutSettings::default()
        });
        for (font_index, run) in runs.iter().enumerate() {
            let run_start = text_cursor;
            let run_end = run_start + run.text.len();
            let rendered_text = if let Some(breaks) = pipeline_breaks.as_deref() {
                while break_cursor < breaks.len() && breaks[break_cursor] < run_start {
                    break_cursor += 1;
                }
                let first_break = break_cursor;
                while break_cursor < breaks.len() && breaks[break_cursor] <= run_end {
                    break_cursor += 1;
                }
                let local_breaks = breaks[first_break..break_cursor]
                    .iter()
                    .map(|break_at| break_at - run_start)
                    .collect::<Vec<_>>();
                insert_soft_wraps(&run.text, &local_breaks)
            } else {
                Cow::Borrowed(run.text.as_str())
            };
            layout.append(
                &fonts,
                &fontdue::layout::TextStyle::with_user_data(
                    &rendered_text,
                    run.style.font_size,
                    font_index,
                    (
                        run.style.color,
                        run.style.underline,
                        run.style.background_color,
                    ),
                ),
            );
            text_cursor = run_end;
        }
        self.draw_rich_text_backgrounds(&layout)?;
        self.draw_glyphs(&layout, &fonts, |_, (color, _, _)| *color)?;
        if let Some(lines) = layout.lines() {
            for line in lines {
                for glyph in &layout.glyphs()[line.glyph_start..=line.glyph_end] {
                    let (color, underline, _) = glyph.user_data;
                    if underline {
                        let underline_rect = fission_render::LayoutRect::new(
                            glyph.x,
                            line.baseline_y + 1.5,
                            glyph.width as f32,
                            (glyph.key.px / 14.0).max(1.0),
                        );
                        self.draw_rect(underline_rect, Some(&Fill::Solid(color)), None, 0.0, None)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn draw_rich_text_backgrounds(
        &mut self,
        layout: &Layout<(RenderColor, bool, Option<RenderColor>)>,
    ) -> Result<()> {
        let mut segments = Vec::new();
        if let Some(lines) = layout.lines() {
            for line in lines {
                if line.glyph_start > line.glyph_end || line.glyph_end >= layout.glyphs().len() {
                    continue;
                }
                let glyphs = &layout.glyphs()[line.glyph_start..=line.glyph_end];
                let top = line.baseline_y - line.max_ascent;
                let height = (line.max_ascent - line.min_descent).max(1.0);
                let mut current: Option<(RenderColor, f32, f32)> = None;

                for (index, glyph) in glyphs.iter().enumerate() {
                    let Some(background) = glyph.user_data.2 else {
                        if let Some((color, left, right)) = current.take() {
                            segments.push((
                                fission_render::LayoutRect::new(
                                    left,
                                    top,
                                    (right - left).max(0.0),
                                    height,
                                ),
                                color,
                            ));
                        }
                        continue;
                    };
                    let next_x = glyphs
                        .get(index + 1)
                        .map(|next| next.x)
                        .unwrap_or(glyph.x + glyph.width as f32);
                    let right = next_x.max(glyph.x + glyph.width as f32);
                    match &mut current {
                        Some((color, _, current_right)) if *color == background => {
                            *current_right = current_right.max(right);
                        }
                        Some(_) => {
                            let (color, left, right) = current.take().unwrap();
                            segments.push((
                                fission_render::LayoutRect::new(
                                    left,
                                    top,
                                    (right - left).max(0.0),
                                    height,
                                ),
                                color,
                            ));
                            current = Some((background, glyph.x, right));
                        }
                        None => current = Some((background, glyph.x, right)),
                    }
                }

                if let Some((color, left, right)) = current {
                    segments.push((
                        fission_render::LayoutRect::new(left, top, (right - left).max(0.0), height),
                        color,
                    ));
                }
            }
        }

        for (rect, color) in segments {
            if rect.width() > 0.0 && rect.height() > 0.0 {
                self.draw_rect(rect, Some(&Fill::Solid(color)), None, 0.0, None)?;
            }
        }
        Ok(())
    }

    fn draw_layout_underlines<U: Copy + Clone>(
        &mut self,
        layout: &Layout<U>,
        color: RenderColor,
        size: f32,
    ) -> Result<()> {
        if let Some(lines) = layout.lines() {
            for line in lines {
                if line.glyph_start > line.glyph_end || line.glyph_end >= layout.glyphs().len() {
                    continue;
                }
                let first = &layout.glyphs()[line.glyph_start];
                let last = &layout.glyphs()[line.glyph_end];
                let underline_rect = fission_render::LayoutRect::new(
                    first.x,
                    line.baseline_y + 1.5,
                    (last.x + last.width as f32 - first.x).max(1.0),
                    (size / 14.0).max(1.0),
                );
                self.draw_rect(underline_rect, Some(&Fill::Solid(color)), None, 0.0, None)?;
            }
        }
        Ok(())
    }

    fn draw_glyphs<U: Copy + Clone>(
        &mut self,
        layout: &Layout<U>,
        fonts: &[&fontdue::Font],
        color_for: impl Fn(&fontdue::layout::GlyphPosition<U>, &U) -> RenderColor,
    ) -> Result<()> {
        let transform = self.current_device_transform();
        let clip = self.current_clip().cloned();
        let surface = self.current_surface_mut();

        for glyph in layout.glyphs() {
            if glyph.width == 0 || glyph.height == 0 {
                continue;
            }
            let color = color_for(glyph, &glyph.user_data);
            let (draw_x, draw_y, px, draw_transform) = if transform.is_scale_translate()
                && transform.sx > 0.0
                && transform.sy > 0.0
                && (transform.sx - transform.sy).abs() < 0.01
            {
                (
                    (glyph.x * transform.sx + transform.tx).round() as i32,
                    (glyph.y * transform.sy + transform.ty).round() as i32,
                    (glyph.key.px * transform.sx).max(1.0),
                    Transform::identity(),
                )
            } else {
                (
                    glyph.x.round() as i32,
                    glyph.y.round() as i32,
                    glyph.key.px,
                    transform,
                )
            };
            let font = fonts.get(glyph.font_index).copied().ok_or_else(|| {
                anyhow!(
                    "software text layout selected missing font index {} from {} fonts",
                    glyph.font_index,
                    fonts.len()
                )
            })?;
            let (metrics, bitmap) = font.rasterize_indexed(glyph.key.glyph_index, px);
            if metrics.width == 0 || metrics.height == 0 || bitmap.is_empty() {
                continue;
            }

            let mut rgba = Vec::with_capacity(metrics.width * metrics.height * 4);
            for coverage in bitmap {
                let premul = rgba_to_premul(color, coverage);
                rgba.extend_from_slice(&[
                    premul.red(),
                    premul.green(),
                    premul.blue(),
                    premul.alpha(),
                ]);
            }
            let size = tiny_skia::IntSize::from_wh(metrics.width as u32, metrics.height as u32)
                .ok_or_else(|| anyhow!("invalid glyph pixmap size"))?;
            let pixmap = Pixmap::from_vec(rgba, size)
                .ok_or_else(|| anyhow!("failed to create glyph pixmap"))?;
            surface.draw_pixmap(
                draw_x,
                draw_y,
                pixmap.as_ref(),
                &PixmapPaint::default(),
                draw_transform,
                clip.as_ref(),
            );
        }
        Ok(())
    }

    fn draw_image(
        &mut self,
        rect: fission_render::LayoutRect,
        request: &ImageRequest,
        fit: ImageFit,
        alignment: ImageAlignment,
    ) -> Result<()> {
        let rect_w = rect.width();
        let rect_h = rect.height();
        if rect_w <= 0.0 || rect_h <= 0.0 {
            return Ok(());
        }

        let request = image_request_with_default_cache_size(request, rect);
        let image = match cached_image(&request) {
            ImageCacheLookup::Ready(image) => image,
            ImageCacheLookup::Loading => return Ok(()),
            ImageCacheLookup::Failed => {
                return Err(anyhow!(
                    "software renderer could not load image resource {}",
                    request.stable_cache_key()
                ));
            }
        };
        let img_w = image.width() as f32;
        let img_h = image.height() as f32;
        if img_w <= 0.0 || img_h <= 0.0 {
            return Ok(());
        }

        let (scale_x, scale_y, dx, dy) = match fit {
            ImageFit::Fill => (rect_w / img_w, rect_h / img_h, rect.origin.x, rect.origin.y),
            ImageFit::Contain => {
                let scale = (rect_w / img_w).min(rect_h / img_h);
                let w = img_w * scale;
                let h = img_h * scale;
                let (offset_x, offset_y) = aligned_offset(rect_w - w, rect_h - h, alignment);
                (
                    scale,
                    scale,
                    rect.origin.x + offset_x,
                    rect.origin.y + offset_y,
                )
            }
            ImageFit::Cover => {
                let scale = (rect_w / img_w).max(rect_h / img_h);
                let w = img_w * scale;
                let h = img_h * scale;
                let (offset_x, offset_y) = aligned_offset(rect_w - w, rect_h - h, alignment);
                (
                    scale,
                    scale,
                    rect.origin.x + offset_x,
                    rect.origin.y + offset_y,
                )
            }
            ImageFit::None => (1.0, 1.0, rect.origin.x, rect.origin.y),
        };
        let transform = self.device_transform(
            self.current_state()
                .transform
                .pre_translate(dx, dy)
                .pre_scale(scale_x, scale_y),
        );
        self.with_temporary_clip_rect(rect, |this| {
            let clip = this.current_clip().cloned();
            let surface = this.current_surface_mut();
            let mut paint = PixmapPaint::default();
            paint.quality = FilterQuality::Bilinear;
            surface.draw_pixmap(
                0,
                0,
                image.as_ref().as_ref(),
                &paint,
                transform,
                clip.as_ref(),
            );
            Ok(())
        })
    }

    fn draw_path(
        &mut self,
        path: &str,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
        bounds: fission_render::LayoutRect,
    ) -> Result<()> {
        let bez = BezPath::from_svg(path)
            .map_err(|error| anyhow!("invalid software-renderer path data: {error}"))?;
        let path = bez_to_tiny_path(&bez)
            .ok_or_else(|| anyhow!("software renderer could not construct the requested path"))?;
        let transform = self.device_transform(
            self.current_state()
                .transform
                .pre_translate(bounds.origin.x, bounds.origin.y),
        );
        let clip = self.current_clip().cloned();
        let surface = self.current_surface_mut();
        let paint_bounds =
            fission_render::LayoutRect::new(0.0, 0.0, bounds.width(), bounds.height());
        if let Some(fill) = fill {
            let paint = fill_paint(fill, paint_bounds);
            surface.fill_path(
                &path,
                &paint,
                TinyFillRule::Winding,
                transform,
                clip.as_ref(),
            );
        }
        if let Some(stroke) = stroke {
            let paint = fill_paint(&stroke.fill, paint_bounds);
            let style = stroke_style(stroke);
            surface.stroke_path(&path, &paint, &style, transform, clip.as_ref());
        }
        Ok(())
    }

    fn draw_svg(
        &mut self,
        content: &str,
        fill: Option<&Fill>,
        stroke: Option<&Stroke>,
        bounds: fission_render::LayoutRect,
    ) -> Result<()> {
        if fill.is_none() && stroke.is_none() {
            return Err(anyhow!(
                "software SVG rendering requires an explicit Fission fill or stroke"
            ));
        }
        let entry = svg_cache_entry(content);
        if let Some(error) = &entry.error {
            return Err(anyhow!("software renderer rejected SVG content: {error}"));
        }
        if entry.shapes.is_empty() {
            return Err(anyhow!(
                "software renderer rejected SVG content with no supported visible shapes"
            ));
        }
        let (vb_x, vb_y, vb_w, vb_h) =
            entry
                .view_box
                .unwrap_or((0.0, 0.0, bounds.width(), bounds.height()));
        let rect_w = bounds.width();
        let rect_h = bounds.height();
        let (scale, dx, dy) = if vb_w > 0.0 && vb_h > 0.0 && rect_w > 0.0 && rect_h > 0.0 {
            let scale = (rect_w / vb_w).min(rect_h / vb_h);
            let scaled_w = vb_w * scale;
            let scaled_h = vb_h * scale;
            (
                scale,
                bounds.origin.x + (rect_w - scaled_w) / 2.0 - vb_x * scale,
                bounds.origin.y + (rect_h - scaled_h) / 2.0 - vb_y * scale,
            )
        } else {
            (1.0, bounds.origin.x, bounds.origin.y)
        };
        let transform = self.device_transform(
            self.current_state()
                .transform
                .pre_translate(dx, dy)
                .pre_scale(scale, scale),
        );
        let clip = self.current_clip().cloned();
        let surface = self.current_surface_mut();
        let paint_bounds = fission_render::LayoutRect::new(vb_x, vb_y, vb_w, vb_h);

        for shape in &entry.shapes {
            match shape {
                SvgShape::Path(bez) => {
                    let path = bez_to_tiny_path(bez).ok_or_else(|| {
                        anyhow!("software renderer could not construct a parsed SVG path")
                    })?;
                    if let Some(fill) = fill {
                        let paint = fill_paint(fill, paint_bounds);
                        surface.fill_path(
                            &path,
                            &paint,
                            TinyFillRule::Winding,
                            transform,
                            clip.as_ref(),
                        );
                    }
                    if let Some(stroke) = stroke {
                        let paint = fill_paint(&stroke.fill, paint_bounds);
                        let style = stroke_style(stroke);
                        surface.stroke_path(&path, &paint, &style, transform, clip.as_ref());
                    }
                }
                SvgShape::Rect {
                    x,
                    y,
                    width,
                    height,
                } => {
                    let path = rect_path(fission_render::LayoutRect::new(*x, *y, *width, *height))
                        .ok_or_else(|| {
                            anyhow!("software renderer could not construct a parsed SVG rectangle")
                        })?;
                    if let Some(fill) = fill {
                        let paint = fill_paint(fill, paint_bounds);
                        surface.fill_path(
                            &path,
                            &paint,
                            TinyFillRule::Winding,
                            transform,
                            clip.as_ref(),
                        );
                    }
                    if let Some(stroke) = stroke {
                        let paint = fill_paint(&stroke.fill, paint_bounds);
                        let style = stroke_style(stroke);
                        surface.stroke_path(&path, &paint, &style, transform, clip.as_ref());
                    }
                }
            }
        }

        Ok(())
    }
}

fn software_affine_transform(matrix: [f32; 16]) -> Result<Transform> {
    if !is_2d_affine_transform(&matrix) {
        return Err(anyhow!(
            "software renderer requires a finite two-dimensional affine transform"
        ));
    }
    Ok(Transform::from_row(
        matrix[0], matrix[1], matrix[4], matrix[5], matrix[12], matrix[13],
    ))
}

fn validate_software_text_options(
    caret_index: Option<usize>,
    caret_color: Option<RenderColor>,
    caret_width: Option<f32>,
    caret_height: Option<f32>,
    caret_radius: Option<f32>,
    paragraph_style: Option<fission_ir::op::TextParagraphStyle>,
    node_id: Option<fission_ir::WidgetId>,
) -> Result<()> {
    if caret_index.is_some()
        || caret_color.is_some()
        || caret_width.is_some()
        || caret_height.is_some()
        || caret_radius.is_some()
    {
        return Err(anyhow!(
            "software renderer does not yet support text caret painting for node {node_id:?}"
        ));
    }
    if paragraph_style.is_some_and(|style| style != Default::default()) {
        return Err(anyhow!(
            "software renderer does not yet support non-default paragraph styling for node {node_id:?}"
        ));
    }
    Ok(())
}

fn validate_software_rich_text_styles(
    runs: &[TextRun],
    node_id: Option<fission_ir::WidgetId>,
) -> Result<()> {
    if runs.iter().any(|run| {
        run.style.locale.is_some()
            || run.style.line_height.is_some()
            || run.style.letter_spacing != 0.0
    }) {
        return Err(anyhow!(
            "software renderer does not yet support rich-text locale, line-height, or letter-spacing overrides for node {node_id:?}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_rasterization_uses_the_font_index_selected_during_layout() {
        let font = default_font();
        let layout_fonts = [font, font];
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings::default());
        layout.append(
            &layout_fonts,
            &FontdueTextStyle::with_user_data(
                "A",
                16.0,
                1,
                RenderColor {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            ),
        );
        assert!(layout.glyphs().iter().any(|glyph| glyph.font_index == 1));

        let mut renderer = SoftwareRenderer::new_with_scale(
            32,
            32,
            RenderColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            1.0,
            None,
        )
        .unwrap();
        let error = renderer
            .draw_glyphs(&layout, &layout_fonts[..1], |_, color| *color)
            .unwrap_err()
            .to_string();

        assert!(error.contains("missing font index 1 from 1 fonts"));
        renderer
            .draw_glyphs(&layout, &layout_fonts, |_, color| *color)
            .expect("the selected layout font is available");
    }

    #[test]
    fn non_affine_transform_is_rejected_instead_of_flattened() {
        let mut perspective = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        perspective[3] = 0.25;

        assert!(software_affine_transform(perspective).is_err());
    }
}
