use std::borrow::Cow;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, OnceLock,
};

use fission_ir::op::{HttpHeader, ImageAlignment, ImageRequest, ImageSource};
use fission_layout::{LineMetric, TextMeasurer};
use fission_render::{
    image_cache_store::ImageCacheStore, Color as RenderColor, Fill, LineCap, LineJoin, Stroke,
};
use kurbo::{BezPath, PathEl, Rect as KurboRect, RoundedRect, Shape};
use tiny_skia::{
    Color, GradientStop, LineCap as TinyLineCap, LineJoin as TinyLineJoin, Mask, Paint, Path,
    PathBuilder, Pixmap, Point, PremultipliedColorU8, Shader, SpreadMode, Stroke as TinyStroke,
    Transform,
};

const DEFAULT_IMAGE_CACHE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct DrawState {
    pub(super) transform: Transform,
    pub(super) clip: Option<Mask>,
    pub(super) surface: usize,
    pub(super) layer_alpha: Option<f32>,
}

#[derive(Debug, Clone)]
pub(super) struct SvgCacheEntry {
    pub(super) view_box: Option<(f32, f32, f32, f32)>,
    pub(super) shapes: Vec<SvgShape>,
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) enum SvgShape {
    Path(BezPath),
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
}

static IMAGE_CACHE: OnceLock<ImageCacheStore<ImageCacheEntry>> = OnceLock::new();
static SVG_CACHE: OnceLock<Mutex<HashMap<String, Arc<SvgCacheEntry>>>> = OnceLock::new();
static IMAGE_CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);
static IMAGE_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static IMAGE_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static IMAGE_LOADS_STARTED: AtomicU64 = AtomicU64::new(0);
static IMAGE_LOADS_COMPLETED: AtomicU64 = AtomicU64::new(0);
static IMAGE_LOADS_FAILED: AtomicU64 = AtomicU64::new(0);
static IMAGE_CACHE_EVICTIONS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
enum ImageCacheEntry {
    Ready(Arc<Pixmap>),
    Loading,
    Failed,
}

pub(super) enum ImageCacheLookup {
    Ready(Arc<Pixmap>),
    Loading,
    Failed,
}

/// Observable software image-cache state used by host scheduling diagnostics.
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
}

impl ImageCacheEntry {
    fn weight(&self) -> u32 {
        match self {
            Self::Ready(image) => pixmap_byte_len(image).min(u64::from(u32::MAX)) as u32,
            Self::Loading | Self::Failed => 1,
        }
    }
}

fn image_cache() -> &'static ImageCacheStore<ImageCacheEntry> {
    IMAGE_CACHE.get_or_init(build_image_cache)
}

fn build_image_cache() -> ImageCacheStore<ImageCacheEntry> {
    ImageCacheStore::new(
        "fission-software-images",
        configured_image_cache_bytes(),
        ImageCacheEntry::weight,
        || {
            IMAGE_CACHE_EVICTIONS.fetch_add(1, Ordering::AcqRel);
        },
    )
}

pub(super) fn configured_image_cache_bytes() -> u64 {
    std::env::var("FISSION_IMAGE_CACHE_BYTES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_IMAGE_CACHE_BYTES)
}

pub(super) fn pixmap_byte_len(image: &Pixmap) -> u64 {
    u64::from(image.width())
        .saturating_mul(u64::from(image.height()))
        .saturating_mul(4)
}

pub(super) fn image_request_with_default_cache_size(
    request: &ImageRequest,
    rect: fission_render::LayoutRect,
) -> ImageRequest {
    if request.cache_width.is_some() && request.cache_height.is_some() {
        return request.clone();
    }
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return request.clone();
    }

    // tiny-skia applies the renderer scale through the draw transform. Keeping
    // cache dimensions in logical pixels prevents high-DPI images from being
    // over-sized and then clipped as pattern fills.
    let mut request = request.clone();
    request.cache_width = Some(cache_dimension_from_extent(rect.width()));
    request.cache_height = Some(cache_dimension_from_extent(rect.height()));
    request
}

pub(super) fn cache_dimension_from_extent(extent: f32) -> u32 {
    if !extent.is_finite() {
        return 1;
    }
    extent.ceil().clamp(1.0, u32::MAX as f32) as u32
}

/// Monotonic generation incremented when an asynchronous image load finishes.
pub fn image_cache_generation() -> u64 {
    IMAGE_CACHE_GENERATION.load(Ordering::Acquire)
}

/// Whether at least one image request is currently loading.
pub fn image_cache_has_pending() -> bool {
    image_cache()
        .values()
        .into_iter()
        .any(|entry| matches!(entry, ImageCacheEntry::Loading))
}

/// Snapshot current image-cache occupancy and load counters.
pub fn image_cache_stats() -> ImageCacheStats {
    image_cache().run_pending_tasks();
    ImageCacheStats {
        entries: image_cache().entry_count(),
        weighted_bytes: image_cache().weighted_size(),
        max_bytes: configured_image_cache_bytes(),
        pending: image_cache()
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
    }
}

pub(super) fn svg_cache() -> &'static Mutex<HashMap<String, Arc<SvgCacheEntry>>> {
    SVG_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn rgba_to_premul(color: RenderColor, coverage: u8) -> PremultipliedColorU8 {
    let alpha = ((u16::from(color.a) * u16::from(coverage)) / 255) as u8;
    PremultipliedColorU8::from_rgba(
        ((u16::from(color.r) * u16::from(alpha)) / 255) as u8,
        ((u16::from(color.g) * u16::from(alpha)) / 255) as u8,
        ((u16::from(color.b) * u16::from(alpha)) / 255) as u8,
        alpha,
    )
    .unwrap_or(PremultipliedColorU8::TRANSPARENT)
}

pub(super) fn tiny_color(color: RenderColor) -> Color {
    Color::from_rgba8(color.r, color.g, color.b, color.a)
}

pub(super) fn normalized_fill_point(
    bounds: fission_render::LayoutRect,
    point: (f32, f32),
) -> Point {
    Point::from_xy(
        bounds.origin.x + bounds.width() * point.0,
        bounds.origin.y + bounds.height() * point.1,
    )
}

pub(super) fn fill_shader(
    fill: &Fill,
    bounds: fission_render::LayoutRect,
) -> Option<Shader<'static>> {
    match fill {
        Fill::Solid(color) => Some(Shader::SolidColor(tiny_color(*color))),
        Fill::LinearGradient { start, end, stops } => {
            let stops = stops
                .iter()
                .map(|(offset, color)| GradientStop::new(*offset, tiny_color(*color)))
                .collect::<Vec<_>>();
            tiny_skia::LinearGradient::new(
                normalized_fill_point(bounds, *start),
                normalized_fill_point(bounds, *end),
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            )
        }
        Fill::RadialGradient {
            center,
            radius,
            stops,
        } => {
            let stops = stops
                .iter()
                .map(|(offset, color)| GradientStop::new(*offset, tiny_color(*color)))
                .collect::<Vec<_>>();
            tiny_skia::RadialGradient::new(
                normalized_fill_point(bounds, *center),
                normalized_fill_point(bounds, *center),
                radius * bounds.width().max(bounds.height()),
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            )
        }
    }
}

pub(super) fn fill_paint(fill: &Fill, bounds: fission_render::LayoutRect) -> Paint<'static> {
    let mut paint = Paint::default();
    if let Some(shader) = fill_shader(fill, bounds) {
        paint.shader = shader;
    }
    paint.anti_alias = true;
    paint
}

pub(super) fn normalized_scale_factor(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

pub(super) fn stroke_style(stroke: &Stroke) -> TinyStroke {
    let mut style = TinyStroke::default();
    style.width = stroke.width;
    style.line_cap = match stroke.line_cap {
        LineCap::Butt => TinyLineCap::Butt,
        LineCap::Round => TinyLineCap::Round,
        LineCap::Square => TinyLineCap::Square,
    };
    style.line_join = match stroke.line_join {
        LineJoin::Miter => TinyLineJoin::Miter,
        LineJoin::Round => TinyLineJoin::Round,
        LineJoin::Bevel => TinyLineJoin::Bevel,
    };
    if let Some(dash_array) = &stroke.dash_array {
        style.dash = tiny_skia::StrokeDash::new(dash_array.clone(), 0.0);
    }
    style
}

pub(super) fn rounded_rect_path(rect: fission_render::LayoutRect, radius: f32) -> Option<Path> {
    let rounded = RoundedRect::from_rect(
        KurboRect::new(
            rect.origin.x as f64,
            rect.origin.y as f64,
            rect.right() as f64,
            rect.bottom() as f64,
        ),
        radius as f64,
    );
    bez_to_tiny_path(&rounded.to_path(0.1))
}

pub(super) fn rect_path(rect: fission_render::LayoutRect) -> Option<Path> {
    let bez = KurboRect::new(
        rect.origin.x as f64,
        rect.origin.y as f64,
        rect.right() as f64,
        rect.bottom() as f64,
    )
    .to_path(0.1);
    bez_to_tiny_path(&bez)
}

pub(super) fn bez_to_tiny_path(path: &BezPath) -> Option<Path> {
    let mut builder = PathBuilder::new();
    for el in path.elements() {
        match el {
            PathEl::MoveTo(p) => builder.move_to(p.x as f32, p.y as f32),
            PathEl::LineTo(p) => builder.line_to(p.x as f32, p.y as f32),
            PathEl::QuadTo(p1, p2) => {
                builder.quad_to(p1.x as f32, p1.y as f32, p2.x as f32, p2.y as f32)
            }
            PathEl::CurveTo(p1, p2, p3) => builder.cubic_to(
                p1.x as f32,
                p1.y as f32,
                p2.x as f32,
                p2.y as f32,
                p3.x as f32,
                p3.y as f32,
            ),
            PathEl::ClosePath => builder.close(),
        }
    }
    builder.finish()
}

pub(super) fn parse_svg_entry(content: &str) -> SvgCacheEntry {
    let parse_view_box = |data: &str| -> Option<(f32, f32, f32, f32)> {
        let key = "viewBox=\"";
        let start = data.find(key)?;
        let rest = &data[start + key.len()..];
        let end = rest.find('"')?;
        let nums: Vec<f32> = rest[..end]
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .filter(|value: &f32| value.is_finite())
            .collect();
        (nums.len() == 4).then_some((nums[0], nums[1], nums[2], nums[3]))
    };

    let view_box = parse_view_box(content);
    let mut shapes = Vec::new();
    let mut error = if content.contains("viewBox=") && view_box.is_none() {
        Some("viewBox must contain four finite numeric values".to_string())
    } else {
        None
    };
    for tag in content.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or("");
        let tag_name = tag.split_whitespace().next().unwrap_or("");
        if tag_name == "path" {
            let parsed = tag
                .find("d=\"")
                .and_then(|start| {
                    let after = &tag[start + 3..];
                    after.find('"').map(|end| &after[..end])
                })
                .ok_or_else(|| "path element is missing a quoted d attribute".to_string())
                .and_then(|data| {
                    let mut data = data.to_string();
                    data = data.replace("M0 0h24v24H0z", "");
                    data = data.replace("M0 0h24v24H0V0z", "");
                    data = data.replace("M0,0h24v24H0V0z", "");
                    if data.trim().is_empty() {
                        Ok(None)
                    } else {
                        BezPath::from_svg(&data)
                            .map(Some)
                            .map_err(|parse_error| format!("invalid path data: {parse_error}"))
                    }
                });
            match parsed {
                Ok(Some(path)) => shapes.push(SvgShape::Path(path)),
                Ok(None) => {}
                Err(parse_error) => {
                    error.get_or_insert(parse_error);
                }
            };
        } else if tag_name == "rect" {
            if tag.contains("fill=\"none\"") || tag.contains("fill='none'") {
                continue;
            }
            let parse_attr = |name: &str, default: f32| -> Result<f32, String> {
                if let Some(pos) = tag.find(&format!("{}=\"", name)) {
                    let after = &tag[pos + name.len() + 2..];
                    if let Some(end) = after.find('"') {
                        return after[..end]
                            .parse()
                            .map_err(|_| format!("rect {name} attribute is not numeric"));
                    }
                }
                Ok(default)
            };
            match (
                parse_attr("x", 0.0),
                parse_attr("y", 0.0),
                parse_attr("width", 0.0),
                parse_attr("height", 0.0),
            ) {
                (Ok(x), Ok(y), Ok(width), Ok(height)) if width > 0.0 && height > 0.0 => {
                    shapes.push(SvgShape::Rect {
                        x,
                        y,
                        width,
                        height,
                    });
                }
                (Ok(_), Ok(_), Ok(_), Ok(_)) => {}
                values => {
                    let parse_error = [
                        values.0.err(),
                        values.1.err(),
                        values.2.err(),
                        values.3.err(),
                    ]
                    .into_iter()
                    .flatten()
                    .next()
                    .unwrap_or_else(|| "invalid rect element".to_string());
                    error.get_or_insert(parse_error);
                }
            }
        } else if tag_name == "polygon" {
            let points = tag.find("points=\"").and_then(|start| {
                let after = &tag[start + 8..];
                after.find('"').map(|end| &after[..end])
            });
            let Some(points) = points else {
                error.get_or_insert_with(|| {
                    "polygon element is missing a quoted points attribute".to_string()
                });
                continue;
            };
            let nums = points
                .split(|character: char| character.is_whitespace() || character == ',')
                .filter(|value| !value.is_empty())
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>();
            match nums {
                Ok(nums) if nums.len() >= 4 && nums.len() % 2 == 0 => {
                    let mut bez = BezPath::new();
                    bez.move_to((nums[0], nums[1]));
                    for point in nums[2..].chunks_exact(2) {
                        bez.line_to((point[0], point[1]));
                    }
                    bez.close_path();
                    shapes.push(SvgShape::Path(bez));
                }
                _ => {
                    error.get_or_insert_with(|| {
                        "polygon points must be numeric coordinate pairs".to_string()
                    });
                }
            }
        } else if matches!(tag_name, "" | "svg" | "g" | "title" | "desc")
            || tag_name.starts_with('/')
            || tag_name.starts_with('!')
            || tag_name.starts_with('?')
        {
            if tag_name == "g" && tag.contains("transform=") {
                error.get_or_insert_with(|| {
                    "group transforms are not supported by the software SVG renderer".to_string()
                });
            }
        } else {
            error.get_or_insert_with(|| format!("unsupported SVG element <{tag_name}>"));
        }
    }

    SvgCacheEntry {
        view_box,
        shapes,
        error,
    }
}

pub(super) fn svg_cache_entry(content: &str) -> Arc<SvgCacheEntry> {
    if let Some(entry) = svg_cache().lock().unwrap().get(content) {
        return Arc::clone(entry);
    }
    let parsed = Arc::new(parse_svg_entry(content));
    let mut cache = svg_cache().lock().unwrap();
    cache
        .entry(content.to_owned())
        .or_insert_with(|| Arc::clone(&parsed));
    parsed
}

pub(super) fn wrap_max_width(bounds_width: f32, font_size: f32, wrap: bool) -> Option<f32> {
    if !wrap || bounds_width <= 0.0 {
        return None;
    }
    // The retained text bounds track ink-box width more closely than advance width.
    // Give the software layout a small amount of slack so short labels do not wrap
    // spuriously when their final advance slightly exceeds the reported bounds.
    Some(bounds_width.ceil() + font_size * 0.5)
}

pub(super) fn pipeline_wrap_breaks(
    measurer: Option<&dyn TextMeasurer>,
    text: &str,
    font_size: f32,
    bounds_width: f32,
    wrap: bool,
) -> Option<Vec<usize>> {
    if !wrap || bounds_width <= 0.0 {
        return None;
    }
    let measurer = measurer?;
    let lines = measurer.get_line_metrics(text, font_size, Some(bounds_width));
    if lines.is_empty() {
        return None;
    }
    Some(soft_wrap_breaks(text, &lines))
}

pub(super) fn soft_wrap_breaks(text: &str, lines: &[LineMetric]) -> Vec<usize> {
    lines
        .windows(2)
        .filter_map(|pair| {
            let end = pair[0].end_index.min(text.len());
            let next_start = pair[1].start_index.min(text.len());
            if !text.is_char_boundary(end) || !text.is_char_boundary(next_start) {
                return None;
            }
            let gap_start = end.min(next_start);
            let gap_end = end.max(next_start);
            let already_broken = text[..end].ends_with('\r')
                || text[..end].ends_with('\n')
                || text[end..].starts_with('\r')
                || text[end..].starts_with('\n')
                || text[gap_start..gap_end]
                    .chars()
                    .any(|character| matches!(character, '\r' | '\n'));
            (!already_broken).then_some(end)
        })
        .collect()
}

pub(super) fn insert_soft_wraps<'a>(text: &'a str, breaks: &[usize]) -> Cow<'a, str> {
    if breaks.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut wrapped = String::with_capacity(text.len() + breaks.len());
    let mut cursor = 0;
    for &break_at in breaks {
        if break_at < cursor || break_at > text.len() || !text.is_char_boundary(break_at) {
            continue;
        }
        wrapped.push_str(&text[cursor..break_at]);
        wrapped.push('\n');
        cursor = break_at;
    }
    wrapped.push_str(&text[cursor..]);
    Cow::Owned(wrapped)
}

pub(super) fn cached_image(request: &ImageRequest) -> ImageCacheLookup {
    let key = request.stable_cache_key();
    if let Some(entry) = image_cache().get(&key) {
        return match entry {
            ImageCacheEntry::Ready(image) => {
                IMAGE_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                ImageCacheLookup::Ready(Arc::clone(&image))
            }
            ImageCacheEntry::Loading => ImageCacheLookup::Loading,
            ImageCacheEntry::Failed => ImageCacheLookup::Failed,
        };
    }

    IMAGE_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    IMAGE_LOADS_STARTED.fetch_add(1, Ordering::Relaxed);
    image_cache().insert(key.clone(), ImageCacheEntry::Loading);
    spawn_image_load(key, request.clone());
    ImageCacheLookup::Loading
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn decode_image_from_path(
    path: &str,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    image::open(path)
        .ok()
        .and_then(|image| decode_dynamic_image(image, cache_width, cache_height))
}

pub(super) fn decode_image_from_bytes(
    bytes: &[u8],
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    image::load_from_memory(bytes)
        .ok()
        .and_then(|image| decode_dynamic_image(image, cache_width, cache_height))
}

pub(super) fn decode_dynamic_image(
    mut image: image::DynamicImage,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    if let (Some(width), Some(height)) = (cache_width, cache_height) {
        if width > 0 && height > 0 {
            image = image.resize(width, height, image::imageops::FilterType::Triangle);
        }
    }
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let size = tiny_skia::IntSize::from_wh(width, height)?;
    let mut premultiplied = rgba.into_raw();
    for pixel in premultiplied.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        pixel[0] = (u16::from(pixel[0]) * alpha / 255) as u8;
        pixel[1] = (u16::from(pixel[1]) * alpha / 255) as u8;
        pixel[2] = (u16::from(pixel[2]) * alpha / 255) as u8;
    }
    Pixmap::from_vec(premultiplied, size).map(Arc::new)
}

pub(super) fn complete_image_load(key: String, image: Option<Arc<Pixmap>>) {
    if image.is_some() {
        IMAGE_LOADS_COMPLETED.fetch_add(1, Ordering::AcqRel);
    } else {
        IMAGE_LOADS_FAILED.fetch_add(1, Ordering::AcqRel);
    }
    image_cache().insert(
        key,
        image
            .map(ImageCacheEntry::Ready)
            .unwrap_or(ImageCacheEntry::Failed),
    );
    IMAGE_CACHE_GENERATION.fetch_add(1, Ordering::AcqRel);
}

pub(super) fn aligned_offset(
    extra_width: f32,
    extra_height: f32,
    alignment: ImageAlignment,
) -> (f32, f32) {
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
pub(super) fn spawn_image_load(key: String, request: ImageRequest) {
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
pub(super) fn spawn_image_load(key: String, request: ImageRequest) {
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
pub(super) fn fetch_network_image(
    url: &str,
    headers: Vec<HttpHeader>,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    let mut request = ureq::get(url).set("User-Agent", "FissionImageLoader/0.2");
    for header in headers {
        request = request.set(&header.name, &header.value);
    }
    request
        .call()
        .ok()
        .and_then(|response| {
            let mut bytes = Vec::new();
            response.into_reader().read_to_end(&mut bytes).ok()?;
            image::load_from_memory(&bytes).ok()
        })
        .and_then(|image| decode_dynamic_image(image, cache_width, cache_height))
}

#[cfg(test)]
mod tests;
