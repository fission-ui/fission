//! Driver-owned SVG document caching and Fission-paint geometry lowering.
//!
//! Document-paint SVGs stay as Skia `SvgDocument` handles. SVGs carrying a
//! Fission fill or stroke use the same deliberately bounded geometry profile
//! already claimed by the Vello and software renderers, then flow through the
//! ordinary Skia path and paint commands.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use fission_render::diagnostics::{CacheDiagnostics, DiagnosticCategory};
use fission_render::image_cache_store::ImageCacheStore;
use fission_render::{LayoutPoint, LayoutRect};
use fission_skia_sys::SvgDocument;
use kurbo::BezPath;

const CACHE_NAME: &str = "fission-render-skia-svg-documents";
const DEFAULT_SVG_CACHE_BYTES: u64 = 16 * 1024 * 1024;
const DOCUMENT_WEIGHT_MULTIPLIER: usize = 4;
const DOCUMENT_WEIGHT_OVERHEAD: usize = 4 * 1024;

#[derive(Debug, Clone)]
struct CachedSvgDocument {
    document: SvgDocument,
    cache_weight: u32,
}

/// One renderer session's disposable cache of parsed SkSVGDOM documents.
///
/// The complete source string is the cache key. Rust maps compare the key for
/// equality after hashing, so a digest collision can never select a different
/// document. The budget is a conservative source-derived cache weight because
/// SkSVGDOM does not expose retained allocation accounting.
pub(crate) struct SkiaSvgCache {
    store: ImageCacheStore<CachedSvgDocument>,
    budget_bytes: u64,
    evictions: Arc<AtomicU64>,
}

impl Default for SkiaSvgCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SkiaSvgCache {
    pub(crate) fn new() -> Self {
        Self::with_budget_bytes(configured_svg_cache_bytes())
    }

    pub(crate) fn with_budget_bytes(budget_bytes: u64) -> Self {
        let budget_bytes = budget_bytes.clamp(1, u64::from(u32::MAX));
        let evictions = Arc::new(AtomicU64::new(0));
        Self {
            store: document_store(budget_bytes, Arc::clone(&evictions)),
            budget_bytes,
            evictions,
        }
    }

    pub(crate) fn get_or_parse(&self, content: &str) -> Result<SvgDocument, SvgError> {
        if let Some(cached) = self.store.get(content) {
            return Ok(cached.document);
        }

        let cache_weight = document_cache_weight(content.len());
        if u64::from(cache_weight) > self.budget_bytes {
            return Err(SvgError::DocumentTooLarge {
                source_bytes: content.len(),
                cache_weight: u64::from(cache_weight),
                budget_bytes: self.budget_bytes,
            });
        }
        let document =
            SvgDocument::parse(content.as_bytes()).map_err(|error| SvgError::DocumentParse {
                message: error.to_string(),
            })?;
        debug_assert_eq!(document.source_bytes_len(), content.len());
        self.store.insert(
            content.to_owned(),
            CachedSvgDocument {
                document: document.clone(),
                cache_weight,
            },
        );
        Ok(document)
    }

    pub(crate) fn clear(&self) {
        self.store.invalidate_all();
        self.store.run_pending_tasks();
    }

    pub(crate) fn diagnostics(&self) -> CacheDiagnostics {
        self.store.run_pending_tasks();
        CacheDiagnostics {
            name: CACHE_NAME.into(),
            entries: self.store.entry_count(),
            used_bytes: self.store.weighted_size(),
            budget_bytes: Some(self.budget_bytes),
            evictions: self.evictions.load(Ordering::Acquire),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SvgGeometry {
    pub(crate) view_box: Option<(f64, f64, f64, f64)>,
    pub(crate) paths: Vec<BezPath>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SvgGeometryPlacement {
    pub(crate) source_bounds: LayoutRect,
    pub(crate) translation: LayoutPoint,
    pub(crate) scale: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SvgError {
    InvalidBounds,
    DocumentTooLarge {
        source_bytes: usize,
        cache_weight: u64,
        budget_bytes: u64,
    },
    DocumentParse {
        message: String,
    },
    GeometryParse {
        message: String,
    },
    NoVisibleGeometry,
    GeometryOverflow,
}

impl SvgError {
    pub(crate) fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::InvalidBounds => "skia-svg-bounds-invalid",
            Self::DocumentTooLarge { .. } => "skia-svg-document-too-large",
            Self::DocumentParse { .. } => "skia-svg-document-invalid",
            Self::GeometryParse { .. } => "skia-svg-geometry-invalid",
            Self::NoVisibleGeometry => "skia-svg-geometry-empty",
            Self::GeometryOverflow => "skia-svg-geometry-overflow",
        }
    }

    pub(crate) fn diagnostic_category(&self) -> DiagnosticCategory {
        match self {
            Self::InvalidBounds | Self::GeometryOverflow => DiagnosticCategory::Capability,
            Self::DocumentTooLarge { .. }
            | Self::DocumentParse { .. }
            | Self::GeometryParse { .. }
            | Self::NoVisibleGeometry => DiagnosticCategory::Resource,
        }
    }
}

impl fmt::Display for SvgError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBounds => formatter
                .write_str("Skia SVG bounds and edges must be finite with non-negative dimensions"),
            Self::DocumentTooLarge {
                source_bytes,
                cache_weight,
                budget_bytes,
            } => write!(
                formatter,
                "Skia SVG source has {source_bytes} bytes and cache weight {cache_weight}, exceeding the {budget_bytes}-byte document-cache budget"
            ),
            Self::DocumentParse { message } => {
                write!(formatter, "Skia could not parse the SVG document: {message}")
            }
            Self::GeometryParse { message } => write!(
                formatter,
                "Skia could not lower SVG geometry for Fission paint: {message}"
            ),
            Self::NoVisibleGeometry => formatter.write_str(
                "Skia SVG geometry lowering found no supported visible path, rectangle, or polygon",
            ),
            Self::GeometryOverflow => formatter
                .write_str("Skia SVG geometry overflows the finite Fission coordinate range"),
        }
    }
}

impl std::error::Error for SvgError {}

pub(crate) fn validate_svg_bounds(bounds: LayoutRect) -> Result<(), SvgError> {
    let values = [
        bounds.x(),
        bounds.y(),
        bounds.width(),
        bounds.height(),
        bounds.right(),
        bounds.bottom(),
    ];
    if values.iter().all(|value| value.is_finite())
        && bounds.width() >= 0.0
        && bounds.height() >= 0.0
    {
        Ok(())
    } else {
        Err(SvgError::InvalidBounds)
    }
}

pub(crate) fn place_svg_geometry(
    geometry: &SvgGeometry,
    bounds: LayoutRect,
) -> Result<SvgGeometryPlacement, SvgError> {
    validate_svg_bounds(bounds)?;
    let (vb_x, vb_y, vb_width, vb_height) = geometry.view_box.unwrap_or((
        0.0,
        0.0,
        f64::from(bounds.width()),
        f64::from(bounds.height()),
    ));
    let bounds_width = f64::from(bounds.width());
    let bounds_height = f64::from(bounds.height());
    let (scale, translate_x, translate_y) =
        if vb_width > 0.0 && vb_height > 0.0 && bounds_width > 0.0 && bounds_height > 0.0 {
            let scale = (bounds_width / vb_width).min(bounds_height / vb_height);
            let scaled_width = vb_width * scale;
            let scaled_height = vb_height * scale;
            (
                scale,
                f64::from(bounds.x()) + (bounds_width - scaled_width) / 2.0 - vb_x * scale,
                f64::from(bounds.y()) + (bounds_height - scaled_height) / 2.0 - vb_y * scale,
            )
        } else {
            (1.0, f64::from(bounds.x()), f64::from(bounds.y()))
        };

    let source_bounds = LayoutRect::new(
        finite_f32(vb_x)?,
        finite_f32(vb_y)?,
        finite_f32(vb_width)?,
        finite_f32(vb_height)?,
    );
    if !source_bounds.right().is_finite() || !source_bounds.bottom().is_finite() {
        return Err(SvgError::GeometryOverflow);
    }
    let scale = finite_f32(scale)?;
    if scale <= 0.0 {
        return Err(SvgError::GeometryOverflow);
    }
    Ok(SvgGeometryPlacement {
        source_bounds,
        translation: LayoutPoint::new(finite_f32(translate_x)?, finite_f32(translate_y)?),
        scale,
    })
}

/// Parses the existing cross-backend `GeometryWithFissionPaint` SVG subset.
pub(crate) fn parse_svg_geometry(content: &str) -> Result<SvgGeometry, SvgError> {
    let mut view_box = None;
    let mut paths = Vec::new();

    for tag in content.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or("");
        let tag_name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('/');

        match tag_name {
            "path" => {
                reject_transform(tag, "path")?;
                let data = quoted_attribute(tag, "d")?.ok_or_else(|| {
                    geometry_error("path element is missing a quoted d attribute")
                })?;
                let data = strip_transparent_icon_bounds(data);
                if data.trim().is_empty() {
                    continue;
                }
                let path = BezPath::from_svg(&data).map_err(|error| {
                    geometry_error(format!("path element contains invalid d data: {error}"))
                })?;
                paths.push(path);
            }
            "rect" => {
                if quoted_attribute(tag, "fill")?.is_some_and(|fill| fill == "none") {
                    continue;
                }
                reject_transform(tag, "rect")?;
                let x = numeric_attribute(tag, "x", 0.0)?;
                let y = numeric_attribute(tag, "y", 0.0)?;
                let width = numeric_attribute(tag, "width", 0.0)?;
                let height = numeric_attribute(tag, "height", 0.0)?;
                if width < 0.0 || height < 0.0 {
                    return Err(geometry_error(
                        "rect width and height attributes must not be negative",
                    ));
                }
                if width > 0.0 && height > 0.0 {
                    paths.push(rectangle_path(x, y, width, height)?);
                }
            }
            "polygon" => {
                reject_transform(tag, "polygon")?;
                let points = quoted_attribute(tag, "points")?.ok_or_else(|| {
                    geometry_error("polygon element is missing a quoted points attribute")
                })?;
                paths.push(polygon_path(points)?);
            }
            "svg" => {
                reject_transform(tag, "svg")?;
                if let Some(value) = quoted_attribute(tag, "viewBox")? {
                    view_box = Some(parse_view_box(value)?);
                }
            }
            "g" => reject_transform(tag, "g")?,
            "" | "title" | "desc" => {}
            name if name.starts_with('/') || name.starts_with('!') || name.starts_with('?') => {}
            unsupported => {
                return Err(geometry_error(format!(
                    "unsupported SVG element <{unsupported}>"
                )))
            }
        }
    }

    if paths.is_empty() {
        return Err(SvgError::NoVisibleGeometry);
    }
    Ok(SvgGeometry { view_box, paths })
}

fn quoted_attribute<'a>(tag: &'a str, name: &str) -> Result<Option<&'a str>, SvgError> {
    let mut search_from = 0;
    let start = loop {
        let Some(relative_start) = tag[search_from..].find(name) else {
            return Ok(None);
        };
        let start = search_from + relative_start;
        let before_is_boundary = start == 0
            || tag[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        let after_name = start + name.len();
        let after_is_boundary = after_name == tag.len()
            || tag[after_name..]
                .chars()
                .next()
                .is_some_and(|character| character == '=' || character.is_whitespace());
        if before_is_boundary && after_is_boundary {
            break start;
        }
        search_from = after_name;
    };

    let after_name = tag[start + name.len()..].trim_start();
    let after_equals = after_name
        .strip_prefix('=')
        .ok_or_else(|| geometry_error(format!("{name} attribute is missing '='")))?;
    let value = after_equals.trim_start();
    let quote = value
        .chars()
        .next()
        .filter(|quote| matches!(quote, '\'' | '"'))
        .ok_or_else(|| geometry_error(format!("{name} attribute must use a quoted value")))?;
    let value = &value[quote.len_utf8()..];
    let end = value.find(quote).ok_or_else(|| {
        geometry_error(format!("{name} attribute has an unterminated quoted value"))
    })?;
    Ok(Some(&value[..end]))
}

fn numeric_attribute(tag: &str, name: &str, default: f64) -> Result<f64, SvgError> {
    let Some(value) = quoted_attribute(tag, name)? else {
        return Ok(default);
    };
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| geometry_error(format!("{name} attribute must be a finite number")))
}

fn reject_transform(tag: &str, element: &str) -> Result<(), SvgError> {
    if quoted_attribute(tag, "transform")?.is_some() {
        Err(geometry_error(format!(
            "transform attributes on <{element}> are not supported by Fission-paint SVG geometry"
        )))
    } else {
        Ok(())
    }
}

fn parse_view_box(value: &str) -> Result<(f64, f64, f64, f64), SvgError> {
    let numbers = value
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>();
    match numbers {
        Ok(numbers)
            if numbers.len() == 4
                && numbers.iter().all(|value| value.is_finite())
                && numbers[2] >= 0.0
                && numbers[3] >= 0.0
                && (numbers[0] + numbers[2]).is_finite()
                && (numbers[1] + numbers[3]).is_finite() =>
        {
            Ok((numbers[0], numbers[1], numbers[2], numbers[3]))
        }
        _ => Err(geometry_error(
            "viewBox must contain four finite numeric values with non-negative dimensions",
        )),
    }
}

fn polygon_path(points: &str) -> Result<BezPath, SvgError> {
    let numbers = points
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>();
    let numbers = match numbers {
        Ok(numbers)
            if numbers.len() >= 4
                && numbers.len() % 2 == 0
                && numbers.iter().all(|value| value.is_finite()) =>
        {
            numbers
        }
        _ => {
            return Err(geometry_error(
                "polygon points must be finite numeric coordinate pairs",
            ))
        }
    };

    let mut path = BezPath::new();
    path.move_to((numbers[0], numbers[1]));
    for point in numbers[2..].chunks_exact(2) {
        path.line_to((point[0], point[1]));
    }
    path.close_path();
    Ok(path)
}

fn rectangle_path(x: f64, y: f64, width: f64, height: f64) -> Result<BezPath, SvgError> {
    let right = x + width;
    let bottom = y + height;
    if !right.is_finite() || !bottom.is_finite() {
        return Err(SvgError::GeometryOverflow);
    }
    let mut path = BezPath::new();
    path.move_to((x, y));
    path.line_to((right, y));
    path.line_to((right, bottom));
    path.line_to((x, bottom));
    path.close_path();
    Ok(path)
}

fn strip_transparent_icon_bounds(data: &str) -> String {
    data.replace("M0 0h24v24H0z", "")
        .replace("M0 0h24v24H0V0z", "")
        .replace("M0,0h24v24H0V0z", "")
}

fn finite_f32(value: f64) -> Result<f32, SvgError> {
    let value = value as f32;
    value
        .is_finite()
        .then_some(value)
        .ok_or(SvgError::GeometryOverflow)
}

fn geometry_error(message: impl Into<String>) -> SvgError {
    SvgError::GeometryParse {
        message: message.into(),
    }
}

fn document_store(
    budget_bytes: u64,
    evictions: Arc<AtomicU64>,
) -> ImageCacheStore<CachedSvgDocument> {
    ImageCacheStore::new(
        CACHE_NAME,
        budget_bytes,
        |entry: &CachedSvgDocument| entry.cache_weight,
        move || {
            evictions.fetch_add(1, Ordering::AcqRel);
        },
    )
}

fn document_cache_weight(source_bytes: usize) -> u32 {
    source_bytes
        .saturating_mul(DOCUMENT_WEIGHT_MULTIPLIER)
        .saturating_add(DOCUMENT_WEIGHT_OVERHEAD)
        .clamp(1, u32::MAX as usize) as u32
}

fn configured_svg_cache_bytes() -> u64 {
    configured_svg_cache_bytes_from(std::env::var("FISSION_SVG_CACHE_BYTES").ok().as_deref())
}

fn configured_svg_cache_bytes_from(value: Option<&str>) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SVG_CACHE_BYTES)
        .min(u64::from(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::PathEl;

    #[test]
    fn geometry_profile_matches_existing_paths_rectangles_polygons_and_view_box() {
        let geometry = parse_svg_geometry(
            r#"<svg viewBox='0 0 24 12'><path d='M0 0 L4 4 Z'/><rect x='5' y='2' width='3' height='4'/><polygon points='10,1 12,3 9,4'/></svg>"#,
        )
        .unwrap();

        assert_eq!(geometry.view_box, Some((0.0, 0.0, 24.0, 12.0)));
        assert_eq!(geometry.paths.len(), 3);
        assert!(matches!(geometry.paths[0].elements()[0], PathEl::MoveTo(_)));
        assert!(matches!(
            geometry.paths[1].elements().last(),
            Some(PathEl::ClosePath)
        ));
        assert!(matches!(
            geometry.paths[2].elements().last(),
            Some(PathEl::ClosePath)
        ));
    }

    #[test]
    fn geometry_profile_rejects_unsupported_elements_and_transforms() {
        let unsupported =
            parse_svg_geometry("<svg><circle cx='2' cy='2' r='2'/></svg>").unwrap_err();
        assert!(matches!(unsupported, SvgError::GeometryParse { .. }));
        assert!(unsupported
            .to_string()
            .contains("unsupported SVG element <circle>"));

        let transform =
            parse_svg_geometry("<svg><g transform='scale(2)'><path d='M0 0L1 1'/></g></svg>")
                .unwrap_err();
        assert!(transform
            .to_string()
            .contains("transform attributes on <g>"));
    }

    #[test]
    fn view_box_is_centered_with_aspect_fit_like_existing_renderers() {
        let geometry = SvgGeometry {
            view_box: Some((0.0, 0.0, 200.0, 100.0)),
            paths: vec![rectangle_path(0.0, 0.0, 200.0, 100.0).unwrap()],
        };

        let placement =
            place_svg_geometry(&geometry, LayoutRect::new(10.0, 20.0, 100.0, 100.0)).unwrap();

        assert_eq!(
            placement.source_bounds,
            LayoutRect::new(0.0, 0.0, 200.0, 100.0)
        );
        assert_eq!(placement.translation, LayoutPoint::new(10.0, 45.0));
        assert_eq!(placement.scale, 0.5);
    }

    #[test]
    fn invalid_bounds_and_cache_configuration_have_stable_contracts() {
        assert_eq!(
            validate_svg_bounds(LayoutRect::new(0.0, 0.0, -1.0, 2.0)),
            Err(SvgError::InvalidBounds)
        );
        assert_eq!(
            validate_svg_bounds(LayoutRect::new(f32::MAX, 0.0, f32::MAX, 2.0)),
            Err(SvgError::InvalidBounds)
        );
        assert_eq!(configured_svg_cache_bytes_from(None), 16 * 1024 * 1024);
        assert_eq!(configured_svg_cache_bytes_from(Some("0")), 16 * 1024 * 1024);
        assert_eq!(configured_svg_cache_bytes_from(Some("8192")), 8192);
        assert_eq!(document_cache_weight(1), 4100);
    }

    #[test]
    fn oversized_documents_are_rejected_before_native_parse() {
        let cache = SkiaSvgCache::with_budget_bytes(4_096);

        let error = cache.get_or_parse("x").unwrap_err();

        assert!(matches!(
            error,
            SvgError::DocumentTooLarge {
                source_bytes: 1,
                cache_weight: 4_100,
                budget_bytes: 4_096,
            }
        ));
        assert_eq!(error.diagnostic_code(), "skia-svg-document-too-large");
    }

    #[cfg(feature = "test-shim")]
    #[test]
    fn document_cache_reuses_exact_source_without_digest_collisions() {
        let first_content = "<svg viewBox='0 0 2 1'><rect width='2' height='1'/></svg>";
        let second_content = "<svg viewBox='0 0 3 1'><rect width='3' height='1'/></svg>";
        let cache = SkiaSvgCache::with_budget_bytes(16_384);

        let first = cache.get_or_parse(first_content).unwrap();
        let reused = cache.get_or_parse(first_content).unwrap();
        let second = cache.get_or_parse(second_content).unwrap();
        let diagnostics = cache.diagnostics();

        assert_eq!(first, reused);
        assert_ne!(first, second);
        assert_eq!(first.source_bytes_len(), first_content.len());
        assert_eq!(second.source_bytes_len(), second_content.len());
        assert_eq!(diagnostics.name, CACHE_NAME);
        assert_eq!(diagnostics.entries, 2);
        assert_eq!(
            diagnostics.used_bytes,
            u64::from(document_cache_weight(first_content.len()))
                + u64::from(document_cache_weight(second_content.len()))
        );
        assert_eq!(diagnostics.budget_bytes, Some(16_384));

        cache.clear();
        let cleared = cache.diagnostics();
        assert_eq!(cleared.entries, 0);
        assert_eq!(cleared.used_bytes, 0);
    }
}
