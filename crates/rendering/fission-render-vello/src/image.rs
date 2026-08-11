use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;

use fission_ir::op::{HttpHeader, ImageAlignment, ImageRequest, ImageSource};
use fission_render::image_cache_store::ImageCacheStore;
use lazy_static::lazy_static;
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};

use crate::renderer::VelloRenderer;

const DEFAULT_IMAGE_CACHE_BYTES: u64 = 50 * 1024 * 1024;

lazy_static! {
    pub(crate) static ref IMAGE_CACHE: ImageCacheStore<ImageCacheEntry> = build_image_cache();
}

static IMAGE_CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMAGE_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMAGE_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMAGE_LOADS_STARTED: AtomicU64 = AtomicU64::new(0);
static IMAGE_LOADS_COMPLETED: AtomicU64 = AtomicU64::new(0);
static IMAGE_LOADS_FAILED: AtomicU64 = AtomicU64::new(0);
static IMAGE_CACHE_EVICTIONS: AtomicU64 = AtomicU64::new(0);
pub(crate) static IMAGE_OFFSCREEN_SKIPS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(crate) enum ImageCacheEntry {
    Ready(Arc<ImageData>),
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

fn image_byte_len(image: &ImageData) -> u64 {
    u64::from(image.width)
        .saturating_mul(u64::from(image.height))
        .saturating_mul(4)
}

pub(crate) fn image_request_with_default_cache_size(
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

pub(crate) fn aligned_offset(
    extra_width: f64,
    extra_height: f64,
    alignment: ImageAlignment,
) -> (f64, f64) {
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
pub(crate) fn spawn_image_load(key: String, request: ImageRequest) {
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
pub(crate) fn spawn_image_load(key: String, request: ImageRequest) {
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
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
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

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
    }
}
