//! Safe, owned access to the batched native SkParagraph ABI.

use std::mem;
use std::ptr;
use std::slice;

use crate::error::status_result;
use crate::{ffi, BuildInfo, Error, ErrorKind, Result, ABI_VERSION, SKIA_REVISION};

pub use ffi::{
    ParagraphCaret, ParagraphCluster, ParagraphHitRegion, ParagraphInlineBox, ParagraphLine,
    ParagraphRect, ParagraphSize, UnresolvedGlyph,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ParagraphRange {
    pub start: u64,
    pub end: u64,
}

impl ParagraphRange {
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ParagraphColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl ParagraphColor {
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParagraphFontVariation {
    pub tag: u32,
    pub value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParagraphFontFeature {
    pub tag: u32,
    pub value: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ParagraphFontSlant {
    #[default]
    Normal,
    Italic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphTextStyleRun {
    pub range: ParagraphRange,
    pub font_size: f32,
    pub color: ParagraphColor,
    pub underline: bool,
    pub font_family: Option<String>,
    pub locale: Option<String>,
    pub font_weight: u16,
    pub font_slant: ParagraphFontSlant,
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
    pub background_color: Option<ParagraphColor>,
    pub font_width: f32,
    pub word_spacing: f32,
    pub variations: Vec<ParagraphFontVariation>,
    pub features: Vec<ParagraphFontFeature>,
}

impl ParagraphTextStyleRun {
    pub fn new(range: ParagraphRange, font_size: f32, color: ParagraphColor) -> Self {
        Self {
            range,
            font_size,
            color,
            underline: false,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_slant: ParagraphFontSlant::Normal,
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
            font_width: 1.0,
            word_spacing: 0.0,
            variations: Vec::new(),
            features: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ParagraphTextAlign {
    #[default]
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ParagraphOverflow {
    #[default]
    Clip,
    Ellipsis,
    Fade,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ParagraphTextDirection {
    #[default]
    Auto,
    LeftToRight,
    RightToLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ParagraphTextWidthBasis {
    #[default]
    Parent,
    LongestLine,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParagraphStyle {
    pub text_align: ParagraphTextAlign,
    pub max_lines: Option<u64>,
    pub overflow: ParagraphOverflow,
    pub text_direction: ParagraphTextDirection,
    pub text_width_basis: ParagraphTextWidthBasis,
    pub strut_line_height: Option<f32>,
    pub apply_height_to_first_ascent: bool,
    pub apply_height_to_last_descent: bool,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            text_align: ParagraphTextAlign::Left,
            max_lines: None,
            overflow: ParagraphOverflow::Clip,
            text_direction: ParagraphTextDirection::Auto,
            text_width_basis: ParagraphTextWidthBasis::Parent,
            strut_line_height: None,
            apply_height_to_first_ascent: true,
            apply_height_to_last_descent: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParagraphInlineObject {
    pub id: u64,
    pub range: ParagraphRange,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParagraphPreedit {
    pub range: ParagraphRange,
    pub selection: ParagraphRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphRequest {
    pub text: String,
    pub style_runs: Vec<ParagraphTextStyleRun>,
    pub paragraph_style: ParagraphStyle,
    pub width_constraint: Option<f32>,
    pub wrap: bool,
    pub locale: Option<String>,
    pub inline_objects: Vec<ParagraphInlineObject>,
    pub selection: Option<ParagraphRange>,
    pub preedit: Option<ParagraphPreedit>,
    pub font_catalog_generation: u64,
    pub fallback_families: Vec<String>,
}

impl ParagraphRequest {
    pub fn new(text: impl Into<String>, style_runs: Vec<ParagraphTextStyleRun>) -> Self {
        Self {
            text: text.into(),
            style_runs,
            paragraph_style: ParagraphStyle::default(),
            width_constraint: None,
            wrap: true,
            locale: None,
            inline_objects: Vec::new(),
            selection: None,
            preedit: None,
            font_catalog_generation: 0,
            fallback_families: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParagraphCapabilities(u64);

impl ParagraphCapabilities {
    pub const BIDIRECTIONAL_TEXT: u64 = ffi::PARAGRAPH_BIDIRECTIONAL_TEXT;
    pub const VARIABLE_FONTS: u64 = ffi::PARAGRAPH_VARIABLE_FONTS;
    pub const FONT_FEATURES: u64 = ffi::PARAGRAPH_FONT_FEATURES;
    pub const INLINE_OBJECTS: u64 = ffi::PARAGRAPH_INLINE_OBJECTS;
    pub const CLUSTER_MAPPING: u64 = ffi::PARAGRAPH_CLUSTER_MAPPING;
    pub const HIT_TESTING: u64 = ffi::PARAGRAPH_HIT_TESTING;
    pub const CARET_GEOMETRY: u64 = ffi::PARAGRAPH_CARET_GEOMETRY;
    pub const SELECTION_GEOMETRY: u64 = ffi::PARAGRAPH_SELECTION_GEOMETRY;
    pub const UNRESOLVED_GLYPHS: u64 = ffi::PARAGRAPH_UNRESOLVED_GLYPHS;

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn supports(self, capability: u64) -> bool {
        self.0 & capability == capability
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParagraphDirection {
    LeftToRight,
    RightToLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParagraphAffinity {
    Upstream,
    Downstream,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphOutput {
    pub index_encoding: u32,
    pub capabilities: ParagraphCapabilities,
    pub size: ParagraphSize,
    pub min_intrinsic_width: f32,
    pub max_intrinsic_width: f32,
    pub first_baseline: Option<f32>,
    pub last_baseline: Option<f32>,
    pub lines: Box<[ParagraphLine]>,
    pub clusters: Box<[ParagraphCluster]>,
    pub carets: Box<[ParagraphCaret]>,
    pub hit_regions: Box<[ParagraphHitRegion]>,
    pub inline_boxes: Box<[ParagraphInlineBox]>,
    pub unresolved_glyphs: Box<[UnresolvedGlyph]>,
    pub unresolved_codepoints: Box<[u32]>,
}

/// Stateless, thread-safe entrypoint to the synchronous native paragraph ABI.
///
/// Native Skia objects are created and consumed entirely inside each call. No
/// native pointer or thread-affine graphics handle is retained by this value.
#[derive(Debug, Clone, Copy)]
pub struct ParagraphEngine {
    capabilities: ParagraphCapabilities,
}

impl ParagraphEngine {
    pub fn new() -> Result<Self> {
        let info = BuildInfo::query()?;
        if info.abi_version != ABI_VERSION || info.skia_revision != SKIA_REVISION {
            return Err(Error::local(
                ErrorKind::AbiMismatch,
                "ParagraphEngine::new",
                "paragraph bridge identity does not match the pinned Fission Skia ABI",
            ));
        }
        if info.feature_bits & ffi::FEATURE_PARAGRAPH == 0 {
            return Err(Error::local(
                ErrorKind::Unsupported,
                "ParagraphEngine::new",
                "bridge does not advertise the paragraph feature",
            ));
        }
        let mut capabilities = 0;
        let mut error = ffi::Error::default();
        // SAFETY: both outputs are initialized and valid for the call.
        let status =
            unsafe { ffi::fission_skia_paragraph_capabilities(&mut capabilities, &mut error) };
        status_result(status, &error)?;
        if capabilities & !ffi::PARAGRAPH_ALL_CAPABILITIES != 0 {
            return Err(Error::local(
                ErrorKind::AbiMismatch,
                "ParagraphEngine::new",
                format!("bridge returned unknown paragraph capability bits 0x{capabilities:016x}"),
            ));
        }
        Ok(Self {
            capabilities: ParagraphCapabilities(capabilities),
        })
    }

    pub const fn capabilities(&self) -> ParagraphCapabilities {
        self.capabilities
    }

    pub fn layout(&self, request: &ParagraphRequest) -> Result<ParagraphOutput> {
        let encoded = EncodedRequest::new(request)?;
        let mut handle = 0;
        let mut error = ffi::Error::default();
        // SAFETY: encoded owns every nested pointer in its raw request until
        // the synchronous call returns. The output handle is valid for writes.
        let status =
            unsafe { ffi::fission_skia_paragraph_layout(&encoded.raw, &mut handle, &mut error) };
        status_result(status, &error)?;
        if handle == 0 {
            return Err(Error::local(
                ErrorKind::Internal,
                "ParagraphEngine::layout",
                "bridge reported success with a null paragraph result handle",
            ));
        }
        let result = RawResult(handle);
        let mut view = ffi::ParagraphResultView::default();
        let mut error = ffi::Error::default();
        // SAFETY: result owns a live handle and view is initialized for writes.
        let status =
            unsafe { ffi::fission_skia_paragraph_result_get_view(result.0, &mut view, &mut error) };
        status_result(status, &error)?;
        if view.index_encoding != ffi::INDEX_UTF8 {
            return Err(Error::local(
                ErrorKind::AbiMismatch,
                "ParagraphEngine::layout",
                format!(
                    "bridge returned undeclared paragraph index encoding {}",
                    view.index_encoding
                ),
            ));
        }
        if view.capabilities != self.capabilities.bits() {
            return Err(Error::local(
                ErrorKind::AbiMismatch,
                "ParagraphEngine::layout",
                "paragraph result capability mask differs from the queried bridge mask",
            ));
        }
        // SAFETY: the opaque result keeps every scalar array alive and immutable
        // until `result` is dropped after these copies complete.
        unsafe { owned_output(&view) }
    }
}

struct RawResult(ffi::ParagraphResultHandle);

impl Drop for RawResult {
    fn drop(&mut self) {
        let mut error = ffi::Error::default();
        // SAFETY: this guard is the sole safe owner and destroys the handle once.
        let status = unsafe { ffi::fission_skia_paragraph_result_destroy(self.0, &mut error) };
        debug_assert_eq!(
            status,
            ffi::STATUS_OK,
            "paragraph result destruction failed"
        );
    }
}

struct EncodedRequest {
    raw: ffi::ParagraphRequest,
    _variations: Vec<Box<[ffi::FontVariation]>>,
    _features: Vec<Box<[ffi::FontFeature]>>,
    _style_runs: Box<[ffi::TextStyleRun]>,
    _inline_objects: Box<[ffi::InlineObject]>,
    _fallback_families: Box<[ffi::Utf8Slice]>,
}

impl EncodedRequest {
    fn new(request: &ParagraphRequest) -> Result<Self> {
        if request.text.len() > u64::MAX as usize {
            return Err(invalid_request(
                "text length exceeds the native u64 contract",
            ));
        }
        let mut variations = Vec::with_capacity(request.style_runs.len());
        let mut features = Vec::with_capacity(request.style_runs.len());
        for style in &request.style_runs {
            variations.push(
                style
                    .variations
                    .iter()
                    .map(|value| ffi::FontVariation {
                        tag: value.tag,
                        value: value.value,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
            features.push(
                style
                    .features
                    .iter()
                    .map(|value| ffi::FontFeature {
                        tag: value.tag,
                        value: value.value,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        }
        let style_runs = request
            .style_runs
            .iter()
            .enumerate()
            .map(|(index, style)| {
                let mut flags = 0;
                if style.underline {
                    flags |= ffi::TEXT_STYLE_UNDERLINE;
                }
                if style.line_height.is_some() {
                    flags |= ffi::TEXT_STYLE_HAS_LINE_HEIGHT;
                }
                if style.background_color.is_some() {
                    flags |= ffi::TEXT_STYLE_HAS_BACKGROUND;
                }
                ffi::TextStyleRun {
                    struct_size: mem::size_of::<ffi::TextStyleRun>() as u32,
                    flags,
                    range: raw_range(style.range),
                    font_size: style.font_size,
                    color: raw_color(style.color),
                    font_family: optional_utf8(style.font_family.as_deref()),
                    locale: optional_utf8(style.locale.as_deref()),
                    font_weight: style.font_weight,
                    font_slant: match style.font_slant {
                        ParagraphFontSlant::Normal => ffi::FONT_SLANT_NORMAL,
                        ParagraphFontSlant::Italic => ffi::FONT_SLANT_ITALIC,
                    },
                    line_height: style.line_height.unwrap_or(0.0),
                    letter_spacing: style.letter_spacing,
                    background_color: raw_color(style.background_color.unwrap_or_default()),
                    font_width: style.font_width,
                    word_spacing: style.word_spacing,
                    variations: variations[index].as_ptr(),
                    variation_count: variations[index].len(),
                    features: features[index].as_ptr(),
                    feature_count: features[index].len(),
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let inline_objects = request
            .inline_objects
            .iter()
            .map(|value| ffi::InlineObject {
                struct_size: mem::size_of::<ffi::InlineObject>() as u32,
                reserved: 0,
                id: value.id,
                range: raw_range(value.range),
                width: value.width,
                height: value.height,
                baseline: value.baseline,
                reserved_scalar: 0.0,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let fallback_families = request
            .fallback_families
            .iter()
            .map(|value| utf8(value))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut paragraph_flags = 0;
        if request.paragraph_style.max_lines.is_some() {
            paragraph_flags |= ffi::PARAGRAPH_STYLE_HAS_MAX_LINES;
        }
        if request.paragraph_style.strut_line_height.is_some() {
            paragraph_flags |= ffi::PARAGRAPH_STYLE_HAS_STRUT_HEIGHT;
        }
        if request.paragraph_style.apply_height_to_first_ascent {
            paragraph_flags |= ffi::PARAGRAPH_STYLE_APPLY_FIRST_ASCENT;
        }
        if request.paragraph_style.apply_height_to_last_descent {
            paragraph_flags |= ffi::PARAGRAPH_STYLE_APPLY_LAST_DESCENT;
        }
        let paragraph_style = ffi::ParagraphStyle {
            struct_size: mem::size_of::<ffi::ParagraphStyle>() as u32,
            flags: paragraph_flags,
            text_align: text_align(request.paragraph_style.text_align),
            overflow: overflow(request.paragraph_style.overflow),
            text_direction: text_direction(request.paragraph_style.text_direction),
            text_width_basis: text_width_basis(request.paragraph_style.text_width_basis),
            max_lines: request.paragraph_style.max_lines.unwrap_or(0),
            strut_line_height: request.paragraph_style.strut_line_height.unwrap_or(0.0),
            reserved: 0,
        };
        let mut request_flags = 0;
        if request.wrap {
            request_flags |= ffi::PARAGRAPH_REQUEST_WRAP;
        }
        if request.width_constraint.is_some() {
            request_flags |= ffi::PARAGRAPH_REQUEST_HAS_WIDTH;
        }
        if request.selection.is_some() {
            request_flags |= ffi::PARAGRAPH_REQUEST_HAS_SELECTION;
        }
        if request.preedit.is_some() {
            request_flags |= ffi::PARAGRAPH_REQUEST_HAS_PREEDIT;
        }
        let raw = ffi::ParagraphRequest {
            struct_size: mem::size_of::<ffi::ParagraphRequest>() as u32,
            flags: request_flags,
            text: utf8(&request.text),
            style_runs: style_runs.as_ptr(),
            style_run_count: style_runs.len(),
            paragraph_style,
            width_constraint: request.width_constraint.unwrap_or(0.0),
            reserved: 0,
            locale: optional_utf8(request.locale.as_deref()),
            inline_objects: inline_objects.as_ptr(),
            inline_object_count: inline_objects.len(),
            selection: request.selection.map(raw_range).unwrap_or_default(),
            preedit: request
                .preedit
                .map(|value| ffi::Preedit {
                    range: raw_range(value.range),
                    selection: raw_range(value.selection),
                })
                .unwrap_or_default(),
            font_catalog_generation: request.font_catalog_generation,
            fallback_families: fallback_families.as_ptr(),
            fallback_family_count: fallback_families.len(),
        };
        Ok(Self {
            raw,
            _variations: variations,
            _features: features,
            _style_runs: style_runs,
            _inline_objects: inline_objects,
            _fallback_families: fallback_families,
        })
    }
}

fn raw_range(value: ParagraphRange) -> ffi::TextRange {
    ffi::TextRange {
        start: value.start,
        end: value.end,
    }
}

fn raw_color(value: ParagraphColor) -> ffi::Rgba8 {
    ffi::Rgba8 {
        red: value.red,
        green: value.green,
        blue: value.blue,
        alpha: value.alpha,
    }
}

fn utf8(value: &str) -> ffi::Utf8Slice {
    ffi::Utf8Slice {
        data: value.as_ptr(),
        length: value.len(),
    }
}

fn optional_utf8(value: Option<&str>) -> ffi::Utf8Slice {
    value.map(utf8).unwrap_or(ffi::Utf8Slice {
        data: ptr::null(),
        length: 0,
    })
}

fn text_align(value: ParagraphTextAlign) -> u32 {
    match value {
        ParagraphTextAlign::Left => ffi::TEXT_ALIGN_LEFT,
        ParagraphTextAlign::Right => ffi::TEXT_ALIGN_RIGHT,
        ParagraphTextAlign::Center => ffi::TEXT_ALIGN_CENTER,
        ParagraphTextAlign::Justify => ffi::TEXT_ALIGN_JUSTIFY,
        ParagraphTextAlign::Start => ffi::TEXT_ALIGN_START,
        ParagraphTextAlign::End => ffi::TEXT_ALIGN_END,
    }
}

fn overflow(value: ParagraphOverflow) -> u32 {
    match value {
        ParagraphOverflow::Clip => ffi::TEXT_OVERFLOW_CLIP,
        ParagraphOverflow::Ellipsis => ffi::TEXT_OVERFLOW_ELLIPSIS,
        ParagraphOverflow::Fade => ffi::TEXT_OVERFLOW_FADE,
        ParagraphOverflow::Visible => ffi::TEXT_OVERFLOW_VISIBLE,
    }
}

fn text_direction(value: ParagraphTextDirection) -> u32 {
    match value {
        ParagraphTextDirection::Auto => ffi::TEXT_DIRECTION_AUTO,
        ParagraphTextDirection::LeftToRight => ffi::TEXT_DIRECTION_LTR,
        ParagraphTextDirection::RightToLeft => ffi::TEXT_DIRECTION_RTL,
    }
}

fn text_width_basis(value: ParagraphTextWidthBasis) -> u32 {
    match value {
        ParagraphTextWidthBasis::Parent => ffi::TEXT_WIDTH_BASIS_PARENT,
        ParagraphTextWidthBasis::LongestLine => ffi::TEXT_WIDTH_BASIS_LONGEST_LINE,
    }
}

unsafe fn owned_output(view: &ffi::ParagraphResultView) -> Result<ParagraphOutput> {
    Ok(ParagraphOutput {
        index_encoding: view.index_encoding,
        capabilities: ParagraphCapabilities(view.capabilities),
        size: view.size,
        min_intrinsic_width: view.min_intrinsic_width,
        max_intrinsic_width: view.max_intrinsic_width,
        first_baseline: (view.has_first_baseline != 0).then_some(view.first_baseline),
        last_baseline: (view.has_last_baseline != 0).then_some(view.last_baseline),
        // SAFETY: all pointers are owned by the live opaque result; helper
        // checks null/count consistency and copies before it can be destroyed.
        lines: unsafe { copy_array(view.lines, view.line_count, "lines")? },
        clusters: unsafe { copy_array(view.clusters, view.cluster_count, "clusters")? },
        carets: unsafe { copy_array(view.carets, view.caret_count, "carets")? },
        hit_regions: unsafe { copy_array(view.hit_regions, view.hit_region_count, "hit_regions")? },
        inline_boxes: unsafe {
            copy_array(view.inline_boxes, view.inline_box_count, "inline_boxes")?
        },
        unresolved_glyphs: unsafe {
            copy_array(
                view.unresolved_glyphs,
                view.unresolved_glyph_count,
                "unresolved_glyphs",
            )?
        },
        unresolved_codepoints: unsafe {
            copy_array(
                view.unresolved_codepoints,
                view.unresolved_codepoint_count,
                "unresolved_codepoints",
            )?
        },
    })
}

unsafe fn copy_array<T: Copy>(
    pointer: *const T,
    count: usize,
    field: &'static str,
) -> Result<Box<[T]>> {
    if count == 0 {
        return Ok(Box::new([]));
    }
    if pointer.is_null() || count > isize::MAX as usize / mem::size_of::<T>() {
        return Err(Error::local(
            ErrorKind::Internal,
            "ParagraphEngine::layout",
            format!("bridge returned an invalid {field} array"),
        ));
    }
    // SAFETY: caller holds the native result alive, pointer/count were checked,
    // and C records have the same repr(C) layout as T.
    Ok(unsafe { slice::from_raw_parts(pointer, count) }
        .to_vec()
        .into_boxed_slice())
}

fn invalid_request(message: &str) -> Error {
    Error::local(
        ErrorKind::InvalidArgument,
        "ParagraphEngine::layout",
        message,
    )
}
