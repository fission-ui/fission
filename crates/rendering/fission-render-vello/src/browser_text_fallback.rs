//! Browser-native glyph fallback for the Web renderer.
//!
//! Fontique cannot enumerate fonts installed on a browser host. When Parley emits a
//! `.notdef` glyph, use the browser's Canvas 2D text stack for that run. The browser
//! can apply its own script and emoji fallback without embedding a large CJK or emoji
//! font in every WebAssembly app.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;
use vello_cpu::peniko::ImageAlphaType;
use vello_cpu::{PixelMetadata, Pixmap};
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub(crate) const RASTER_PADDING: f32 = 1.0;
const MAX_CANVAS_DIMENSION: f32 = 16_384.0;
const MAX_CACHED_RUNS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    text: String,
    font_size: u32,
    width: u32,
    ascent: u32,
    descent: u32,
    scale: u32,
    color: [u8; 4],
    family: String,
    weight: u16,
    italic: bool,
}

lazy_static! {
    static ref RASTER_CACHE: Mutex<HashMap<CacheKey, Arc<Pixmap>>> = Mutex::new(HashMap::new());
}

/// Renders one unsupported text run through the browser's native font fallback.
///
/// The returned image includes one logical pixel of transparent padding on each
/// side. `width`, `ascent`, and `descent` come from Parley's layout, so browser
/// fallback glyphs stay within the space already allocated by the text layout.
pub(crate) fn rasterize(
    text: &str,
    font_size: f32,
    width: f32,
    ascent: f32,
    descent: f32,
    scale: f32,
    color: [u8; 4],
    family: &str,
    weight: u16,
    italic: bool,
) -> Option<Arc<Pixmap>> {
    if text.is_empty()
        || !font_size.is_finite()
        || !width.is_finite()
        || !ascent.is_finite()
        || !descent.is_finite()
        || !scale.is_finite()
        || font_size <= 0.0
        || width <= 0.0
        || scale <= 0.0
    {
        return None;
    }

    let ascent = ascent.max(font_size * 0.5);
    let descent = descent.max(font_size * 0.1);
    let logical_width = width + RASTER_PADDING * 2.0;
    let logical_height = ascent + descent + RASTER_PADDING * 2.0;
    let pixel_width = pixel_dimension(logical_width, scale)?;
    let pixel_height = pixel_dimension(logical_height, scale)?;
    let key = CacheKey {
        text: text.to_owned(),
        font_size: font_size.to_bits(),
        width: width.to_bits(),
        ascent: ascent.to_bits(),
        descent: descent.to_bits(),
        scale: scale.to_bits(),
        color,
        family: family.to_owned(),
        weight,
        italic,
    };

    if let Some(image) = RASTER_CACHE.lock().ok()?.get(&key).cloned() {
        return Some(image);
    }

    let image = rasterize_uncached(
        text,
        font_size,
        width,
        ascent,
        scale,
        pixel_width,
        pixel_height,
        color,
        family,
        weight,
        italic,
    )?;
    let image = Arc::new(image);

    let mut cache = RASTER_CACHE.lock().ok()?;
    if cache.len() >= MAX_CACHED_RUNS {
        if let Some(oldest_key) = cache.keys().next().cloned() {
            cache.remove(&oldest_key);
        }
    }
    cache.insert(key, Arc::clone(&image));
    Some(image)
}

fn pixel_dimension(logical: f32, scale: f32) -> Option<u16> {
    let pixels = (logical * scale).ceil();
    (pixels.is_finite() && pixels > 0.0 && pixels <= MAX_CANVAS_DIMENSION).then_some(pixels as u16)
}

#[allow(clippy::too_many_arguments)]
fn rasterize_uncached(
    text: &str,
    font_size: f32,
    width: f32,
    ascent: f32,
    scale: f32,
    pixel_width: u16,
    pixel_height: u16,
    color: [u8; 4],
    family: &str,
    weight: u16,
    italic: bool,
) -> Option<Pixmap> {
    let document = web_sys::window()?.document()?;
    let canvas = document
        .create_element("canvas")
        .ok()?
        .dyn_into::<HtmlCanvasElement>()
        .ok()?;
    canvas.set_width(u32::from(pixel_width));
    canvas.set_height(u32::from(pixel_height));
    let context = canvas
        .get_context("2d")
        .ok()??
        .dyn_into::<CanvasRenderingContext2d>()
        .ok()?;

    context.scale(f64::from(scale), f64::from(scale)).ok()?;
    context.set_text_baseline("alphabetic");
    context.set_text_align("left");
    context.set_font(&format!(
        "{} {} {}px {}, sans-serif, emoji",
        if italic { "italic" } else { "normal" },
        weight.clamp(100, 900),
        font_size,
        css_font_families(family),
    ));
    context.set_fill_style_str(&format!(
        "rgba({}, {}, {}, {})",
        color[0],
        color[1],
        color[2],
        f32::from(color[3]) / 255.0,
    ));
    context
        .fill_text_with_max_width(
            text,
            f64::from(RASTER_PADDING),
            f64::from(ascent + RASTER_PADDING),
            f64::from(width),
        )
        .ok()?;

    let rgba = context
        .get_image_data(0.0, 0.0, f64::from(pixel_width), f64::from(pixel_height))
        .ok()?
        .data()
        .0;
    Some(Pixmap::from_parts(
        rgba,
        pixel_width,
        pixel_height,
        PixelMetadata::new(ImageAlphaType::Alpha, true),
    ))
}

fn css_font_families(families: &str) -> String {
    let mut parsed = families
        .split(',')
        .map(str::trim)
        .filter(|family| !family.is_empty())
        .map(|family| {
            if matches!(
                family.to_ascii_lowercase().as_str(),
                "serif"
                    | "sans-serif"
                    | "monospace"
                    | "cursive"
                    | "fantasy"
                    | "system-ui"
                    | "ui-serif"
                    | "ui-sans-serif"
                    | "ui-monospace"
                    | "ui-rounded"
                    | "emoji"
            ) {
                family.to_owned()
            } else {
                format!("\"{}\"", family.replace('\\', "\\\\").replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        parsed.push("sans-serif".to_owned());
    }
    parsed.join(", ")
}
