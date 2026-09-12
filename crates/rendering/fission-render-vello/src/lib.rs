pub mod cpu;
pub mod gpu;
pub mod painter;
pub mod text;
mod text_effects;
pub use painter::{CpuPainter, GpuImageCache, GpuPainter, GpuUploader, Painter};
pub use parley;
pub use text::VelloTextMeasurer;

use anyhow::Result;
use fission_ir::op::{
    decode_text_paragraph_style, HttpHeader, ImageAlignment, ImageRequest, ImageSource, TextAlign,
    TextDirection, TextHeightBehavior, TextOverflow, TextParagraphStyle, TextWidthBasis,
};
use fission_render::{
    surface_placeholder_color, Color as RenderColor, DisplayList, DisplayOp, LayerClip,
    RenderLayer, RenderNode, RenderScene, Renderer, TextStyle as RenderTextStyle,
};
use vello_cpu::kurbo::{
    Affine, BezPath, Circle, Point, Rect, RoundedRect, RoundedRectRadii, Shape, Stroke, Vec2,
};
use vello_cpu::peniko::{BlendMode, Color, ImageAlphaType, ImageSampler};
use vello_cpu::{Glyph, Image, PaintType, PixelMetadata, Pixmap};

fn text_style_requires_rich_layout(style: &RenderTextStyle) -> bool {
    text::text_style_requires_rich_layout(style)
}

fn map_color(c: &fission_render::Color) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a).into()
}

fn normalized_point(bounds: Rect, point: (f32, f32)) -> Point {
    Point::new(
        bounds.x0 + bounds.width() * point.0 as f64,
        bounds.y0 + bounds.height() * point.1 as f64,
    )
}

/// Maps the IR's extend mode onto peniko's.
///
/// Vello implements all three, so nothing is lost here.
/// Maps the IR's per-corner radii onto kurbo's.
fn kurbo_radii(radii: fission_ir::CornerRadii) -> RoundedRectRadii {
    RoundedRectRadii::new(
        radii.top_left as f64,
        radii.top_right as f64,
        radii.bottom_right as f64,
        radii.bottom_left as f64,
    )
}

fn map_extend(extend: fission_ir::GradientExtend) -> vello_cpu::peniko::Extend {
    match extend {
        fission_ir::GradientExtend::Pad => vello_cpu::peniko::Extend::Pad,
        fission_ir::GradientExtend::Repeat => vello_cpu::peniko::Extend::Repeat,
        fission_ir::GradientExtend::Reflect => vello_cpu::peniko::Extend::Reflect,
    }
}

fn map_fill_to_brush(f: &fission_render::Fill, bounds: Rect) -> PaintType {
    fn gradient_stops<C: Copy>(
        stops: &[(f32, C)],
        to_color: impl Fn(&C) -> Color,
    ) -> Vec<vello_cpu::peniko::ColorStop> {
        stops
            .iter()
            .map(|(offset, color)| vello_cpu::peniko::ColorStop {
                offset: *offset,
                color: to_color(color).into(),
            })
            .collect()
    }

    match f {
        fission_render::Fill::Solid(c) => PaintType::from(map_color(c)),
        fission_render::Fill::LinearGradient {
            start,
            end,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_linear(
                normalized_point(bounds, *start),
                normalized_point(bounds, *end),
            )
            .with_extend(map_extend(*extend))
            .with_stops(gradient_stops(stops, map_color).as_slice()),
        ),
        fission_render::Fill::RadialGradient {
            center,
            radius,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_radial(
                normalized_point(bounds, *center),
                radius * bounds.width().max(bounds.height()) as f32,
            )
            .with_extend(map_extend(*extend))
            .with_stops(gradient_stops(stops, map_color).as_slice()),
        ),
        fission_render::Fill::SweepGradient {
            center,
            start_angle,
            end_angle,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_sweep(
                normalized_point(bounds, *center),
                *start_angle,
                *end_angle,
            )
            .with_extend(map_extend(*extend))
            .with_stops(gradient_stops(stops, map_color).as_slice()),
        ),
    }
}

fn map_text_fill_to_brush(f: &fission_ir::op::Fill, bounds: Rect) -> PaintType {
    fn ir_stops(stops: &[(f32, fission_ir::op::Color)]) -> Vec<vello_cpu::peniko::ColorStop> {
        stops
            .iter()
            .map(|(offset, color)| vello_cpu::peniko::ColorStop {
                offset: *offset,
                color: Color::from_rgba8(color.r, color.g, color.b, color.a).into(),
            })
            .collect()
    }

    match f {
        fission_ir::op::Fill::Solid(c) => PaintType::from(Color::from_rgba8(c.r, c.g, c.b, c.a)),
        fission_ir::op::Fill::LinearGradient {
            start,
            end,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_linear(
                normalized_point(bounds, *start),
                normalized_point(bounds, *end),
            )
            .with_extend(map_extend(*extend))
            .with_stops(ir_stops(stops).as_slice()),
        ),
        fission_ir::op::Fill::RadialGradient {
            center,
            radius,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_radial(
                normalized_point(bounds, *center),
                radius * bounds.width().max(bounds.height()) as f32,
            )
            .with_extend(map_extend(*extend))
            .with_stops(ir_stops(stops).as_slice()),
        ),
        fission_ir::op::Fill::SweepGradient {
            center,
            start_angle,
            end_angle,
            stops,
            extend,
        } => PaintType::from(
            vello_cpu::peniko::Gradient::new_sweep(
                normalized_point(bounds, *center),
                *start_angle,
                *end_angle,
            )
            .with_extend(map_extend(*extend))
            .with_stops(ir_stops(stops).as_slice()),
        ),
    }
}

fn map_stroke(s: &fission_render::Stroke, bounds: Rect) -> (vello_cpu::kurbo::Stroke, PaintType) {
    let cap = match s.line_cap {
        fission_render::LineCap::Butt => vello_cpu::kurbo::Cap::Butt,
        fission_render::LineCap::Round => vello_cpu::kurbo::Cap::Round,
        fission_render::LineCap::Square => vello_cpu::kurbo::Cap::Square,
    };
    let join = match s.line_join {
        fission_render::LineJoin::Miter => vello_cpu::kurbo::Join::Miter,
        fission_render::LineJoin::Round => vello_cpu::kurbo::Join::Round,
        fission_render::LineJoin::Bevel => vello_cpu::kurbo::Join::Bevel,
    };

    let mut stroke = vello_cpu::kurbo::Stroke::new(s.width as f64)
        .with_caps(cap)
        .with_join(join);
    if let Some(dash) = &s.dash_array {
        let dashes: Vec<f64> = dash.iter().map(|v| *v as f64).collect();
        stroke = stroke.with_dashes(0.0, dashes);
    }

    (stroke, map_fill_to_brush(&s.fill, bounds))
}

use crate::text::ParleyBrush;
use fission_render::image_cache_store::ImageCacheStore;
use lazy_static::lazy_static;
use parley::layout::{Alignment as ParleyAlignment, AlignmentOptions, PositionedLayoutItem};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

const DEFAULT_IMAGE_CACHE_BYTES: u64 = 50 * 1024 * 1024;
const PARAGRAPH_FADE_SLICE_COUNT: usize = 8;
const PARAGRAPH_FADE_MIN_SPAN: f32 = 8.0;
const PARAGRAPH_FADE_RIGHT_MULTIPLIER: f32 = 1.5;
const PARAGRAPH_FADE_BOTTOM_FRACTION: f32 = 0.5;
const TEXT_CULL_PADDING: f32 = 8.0;
/// Slack added when re-breaking a paragraph for alignment, so rounding cannot wrap its widest line.
const TEXT_ALIGNMENT_BREAK_EPSILON: f32 = 0.01;
const LTR_DIRECTION_MARK: &str = "\u{200E}";
const RTL_DIRECTION_MARK: &str = "\u{200F}";

#[derive(Debug, Clone, Copy, PartialEq)]
struct ParagraphLineVisualBounds {
    left: f32,
    right: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParagraphFade {
    Right { start: f32, end: f32 },
    Bottom { start: f32, end: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TextClip {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl TextClip {
    fn intersects_y(self, top: f32, bottom: f32) -> bool {
        self.bottom >= self.top && bottom >= self.top && top <= self.bottom
    }

    fn intersects_x(self, left: f32, right: f32) -> bool {
        self.right >= self.left && right >= self.left && left <= self.right
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TextBackgroundSegment {
    left: f32,
    right: f32,
}

fn text_background_segments_for_cluster_ranges(
    clusters: impl IntoIterator<Item = (std::ops::Range<usize>, f32, f32)>,
    style_range: &std::ops::Range<usize>,
    clip: Option<TextClip>,
) -> Vec<TextBackgroundSegment> {
    let mut segments = Vec::new();
    let mut current: Option<TextBackgroundSegment> = None;

    for (cluster_range, cluster_left, cluster_right) in clusters {
        let overlaps =
            style_range.start < cluster_range.end && style_range.end > cluster_range.start;
        if !overlaps {
            if let Some(segment) = current.take() {
                segments.push(segment);
            }
            continue;
        }

        let mut left = cluster_left.min(cluster_right);
        let mut right = cluster_left.max(cluster_right);
        if let Some(clip) = clip {
            left = left.max(clip.left);
            right = right.min(clip.right);
        }
        if right <= left {
            if let Some(segment) = current.take() {
                segments.push(segment);
            }
            continue;
        }

        match &mut current {
            Some(segment) if left <= segment.right + 0.5 => {
                segment.right = segment.right.max(right);
            }
            Some(_) => {
                segments.push(current.take().expect("segment checked above"));
                current = Some(TextBackgroundSegment { left, right });
            }
            None => {
                current = Some(TextBackgroundSegment { left, right });
            }
        }
    }

    if let Some(segment) = current {
        segments.push(segment);
    }

    segments
}

#[derive(Debug, Clone)]
struct PreparedParagraphLayout {
    text: String,
    base_style: RenderTextStyle,
    styles: Vec<(std::ops::Range<usize>, RenderTextStyle)>,
    inline_boxes: Vec<crate::text::RichInlineBox>,
    caret_index: Option<usize>,
    #[allow(dead_code)]
    text_byte_offset: usize,
}

fn paragraph_style_with_strut(
    style: &RenderTextStyle,
    paragraph: TextParagraphStyle,
) -> RenderTextStyle {
    let mut style = style.clone();
    if let Some(strut_line_height) = paragraph.strut_line_height {
        style.line_height = Some(
            style
                .line_height
                .map_or(strut_line_height, |height| height.max(strut_line_height)),
        );
    }
    style
}

fn prepare_paragraph_layout(
    text: &str,
    base_style: &RenderTextStyle,
    paragraph: TextParagraphStyle,
    inline_boxes: &[crate::text::RichInlineBox],
    styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    caret_index: Option<usize>,
) -> PreparedParagraphLayout {
    let base_style = paragraph_style_with_strut(base_style, paragraph);
    let mut styles = if styles.is_empty() && !text.is_empty() {
        vec![(0..text.len(), base_style.clone())]
    } else {
        styles
            .iter()
            .map(|(range, style)| (range.clone(), paragraph_style_with_strut(style, paragraph)))
            .collect()
    };
    let mut inline_boxes = inline_boxes.to_vec();
    let mut text = text.to_string();
    let mut caret_index = caret_index;
    let mut text_byte_offset = 0usize;

    let direction_mark = match paragraph.text_direction {
        TextDirection::Auto => None,
        TextDirection::Ltr => Some(LTR_DIRECTION_MARK),
        TextDirection::Rtl => Some(RTL_DIRECTION_MARK),
    };

    if let Some(direction_mark) =
        direction_mark.filter(|_| !text.is_empty() || !inline_boxes.is_empty())
    {
        let prefix_len = direction_mark.len();
        text_byte_offset = prefix_len;
        text.insert_str(0, direction_mark);
        for (range, _) in &mut styles {
            range.start += prefix_len;
            range.end += prefix_len;
        }
        styles.insert(0, (0..prefix_len, base_style.clone()));
        for inline_box in &mut inline_boxes {
            inline_box.index += prefix_len;
        }
        caret_index = caret_index.map(|index| index + prefix_len);
    }

    PreparedParagraphLayout {
        text,
        base_style,
        styles,
        inline_boxes,
        caret_index,
        text_byte_offset,
    }
}

fn paragraph_line_trim(
    line: &parley::layout::Line<'_, ParleyBrush>,
    behavior: TextHeightBehavior,
    is_first_visible_line: bool,
    is_last_visible_line: bool,
) -> (f32, f32) {
    let metrics = line.metrics();
    let top_trim = if is_first_visible_line && !behavior.apply_height_to_first_ascent {
        (metrics.baseline - metrics.ascent).max(0.0)
    } else {
        0.0
    };
    let bottom_trim = if is_last_visible_line && !behavior.apply_height_to_last_descent {
        (metrics.line_height - (metrics.baseline + metrics.descent)).max(0.0)
    } else {
        0.0
    };
    (top_trim, bottom_trim)
}

fn paragraph_y_offset(
    line: Option<&parley::layout::Line<'_, ParleyBrush>>,
    behavior: TextHeightBehavior,
    is_last_visible_line: bool,
) -> f32 {
    line.map_or(0.0, |line| {
        let (top_trim, _) = paragraph_line_trim(line, behavior, true, is_last_visible_line);
        -top_trim
    })
}

fn paragraph_alignment(text_align: TextAlign) -> ParleyAlignment {
    match text_align {
        TextAlign::Start => ParleyAlignment::Start,
        TextAlign::Left => ParleyAlignment::Left,
        TextAlign::Center => ParleyAlignment::Center,
        TextAlign::Right => ParleyAlignment::Right,
        TextAlign::End => ParleyAlignment::End,
        TextAlign::Justify => ParleyAlignment::Justify,
    }
}

fn paragraph_alignment_options(text_align: TextAlign) -> AlignmentOptions {
    AlignmentOptions {
        align_when_overflowing: !matches!(text_align, TextAlign::Justify),
    }
}

fn paragraph_alignment_width(
    layout: &parley::layout::Layout<ParleyBrush>,
    bounds: fission_render::LayoutRect,
    paragraph: TextParagraphStyle,
) -> Option<f32> {
    let width = match paragraph.text_width_basis {
        TextWidthBasis::Parent => bounds.width(),
        TextWidthBasis::LongestLine => layout.width(),
    };

    (width.is_finite() && width > 0.0).then_some(width)
}

fn paragraph_line_visual_bounds(
    line: &parley::layout::Line<'_, ParleyBrush>,
) -> Option<ParagraphLineVisualBounds> {
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;

    for item in line.items() {
        match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => {
                left = left.min(glyph_run.offset());
                right = right.max(glyph_run.offset() + glyph_run.advance());
            }
            PositionedLayoutItem::InlineBox(inline_box) => {
                left = left.min(inline_box.x);
                right = right.max(inline_box.x + inline_box.width);
            }
        }
    }

    if left.is_finite() && right.is_finite() {
        Some(ParagraphLineVisualBounds { left, right })
    } else {
        None
    }
}

fn paragraph_fade(
    paragraph: TextParagraphStyle,
    bounds: fission_render::LayoutRect,
    line_height: f32,
    line_width: f32,
    is_last_visible_line: bool,
    has_more_lines: bool,
    overflows_horizontally: bool,
) -> Option<ParagraphFade> {
    if !matches!(paragraph.overflow, TextOverflow::Fade) || !is_last_visible_line {
        return None;
    }

    if has_more_lines {
        let fade_height = (line_height * PARAGRAPH_FADE_BOTTOM_FRACTION)
            .max(1.0)
            .min(bounds.height().max(1.0));
        return Some(ParagraphFade::Bottom {
            start: (line_height - fade_height).max(0.0),
            end: line_height,
        });
    }

    if !overflows_horizontally || bounds.width() <= 0.0 {
        return None;
    }

    let fade_width = line_width
        .min(bounds.width())
        .min((line_height * PARAGRAPH_FADE_RIGHT_MULTIPLIER).max(PARAGRAPH_FADE_MIN_SPAN));
    if fade_width <= 0.0 {
        return None;
    }

    Some(ParagraphFade::Right {
        start: (bounds.width() - fade_width).max(0.0),
        end: bounds.width(),
    })
}

lazy_static! {
    static ref IMAGE_CACHE: ImageCacheStore<ImageCacheEntry> = build_image_cache();
    static ref SVG_CACHE: Mutex<HashMap<u64, Arc<SvgCacheEntry>>> = Mutex::new(HashMap::new());
}

static IMAGE_CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);
static IMAGE_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static IMAGE_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static IMAGE_LOADS_STARTED: AtomicU64 = AtomicU64::new(0);
static IMAGE_LOADS_COMPLETED: AtomicU64 = AtomicU64::new(0);
static IMAGE_LOADS_FAILED: AtomicU64 = AtomicU64::new(0);
static IMAGE_CACHE_EVICTIONS: AtomicU64 = AtomicU64::new(0);
static IMAGE_OFFSCREEN_SKIPS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
enum ImageCacheEntry {
    Ready(Arc<Pixmap>),
    Loading,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageCacheStats {
    pub entries: u64,
    pub weighted_bytes: u64,
    pub max_bytes: u64,
    pub pending: u64,
    pub hits: u64,
    pub misses: u64,
    pub loads_started: u64,
    pub loads_completed: u64,
    pub loads_failed: u64,
    pub evictions: u64,
    pub offscreen_skips: u64,
}

impl ImageCacheEntry {
    fn weight(&self) -> u32 {
        match self {
            Self::Ready(image) => image_byte_len(image).min(u64::from(u32::MAX)) as u32,
            // Pending and failed entries should not consume meaningful byte budget,
            // but keeping a non-zero weight prevents unlimited metadata growth.
            Self::Loading | Self::Failed => 1,
        }
    }
}

fn build_image_cache() -> ImageCacheStore<ImageCacheEntry> {
    ImageCacheStore::new(
        "fission-render-vello-images",
        configured_image_cache_bytes(),
        ImageCacheEntry::weight,
        || {
            IMAGE_CACHE_EVICTIONS.fetch_add(1, Ordering::AcqRel);
        },
    )
}

fn configured_image_cache_bytes() -> u64 {
    std::env::var("FISSION_IMAGE_CACHE_BYTES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_IMAGE_CACHE_BYTES)
}

fn image_byte_len(image: &Pixmap) -> u64 {
    u64::from(image.width())
        .saturating_mul(u64::from(image.height()))
        .saturating_mul(4)
}

fn image_request_with_default_cache_size(
    request: &ImageRequest,
    rect: Rect,
    transform: Affine,
) -> ImageRequest {
    if request.cache_width.is_some() && request.cache_height.is_some() {
        return request.clone();
    }

    let transformed = VelloRenderer::transform_rect_bounds(transform, rect);
    if transformed.width() <= 0.0 || transformed.height() <= 0.0 {
        return request.clone();
    }

    let mut request = request.clone();
    request.cache_width = Some(cache_dimension_from_extent(transformed.width()));
    request.cache_height = Some(cache_dimension_from_extent(transformed.height()));
    request
}

fn cache_dimension_from_extent(extent: f64) -> u32 {
    if !extent.is_finite() {
        return 1;
    }
    extent.ceil().clamp(1.0, f64::from(u32::MAX)) as u32
}

pub fn image_cache_generation() -> u64 {
    IMAGE_CACHE_GENERATION.load(Ordering::Acquire)
}

pub fn image_cache_has_pending() -> bool {
    IMAGE_CACHE
        .values()
        .into_iter()
        .any(|entry| matches!(entry, ImageCacheEntry::Loading))
}

pub fn image_cache_stats() -> ImageCacheStats {
    IMAGE_CACHE.run_pending_tasks();
    ImageCacheStats {
        entries: IMAGE_CACHE.entry_count(),
        weighted_bytes: IMAGE_CACHE.weighted_size(),
        max_bytes: configured_image_cache_bytes(),
        pending: IMAGE_CACHE
            .values()
            .into_iter()
            .filter(|entry| matches!(entry, ImageCacheEntry::Loading))
            .count() as u64,
        hits: IMAGE_CACHE_HITS.load(Ordering::Acquire),
        misses: IMAGE_CACHE_MISSES.load(Ordering::Acquire),
        loads_started: IMAGE_LOADS_STARTED.load(Ordering::Acquire),
        loads_completed: IMAGE_LOADS_COMPLETED.load(Ordering::Acquire),
        loads_failed: IMAGE_LOADS_FAILED.load(Ordering::Acquire),
        evictions: IMAGE_CACHE_EVICTIONS.load(Ordering::Acquire),
        offscreen_skips: IMAGE_OFFSCREEN_SKIPS.load(Ordering::Acquire),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_image_from_path(
    path: &str,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    let img = image::open(path).ok()?;
    decode_dynamic_image(img, cache_width, cache_height)
}

fn decode_image_from_bytes(
    bytes: &[u8],
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    let img = image::load_from_memory(bytes).ok()?;
    decode_dynamic_image(img, cache_width, cache_height)
}

fn decode_dynamic_image(
    mut img: image::DynamicImage,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    if let (Some(width), Some(height)) = (cache_width, cache_height) {
        if width > 0 && height > 0 {
            img = img.resize(width, height, image::imageops::FilterType::Triangle);
        }
    }
    let img = img.to_rgba8();
    let (width, height) = img.dimensions();
    // The renderers address pixmaps with 16-bit dimensions.
    let (Ok(width), Ok(height)) = (u16::try_from(width), u16::try_from(height)) else {
        return None;
    };
    Some(Arc::new(Pixmap::from_parts(
        img.into_raw(),
        width,
        height,
        PixelMetadata::new(ImageAlphaType::Alpha, true),
    )))
}

fn complete_image_load(key: String, image: Option<Arc<Pixmap>>) {
    if image.is_some() {
        IMAGE_LOADS_COMPLETED.fetch_add(1, Ordering::AcqRel);
    } else {
        IMAGE_LOADS_FAILED.fetch_add(1, Ordering::AcqRel);
    }
    IMAGE_CACHE.insert(
        key,
        image
            .map(ImageCacheEntry::Ready)
            .unwrap_or(ImageCacheEntry::Failed),
    );
    IMAGE_CACHE_GENERATION.fetch_add(1, Ordering::AcqRel);
}

fn aligned_offset(extra_width: f64, extra_height: f64, alignment: ImageAlignment) -> (f64, f64) {
    let x = match alignment {
        ImageAlignment::TopStart | ImageAlignment::CenterStart | ImageAlignment::BottomStart => 0.0,
        ImageAlignment::TopCenter | ImageAlignment::Center | ImageAlignment::BottomCenter => {
            extra_width / 2.0
        }
        ImageAlignment::TopEnd | ImageAlignment::CenterEnd | ImageAlignment::BottomEnd => {
            extra_width
        }
    };
    let y = match alignment {
        ImageAlignment::TopStart | ImageAlignment::TopCenter | ImageAlignment::TopEnd => 0.0,
        ImageAlignment::CenterStart | ImageAlignment::Center | ImageAlignment::CenterEnd => {
            extra_height / 2.0
        }
        ImageAlignment::BottomStart | ImageAlignment::BottomCenter | ImageAlignment::BottomEnd => {
            extra_height
        }
    };
    (x, y)
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_image_load(key: String, request: ImageRequest) {
    std::thread::spawn(move || {
        let image = match request.source {
            ImageSource::Asset { path } | ImageSource::File { path } => {
                decode_image_from_path(&path, request.cache_width, request.cache_height)
            }
            ImageSource::Memory { bytes, .. } => {
                decode_image_from_bytes(&bytes, request.cache_width, request.cache_height)
            }
            ImageSource::Network { url, headers, .. } => {
                fetch_network_image(&url, headers, request.cache_width, request.cache_height)
            }
            ImageSource::SvgText { .. } => None,
        };
        complete_image_load(key, image);
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_image_load(key: String, request: ImageRequest) {
    match request.source {
        ImageSource::Memory { bytes, .. } => {
            let image = decode_image_from_bytes(&bytes, request.cache_width, request.cache_height);
            complete_image_load(key, image);
        }
        ImageSource::Asset { path } => {
            wasm_bindgen_futures::spawn_local(async move {
                let image = fetch_wasm_image_bytes(&path, Vec::new())
                    .await
                    .and_then(|bytes| {
                        decode_image_from_bytes(&bytes, request.cache_width, request.cache_height)
                    });
                complete_image_load(key, image);
            });
        }
        ImageSource::Network { url, headers, .. } => {
            wasm_bindgen_futures::spawn_local(async move {
                let image = fetch_wasm_image_bytes(&url, headers)
                    .await
                    .and_then(|bytes| {
                        decode_image_from_bytes(&bytes, request.cache_width, request.cache_height)
                    });
                complete_image_load(key, image);
            });
        }
        ImageSource::File { .. } | ImageSource::SvgText { .. } => {
            complete_image_load(key, None);
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_wasm_image_bytes(url: &str, headers: Vec<HttpHeader>) -> Option<Vec<u8>> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window()?;
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::Cors);
    let request = web_sys::Request::new_with_str_and_init(url, &init).ok()?;
    for header in headers {
        request.headers().set(&header.name, &header.value).ok()?;
    }
    let response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .ok()?;
    let response = response.dyn_into::<web_sys::Response>().ok()?;
    if !response.ok() {
        return None;
    }
    let buffer = wasm_bindgen_futures::JsFuture::from(response.array_buffer().ok()?)
        .await
        .ok()?;
    let bytes = js_sys::Uint8Array::new(&buffer);
    let mut out = vec![0; bytes.length() as usize];
    bytes.copy_to(&mut out);
    Some(out)
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_network_image(
    url: &str,
    headers: Vec<HttpHeader>,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    let mut request = ureq::get(url).set("User-Agent", "FissionImageLoader/0.2");
    for header in headers {
        request = request.set(&header.name, &header.value);
    }
    let response = request.call().ok()?;
    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes).ok()?;
    let image = image::load_from_memory(&bytes).ok()?;
    decode_dynamic_image(image, cache_width, cache_height)
}

#[cfg(test)]
mod image_tests {
    use super::*;
    use std::io::Cursor;
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    fn tiny_png() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageOutputFormat::Png)
            .expect("encode png");
        bytes.into_inner()
    }

    fn serve_once(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test image server");
        let url = format!("http://{}", listener.local_addr().expect("local addr"));
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut request = [0_u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
            let _ = std::io::Write::write_all(&mut stream, &body);
            let _ = std::io::Write::flush(&mut stream);
        });
        url
    }

    #[test]
    fn memory_image_load_populates_cache_off_thread() {
        let request = ImageRequest {
            source: ImageSource::Memory {
                bytes: tiny_png(),
                mime_type: Some("image/png".into()),
            },
            cache_width: Some(1),
            cache_height: Some(1),
            ..Default::default()
        };
        let key = request.stable_cache_key();
        IMAGE_CACHE.invalidate(&key);
        IMAGE_CACHE.run_pending_tasks();
        let before = image_cache_generation();

        spawn_image_load(key.clone(), request);

        let deadline = Instant::now() + Duration::from_secs(2);
        while image_cache_generation() == before && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        let Some(ImageCacheEntry::Ready(image)) = IMAGE_CACHE.get(&key) else {
            panic!("expected decoded image in cache");
        };
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
    }

    #[test]
    fn missing_cache_size_defaults_to_transformed_draw_rect() {
        let request = ImageRequest {
            source: ImageSource::Memory {
                bytes: tiny_png(),
                mime_type: Some("image/png".into()),
            },
            ..Default::default()
        };

        let resolved = image_request_with_default_cache_size(
            &request,
            Rect::new(0.0, 0.0, 80.2, 40.1),
            Affine::scale(2.0),
        );

        assert_eq!(resolved.cache_width, Some(161));
        assert_eq!(resolved.cache_height, Some(81));
    }

    #[test]
    fn explicit_cache_size_is_preserved() {
        let request = ImageRequest {
            source: ImageSource::Memory {
                bytes: tiny_png(),
                mime_type: Some("image/png".into()),
            },
            cache_width: Some(320),
            cache_height: Some(180),
            ..Default::default()
        };

        let resolved = image_request_with_default_cache_size(
            &request,
            Rect::new(0.0, 0.0, 80.0, 40.0),
            Affine::scale(2.0),
        );

        assert_eq!(resolved.cache_width, Some(320));
        assert_eq!(resolved.cache_height, Some(180));
    }

    #[test]
    fn network_image_fetch_decodes_png_response() {
        let url = serve_once(tiny_png());
        let image = fetch_network_image(&url, Vec::new(), Some(1), Some(1))
            .expect("fetch and decode test image");

        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
    }
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
    use super::{
        map_fill_to_brush, paragraph_alignment, paragraph_fade, paragraph_line_trim,
        paragraph_line_visual_bounds, paragraph_y_offset, parse_svg_entry,
        text_background_segments_for_cluster_ranges, Painter, ParagraphFade, SvgShape,
        TextBackgroundSegment, TextClip, VelloRenderer, VelloTextMeasurer,
    };
    use fission_ir::op::{
        FontStyle, MouseCursor, RichTextAnnotation, TextAlign, TextDirection, TextHeightBehavior,
        TextOverflow, TextParagraphStyle, TextWidthBasis,
    };
    use fission_ir::{semantics::ActionTrigger, ActionEntry};
    use fission_layout::TextMeasurer;
    use fission_render::{
        Color as RenderColor, DisplayList, DisplayOp, Fill as RenderFill, LayoutPoint, LayoutRect,
        RenderScene, Renderer, TextStyle as RenderTextStyle,
    };
    use parley::FontContext;
    use std::sync::{Arc, Mutex};
    use vello_cpu::filter_effects::Filter;
    use vello_cpu::kurbo::{Affine, BezPath, Point, Rect, Shape, Stroke};
    use vello_cpu::peniko::{BlendMode, Color, Compose, FontData, GradientKind};
    use vello_cpu::{Glyph, ImageSource, PaintType, Pixmap};

    /// A painter that records what the encoder asked for instead of drawing it.
    #[derive(Default)]
    struct RecordingPainter {
        glyphs: usize,
        paths: usize,
        rects: usize,
        clip_layers: usize,
        layers: Vec<(Option<Rect>, BlendMode, f32)>,
        blurred_rects: Vec<(Rect, bool)>,
        backdrop_filters: Vec<Rect>,
    }

    impl Painter for RecordingPainter {
        fn fill_path(&mut self, _: Affine, _: PaintType, _: &BezPath) {
            self.paths += 1;
        }
        fn fill_rect(&mut self, _: Affine, _: PaintType, _: &Rect) {
            self.rects += 1;
        }
        fn stroke_path(&mut self, _: Affine, _: &Stroke, _: PaintType, _: &BezPath) {
            self.paths += 1;
        }
        fn fill_blurred_rounded_rect(
            &mut self,
            _: Affine,
            _: Color,
            rect: &Rect,
            _: f32,
            _: f32,
            invert: bool,
        ) {
            self.blurred_rects.push((*rect, invert));
        }
        fn push_layer(
            &mut self,
            _: Affine,
            clip: Option<&BezPath>,
            blend: BlendMode,
            opacity: f32,
        ) {
            if clip.is_some() {
                self.clip_layers += 1;
            }
            self.layers
                .push((clip.map(|clip| clip.bounding_box()), blend, opacity));
        }
        fn pop_layer(&mut self) {}
        fn apply_backdrop_filter(&mut self, _: Affine, clip: &BezPath, _: Filter) {
            self.backdrop_filters.push(clip.bounding_box());
        }
        fn fill_glyphs(&mut self, _: Affine, _: PaintType, _: &FontData, _: f32, glyphs: &[Glyph]) {
            self.glyphs += glyphs.len();
        }
        fn image_source(&mut self, image: &Arc<Pixmap>) -> ImageSource {
            ImageSource::Pixmap(Arc::clone(image))
        }
    }

    #[test]
    fn normalized_gradient_geometry_maps_to_painted_bounds() {
        let brush = map_fill_to_brush(
            &RenderFill::LinearGradient {
                start: (0.0, 0.25),
                end: (1.0, 0.75),
                stops: vec![
                    (
                        0.0,
                        RenderColor {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        },
                    ),
                    (
                        1.0,
                        RenderColor {
                            r: 255,
                            g: 255,
                            b: 255,
                            a: 255,
                        },
                    ),
                ],
                extend: Default::default(),
            },
            Rect::new(20.0, 40.0, 220.0, 140.0),
        );

        let PaintType::Gradient(gradient) = brush else {
            panic!("expected gradient brush");
        };
        let GradientKind::Linear(position) = gradient.kind else {
            panic!("expected linear gradient");
        };
        assert_eq!(position.start, Point::new(20.0, 65.0));
        assert_eq!(position.end, Point::new(220.0, 115.0));
    }

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

    #[test]
    fn paragraph_fade_prefers_bottom_when_extra_lines_are_clipped() {
        assert_eq!(
            paragraph_fade(
                TextParagraphStyle {
                    text_align: TextAlign::Start,
                    max_lines: Some(1),
                    overflow: TextOverflow::Fade,
                    ..Default::default()
                },
                LayoutRect::new(0.0, 0.0, 120.0, 20.0),
                18.0,
                90.0,
                true,
                true,
                false,
            ),
            Some(ParagraphFade::Bottom {
                start: 9.0,
                end: 18.0,
            })
        );
    }

    fn test_renderer(painter: &mut RecordingPainter) -> VelloRenderer<'_> {
        let measurer = Arc::new(VelloTextMeasurer::new(Arc::new(Mutex::new(
            FontContext::new(),
        ))));
        VelloRenderer::new(painter, measurer, 1.0)
    }

    fn test_style() -> RenderTextStyle {
        RenderTextStyle {
            font_size: 16.0,
            color: RenderColor {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            underline: false,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_style: FontStyle::Normal,
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
            typography: Default::default(),
        }
    }

    #[test]
    fn text_background_segments_match_selected_clusters_only() {
        let clusters = vec![
            (0..1, 0.0, 10.0),
            (1..2, 10.0, 20.0),
            (2..3, 20.0, 30.0),
            (3..4, 30.0, 40.0),
        ];

        let segments = text_background_segments_for_cluster_ranges(clusters, &(1..2), None);

        assert_eq!(
            segments,
            vec![TextBackgroundSegment {
                left: 10.0,
                right: 20.0
            }]
        );
    }

    #[test]
    fn text_background_segments_clip_and_merge_adjacent_clusters() {
        let clusters = vec![
            (0..1, 0.0, 10.0),
            (1..2, 10.0, 20.0),
            (2..3, 20.0, 30.0),
            (3..4, 30.0, 40.0),
        ];

        let segments = text_background_segments_for_cluster_ranges(
            clusters,
            &(1..3),
            Some(TextClip {
                left: 12.0,
                right: 27.0,
                top: 0.0,
                bottom: 40.0,
            }),
        );

        assert_eq!(
            segments,
            vec![TextBackgroundSegment {
                left: 12.0,
                right: 27.0
            }]
        );
    }

    #[test]
    fn selected_character_background_does_not_span_full_line() {
        let mut painter = RecordingPainter::default();
        let renderer = test_renderer(&mut painter);
        let base_style = test_style();
        let mut selected_style = test_style();
        selected_style.background_color = Some(RenderColor {
            r: 0,
            g: 80,
            b: 255,
            a: 120,
        });
        let text = "abcdef";
        let styles = vec![(0..text.len(), base_style.clone()), (2..3, selected_style)];
        let layout = renderer.paragraph_layout(
            text,
            &base_style,
            false,
            LayoutRect::new(0.0, 0.0, 240.0, 40.0),
            TextParagraphStyle::default(),
            &[],
            &styles,
        );
        let line = layout.lines().next().expect("single test line");
        let line_bounds = paragraph_line_visual_bounds(&line).expect("line visual bounds");
        let mut segments = Vec::new();
        for run in line.runs() {
            segments.extend(text_background_segments_for_cluster_ranges(
                run.visual_clusters().filter_map(|cluster| {
                    let left = cluster.visual_offset()?;
                    Some((cluster.text_range(), left, left + cluster.advance()))
                }),
                &(2..3),
                None,
            ));
        }

        assert_eq!(segments.len(), 1);
        let selected = segments[0];
        assert!(selected.left > line_bounds.left + 1.0);
        assert!(selected.right < line_bounds.right - 1.0);
        assert!(
            selected.right - selected.left < (line_bounds.right - line_bounds.left) * 0.5,
            "single selected character should not highlight the full line"
        );
    }

    #[test]
    fn justify_alignment_stretches_non_terminal_lines() {
        let mut painter = RecordingPainter::default();
        let renderer = test_renderer(&mut painter);
        let style = test_style();
        let text = "one two three four five six seven eight";
        let bounds = LayoutRect::new(0.0, 0.0, 90.0, 200.0);
        let styles = vec![(0..text.len(), style.clone())];

        let start_layout = renderer.paragraph_layout(
            text,
            &style,
            true,
            bounds,
            TextParagraphStyle {
                text_align: TextAlign::Start,
                max_lines: None,
                overflow: TextOverflow::Visible,
                ..Default::default()
            },
            &[],
            &styles,
        );
        let justify_layout = renderer.paragraph_layout(
            text,
            &style,
            true,
            bounds,
            TextParagraphStyle {
                text_align: TextAlign::Justify,
                max_lines: None,
                overflow: TextOverflow::Visible,
                ..Default::default()
            },
            &[],
            &styles,
        );

        let start_lines: Vec<_> = start_layout.lines().collect();
        let justify_lines: Vec<_> = justify_layout.lines().collect();
        assert!(start_lines.len() > 1, "expected the sample text to wrap");
        assert_eq!(start_lines.len(), justify_lines.len());

        let start_first = paragraph_line_visual_bounds(&start_lines[0]).unwrap();
        let start_last = paragraph_line_visual_bounds(start_lines.last().unwrap()).unwrap();
        let justify_first = paragraph_line_visual_bounds(&justify_lines[0]).unwrap();
        let justify_last = paragraph_line_visual_bounds(justify_lines.last().unwrap()).unwrap();

        assert!(justify_first.right > start_first.right + 1.0);
        assert!(justify_first.right - justify_first.left > start_first.right - start_first.left);
        assert!(justify_last.right - justify_last.left <= start_last.right - start_last.left + 0.5);
        assert_eq!(
            paragraph_alignment(TextAlign::Justify),
            super::ParleyAlignment::Justify
        );
    }

    #[test]
    fn longest_line_width_basis_aligns_against_content_width() {
        let mut painter = RecordingPainter::default();
        let renderer = test_renderer(&mut painter);
        let style = test_style();
        let text = "paragraph width\nshort";
        let bounds = LayoutRect::new(0.0, 0.0, 220.0, 80.0);
        let styles = vec![(0..text.len(), style.clone())];

        let parent_layout = renderer.paragraph_layout(
            text,
            &style,
            false,
            bounds,
            TextParagraphStyle {
                text_align: TextAlign::Center,
                text_width_basis: TextWidthBasis::Parent,
                ..Default::default()
            },
            &[],
            &styles,
        );
        let longest_line_layout = renderer.paragraph_layout(
            text,
            &style,
            false,
            bounds,
            TextParagraphStyle {
                text_align: TextAlign::Center,
                text_width_basis: TextWidthBasis::LongestLine,
                ..Default::default()
            },
            &[],
            &styles,
        );

        let parent_lines: Vec<_> = parent_layout.lines().collect();
        let longest_line_lines: Vec<_> = longest_line_layout.lines().collect();
        let parent_first = paragraph_line_visual_bounds(&parent_lines[0]).unwrap();
        let parent_second = paragraph_line_visual_bounds(&parent_lines[1]).unwrap();
        let longest_first = paragraph_line_visual_bounds(&longest_line_lines[0]).unwrap();
        let longest_second = paragraph_line_visual_bounds(&longest_line_lines[1]).unwrap();

        assert!(parent_first.left > longest_first.left + 5.0);
        assert!(parent_second.left > longest_second.left + 5.0);
        assert!((longest_first.left - bounds.x()).abs() < 1.0);
    }

    #[test]
    fn fade_overflow_adds_renderer_side_clips() {
        let mut painter = RecordingPainter::default();
        let mut renderer = test_renderer(&mut painter);
        let style = test_style();
        let text = "this line should visibly fade instead of only clipping";

        renderer.render_paragraph_text(
            text,
            &style,
            false,
            LayoutPoint::new(0.0, 0.0),
            LayoutRect::new(0.0, 0.0, 80.0, 24.0),
            TextParagraphStyle {
                text_align: TextAlign::Start,
                max_lines: None,
                overflow: TextOverflow::Fade,
                ..Default::default()
            },
            &[],
            &[(0..text.len(), style.clone())],
            None,
            None,
            None,
            None,
            None,
            None,
        );
        drop(renderer);

        assert!(
            painter.clip_layers > 0,
            "fade overflow should add internal clip layers"
        );
    }

    #[test]
    fn simple_text_rendering_culls_glyphs_outside_bounds() {
        let mut painter = RecordingPainter::default();
        let mut renderer = test_renderer(&mut painter);
        let text = "M".repeat(20_000);

        renderer.render_text(
            &text,
            16.0,
            RenderColor {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            false,
            false,
            LayoutPoint::new(0.0, 0.0),
            LayoutRect::new(0.0, 0.0, 120.0, 32.0),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &[],
            &[],
        );
        drop(renderer);

        let glyphs = painter.glyphs;
        assert!(glyphs > 0, "visible glyphs should still be encoded");
        assert!(
            glyphs < 256,
            "renderer should not encode the full off-bounds text run; glyphs={glyphs}"
        );
    }

    #[test]
    fn rich_text_effects_are_encoded_through_the_normal_vello_path() {
        let text = "effects";
        let mut style = test_style();
        style.typography.shadows.push(fission_ir::op::TextShadow {
            color: fission_ir::op::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 160,
            },
            offset: (2.0, 3.0),
            blur_radius: 6.0,
        });
        style.typography.decoration.lines.underline = true;
        style.typography.decoration.style = fission_ir::op::TextDecorationStyle::Wavy;
        let styles = vec![(0..text.len(), style.clone())];
        let mut painter = RecordingPainter::default();
        let mut renderer = test_renderer(&mut painter);

        renderer.render_text(
            text,
            style.font_size,
            style.color,
            false,
            false,
            LayoutPoint::new(0.0, 0.0),
            LayoutRect::new(0.0, 0.0, 180.0, 48.0),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &[],
            &styles,
        );
        drop(renderer);

        assert!(
            painter.glyphs > text.chars().count(),
            "blurred shadow samples must encode additional glyph draws"
        );
        assert!(
            painter.paths > 0,
            "wavy decoration must contribute path geometry"
        );
    }

    #[test]
    fn rich_multiline_text_encodes_only_lines_inside_the_visible_bounds() {
        let text = "version = 4\n".repeat(10_000);
        let style = test_style();
        let styles = vec![(0..text.len(), style.clone())];
        let mut painter = RecordingPainter::default();
        let mut renderer = test_renderer(&mut painter);
        let viewport = Rect::new(0.0, 0.0, 400.0, 80.0);
        let transform = renderer.current_transform;
        renderer.painter.push_layer(
            transform,
            Some(&viewport.to_path(0.1)),
            BlendMode::default(),
            1.0,
        );
        renderer.push_clip_bounds(viewport);

        renderer.render_text(
            &text,
            style.font_size,
            style.color,
            false,
            false,
            LayoutPoint::new(0.0, 0.0),
            LayoutRect::new(0.0, 0.0, 400.0, 200_000.0),
            None,
            Some(0),
            Some(style.color),
            Some(2.0),
            None,
            None,
            None,
            &[],
            &styles,
        );
        renderer.painter.pop_layer();
        drop(renderer);

        let glyphs = painter.glyphs;
        assert!(
            glyphs < 256,
            "renderer should not encode offscreen rich-text lines; glyphs={glyphs}"
        );
    }

    #[test]
    fn explicit_text_direction_realigns_neutral_content() {
        let mut painter = RecordingPainter::default();
        let renderer = test_renderer(&mut painter);
        let style = test_style();
        let text = "12345";
        let bounds = LayoutRect::new(0.0, 0.0, 120.0, 40.0);
        let styles = vec![(0..text.len(), style.clone())];

        let ltr_layout = renderer.paragraph_layout(
            text,
            &style,
            false,
            bounds,
            TextParagraphStyle {
                text_align: TextAlign::Start,
                text_direction: TextDirection::Ltr,
                ..Default::default()
            },
            &[],
            &styles,
        );
        let rtl_layout = renderer.paragraph_layout(
            text,
            &style,
            false,
            bounds,
            TextParagraphStyle {
                text_align: TextAlign::Start,
                text_direction: TextDirection::Rtl,
                ..Default::default()
            },
            &[],
            &styles,
        );

        let ltr_bounds = paragraph_line_visual_bounds(&ltr_layout.lines().next().unwrap()).unwrap();
        let rtl_bounds = paragraph_line_visual_bounds(&rtl_layout.lines().next().unwrap()).unwrap();

        assert!(rtl_bounds.left > ltr_bounds.left + 5.0);
    }

    #[test]
    fn paragraph_strut_height_raises_line_metrics() {
        let mut painter = RecordingPainter::default();
        let renderer = test_renderer(&mut painter);
        let style = test_style();
        let text = "line";
        let styles = vec![(0..text.len(), style.clone())];

        let default_layout = renderer.paragraph_layout(
            text,
            &style,
            false,
            LayoutRect::new(0.0, 0.0, 80.0, 40.0),
            TextParagraphStyle::default(),
            &[],
            &styles,
        );
        let strut_layout = renderer.paragraph_layout(
            text,
            &style,
            false,
            LayoutRect::new(0.0, 0.0, 80.0, 40.0),
            TextParagraphStyle {
                strut_line_height: Some(28.0),
                ..Default::default()
            },
            &[],
            &styles,
        );

        let default_height = default_layout.lines().next().unwrap().metrics().line_height;
        let strut_height = strut_layout.lines().next().unwrap().metrics().line_height;

        assert!(strut_height > default_height + 5.0);
    }

    #[test]
    fn text_height_behavior_can_trim_first_line_leading() {
        let mut painter = RecordingPainter::default();
        let renderer = test_renderer(&mut painter);
        let mut style = test_style();
        style.line_height = Some(30.0);
        let text = "trimmed";
        let styles = vec![(0..text.len(), style.clone())];
        let behavior = TextHeightBehavior {
            apply_height_to_first_ascent: false,
            apply_height_to_last_descent: true,
        };
        let layout = renderer.paragraph_layout(
            text,
            &style,
            false,
            LayoutRect::new(0.0, 0.0, 120.0, 60.0),
            TextParagraphStyle {
                text_height_behavior: behavior,
                ..Default::default()
            },
            &[],
            &styles,
        );
        let lines: Vec<_> = layout.lines().collect();
        let (top_trim, bottom_trim) = paragraph_line_trim(&lines[0], behavior, true, true);

        assert!(top_trim > 0.0);
        assert_eq!(bottom_trim, 0.0);
        assert!(paragraph_y_offset(lines.first(), behavior, true) < 0.0);
    }

    #[test]
    fn rich_text_annotation_hit_testing_prefers_nested_span_metadata() {
        let mut painter = RecordingPainter::default();
        let renderer = test_renderer(&mut painter);
        let style = test_style();
        let text = "Read docs now";
        let styles = vec![(0..text.len(), style.clone())];
        let bounds = LayoutRect::new(0.0, 0.0, 160.0, 40.0);
        let annotations = vec![
            RichTextAnnotation {
                range: 0..13,
                semantics_label: None,
                semantics_identifier: None,
                spell_out: None,
                mouse_cursor: Some(MouseCursor::Pointer),
                actions: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: 1,
                    payload_data: Some(vec![1]),
                }],
            },
            RichTextAnnotation {
                range: 5..9,
                semantics_label: Some("documentation".into()),
                semantics_identifier: Some("docs-link".into()),
                spell_out: Some(true),
                mouse_cursor: None,
                actions: vec![ActionEntry {
                    trigger: ActionTrigger::HoverEnter,
                    action_id: 2,
                    payload_data: Some(vec![2]),
                }],
            },
        ];
        let layout = renderer.paragraph_layout(
            text,
            &style,
            false,
            bounds,
            TextParagraphStyle::default(),
            &[],
            &styles,
        );
        let line = layout.lines().next().unwrap();
        let x_start = renderer
            .measurer
            .get_caret_position(text, style.font_size, None, 5)
            .0;
        let x_end = renderer
            .measurer
            .get_caret_position(text, style.font_size, None, 9)
            .0;
        let y = line.metrics().baseline - (line.metrics().ascent * 0.5);

        let resolved = renderer
            .paragraph_annotation_at_point(
                text,
                &style,
                false,
                bounds,
                TextParagraphStyle::default(),
                &[],
                &styles,
                &annotations,
                (x_start + x_end) * 0.5,
                y,
            )
            .expect("nested annotation hit");

        assert_eq!(resolved.range, 5..9);
        assert_eq!(resolved.semantics_label.as_deref(), Some("documentation"));
        assert_eq!(resolved.semantics_identifier.as_deref(), Some("docs-link"));
        assert_eq!(resolved.mouse_cursor, Some(MouseCursor::Pointer));
        assert!(resolved
            .actions
            .iter()
            .any(|action| { action.trigger == ActionTrigger::Default && action.action_id == 1 }));
        assert!(resolved.actions.iter().any(|action| {
            action.trigger == ActionTrigger::HoverEnter && action.action_id == 2
        }));
    }

    fn black() -> RenderColor {
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    fn solid_stroke(width: f32) -> fission_render::Stroke {
        fission_render::Stroke {
            fill: RenderFill::Solid(black()),
            width,
            dash_array: None,
            line_cap: fission_render::LineCap::Butt,
            line_join: fission_render::LineJoin::Miter,
        }
    }

    fn render_ops(ops: Vec<DisplayOp>) -> RecordingPainter {
        let bounds = LayoutRect::new(0.0, 0.0, 200.0, 200.0);
        let mut list = DisplayList::new(bounds);
        for op in ops {
            list.push(op);
        }
        let mut painter = RecordingPainter::default();
        test_renderer(&mut painter)
            .render_scene(&RenderScene::from_display_list(list))
            .expect("render test scene");
        painter
    }

    fn draw_rect(
        stroke: Option<fission_render::Stroke>,
        border_sides: Option<fission_render::BorderSides>,
        shadow: Option<fission_render::BoxShadow>,
    ) -> DisplayOp {
        let rect = LayoutRect::new(10.0, 10.0, 100.0, 60.0);
        DisplayOp::DrawRect {
            rect,
            fill: Some(RenderFill::Solid(black())),
            stroke,
            corner_radius: 8.0,
            shadow,
            bounds: rect,
            node_id: None,
            corner_radii: None,
            border_sides,
        }
    }

    #[test]
    fn backdrop_filters_reach_the_painter_clipped_to_the_rounded_box() {
        let rect = LayoutRect::new(20.0, 30.0, 80.0, 40.0);
        let painter = render_ops(vec![DisplayOp::BackdropFilter {
            rect,
            filter: fission_ir::op::BackdropFilter::Chain(vec![
                fission_ir::op::BackdropFilter::Blur(12.0),
                fission_ir::op::BackdropFilter::Saturate(1.8),
            ]),
            corner_radius: 10.0,
            bounds: rect,
            node_id: None,
            corner_radii: None,
        }]);
        assert_eq!(painter.backdrop_filters.len(), 1);
        let clip = painter.backdrop_filters[0];
        assert!((clip.x0 - 20.0).abs() < 0.01 && (clip.x1 - 100.0).abs() < 0.01);
    }

    #[test]
    fn a_no_op_backdrop_filter_does_not_cost_a_layer() {
        let rect = LayoutRect::new(20.0, 30.0, 80.0, 40.0);
        let painter = render_ops(vec![DisplayOp::BackdropFilter {
            rect,
            filter: fission_ir::op::BackdropFilter::Blur(0.0),
            corner_radius: 0.0,
            bounds: rect,
            node_id: None,
            corner_radii: None,
        }]);
        assert!(painter.backdrop_filters.is_empty());
    }

    #[test]
    fn a_uniform_border_fills_a_single_ring_without_wedges() {
        let painter = render_ops(vec![draw_rect(Some(solid_stroke(2.0)), None, None)]);
        // Background plus one ring; no per-edge clip layers.
        assert_eq!(painter.paths, 2);
        assert_eq!(painter.clip_layers, 0);
    }

    #[test]
    fn differing_edges_are_painted_through_corner_wedges() {
        let sides = fission_render::BorderSides {
            top: Some(solid_stroke(1.0)),
            right: None,
            bottom: Some(solid_stroke(4.0)),
            left: None,
        };
        let painter = render_ops(vec![draw_rect(None, Some(sides), None)]);
        assert_eq!(
            painter.clip_layers, 2,
            "each stroked edge is clipped to its own wedge"
        );
        assert_eq!(painter.paths, 3, "background plus one ring fill per edge");
    }

    #[test]
    fn inset_shadows_paint_an_inverted_blur_inside_the_box() {
        let shadow = fission_render::BoxShadow {
            color: black(),
            blur_radius: 6.0,
            spread_radius: 2.0,
            offset: (0.0, 0.0),
            inset: true,
        };
        let painter = render_ops(vec![draw_rect(None, None, Some(shadow))]);
        assert_eq!(painter.blurred_rects.len(), 1);
        let (rect, invert) = painter.blurred_rects[0];
        assert!(
            invert,
            "an inset shadow is the inverse of a blurred rectangle"
        );
        assert!(rect.x0 > 10.0 && rect.x1 < 110.0, "shrunk by the spread");
        assert_eq!(painter.clip_layers, 1, "and confined to the box");
    }

    #[test]
    fn additive_blend_maps_to_a_real_compose_mode() {
        let blend = super::painter::blend_mode(fission_ir::BlendMode::Plus);
        assert_eq!(blend.compose, Compose::Plus);
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

/// Encodes a Fission render scene through a [`Painter`].
///
/// The same encoder drives the GPU and the CPU renderers, so both draw text, borders, images,
/// gradients and filters from one implementation.
pub struct VelloRenderer<'a> {
    painter: &'a mut dyn Painter,
    measurer: Arc<VelloTextMeasurer>,
    transform_stack: Vec<Affine>,
    current_transform: Affine,
    layer_count_stack: Vec<usize>,
    current_layer_count: usize,
    clip_stack: Vec<Rect>,
}

impl<'a> VelloRenderer<'a> {
    pub fn new(
        painter: &'a mut dyn Painter,
        measurer: Arc<VelloTextMeasurer>,
        scale_factor: f64,
    ) -> Self {
        Self {
            painter,
            measurer,
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

    fn transform_rect_bounds(transform: Affine, rect: Rect) -> Rect {
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

    fn get_image(&self, request: &ImageRequest) -> Option<Arc<Pixmap>> {
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

    fn affine_from_mat4(matrix: &[f32; 16]) -> Affine {
        let m00 = matrix[0] as f64;
        let m10 = matrix[1] as f64;
        let m01 = matrix[4] as f64;
        let m11 = matrix[5] as f64;
        let dx = matrix[12] as f64;
        let dy = matrix[13] as f64;
        Affine::new([m00, m10, m01, m11, dx, dy])
    }

    fn with_clip_rect<F>(&mut self, rect: Rect, draw: F)
    where
        F: FnOnce(&mut Self),
    {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }

        self.painter.push_layer(
            self.current_transform,
            Some(&rect.to_path(0.1)),
            BlendMode::default(),
            1.0,
        );
        draw(self);
        self.painter.pop_layer();
    }

    fn with_alpha_clip_rect<F>(&mut self, rect: Rect, alpha: f32, draw: F)
    where
        F: FnOnce(&mut Self),
    {
        if alpha <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }

        self.painter.push_layer(
            self.current_transform,
            Some(&rect.to_path(0.1)),
            BlendMode::default(),
            alpha,
        );
        draw(self);
        self.painter.pop_layer();
    }

    fn paragraph_base_style(
        base_size: f32,
        base_color: RenderColor,
        underline: bool,
    ) -> RenderTextStyle {
        RenderTextStyle {
            font_size: base_size,
            color: base_color,
            underline,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_style: fission_ir::op::FontStyle::Normal,
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
            typography: Default::default(),
        }
    }

    fn resolve_ellipsis_style(
        &self,
        line_range: std::ops::Range<usize>,
        base_style: &RenderTextStyle,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    ) -> RenderTextStyle {
        styles
            .iter()
            .rev()
            .find(|(range, _)| range.start < line_range.end && range.end > line_range.start)
            .map(|(_, style)| style.clone())
            .unwrap_or_else(|| base_style.clone())
    }

    fn ellipsis_metrics(&self, style: &RenderTextStyle) -> (f32, f32) {
        let ellipsis = "...";
        if text_style_requires_rich_layout(style) {
            let layout = self.measurer.layout_rich(
                ellipsis,
                style.font_size,
                style.color,
                &[(0..ellipsis.len(), style.clone())],
                &[],
                None,
            );
            let metrics = layout
                .lines()
                .next()
                .map(|line| (line.metrics().advance, line.metrics().baseline));
            if let Some(metrics) = metrics {
                return metrics;
            }
        } else {
            let layout = self.measurer.get_layout(ellipsis, style.font_size, None);
            let metrics = layout
                .lines()
                .next()
                .map(|line| (line.metrics().advance, line.metrics().baseline));
            if let Some(metrics) = metrics {
                return metrics;
            }
        }

        (style.font_size, style.font_size)
    }

    #[cfg(test)]
    fn paragraph_layout(
        &self,
        text: &str,
        base_style: &RenderTextStyle,
        wrap: bool,
        bounds: fission_render::LayoutRect,
        paragraph: TextParagraphStyle,
        inline_boxes: &[crate::text::RichInlineBox],
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    ) -> parley::layout::Layout<ParleyBrush> {
        let prepared =
            prepare_paragraph_layout(text, base_style, paragraph, inline_boxes, styles, None);
        self.paragraph_layout_from_prepared(&prepared, wrap, bounds, paragraph)
    }

    fn paragraph_layout_from_prepared(
        &self,
        prepared: &PreparedParagraphLayout,
        wrap: bool,
        bounds: fission_render::LayoutRect,
        paragraph: TextParagraphStyle,
    ) -> parley::layout::Layout<ParleyBrush> {
        let mut layout = (*self.measurer.layout_rich(
            &prepared.text,
            prepared.base_style.font_size,
            prepared.base_style.color,
            &prepared.styles,
            &prepared.inline_boxes,
            if wrap && bounds.width() > 0.0 {
                Some(bounds.width() as f32)
            } else {
                None
            },
        ))
        .clone();

        if let Some(alignment_width) = paragraph_alignment_width(&layout, bounds, paragraph) {
            // parley aligns lines against the width they were broken at. Break again at the
            // alignment width when no line is wider than it: the lines come out the same, and
            // alignment then happens against the parent or longest-line width the paragraph asked
            // for rather than whatever width layout happened to use. A small allowance keeps a
            // line exactly as wide as the longest line from wrapping on rounding.
            if layout.width() <= alignment_width + TEXT_ALIGNMENT_BREAK_EPSILON {
                layout.break_all_lines(Some(alignment_width + TEXT_ALIGNMENT_BREAK_EPSILON));
            }
            layout.align(
                paragraph_alignment(paragraph.text_align),
                paragraph_alignment_options(paragraph.text_align),
            );
        }

        layout
    }

    #[cfg(test)]
    fn paragraph_annotation_at_point(
        &self,
        text: &str,
        base_style: &RenderTextStyle,
        wrap: bool,
        bounds: fission_render::LayoutRect,
        paragraph: TextParagraphStyle,
        inline_boxes: &[crate::text::RichInlineBox],
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        annotations: &[fission_ir::op::RichTextAnnotation],
        x: f32,
        y: f32,
    ) -> Option<fission_ir::op::RichTextAnnotation> {
        if annotations.is_empty() {
            return None;
        }

        let prepared =
            prepare_paragraph_layout(text, base_style, paragraph, inline_boxes, styles, None);
        let layout = self.paragraph_layout_from_prepared(&prepared, wrap, bounds, paragraph);
        let total_lines = layout.lines().count();
        let visible_lines = paragraph
            .max_lines
            .map(|lines| lines.min(total_lines))
            .unwrap_or(total_lines);
        let local_y = y - paragraph_y_offset(
            layout.lines().next().as_ref(),
            paragraph.text_height_behavior,
            visible_lines == 1,
        );
        let idx = crate::text::VelloTextMeasurer::hit_test_layout_index_at_point(
            &prepared.text,
            &layout,
            x,
            local_y,
        )?;
        let raw_idx = idx
            .saturating_sub(prepared.text_byte_offset)
            .min(text.len());
        crate::text::resolve_rich_text_annotation_at_index(text, annotations, raw_idx)
    }

    fn draw_text_decoration(
        &mut self,
        style: fission_ir::op::TextDecorationStyle,
        x0: f64,
        x1: f64,
        y: f64,
        thickness: f64,
        color: Color,
    ) {
        for primitive in text_effects::decoration_primitives(style, x0, x1, y, thickness) {
            match primitive {
                text_effects::DecorationPrimitive::Rect { x0, y0, x1, y1 } => {
                    self.painter.fill_rect(
                        self.current_transform,
                        PaintType::from(color),
                        &Rect::new(x0, y0, x1, y1),
                    )
                }
                text_effects::DecorationPrimitive::Circle { x, y, radius } => {
                    self.painter.fill_path(
                        self.current_transform,
                        PaintType::from(color),
                        &Circle::new((x, y), radius).to_path(0.1),
                    )
                }
                text_effects::DecorationPrimitive::Wave { points, width } => {
                    let mut path = BezPath::new();
                    if let Some((first, rest)) = points.split_first() {
                        path.move_to(*first);
                        for point in rest {
                            path.line_to(*point);
                        }
                        self.painter.stroke_path(
                            self.current_transform,
                            &Stroke::new(width),
                            PaintType::from(color),
                            &path,
                        );
                    }
                }
            }
        }
    }

    fn draw_paragraph_line(
        &mut self,
        line: &parley::layout::Line<'_, ParleyBrush>,
        position: fission_render::LayoutPoint,
        top_y: f32,
        line_height: f32,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        clip: Option<TextClip>,
    ) {
        if let Some(clip) = clip {
            if !clip.intersects_y(top_y, top_y + line_height) {
                return;
            }
        }
        self.draw_paragraph_line_backgrounds(line, position, top_y, line_height, styles, clip);
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                let run_left = glyph_run.offset();
                let run_right = glyph_run.offset() + glyph_run.advance();
                if let Some(clip) = clip {
                    if !clip.intersects_x(run_left, run_right) {
                        continue;
                    }
                }
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
                let rich_style = styles.iter().find_map(|(range, style)| {
                    let run_range = run.text_range();
                    (range.start < run_range.end && range.end > run_range.start).then_some(style)
                });
                let paint_bounds = Rect::new(
                    position.x as f64 + run_left as f64,
                    position.y as f64 + top_y as f64,
                    position.x as f64 + run_right as f64,
                    position.y as f64 + (top_y + line_height) as f64,
                );
                let foreground = rich_style
                    .and_then(|style| style.typography.foreground.as_ref())
                    .map(|fill| map_text_fill_to_brush(fill, paint_bounds));

                let metrics = run.metrics();
                let baseline_shift = rich_style
                    .map(|style| {
                        text::typography_baseline_shift(
                            &style.typography,
                            metrics.ascent,
                            metrics.descent,
                            metrics.line_height,
                        )
                    })
                    .unwrap_or(0.0);

                let mut x = glyph_run.offset();
                let y = glyph_run.baseline() + baseline_shift;
                let glyphs = glyph_run
                    .glyphs()
                    .filter_map(|g| {
                        let gx = x + g.x;
                        let gy = y - g.y;
                        x += g.advance;
                        let glyph_right = gx + g.advance.max(1.0);
                        if clip
                            .map(|clip| clip.intersects_x(gx, glyph_right))
                            .unwrap_or(true)
                        {
                            Some(Glyph {
                                id: g.id as u32,
                                x: gx,
                                y: gy,
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if glyphs.is_empty() {
                    continue;
                }

                if let Some(rich_style) = rich_style {
                    for shadow in &rich_style.typography.shadows {
                        for (dx, dy, color) in text_effects::shadow_samples(*shadow) {
                            self.painter.fill_glyphs(
                                self.current_transform
                                    * Affine::translate((
                                        position.x as f64 + dx,
                                        position.y as f64 + dy,
                                    )),
                                PaintType::from(Color::from_rgba8(
                                    color.r, color.g, color.b, color.a,
                                )),
                                font,
                                font_size,
                                &glyphs,
                            );
                        }
                    }
                }

                let paint = foreground.unwrap_or_else(|| PaintType::from(color));
                self.painter.fill_glyphs(
                    self.current_transform
                        * Affine::translate((position.x as f64, position.y as f64)),
                    paint,
                    font,
                    font_size,
                    &glyphs,
                );

                if let Some(decoration) = &style.underline {
                    let offset = decoration.offset.unwrap_or(metrics.underline_offset);
                    let size = decoration.size.unwrap_or(metrics.underline_size).max(1.0);
                    let deco_brush = decoration.brush.clone();
                    let deco_color = Color::from_rgba8(
                        deco_brush.0[0],
                        deco_brush.0[1],
                        deco_brush.0[2],
                        deco_brush.0[3],
                    );

                    let x0 = clip.map(|clip| run_left.max(clip.left)).unwrap_or(run_left);
                    let x1 = clip
                        .map(|clip| run_right.min(clip.right))
                        .unwrap_or(run_right);
                    if x1 <= x0 {
                        continue;
                    }
                    self.draw_text_decoration(
                        rich_style
                            .map(|style| style.typography.decoration.style)
                            .unwrap_or_default(),
                        position.x as f64 + x0 as f64,
                        position.x as f64 + x1 as f64,
                        position.y as f64 + (glyph_run.baseline() + baseline_shift + offset) as f64,
                        size as f64,
                        deco_color,
                    );
                }
                if let Some(decoration) = &style.strikethrough {
                    let offset = decoration.offset.unwrap_or(metrics.strikethrough_offset);
                    let size = decoration
                        .size
                        .unwrap_or(metrics.strikethrough_size)
                        .max(1.0);
                    let deco_brush = decoration.brush.clone();
                    let x0 = clip.map(|clip| run_left.max(clip.left)).unwrap_or(run_left);
                    let x1 = clip
                        .map(|clip| run_right.min(clip.right))
                        .unwrap_or(run_right);
                    if x1 > x0 {
                        self.draw_text_decoration(
                            rich_style
                                .map(|style| style.typography.decoration.style)
                                .unwrap_or_default(),
                            position.x as f64 + x0 as f64,
                            position.x as f64 + x1 as f64,
                            position.y as f64
                                + (glyph_run.baseline() + baseline_shift + offset) as f64,
                            size as f64,
                            Color::from_rgba8(
                                deco_brush.0[0],
                                deco_brush.0[1],
                                deco_brush.0[2],
                                deco_brush.0[3],
                            ),
                        );
                    }
                }
                if let Some(rich_style) =
                    rich_style.filter(|style| style.typography.decoration.lines.overline)
                {
                    let x0 = clip.map(|clip| run_left.max(clip.left)).unwrap_or(run_left);
                    let x1 = clip
                        .map(|clip| run_right.min(clip.right))
                        .unwrap_or(run_right);
                    if x1 > x0 {
                        let thickness = rich_style
                            .typography
                            .decoration
                            .thickness
                            .unwrap_or(1.0)
                            .max(1.0);
                        let color = rich_style
                            .typography
                            .decoration
                            .color
                            .unwrap_or(fission_ir::op::Color::BLACK);
                        self.draw_text_decoration(
                            rich_style.typography.decoration.style,
                            position.x as f64 + x0 as f64,
                            position.x as f64 + x1 as f64,
                            position.y as f64 + top_y as f64 + baseline_shift as f64,
                            thickness as f64,
                            Color::from_rgba8(color.r, color.g, color.b, color.a),
                        );
                    }
                }
            }
        }
    }

    fn draw_paragraph_line_backgrounds(
        &mut self,
        line: &parley::layout::Line<'_, ParleyBrush>,
        position: fission_render::LayoutPoint,
        top_y: f32,
        line_height: f32,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        clip: Option<TextClip>,
    ) {
        if !styles.iter().any(|(_, style)| {
            style.background_color.is_some() || style.typography.background.is_some()
        }) {
            return;
        }

        for run in line.runs() {
            let run_text_range = run.text_range();
            for (range, style) in styles.iter() {
                if style.background_color.is_none() && style.typography.background.is_none() {
                    continue;
                }
                let overlap_start = range.start.max(run_text_range.start);
                let overlap_end = range.end.min(run_text_range.end);
                if overlap_start >= overlap_end {
                    continue;
                }

                let segments = text_background_segments_for_cluster_ranges(
                    run.visual_clusters().filter_map(|cluster| {
                        let left = cluster.visual_offset()?;
                        Some((cluster.text_range(), left, left + cluster.advance()))
                    }),
                    range,
                    clip,
                );
                if segments.is_empty() {
                    continue;
                }

                let y0 = position.y as f64 + top_y as f64;
                for segment in segments {
                    let x0 = position.x as f64 + segment.left as f64;
                    let x1 = position.x as f64 + segment.right as f64;
                    let bg_rect = Rect::new(x0, y0, x1, y0 + line_height as f64);
                    let brush = style
                        .typography
                        .background
                        .as_ref()
                        .map(|fill| map_text_fill_to_brush(fill, bg_rect))
                        .unwrap_or_else(|| {
                            let bg = style.background_color.expect("background checked above");
                            PaintType::from(Color::from_rgba8(bg.r, bg.g, bg.b, bg.a))
                        });
                    self.painter
                        .fill_rect(self.current_transform, brush, &bg_rect);
                }
            }
        }
    }

    fn draw_paragraph_line_with_fade(
        &mut self,
        line: &parley::layout::Line<'_, ParleyBrush>,
        position: fission_render::LayoutPoint,
        bounds: fission_render::LayoutRect,
        top_y: f32,
        line_height: f32,
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        clip: Option<TextClip>,
        fade: ParagraphFade,
    ) {
        let line_top = position.y + top_y;
        let line_bottom = line_top + line_height;

        match fade {
            ParagraphFade::Right { start, end } => {
                let body_end = start.max(0.0);
                if body_end > 0.0 {
                    let clip_rect = Rect::new(
                        bounds.x() as f64,
                        line_top as f64,
                        (position.x + body_end).min(bounds.right()) as f64,
                        line_bottom as f64,
                    );
                    self.with_clip_rect(clip_rect, |this| {
                        this.draw_paragraph_line(line, position, top_y, line_height, styles, clip);
                    });
                }

                let fade_width = end - start;
                for slice in 0..PARAGRAPH_FADE_SLICE_COUNT {
                    let slice_start =
                        start + fade_width * slice as f32 / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let slice_end =
                        start + fade_width * (slice + 1) as f32 / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let alpha = 1.0 - (slice as f32 + 0.5) / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let clip_rect = Rect::new(
                        (position.x + slice_start).max(bounds.x()) as f64,
                        line_top as f64,
                        (position.x + slice_end).min(bounds.right()) as f64,
                        line_bottom as f64,
                    );
                    self.with_alpha_clip_rect(clip_rect, alpha, |this| {
                        this.draw_paragraph_line(line, position, top_y, line_height, styles, clip);
                    });
                }
            }
            ParagraphFade::Bottom { start, end } => {
                if start > 0.0 {
                    let clip_rect = Rect::new(
                        bounds.x() as f64,
                        line_top as f64,
                        bounds.right() as f64,
                        (line_top + start).min(bounds.bottom()) as f64,
                    );
                    self.with_clip_rect(clip_rect, |this| {
                        this.draw_paragraph_line(line, position, top_y, line_height, styles, clip);
                    });
                }

                let fade_height = end - start;
                for slice in 0..PARAGRAPH_FADE_SLICE_COUNT {
                    let slice_start =
                        start + fade_height * slice as f32 / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let slice_end = start
                        + fade_height * (slice + 1) as f32 / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let alpha = 1.0 - (slice as f32 + 0.5) / PARAGRAPH_FADE_SLICE_COUNT as f32;
                    let clip_rect = Rect::new(
                        bounds.x() as f64,
                        (line_top + slice_start).max(bounds.y()) as f64,
                        bounds.right() as f64,
                        (line_top + slice_end).min(bounds.bottom()) as f64,
                    );
                    self.with_alpha_clip_rect(clip_rect, alpha, |this| {
                        this.draw_paragraph_line(line, position, top_y, line_height, styles, clip);
                    });
                }
            }
        }
    }

    fn render_paragraph_text(
        &mut self,
        text: &str,
        base_style: &RenderTextStyle,
        wrap: bool,
        position: fission_render::LayoutPoint,
        bounds: fission_render::LayoutRect,
        paragraph: TextParagraphStyle,
        inline_boxes: &[crate::text::RichInlineBox],
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
        caret_index: Option<usize>,
        caret_color: Option<RenderColor>,
        caret_width: Option<f32>,
        caret_height: Option<f32>,
        caret_radius: Option<f32>,
        resolved_layout: Option<&fission_render::ResolvedParagraphLayout>,
    ) {
        let prepared = prepare_paragraph_layout(
            text,
            base_style,
            paragraph,
            inline_boxes,
            styles,
            caret_index,
        );
        let layout = self.paragraph_layout_from_prepared(&prepared, wrap, bounds, paragraph);
        let lines: Vec<_> = layout.lines().collect();
        let total_lines = lines.len();
        let visible_lines = paragraph
            .max_lines
            .map(|lines| lines.min(total_lines))
            .unwrap_or(total_lines);
        let draw_position = fission_render::LayoutPoint::new(
            position.x,
            position.y
                + paragraph_y_offset(
                    lines.first(),
                    paragraph.text_height_behavior,
                    visible_lines == 1,
                ),
        );
        let text_clip = self.text_clip(draw_position, bounds);

        for (line_idx, line) in lines.iter().take(visible_lines).enumerate() {
            let metrics = *line.metrics();
            let line_height = metrics
                .line_height
                .max(metrics.ascent + metrics.descent)
                .max(1.0);
            let top_y = metrics.baseline - metrics.ascent;
            let is_last_visible_line = line_idx + 1 == visible_lines;
            let (top_trim, bottom_trim) = paragraph_line_trim(
                line,
                paragraph.text_height_behavior,
                line_idx == 0,
                is_last_visible_line,
            );
            let visual_line_height = (line_height - top_trim - bottom_trim).max(1.0);
            let visual_bounds =
                paragraph_line_visual_bounds(line).unwrap_or(ParagraphLineVisualBounds {
                    left: metrics.offset,
                    right: metrics.offset + metrics.advance,
                });
            let line_width = (visual_bounds.right - visual_bounds.left).max(0.0);
            let has_more_lines = line_idx + 1 < total_lines;
            let overflows_horizontally = bounds.width() > 0.0 && line_width > bounds.width();
            let show_ellipsis = matches!(paragraph.overflow, TextOverflow::Ellipsis)
                && is_last_visible_line
                && (has_more_lines || overflows_horizontally);
            let fade = paragraph_fade(
                paragraph,
                bounds,
                visual_line_height,
                line_width,
                is_last_visible_line,
                has_more_lines,
                overflows_horizontally,
            );

            let ellipsis = show_ellipsis.then(|| {
                let style = self.resolve_ellipsis_style(
                    line.text_range(),
                    &prepared.base_style,
                    &prepared.styles,
                );
                let (width, baseline) = self.ellipsis_metrics(&style);
                let line_end = if bounds.width() > 0.0 {
                    visual_bounds.right.min(bounds.width()).max(0.0)
                } else {
                    visual_bounds.right.max(0.0)
                };
                let left = (line_end - width).max(0.0);
                (style, width, baseline, left)
            });

            if let Some((_, _, _, ellipsis_left)) = ellipsis.as_ref() {
                let clip_rect = Rect::new(
                    bounds.x() as f64,
                    draw_position.y as f64 + top_y as f64,
                    (draw_position.x + *ellipsis_left).max(bounds.x()) as f64,
                    draw_position.y as f64 + top_y as f64 + visual_line_height as f64,
                );
                self.with_clip_rect(clip_rect, |this| {
                    this.draw_paragraph_line(
                        line,
                        draw_position,
                        top_y,
                        visual_line_height,
                        &prepared.styles,
                        text_clip,
                    );
                });
            } else if let Some(fade) = fade {
                self.draw_paragraph_line_with_fade(
                    line,
                    draw_position,
                    bounds,
                    top_y,
                    visual_line_height,
                    &prepared.styles,
                    text_clip,
                    fade,
                );
            } else {
                self.draw_paragraph_line(
                    line,
                    draw_position,
                    top_y,
                    visual_line_height,
                    &prepared.styles,
                    text_clip,
                );
            }

            if let Some((style, width, baseline, ellipsis_left)) = ellipsis {
                let ellipsis_position = fission_render::LayoutPoint::new(
                    draw_position.x + ellipsis_left,
                    draw_position.y + metrics.baseline - baseline,
                );
                let ellipsis_bounds = fission_render::LayoutRect::new(
                    ellipsis_position.x,
                    ellipsis_position.y,
                    width,
                    visual_line_height,
                );
                if text_style_requires_rich_layout(&style) {
                    let ellipsis_styles = vec![(0..3, style.clone())];
                    self.render_text(
                        "...",
                        style.font_size,
                        style.color,
                        style.underline,
                        false,
                        ellipsis_position,
                        ellipsis_bounds,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        &[],
                        &ellipsis_styles,
                    );
                } else {
                    self.render_text(
                        "...",
                        style.font_size,
                        style.color,
                        style.underline,
                        false,
                        ellipsis_position,
                        ellipsis_bounds,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        &[],
                        &[],
                    );
                }
            }
        }

        if let Some(idx) = prepared.caret_index {
            if self.draw_resolved_caret(
                resolved_layout,
                idx,
                position,
                caret_color.unwrap_or(prepared.base_style.color),
                caret_width.unwrap_or(2.0),
                caret_height,
                caret_radius,
            ) {
                return;
            }
            self.draw_caret(
                &layout,
                idx,
                position,
                &prepared.text,
                prepared.base_style.font_size,
                caret_color.unwrap_or(prepared.base_style.color),
                caret_width.unwrap_or(2.0),
                caret_height,
                caret_radius,
                paragraph,
            );
        }
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
        resolved_layout: Option<&fission_render::ResolvedParagraphLayout>,
        caret_index: Option<usize>,
        caret_color: Option<RenderColor>,
        caret_width: Option<f32>,
        caret_height: Option<f32>,
        caret_radius: Option<f32>,
        paragraph_style: Option<TextParagraphStyle>,
        inline_boxes: &[crate::text::RichInlineBox],
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    ) {
        let mut layout_bounds = bounds;
        if wrap {
            if let Some(width) = resolved_layout.and_then(|layout| layout.constraint_width) {
                layout_bounds.size.width = width.max(0.0);
            }
        }
        let paragraph = paragraph_style
            .or_else(|| {
                if caret_index.is_none() {
                    decode_text_paragraph_style(caret_width)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if paragraph != TextParagraphStyle::default() {
            let base_style = Self::paragraph_base_style(base_size, base_color, underline);
            let owned_styles;
            let paragraph_styles = if styles.is_empty() && !text.is_empty() {
                owned_styles = vec![(0..text.len(), base_style.clone())];
                owned_styles.as_slice()
            } else {
                styles
            };
            self.render_paragraph_text(
                text,
                &base_style,
                wrap,
                position,
                layout_bounds,
                paragraph,
                inline_boxes,
                paragraph_styles,
                caret_index,
                caret_color,
                caret_width,
                caret_height,
                caret_radius,
                resolved_layout,
            );
            return;
        }

        let text_clip = self.text_clip(position, bounds);

        // Fast path for simple text using cache
        if styles.is_empty() && inline_boxes.is_empty() {
            let layout = self.measurer.get_layout(
                text,
                base_size,
                if wrap && layout_bounds.width() > 0.0 {
                    Some(layout_bounds.width() as f32)
                } else {
                    None
                },
            );

            // Draw Glyphs (Reused layout logic)
            for line in layout.lines() {
                let metrics = *line.metrics();
                let line_height = metrics
                    .line_height
                    .max(metrics.ascent + metrics.descent)
                    .max(1.0);
                let line_top = metrics.baseline - metrics.ascent;
                if let Some(clip) = text_clip {
                    if !clip.intersects_y(line_top, line_top + line_height) {
                        continue;
                    }
                }
                for item in line.items() {
                    if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                        let run_left = glyph_run.offset();
                        let run_right = glyph_run.offset() + glyph_run.advance();
                        if let Some(clip) = text_clip {
                            if !clip.intersects_x(run_left, run_right) {
                                continue;
                            }
                        }
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

                        let glyphs = glyph_run
                            .glyphs()
                            .filter_map(|g| {
                                let gx = x + g.x;
                                let gy = y - g.y;
                                x += g.advance;
                                let glyph_right = gx + g.advance.max(1.0);
                                if text_clip
                                    .map(|clip| clip.intersects_x(gx, glyph_right))
                                    .unwrap_or(true)
                                {
                                    Some(Glyph {
                                        id: g.id as u32,
                                        x: gx,
                                        y: gy,
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        if glyphs.is_empty() {
                            continue;
                        }

                        self.painter.fill_glyphs(
                            self.current_transform
                                * Affine::translate((position.x as f64, position.y as f64)),
                            PaintType::from(color),
                            font,
                            font_size,
                            &glyphs,
                        );

                        if underline {
                            let metrics = run.metrics();
                            let offset = metrics.underline_offset;
                            let size = metrics.underline_size.max(1.0);
                            let x0 = text_clip
                                .map(|clip| run_left.max(clip.left))
                                .unwrap_or(run_left);
                            let x1 = text_clip
                                .map(|clip| run_right.min(clip.right))
                                .unwrap_or(run_right);
                            if x1 <= x0 {
                                continue;
                            }
                            let x0 = position.x as f64 + x0 as f64;
                            let x1 = position.x as f64 + x1 as f64;
                            let y0 = position.y as f64 + (glyph_run.baseline() + offset) as f64;
                            let rect = Rect::new(x0, y0, x1, y0 + size as f64);
                            self.painter.fill_rect(
                                self.current_transform,
                                PaintType::from(color),
                                &rect,
                            );
                        }
                    }
                }
            }
            if let Some(idx) = caret_index {
                if self.draw_resolved_caret(
                    resolved_layout,
                    idx,
                    position,
                    caret_color.unwrap_or(base_color),
                    caret_width.unwrap_or(2.0),
                    caret_height,
                    caret_radius,
                ) {
                    return;
                }
                self.draw_caret(
                    &layout,
                    idx,
                    position,
                    text,
                    base_size,
                    caret_color.unwrap_or(base_color),
                    caret_width.unwrap_or(2.0),
                    caret_height,
                    caret_radius,
                    paragraph,
                );
            }
            return;
        }

        // Slow path for rich text
        let layout = self.measurer.layout_rich(
            text,
            base_size,
            base_color,
            styles,
            inline_boxes,
            if wrap && layout_bounds.width() > 0.0 {
                Some(layout_bounds.width() as f32)
            } else {
                None
            },
        );

        // Draw Glyphs for rich text (uses brushes from layout)
        for line in layout.lines() {
            let metrics = *line.metrics();
            let line_height = metrics
                .line_height
                .max(metrics.ascent + metrics.descent)
                .max(1.0);
            let line_top = metrics.baseline - metrics.ascent;
            if let Some(clip) = text_clip {
                if !clip.intersects_y(line_top, line_top + line_height) {
                    continue;
                }
            }
            self.draw_paragraph_line(&line, position, line_top, line_height, styles, text_clip);
        }

        if let Some(idx) = caret_index {
            if self.draw_resolved_caret(
                resolved_layout,
                idx,
                position,
                caret_color.unwrap_or(base_color),
                caret_width.unwrap_or(2.0),
                caret_height,
                caret_radius,
            ) {
                return;
            }
            self.draw_caret(
                &layout,
                idx,
                position,
                text,
                base_size,
                caret_color.unwrap_or(base_color),
                caret_width.unwrap_or(2.0),
                caret_height,
                caret_radius,
                paragraph,
            );
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

    fn draw_resolved_caret(
        &mut self,
        resolved: Option<&fission_render::ResolvedParagraphLayout>,
        index: usize,
        position: fission_render::LayoutPoint,
        color: RenderColor,
        width: f32,
        requested_height: Option<f32>,
        radius: Option<f32>,
    ) -> bool {
        let Some(stop) = resolved.and_then(|layout| layout.caret(index, false)) else {
            return false;
        };
        let height = requested_height
            .unwrap_or(stop.height)
            .clamp(1.0, stop.height.max(1.0));
        let top = position.y + stop.position.y + (stop.height - height) * 0.5;
        let left = position.x + stop.position.x;
        let shape = RoundedRect::from_rect(
            Rect::new(
                left as f64,
                top as f64,
                (left + width.max(1.0)) as f64,
                (top + height) as f64,
            ),
            radius.unwrap_or(0.0).max(0.0) as f64,
        );
        self.painter.fill_path(
            self.current_transform,
            PaintType::from(Color::from_rgba8(color.r, color.g, color.b, color.a)),
            &shape.to_path(0.1),
        );
        true
    }

    fn draw_caret(
        &mut self,
        layout: &parley::layout::Layout<ParleyBrush>,
        idx: usize,
        position: fission_render::LayoutPoint,
        text: &str,
        base_size: f32,
        caret_color: RenderColor,
        caret_width: f32,
        caret_height: Option<f32>,
        caret_radius: Option<f32>,
        paragraph: TextParagraphStyle,
    ) {
        let mut caret_drawn = false;
        let lines_count = layout.lines().count();
        let paragraph_y_offset = paragraph_y_offset(
            layout.lines().next().as_ref(),
            paragraph.text_height_behavior,
            lines_count == 1,
        );

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
                let (top_trim, bottom_trim) = paragraph_line_trim(
                    &line,
                    paragraph.text_height_behavior,
                    i == 0,
                    is_last_line,
                );
                let visual_line_height = (line_height - top_trim - bottom_trim).max(1.0);
                let baseline_y = metrics.baseline;
                let visual_bounds =
                    paragraph_line_visual_bounds(&line).unwrap_or(ParagraphLineVisualBounds {
                        left: metrics.offset,
                        right: metrics.offset + metrics.advance,
                    });
                x_pos += visual_bounds.left - metrics.offset;

                let top_y = baseline_y - metrics.ascent;
                let caret_draw_height = caret_height
                    .unwrap_or(visual_line_height)
                    .clamp(1.0, visual_line_height.max(1.0));
                let caret_top = top_y - top_trim + ((visual_line_height - caret_draw_height) * 0.5);

                let caret_shape = RoundedRect::from_rect(
                    Rect::new(
                        position.x as f64 + x_pos as f64,
                        position.y as f64 + paragraph_y_offset as f64 + caret_top as f64,
                        position.x as f64 + x_pos as f64 + caret_width as f64,
                        position.y as f64
                            + paragraph_y_offset as f64
                            + caret_top as f64
                            + caret_draw_height as f64,
                    ),
                    caret_radius.unwrap_or(0.0).max(0.0) as f64,
                );

                self.painter.fill_path(
                    self.current_transform,
                    PaintType::from(Color::from_rgba8(
                        caret_color.r,
                        caret_color.g,
                        caret_color.b,
                        caret_color.a,
                    )),
                    &caret_shape.to_path(0.1),
                );
                caret_drawn = true;
                break;
            }
        }
        if !caret_drawn && idx == 0 && text.is_empty() {
            let mut top_y = position.y as f64;
            let mut height = paragraph
                .strut_line_height
                .unwrap_or(base_size * 1.2)
                .max(1.0) as f64;
            if let Some(line) = layout.lines().next() {
                let metrics = line.metrics();
                top_y = position.y as f64
                    + paragraph_y_offset as f64
                    + (metrics.baseline - metrics.ascent) as f64;
                height = metrics
                    .line_height
                    .max(metrics.ascent + metrics.descent)
                    .max(1.0) as f64;
            }
            let draw_height = caret_height
                .unwrap_or(height as f32)
                .clamp(1.0, height as f32) as f64;
            let caret_top = top_y + ((height - draw_height) * 0.5);
            let caret_shape = RoundedRect::from_rect(
                Rect::new(
                    position.x as f64,
                    caret_top,
                    position.x as f64 + caret_width as f64,
                    caret_top + draw_height,
                ),
                caret_radius.unwrap_or(0.0).max(0.0) as f64,
            );
            self.painter.fill_path(
                self.current_transform,
                PaintType::from(Color::from_rgba8(
                    caret_color.r,
                    caret_color.g,
                    caret_color.b,
                    caret_color.a,
                )),
                &caret_shape.to_path(0.1),
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
                        self.painter.pop_layer();
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
                    let affine = Self::affine_from_mat4(matrix);
                    self.current_transform = self.current_transform * affine;
                }
                // Classic Vello could append a previously encoded scene. Sparse strips bake the
                // transform into strips as they are recorded, so a cached list is drawn like any
                // other; the compositor's layer textures are what avoid redrawing unchanged content.
                DisplayOp::CachedScene { list, .. } => self.render_paint_list(list)?,
                DisplayOp::ClipRect(rect) => {
                    let r = Self::layout_rect_to_rect(*rect);
                    self.push_layer_for(r.to_path(0.1), r, BlendMode::default(), 1.0);
                }
                DisplayOp::ClipRoundedRect { rect, radius } => {
                    let r = Self::layout_rect_to_rect(*rect);
                    let shape = RoundedRect::from_rect(r, *radius as f64);
                    self.push_layer_for(shape.to_path(0.1), r, BlendMode::default(), 1.0);
                }
                DisplayOp::OpacityLayer { alpha, bounds } => {
                    let r = Self::layout_rect_to_rect(*bounds);
                    self.push_layer_for(r.to_path(0.1), r, BlendMode::default(), *alpha);
                }
                DisplayOp::BackdropFilter {
                    rect,
                    filter,
                    corner_radius,
                    corner_radii,
                    ..
                } => {
                    if let Some(filter) = painter::backdrop_filter(filter) {
                        let r = Self::layout_rect_to_rect(*rect);
                        let radii = corner_radii.map(kurbo_radii).unwrap_or_else(|| {
                            RoundedRectRadii::from_single_radius(*corner_radius as f64)
                        });
                        let clip = RoundedRect::from_rect(r, radii).to_path(0.1);
                        self.painter
                            .apply_backdrop_filter(self.current_transform, &clip, filter);
                    }
                }
                DisplayOp::DrawRect {
                    rect,
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                    corner_radii,
                    border_sides,
                    ..
                } => {
                    self.draw_rect(
                        Self::layout_rect_to_rect(*rect),
                        fill.as_ref(),
                        stroke.as_ref(),
                        *corner_radius,
                        shadow.as_ref(),
                        *corner_radii,
                        border_sides.as_ref(),
                    );
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
                    resolved_layout,
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
                        resolved_layout.as_ref(),
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
                    resolved_layout,
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
                                resolved_layout.as_ref(),
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
                        resolved_layout.as_ref(),
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
                    let Some(image) = self.get_image(&request) else {
                        continue;
                    };
                    let rect_w = rect.size.width as f64;
                    let rect_h = rect.size.height as f64;
                    let img_w = f64::from(image.width());
                    let img_h = f64::from(image.height());
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
                            let (offset_x, offset_y) = aligned_offset(
                                rect_w - img_w * scale,
                                rect_h - img_h * scale,
                                *alignment,
                            );
                            (
                                scale,
                                scale,
                                rect.origin.x as f64 + offset_x,
                                rect.origin.y as f64 + offset_y,
                            )
                        }
                        fission_render::ImageFit::Cover => {
                            let scale = (rect_w / img_w).max(rect_h / img_h);
                            let (offset_x, offset_y) = aligned_offset(
                                rect_w - img_w * scale,
                                rect_h - img_h * scale,
                                *alignment,
                            );
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
                    let source = self.painter.image_source(&image);
                    let paint = PaintType::from(Image {
                        image: source,
                        sampler: ImageSampler::default(),
                    });
                    let clip_rect = Self::layout_rect_to_rect(*rect);
                    self.with_clip_rect(clip_rect, |this| {
                        this.painter.fill_rect(
                            transform,
                            paint,
                            &Rect::new(0.0, 0.0, img_w, img_h),
                        );
                    });
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
                        let paint_bounds = Rect::new(
                            0.0,
                            0.0,
                            bounds.size.width as f64,
                            bounds.size.height as f64,
                        );
                        if let Some(f) = fill {
                            self.painter.fill_path(
                                transform,
                                map_fill_to_brush(f, paint_bounds),
                                &bez_path,
                            );
                        }
                        if let Some(s) = stroke {
                            let (stroke_style, paint) = map_stroke(s, paint_bounds);
                            self.painter
                                .stroke_path(transform, &stroke_style, paint, &bez_path);
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
                            (
                                scale,
                                bounds.origin.x as f64 + (rect_w - vb_w * scale) / 2.0
                                    - vb_x * scale,
                                bounds.origin.y as f64 + (rect_h - vb_h * scale) / 2.0
                                    - vb_y * scale,
                            )
                        } else {
                            (1.0, bounds.origin.x as f64, bounds.origin.y as f64)
                        };
                    let svg_transform =
                        self.current_transform * Affine::translate((dx, dy)) * Affine::scale(scale);
                    let paint_bounds = Rect::new(vb_x, vb_y, vb_x + vb_w, vb_y + vb_h);

                    for shape in &entry.shapes {
                        let path = match shape {
                            SvgShape::Path(path) => path.clone(),
                            SvgShape::Rect(rect) => rect.to_path(0.1),
                        };
                        if let Some(f) = fill {
                            self.painter.fill_path(
                                svg_transform,
                                map_fill_to_brush(f, paint_bounds),
                                &path,
                            );
                        }
                        if let Some(s) = stroke {
                            let (stroke_style, paint) = map_stroke(s, paint_bounds);
                            self.painter
                                .stroke_path(svg_transform, &stroke_style, paint, &path);
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
                    self.painter.fill_rect(
                        self.current_transform,
                        PaintType::from(Color::from_rgba8(color.r, color.g, color.b, color.a)),
                        &Self::layout_rect_to_rect(*rect),
                    );
                }
            }
        }
        Ok(())
    }

    fn push_layer_for(&mut self, clip: BezPath, bounds: Rect, blend: BlendMode, opacity: f32) {
        self.painter
            .push_layer(self.current_transform, Some(&clip), blend, opacity);
        self.push_clip_bounds(bounds);
        self.current_layer_count += 1;
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_rect(
        &mut self,
        rect: Rect,
        fill: Option<&fission_render::Fill>,
        stroke: Option<&fission_render::Stroke>,
        corner_radius: f32,
        shadow: Option<&fission_render::BoxShadow>,
        corner_radii: Option<fission_ir::CornerRadii>,
        border_sides: Option<&fission_render::BorderSides>,
    ) {
        let radii = corner_radii
            .map(kurbo_radii)
            .unwrap_or_else(|| RoundedRectRadii::from_single_radius(corner_radius as f64));
        let shape = RoundedRect::from_rect(rect, radii);
        // A blurred rectangle takes one radius, so shadows use the largest corner.
        let shadow_radius = radii
            .top_left
            .max(radii.top_right)
            .max(radii.bottom_right)
            .max(radii.bottom_left) as f32;

        if let Some(shadow) = shadow.filter(|shadow| !shadow.inset) {
            let shadow_rect = (rect + Vec2::new(shadow.offset.0 as f64, shadow.offset.1 as f64))
                .inflate(shadow.spread_radius as f64, shadow.spread_radius as f64);
            self.painter.fill_blurred_rounded_rect(
                self.current_transform,
                map_color(&shadow.color),
                &shadow_rect,
                (shadow_radius + shadow.spread_radius).max(0.0),
                shadow.blur_radius.max(0.0) * 0.5,
                false,
            );
        }

        if let Some(fill) = fill {
            self.painter.fill_path(
                self.current_transform,
                map_fill_to_brush(fill, rect),
                &shape.to_path(0.1),
            );
        }

        // Painted over the background and under the border, as CSS orders them. An inset shadow
        // is the inverse of a blurred rectangle shrunk by the spread and moved by the offset,
        // visible only inside the box.
        if let Some(shadow) = shadow.filter(|shadow| shadow.inset) {
            let inner = (rect + Vec2::new(shadow.offset.0 as f64, shadow.offset.1 as f64))
                .inset(-(shadow.spread_radius as f64));
            self.painter.push_layer(
                self.current_transform,
                Some(&shape.to_path(0.1)),
                BlendMode::default(),
                1.0,
            );
            self.painter.fill_blurred_rounded_rect(
                self.current_transform,
                map_color(&shadow.color),
                &inner,
                (shadow_radius - shadow.spread_radius).max(0.0),
                shadow.blur_radius.max(0.0) * 0.5,
                true,
            );
            self.painter.pop_layer();
        }

        let sides = match border_sides.filter(|sides| !sides.is_empty()) {
            Some(sides) => sides.clone(),
            None => match stroke {
                Some(stroke) => fission_render::BorderSides::uniform(stroke.clone()),
                None => return,
            },
        };
        self.draw_border(rect, radii, &sides);
    }

    /// Paint a box's borders inside its edges, as CSS does.
    ///
    /// Borders sit inside the box so a bordered rectangle covers the same pixels on every
    /// backend; the site shell renders them with `box-sizing: border-box`. Each edge paints the
    /// part of the ring between the outer and inner rounded rectangles that falls in its wedge,
    /// split at each corner along the line from the outer to the inner corner. Differing edges
    /// therefore meet cleanly at rounded corners instead of overlapping or protruding. A dashed
    /// edge strokes the ring's centre line inside its wedge.
    fn draw_border(
        &mut self,
        rect: Rect,
        radii: RoundedRectRadii,
        sides: &fission_render::BorderSides,
    ) {
        let width = |side: &Option<fission_render::Stroke>| {
            side.as_ref()
                .map_or(0.0, |stroke| f64::from(stroke.width.max(0.0)))
        };
        let (top, right, bottom, left) = (
            width(&sides.top),
            width(&sides.right),
            width(&sides.bottom),
            width(&sides.left),
        );
        let inner_x0 = (rect.x0 + left).min(rect.x1);
        let inner_y0 = (rect.y0 + top).min(rect.y1);
        let inner_rect = Rect::new(
            inner_x0,
            inner_y0,
            (rect.x1 - right).max(inner_x0),
            (rect.y1 - bottom).max(inner_y0),
        );
        let inner_radii = RoundedRectRadii::new(
            (radii.top_left - left.max(top)).max(0.0),
            (radii.top_right - right.max(top)).max(0.0),
            (radii.bottom_right - right.max(bottom)).max(0.0),
            (radii.bottom_left - left.max(bottom)).max(0.0),
        );
        let outer = RoundedRect::from_rect(rect, radii);
        let inner = RoundedRect::from_rect(inner_rect, inner_radii);
        let ring = BezPath::from_iter(
            outer
                .path_elements(0.1)
                .chain(inner.to_path(0.1).reverse_subpaths()),
        );

        if let Some(stroke) = sides
            .as_uniform()
            .filter(|stroke| stroke.dash_array.is_none())
        {
            self.painter.fill_path(
                self.current_transform,
                map_fill_to_brush(&stroke.fill, rect),
                &ring,
            );
            return;
        }

        let centre = RoundedRect::from_rect(
            Rect::new(
                rect.x0 + left / 2.0,
                rect.y0 + top / 2.0,
                rect.x1 - right / 2.0,
                rect.y1 - bottom / 2.0,
            ),
            RoundedRectRadii::new(
                (radii.top_left - left.max(top) / 2.0).max(0.0),
                (radii.top_right - right.max(top) / 2.0).max(0.0),
                (radii.bottom_right - right.max(bottom) / 2.0).max(0.0),
                (radii.bottom_left - left.max(bottom) / 2.0).max(0.0),
            ),
        )
        .to_path(0.1);
        let (o, i) = (rect, inner_rect);
        let wedges = [
            (
                &sides.top,
                [(o.x0, o.y0), (o.x1, o.y0), (i.x1, i.y0), (i.x0, i.y0)],
            ),
            (
                &sides.right,
                [(o.x1, o.y0), (o.x1, o.y1), (i.x1, i.y1), (i.x1, i.y0)],
            ),
            (
                &sides.bottom,
                [(o.x1, o.y1), (o.x0, o.y1), (i.x0, i.y1), (i.x1, i.y1)],
            ),
            (
                &sides.left,
                [(o.x0, o.y1), (o.x0, o.y0), (i.x0, i.y0), (i.x0, i.y1)],
            ),
        ];
        for (side, corners) in wedges {
            let Some(stroke) = side else {
                continue;
            };
            if stroke.width <= 0.0 {
                continue;
            }
            let mut wedge = BezPath::new();
            wedge.move_to(corners[0]);
            for corner in &corners[1..] {
                wedge.line_to(*corner);
            }
            wedge.close_path();

            self.painter.push_layer(
                self.current_transform,
                Some(&wedge),
                BlendMode::default(),
                1.0,
            );
            if stroke.dash_array.is_some() {
                let (stroke_style, paint) = map_stroke(stroke, rect);
                self.painter
                    .stroke_path(self.current_transform, &stroke_style, paint, &centre);
            } else {
                self.painter.fill_path(
                    self.current_transform,
                    map_fill_to_brush(&stroke.fill, rect),
                    &ring,
                );
            }
            self.painter.pop_layer();
        }
    }

    fn render_node(&mut self, node: &RenderNode) -> Result<()> {
        match node {
            RenderNode::Paint(list) => self.render_paint_list(list),
            RenderNode::Layer(layer) => self.render_layer(layer),
        }
    }

    fn render_layer(&mut self, layer: &RenderLayer) -> Result<()> {
        let saved_transform = self.current_transform;
        let saved_layer_count = self.current_layer_count;
        let saved_clip_count = self.clip_stack.len();

        if let Some(clip) = &layer.style.clip {
            let (path, bounds) = match clip {
                LayerClip::Rect(rect) => {
                    let r = Self::layout_rect_to_rect(*rect);
                    (r.to_path(0.1), r)
                }
                LayerClip::RoundedRect { rect, radius } => {
                    let r = Self::layout_rect_to_rect(*rect);
                    (RoundedRect::from_rect(r, *radius as f64).to_path(0.1), r)
                }
            };
            self.push_layer_for(path, bounds, BlendMode::default(), 1.0);
        }

        if (layer.style.opacity - 1.0).abs() > 0.001 || !layer.style.blend_mode.is_normal() {
            let r = Self::layout_rect_to_rect(layer.bounds);
            self.push_layer_for(
                r.to_path(0.1),
                r,
                painter::blend_mode(layer.style.blend_mode),
                layer.style.opacity,
            );
        }

        if let Some(transform) = layer.style.transform {
            self.current_transform *= Self::affine_from_mat4(&transform);
        }

        for child in &layer.children {
            self.render_node(child)?;
        }

        while self.current_layer_count > saved_layer_count {
            self.painter.pop_layer();
            self.current_layer_count -= 1;
        }
        self.clip_stack.truncate(saved_clip_count);
        self.current_transform = saved_transform;
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
