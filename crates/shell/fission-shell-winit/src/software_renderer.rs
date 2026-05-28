use anyhow::{anyhow, Result};
use fission_ir::op::{HttpHeader, ImageAlignment, ImageRequest, ImageSource};
use fission_render::{
    surface_placeholder_color, Color as RenderColor, DisplayList, DisplayOp, Fill, ImageFit,
    LineCap, LineJoin, RenderScene, Stroke, TextRun,
};
use fontdue::{
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle as FontdueTextStyle},
    Font, FontSettings,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, OnceLock,
};
use tiny_skia::{
    Color, FillRule as TinyFillRule, FilterQuality, GradientStop, LineCap as TinyLineCap,
    LineJoin as TinyLineJoin, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Point,
    PremultipliedColorU8, Shader, SpreadMode, Stroke as TinyStroke, Transform,
};
use vello::kurbo::{BezPath, PathEl, Rect as KurboRect, RoundedRect, Shape};

#[derive(Clone)]
struct DrawState {
    transform: Transform,
    clip: Option<Mask>,
    surface: usize,
    layer_alpha: Option<f32>,
}

#[derive(Debug, Clone)]
struct SvgCacheEntry {
    view_box: Option<(f32, f32, f32, f32)>,
    shapes: Vec<SvgShape>,
}

#[derive(Debug, Clone)]
enum SvgShape {
    Path(BezPath),
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
}

static DEFAULT_FONT: OnceLock<Font> = OnceLock::new();
static IMAGE_CACHE: OnceLock<Mutex<HashMap<String, ImageCacheEntry>>> = OnceLock::new();
static SVG_CACHE: OnceLock<Mutex<HashMap<u64, Arc<SvgCacheEntry>>>> = OnceLock::new();
static IMAGE_CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
enum ImageCacheEntry {
    Ready(Arc<Pixmap>),
    Loading,
    Failed,
}

fn default_font() -> &'static Font {
    DEFAULT_FONT.get_or_init(|| {
        Font::from_bytes(
            fission_theme::fonts::default_font_bytes(),
            FontSettings::default(),
        )
        .expect("failed to load bundled UI font")
    })
}

fn image_cache() -> &'static Mutex<HashMap<String, ImageCacheEntry>> {
    IMAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn image_cache_generation() -> u64 {
    IMAGE_CACHE_GENERATION.load(Ordering::Acquire)
}

pub(crate) fn image_cache_has_pending() -> bool {
    image_cache()
        .lock()
        .unwrap()
        .values()
        .any(|entry| matches!(entry, ImageCacheEntry::Loading))
}

fn svg_cache() -> &'static Mutex<HashMap<u64, Arc<SvgCacheEntry>>> {
    SVG_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn rgba_to_premul(color: RenderColor, coverage: u8) -> PremultipliedColorU8 {
    let alpha = ((u16::from(color.a) * u16::from(coverage)) / 255) as u8;
    PremultipliedColorU8::from_rgba(
        ((u16::from(color.r) * u16::from(alpha)) / 255) as u8,
        ((u16::from(color.g) * u16::from(alpha)) / 255) as u8,
        ((u16::from(color.b) * u16::from(alpha)) / 255) as u8,
        alpha,
    )
    .unwrap_or(PremultipliedColorU8::TRANSPARENT)
}

fn tiny_color(color: RenderColor) -> Color {
    Color::from_rgba8(color.r, color.g, color.b, color.a)
}

fn fill_shader(fill: &Fill) -> Option<Shader<'static>> {
    match fill {
        Fill::Solid(color) => Some(Shader::SolidColor(tiny_color(*color))),
        Fill::LinearGradient { start, end, stops } => {
            let stops = stops
                .iter()
                .map(|(offset, color)| GradientStop::new(*offset, tiny_color(*color)))
                .collect::<Vec<_>>();
            tiny_skia::LinearGradient::new(
                Point::from_xy(start.0, start.1),
                Point::from_xy(end.0, end.1),
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
                Point::from_xy(center.0, center.1),
                Point::from_xy(center.0, center.1),
                *radius,
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            )
        }
    }
}

fn fill_paint(fill: &Fill) -> Paint<'static> {
    let mut paint = Paint::default();
    if let Some(shader) = fill_shader(fill) {
        paint.shader = shader;
    }
    paint.anti_alias = true;
    paint
}

fn normalized_scale_factor(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn stroke_style(stroke: &Stroke) -> TinyStroke {
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

fn rounded_rect_path(rect: fission_render::LayoutRect, radius: f32) -> Option<Path> {
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

fn rect_path(rect: fission_render::LayoutRect) -> Option<Path> {
    let bez = KurboRect::new(
        rect.origin.x as f64,
        rect.origin.y as f64,
        rect.right() as f64,
        rect.bottom() as f64,
    )
    .to_path(0.1);
    bez_to_tiny_path(&bez)
}

fn bez_to_tiny_path(path: &BezPath) -> Option<Path> {
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

fn svg_cache_key(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn parse_svg_entry(content: &str) -> SvgCacheEntry {
    let parse_view_box = |data: &str| -> Option<(f32, f32, f32, f32)> {
        let key = "viewBox=\"";
        let start = data.find(key)?;
        let rest = &data[start + key.len()..];
        let end = rest.find('"')?;
        let nums: Vec<f32> = rest[..end]
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        (nums.len() == 4).then_some((nums[0], nums[1], nums[2], nums[3]))
    };

    let mut shapes = Vec::new();
    for tag in content.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or("");
        let tag_name = tag.split_whitespace().next().unwrap_or("");
        if tag_name == "path" {
            if let Some(start) = tag.find("d=\"") {
                let after = &tag[start + 3..];
                if let Some(end) = after.find('"') {
                    let mut d = after[..end].to_string();
                    d = d.replace("M0 0h24v24H0z", "");
                    d = d.replace("M0 0h24v24H0V0z", "");
                    d = d.replace("M0,0h24v24H0V0z", "");
                    if !d.trim().is_empty() {
                        if let Ok(path) = BezPath::from_svg(&d) {
                            shapes.push(SvgShape::Path(path));
                        }
                    }
                }
            }
        } else if tag_name == "rect" {
            if tag.contains("fill=\"none\"") || tag.contains("fill='none'") {
                continue;
            }
            let parse_attr = |name: &str| -> f32 {
                if let Some(pos) = tag.find(&format!("{}=\"", name)) {
                    let after = &tag[pos + name.len() + 2..];
                    if let Some(end) = after.find('"') {
                        return after[..end].parse().unwrap_or(0.0);
                    }
                }
                0.0
            };
            let x = parse_attr("x");
            let y = parse_attr("y");
            let width = parse_attr("width");
            let height = parse_attr("height");
            if width > 0.0 && height > 0.0 {
                shapes.push(SvgShape::Rect {
                    x,
                    y,
                    width,
                    height,
                });
            }
        } else if tag_name == "polygon" {
            if let Some(start) = tag.find("points=\"") {
                let after = &tag[start + 8..];
                if let Some(end) = after.find('"') {
                    let nums: Vec<f64> = after[..end]
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

fn svg_cache_entry(content: &str) -> Arc<SvgCacheEntry> {
    let key = svg_cache_key(content);
    if let Some(entry) = svg_cache().lock().unwrap().get(&key) {
        return Arc::clone(entry);
    }
    let parsed = Arc::new(parse_svg_entry(content));
    let mut cache = svg_cache().lock().unwrap();
    cache.entry(key).or_insert_with(|| Arc::clone(&parsed));
    parsed
}

fn wrap_max_width(bounds_width: f32, font_size: f32, wrap: bool) -> Option<f32> {
    if !wrap || bounds_width <= 0.0 {
        return None;
    }
    // The retained text bounds track ink-box width more closely than advance width.
    // Give the software layout a small amount of slack so short labels do not wrap
    // spuriously when their final advance slightly exceeds the reported bounds.
    Some(bounds_width.ceil() + font_size * 0.5)
}

fn cached_image(request: &ImageRequest) -> Option<Arc<Pixmap>> {
    let key = request.stable_cache_key();
    {
        let mut cache = image_cache().lock().unwrap();
        if let Some(entry) = cache.get(&key) {
            return match entry {
                ImageCacheEntry::Ready(image) => Some(Arc::clone(image)),
                ImageCacheEntry::Loading | ImageCacheEntry::Failed => None,
            };
        }
        cache.insert(key.clone(), ImageCacheEntry::Loading);
    }

    spawn_image_load(key, request.clone());
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_image_from_path(
    path: &str,
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    image::open(path)
        .ok()
        .and_then(|image| decode_dynamic_image(image, cache_width, cache_height))
}

fn decode_image_from_bytes(
    bytes: &[u8],
    cache_width: Option<u32>,
    cache_height: Option<u32>,
) -> Option<Arc<Pixmap>> {
    image::load_from_memory(bytes)
        .ok()
        .and_then(|image| decode_dynamic_image(image, cache_width, cache_height))
}

fn decode_dynamic_image(
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
    Pixmap::from_vec(rgba.into_raw(), size).map(Arc::new)
}

fn complete_image_load(key: String, image: Option<Arc<Pixmap>>) {
    let mut cache = image_cache().lock().unwrap();
    cache.insert(
        key,
        image
            .map(ImageCacheEntry::Ready)
            .unwrap_or(ImageCacheEntry::Failed),
    );
    IMAGE_CACHE_GENERATION.fetch_add(1, Ordering::AcqRel);
}

fn aligned_offset(extra_width: f32, extra_height: f32, alignment: ImageAlignment) -> (f32, f32) {
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
mod image_tests {
    use super::*;
    use std::io::Cursor;
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    fn tiny_png() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 128, 255, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("encode png");
        bytes.into_inner()
    }

    fn solid_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba(rgba));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
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
        image_cache().lock().unwrap().remove(&key);
        let before = image_cache_generation();

        spawn_image_load(key.clone(), request);

        let deadline = Instant::now() + Duration::from_secs(2);
        while image_cache_generation() == before && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        let cache = image_cache().lock().unwrap();
        let Some(ImageCacheEntry::Ready(image)) = cache.get(&key) else {
            panic!("expected decoded image in cache");
        };
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
    }

    #[test]
    fn network_image_fetch_decodes_png_response() {
        let url = serve_once(tiny_png());
        let image = fetch_network_image(&url, Vec::new(), Some(1), Some(1))
            .expect("fetch and decode test image");

        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
    }

    #[test]
    fn cached_image_request_paints_visible_pixels() {
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
        image_cache().lock().unwrap().remove(&key);
        let before = image_cache_generation();
        spawn_image_load(key.clone(), request.clone());

        let deadline = Instant::now() + Duration::from_secs(2);
        while image_cache_generation() == before && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        let rect = fission_render::LayoutRect::new(0.0, 0.0, 4.0, 4.0);
        let mut display_list = DisplayList::new(rect);
        display_list.push(DisplayOp::DrawImage {
            rect,
            request,
            fit: ImageFit::Fill,
            alignment: ImageAlignment::Center,
            bounds: rect,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(display_list);
        let transparent = RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
        let pixels = SoftwareRenderer::render(&scene, 4, 4, transparent, 1.0)
            .expect("render software image scene");

        assert!(
            pixels
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && (pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0)),
            "expected image draw to produce visible non-transparent pixels"
        );
    }

    #[test]
    fn high_dpi_render_uses_device_space_without_logical_upscale() {
        let bounds = fission_render::LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        let rect = fission_render::LayoutRect::new(1.0, 1.0, 2.0, 2.0);
        let mut display_list = DisplayList::new(bounds);
        display_list.push(DisplayOp::DrawRect {
            rect,
            fill: Some(Fill::Solid(RenderColor {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds: rect,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(display_list);
        let transparent = RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
        let pixels = SoftwareRenderer::render(&scene, 20, 20, transparent, 2.0)
            .expect("render high-DPI software scene");

        let pixel_at = |x: usize, y: usize| {
            let start = (y * 20 + x) * 4;
            &pixels[start..start + 4]
        };
        assert_eq!(pixel_at(0, 0), &[0, 0, 0, 0]);
        assert_eq!(pixel_at(3, 3), &[255, 0, 0, 255]);
    }

    #[test]
    fn cover_image_draw_is_clipped_to_destination_rect() {
        let request = ImageRequest {
            source: ImageSource::Memory {
                bytes: solid_png(4, 2, [255, 0, 0, 255]),
                mime_type: Some("image/png".into()),
            },
            ..Default::default()
        };
        let key = request.stable_cache_key();
        image_cache().lock().unwrap().remove(&key);
        let before = image_cache_generation();
        spawn_image_load(key.clone(), request.clone());

        let deadline = Instant::now() + Duration::from_secs(2);
        while image_cache_generation() == before && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        let bounds = fission_render::LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        let rect = fission_render::LayoutRect::new(4.0, 4.0, 2.0, 2.0);
        let mut display_list = DisplayList::new(bounds);
        display_list.push(DisplayOp::DrawImage {
            rect,
            request,
            fit: ImageFit::Cover,
            alignment: ImageAlignment::Center,
            bounds: rect,
            node_id: None,
        });
        let scene = RenderScene::from_display_list(display_list);
        let transparent = RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
        let pixels = SoftwareRenderer::render(&scene, 10, 10, transparent, 1.0)
            .expect("render clipped cover image");

        let pixel_at = |x: usize, y: usize| {
            let start = (y * 10 + x) * 4;
            &pixels[start..start + 4]
        };

        assert_eq!(pixel_at(3, 4), &[0, 0, 0, 0]);
        assert_eq!(pixel_at(6, 4), &[0, 0, 0, 0]);
        assert!(
            pixel_at(4, 4)[3] > 0 && pixel_at(4, 4)[0] > 0,
            "expected destination rect to contain image pixels"
        );
    }
}

pub struct SoftwareRenderer {
    width: u32,
    height: u32,
    scale_factor: f32,
    surfaces: Vec<Pixmap>,
    states: Vec<DrawState>,
}

impl SoftwareRenderer {
    fn new_with_scale(
        width: u32,
        height: u32,
        background: RenderColor,
        scale_factor: f32,
    ) -> Result<Self> {
        let mut root = Pixmap::new(width.max(1), height.max(1))
            .ok_or_else(|| anyhow!("failed to allocate software render target"))?;
        root.fill(tiny_color(background));
        Ok(Self {
            width: width.max(1),
            height: height.max(1),
            scale_factor: normalized_scale_factor(scale_factor),
            surfaces: vec![root],
            states: vec![DrawState {
                transform: Transform::identity(),
                clip: None,
                surface: 0,
                layer_alpha: None,
            }],
        })
    }

    pub fn render(
        scene: &RenderScene,
        width: u32,
        height: u32,
        background: RenderColor,
        scale_factor: f32,
    ) -> Result<Vec<u8>> {
        let mut renderer =
            Self::new_with_scale(width.max(1), height.max(1), background, scale_factor)?;
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
                DisplayOp::Translate(point) => {
                    let state = self.current_state_mut();
                    state.transform = state.transform.post_translate(point.x, point.y);
                }
                DisplayOp::Transform(matrix) => {
                    let transform = Transform::from_row(
                        matrix[0], matrix[1], matrix[4], matrix[5], matrix[12], matrix[13],
                    );
                    let state = self.current_state_mut();
                    state.transform = state.transform.post_concat(transform);
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
                    ..
                } => {
                    self.draw_text(text, *position, *size, *color, *bounds, *wrap, *underline)?;
                }
                DisplayOp::DrawRichText {
                    runs,
                    position,
                    bounds,
                    wrap,
                    ..
                } => {
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
                DisplayOp::DrawSurface {
                    rect,
                    surface_id,
                    position,
                    ..
                } => {
                    let color = surface_placeholder_color(*surface_id, *position);
                    self.draw_rect(*rect, Some(&Fill::Solid(color)), None, 0.0, None)?;
                }
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
        let surface = self.current_surface_mut();

        if let Some(shadow) = shadow {
            let shadow_rect = fission_render::LayoutRect::new(
                rect.origin.x + shadow.offset.0,
                rect.origin.y + shadow.offset.1,
                rect.size.width,
                rect.size.height,
            );
            if let Some(shadow_path) = if corner_radius > 0.0 {
                rounded_rect_path(shadow_rect, corner_radius)
            } else {
                rect_path(shadow_rect)
            } {
                let mut paint = Paint::default();
                let mut color = shadow.color;
                if shadow.blur_radius > 0.0 {
                    color.a = ((f32::from(color.a) * 0.65).round() as i32).clamp(0, 255) as u8;
                }
                paint.set_color(tiny_color(color));
                surface.fill_path(
                    &shadow_path,
                    &paint,
                    TinyFillRule::Winding,
                    transform,
                    clip.as_ref(),
                );
            }
        }

        if let Some(fill) = fill {
            let paint = fill_paint(fill);
            surface.fill_path(
                &path,
                &paint,
                TinyFillRule::Winding,
                transform,
                clip.as_ref(),
            );
        }
        if let Some(stroke) = stroke {
            let paint = fill_paint(&stroke.fill);
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
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: position.x,
            y: position.y,
            max_width: wrap_max_width(bounds.width(), size, wrap),
            ..LayoutSettings::default()
        });
        layout.append(&fonts, &FontdueTextStyle::new(text, size, 0));
        self.draw_glyphs(&layout, |_, _| color)?;
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
        let font = default_font();
        let fonts = [font];
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: position.x,
            y: position.y,
            max_width: wrap_max_width(
                bounds.width(),
                runs.first().map(|run| run.style.font_size).unwrap_or(14.0),
                wrap,
            ),
            ..LayoutSettings::default()
        });
        for run in runs {
            layout.append(
                &fonts,
                &fontdue::layout::TextStyle::with_user_data(
                    &run.text,
                    run.style.font_size,
                    0,
                    (
                        run.style.color,
                        run.style.underline,
                        run.style.background_color,
                    ),
                ),
            );
        }
        self.draw_glyphs(&layout, |glyph, (color, _underline, bg)| {
            if bg.is_some() {
                let _ = glyph;
            }
            *color
        })?;
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
        color_for: impl Fn(&fontdue::layout::GlyphPosition<U>, &U) -> RenderColor,
    ) -> Result<()> {
        let font = default_font();
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
        let image = match cached_image(request) {
            Some(image) => image,
            None => return Ok(()),
        };
        let rect_w = rect.width();
        let rect_h = rect.height();
        let img_w = image.width() as f32;
        let img_h = image.height() as f32;
        if rect_w <= 0.0 || rect_h <= 0.0 || img_w <= 0.0 || img_h <= 0.0 {
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
                .post_translate(dx, dy)
                .post_scale(scale_x, scale_y),
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
        let bez = match BezPath::from_svg(path) {
            Ok(path) => path,
            Err(_) => return Ok(()),
        };
        let path = match bez_to_tiny_path(&bez) {
            Some(path) => path,
            None => return Ok(()),
        };
        let transform = self.device_transform(
            self.current_state()
                .transform
                .post_translate(bounds.origin.x, bounds.origin.y),
        );
        let clip = self.current_clip().cloned();
        let surface = self.current_surface_mut();
        if let Some(fill) = fill {
            let paint = fill_paint(fill);
            surface.fill_path(
                &path,
                &paint,
                TinyFillRule::Winding,
                transform,
                clip.as_ref(),
            );
        }
        if let Some(stroke) = stroke {
            let paint = fill_paint(&stroke.fill);
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
        let entry = svg_cache_entry(content);
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
                .post_translate(dx, dy)
                .post_scale(scale, scale),
        );
        let clip = self.current_clip().cloned();
        let surface = self.current_surface_mut();

        for shape in &entry.shapes {
            match shape {
                SvgShape::Path(bez) => {
                    let Some(path) = bez_to_tiny_path(bez) else {
                        continue;
                    };
                    if let Some(fill) = fill {
                        let paint = fill_paint(fill);
                        surface.fill_path(
                            &path,
                            &paint,
                            TinyFillRule::Winding,
                            transform,
                            clip.as_ref(),
                        );
                    }
                    if let Some(stroke) = stroke {
                        let paint = fill_paint(&stroke.fill);
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
                    let Some(path) =
                        rect_path(fission_render::LayoutRect::new(*x, *y, *width, *height))
                    else {
                        continue;
                    };
                    if let Some(fill) = fill {
                        let paint = fill_paint(fill);
                        surface.fill_path(
                            &path,
                            &paint,
                            TinyFillRule::Winding,
                            transform,
                            clip.as_ref(),
                        );
                    }
                    if let Some(stroke) = stroke {
                        let paint = fill_paint(&stroke.fill);
                        let style = stroke_style(stroke);
                        surface.stroke_path(&path, &paint, &style, transform, clip.as_ref());
                    }
                }
            }
        }

        Ok(())
    }
}
