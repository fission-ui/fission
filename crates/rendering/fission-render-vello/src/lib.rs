pub mod text;
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
use vello::kurbo::{Affine, BezPath, Point, Rect, RoundedRect};
// Minimal imports from peniko
use vello::peniko::{
    Blob, Brush, Color, Fill, ImageAlphaType, ImageBrush, ImageData, ImageFormat, ImageSampler, Mix,
};

fn text_style_requires_rich_layout(style: &RenderTextStyle) -> bool {
    text::text_style_requires_rich_layout(style)
}

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
use parley::layout::{Alignment as ParleyAlignment, AlignmentOptions, PositionedLayoutItem};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use vello::{Glyph, Scene};

const PARAGRAPH_FADE_SLICE_COUNT: usize = 8;
const PARAGRAPH_FADE_MIN_SPAN: f32 = 8.0;
const PARAGRAPH_FADE_RIGHT_MULTIPLIER: f32 = 1.5;
const PARAGRAPH_FADE_BOTTOM_FRACTION: f32 = 0.5;
const TEXT_CULL_PADDING: f32 = 8.0;
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
        bottom >= self.top && top <= self.bottom
    }

    fn intersects_x(self, left: f32, right: f32) -> bool {
        right >= self.left && left <= self.right
    }
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
    static ref IMAGE_CACHE: Mutex<HashMap<String, ImageCacheEntry>> = Mutex::new(HashMap::new());
    static ref SVG_CACHE: Mutex<HashMap<u64, Arc<SvgCacheEntry>>> = Mutex::new(HashMap::new());
}

static IMAGE_CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
enum ImageCacheEntry {
    Ready(Arc<ImageData>),
    Loading,
    Failed,
}

pub fn image_cache_generation() -> u64 {
    IMAGE_CACHE_GENERATION.load(Ordering::Acquire)
}

pub fn image_cache_has_pending() -> bool {
    IMAGE_CACHE
        .lock()
        .unwrap()
        .values()
        .any(|entry| matches!(entry, ImageCacheEntry::Loading))
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_image_from_path(
    path: &str,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<ImageData>> {
    let img = image::open(path).ok()?;
    decode_dynamic_image(img, cache_width, cache_height)
}

fn decode_image_from_bytes(
    bytes: &[u8],
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<ImageData>> {
    let img = image::load_from_memory(bytes).ok()?;
    decode_dynamic_image(img, cache_width, cache_height)
}

fn decode_dynamic_image(
    mut img: image::DynamicImage,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<ImageData>> {
    if let (Some(width), Some(height)) = (cache_width, cache_height) {
        if width > 0 && height > 0 {
            img = img.resize(width, height, image::imageops::FilterType::Triangle);
        }
    }
    let img = img.to_rgba8();
    let (width, height) = img.dimensions();
    let data = img.into_raw();
    Some(Arc::new(ImageData {
        data: Blob::new(Arc::new(data)),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width,
        height,
    }))
}

fn complete_image_load(key: String, image: Option<Arc<ImageData>>) {
    let mut cache = IMAGE_CACHE.lock().unwrap();
    cache.insert(
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
) -> Option<Arc<ImageData>> {
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
        IMAGE_CACHE.lock().unwrap().remove(&key);
        let before = image_cache_generation();

        spawn_image_load(key.clone(), request);

        let deadline = Instant::now() + Duration::from_secs(2);
        while image_cache_generation() == before && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        let cache = IMAGE_CACHE.lock().unwrap();
        let Some(ImageCacheEntry::Ready(image)) = cache.get(&key) else {
            panic!("expected decoded image in cache");
        };
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
    }

    #[test]
    fn network_image_fetch_decodes_png_response() {
        let url = serve_once(tiny_png());
        let image = fetch_network_image(&url, Vec::new(), Some(1), Some(1))
            .expect("fetch and decode test image");

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
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
        paragraph_alignment, paragraph_fade, paragraph_line_trim, paragraph_line_visual_bounds,
        paragraph_y_offset, parse_svg_entry, ParagraphFade, RetainedSceneCache, SvgShape,
        VelloRenderer, VelloTextMeasurer,
    };
    use fission_ir::op::{
        FontStyle, MouseCursor, RichTextAnnotation, TextAlign, TextDirection, TextHeightBehavior,
        TextOverflow, TextParagraphStyle, TextWidthBasis,
    };
    use fission_ir::{semantics::ActionTrigger, ActionEntry};
    use fission_layout::TextMeasurer;
    use fission_render::{
        Color as RenderColor, LayoutPoint, LayoutRect, TextStyle as RenderTextStyle,
    };
    use parley::FontContext;
    use std::sync::{Arc, Mutex};
    use vello::Scene;

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

    fn test_renderer<'a>(
        scene: &'a mut Scene,
        cache: &'a mut RetainedSceneCache,
    ) -> VelloRenderer<'a> {
        let measurer = Arc::new(VelloTextMeasurer::new(Arc::new(Mutex::new(
            FontContext::new(),
        ))));
        VelloRenderer::new(scene, measurer, cache, 1.0)
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
        }
    }

    #[test]
    fn justify_alignment_stretches_non_terminal_lines() {
        let mut scene = Scene::new();
        let mut cache = RetainedSceneCache::default();
        let renderer = test_renderer(&mut scene, &mut cache);
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
        let mut scene = Scene::new();
        let mut cache = RetainedSceneCache::default();
        let renderer = test_renderer(&mut scene, &mut cache);
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
        let mut scene = Scene::new();
        let mut cache = RetainedSceneCache::default();
        let mut renderer = test_renderer(&mut scene, &mut cache);
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
        );
        drop(renderer);

        assert!(
            scene.encoding().n_clips > 0,
            "fade overflow should add internal clip layers"
        );
    }

    #[test]
    fn simple_text_rendering_culls_glyphs_outside_bounds() {
        let mut scene = Scene::new();
        let mut cache = RetainedSceneCache::default();
        let mut renderer = test_renderer(&mut scene, &mut cache);
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
            &[],
            &[],
        );
        drop(renderer);

        let glyphs = scene.encoding().resources.glyphs.len();
        assert!(glyphs > 0, "visible glyphs should still be encoded");
        assert!(
            glyphs < 256,
            "renderer should not encode the full off-bounds text run; glyphs={glyphs}"
        );
    }

    #[test]
    fn explicit_text_direction_realigns_neutral_content() {
        let mut scene = Scene::new();
        let mut cache = RetainedSceneCache::default();
        let renderer = test_renderer(&mut scene, &mut cache);
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
        let mut scene = Scene::new();
        let mut cache = RetainedSceneCache::default();
        let renderer = test_renderer(&mut scene, &mut cache);
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
        let mut scene = Scene::new();
        let mut cache = RetainedSceneCache::default();
        let renderer = test_renderer(&mut scene, &mut cache);
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
        let mut scene = Scene::new();
        let mut cache = RetainedSceneCache::default();
        let renderer = test_renderer(&mut scene, &mut cache);
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
    clip_stack: Vec<Rect>,
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
        position: fission_render::LayoutPoint,
        bounds: fission_render::LayoutRect,
    ) -> Option<TextClip> {
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return None;
        }
        Some(TextClip {
            left: bounds.x() - position.x - TEXT_CULL_PADDING,
            right: bounds.right() - position.x + TEXT_CULL_PADDING,
            top: bounds.y() - position.y - TEXT_CULL_PADDING,
            bottom: bounds.bottom() - position.y + TEXT_CULL_PADDING,
        })
    }

    fn get_image(&self, request: &ImageRequest) -> Option<Arc<ImageData>> {
        let key = request.stable_cache_key();
        {
            let mut cache = IMAGE_CACHE.lock().unwrap();
            if let Some(entry) = cache.get(&key) {
                return match entry {
                    ImageCacheEntry::Ready(img) => Some(Arc::clone(img)),
                    ImageCacheEntry::Loading | ImageCacheEntry::Failed => None,
                };
            }
            cache.insert(key.clone(), ImageCacheEntry::Loading);
        }

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
            layout.align(
                Some(alignment_width),
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

                let run_text_range = run.text_range();
                for (range, style) in styles.iter() {
                    if let Some(bg) = &style.background_color {
                        let overlap_start = range.start.max(run_text_range.start);
                        let overlap_end = range.end.min(run_text_range.end);
                        if overlap_start < overlap_end {
                            let bg_color = Color::from_rgba8(bg.r, bg.g, bg.b, bg.a);
                            let x0 = clip.map(|clip| run_left.max(clip.left)).unwrap_or(run_left);
                            let x1 = clip
                                .map(|clip| run_right.min(clip.right))
                                .unwrap_or(run_right);
                            if x1 <= x0 {
                                break;
                            }
                            let x0 = position.x as f64 + x0 as f64;
                            let x1 = position.x as f64 + x1 as f64;
                            let y0 = position.y as f64 + top_y as f64;
                            let bg_rect = Rect::new(x0, y0, x1, y0 + line_height as f64);
                            self.scene.fill(
                                Fill::NonZero,
                                self.current_transform,
                                bg_color,
                                None,
                                &bg_rect,
                            );
                            break;
                        }
                    }
                }

                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
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

                self.scene
                    .draw_glyphs(font)
                    .font_size(font_size)
                    .transform(
                        self.current_transform
                            * Affine::translate((position.x as f64, position.y as f64)),
                    )
                    .brush(color)
                    .draw(Fill::NonZero, glyphs.into_iter());

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

                    let x0 = clip.map(|clip| run_left.max(clip.left)).unwrap_or(run_left);
                    let x1 = clip
                        .map(|clip| run_right.min(clip.right))
                        .unwrap_or(run_right);
                    if x1 <= x0 {
                        continue;
                    }
                    let x0 = position.x as f64 + x0 as f64;
                    let x1 = position.x as f64 + x1 as f64;
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
        let text_clip = Self::text_clip(draw_position, bounds);

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
                        &[],
                        &[],
                    );
                }
            }
        }

        if let Some(idx) = prepared.caret_index {
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
        caret_index: Option<usize>,
        caret_color: Option<RenderColor>,
        caret_width: Option<f32>,
        caret_height: Option<f32>,
        caret_radius: Option<f32>,
        paragraph_style: Option<TextParagraphStyle>,
        inline_boxes: &[crate::text::RichInlineBox],
        styles: &[(std::ops::Range<usize>, RenderTextStyle)],
    ) {
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
                bounds,
                paragraph,
                inline_boxes,
                paragraph_styles,
                caret_index,
                caret_color,
                caret_width,
                caret_height,
                caret_radius,
            );
            return;
        }

        let text_clip = Self::text_clip(position, bounds);

        // Fast path for simple text using cache
        if styles.is_empty() && inline_boxes.is_empty() {
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

                        self.scene
                            .draw_glyphs(font)
                            .font_size(font_size)
                            .transform(
                                self.current_transform
                                    * Affine::translate((position.x as f64, position.y as f64)),
                            )
                            .brush(color)
                            .draw(Fill::NonZero, glyphs.into_iter());

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
            if wrap && bounds.width() > 0.0 {
                Some(bounds.width() as f32)
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
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                    let run_left = glyph_run.offset();
                    let run_right = glyph_run.offset() + glyph_run.advance();
                    if let Some(clip) = text_clip {
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
                                let x0 = text_clip
                                    .map(|clip| run_left.max(clip.left))
                                    .unwrap_or(run_left);
                                let x1 = text_clip
                                    .map(|clip| run_right.min(clip.right))
                                    .unwrap_or(run_right);
                                if x1 <= x0 {
                                    break;
                                }
                                let x0 = position.x as f64 + x0 as f64;
                                let x1 = position.x as f64 + x1 as f64;
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

                    self.scene
                        .draw_glyphs(font)
                        .font_size(font_size)
                        .transform(
                            self.current_transform
                                * Affine::translate((position.x as f64, position.y as f64)),
                        )
                        .brush(color)
                        .draw(Fill::NonZero, glyphs.into_iter());

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

                self.scene.fill(
                    Fill::NonZero,
                    self.current_transform,
                    Color::from_rgba8(caret_color.r, caret_color.g, caret_color.b, caret_color.a),
                    None,
                    &caret_shape,
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
            self.scene.fill(
                Fill::NonZero,
                self.current_transform,
                Color::from_rgba8(caret_color.r, caret_color.g, caret_color.b, caret_color.a),
                None,
                &caret_shape,
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
                    if let Some(image_data) = self.get_image(request) {
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
