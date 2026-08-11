//! Raw scalar declarations for the SkParagraph portion of the C ABI.

use std::ffi::c_uchar;

use super::{Error, Status};

pub type ParagraphResultHandle = u64;

pub const PARAGRAPH_BIDIRECTIONAL_TEXT: u64 = 1 << 0;
pub const PARAGRAPH_VARIABLE_FONTS: u64 = 1 << 1;
pub const PARAGRAPH_FONT_FEATURES: u64 = 1 << 2;
pub const PARAGRAPH_INLINE_OBJECTS: u64 = 1 << 3;
pub const PARAGRAPH_CLUSTER_MAPPING: u64 = 1 << 4;
pub const PARAGRAPH_HIT_TESTING: u64 = 1 << 5;
pub const PARAGRAPH_CARET_GEOMETRY: u64 = 1 << 6;
pub const PARAGRAPH_SELECTION_GEOMETRY: u64 = 1 << 7;
pub const PARAGRAPH_UNRESOLVED_GLYPHS: u64 = 1 << 8;
pub const PARAGRAPH_ALL_CAPABILITIES: u64 = PARAGRAPH_BIDIRECTIONAL_TEXT
    | PARAGRAPH_VARIABLE_FONTS
    | PARAGRAPH_FONT_FEATURES
    | PARAGRAPH_INLINE_OBJECTS
    | PARAGRAPH_CLUSTER_MAPPING
    | PARAGRAPH_HIT_TESTING
    | PARAGRAPH_CARET_GEOMETRY
    | PARAGRAPH_SELECTION_GEOMETRY
    | PARAGRAPH_UNRESOLVED_GLYPHS;

pub const INDEX_UTF8: u32 = 0;
pub const INDEX_UTF16: u32 = 1;

pub const FONT_SLANT_NORMAL: u16 = 0;
pub const FONT_SLANT_ITALIC: u16 = 1;

pub const TEXT_STYLE_UNDERLINE: u32 = 1 << 0;
pub const TEXT_STYLE_HAS_LINE_HEIGHT: u32 = 1 << 1;
pub const TEXT_STYLE_HAS_BACKGROUND: u32 = 1 << 2;

pub const TEXT_ALIGN_LEFT: u32 = 0;
pub const TEXT_ALIGN_RIGHT: u32 = 1;
pub const TEXT_ALIGN_CENTER: u32 = 2;
pub const TEXT_ALIGN_JUSTIFY: u32 = 3;
pub const TEXT_ALIGN_START: u32 = 4;
pub const TEXT_ALIGN_END: u32 = 5;

pub const TEXT_OVERFLOW_CLIP: u32 = 0;
pub const TEXT_OVERFLOW_ELLIPSIS: u32 = 1;
pub const TEXT_OVERFLOW_FADE: u32 = 2;
pub const TEXT_OVERFLOW_VISIBLE: u32 = 3;

pub const TEXT_DIRECTION_AUTO: u32 = 0;
pub const TEXT_DIRECTION_LTR: u32 = 1;
pub const TEXT_DIRECTION_RTL: u32 = 2;

pub const TEXT_WIDTH_BASIS_PARENT: u32 = 0;
pub const TEXT_WIDTH_BASIS_LONGEST_LINE: u32 = 1;

pub const PARAGRAPH_STYLE_HAS_MAX_LINES: u32 = 1 << 0;
pub const PARAGRAPH_STYLE_HAS_STRUT_HEIGHT: u32 = 1 << 1;
pub const PARAGRAPH_STYLE_APPLY_FIRST_ASCENT: u32 = 1 << 2;
pub const PARAGRAPH_STYLE_APPLY_LAST_DESCENT: u32 = 1 << 3;

pub const PARAGRAPH_REQUEST_WRAP: u32 = 1 << 0;
pub const PARAGRAPH_REQUEST_HAS_WIDTH: u32 = 1 << 1;
pub const PARAGRAPH_REQUEST_HAS_SELECTION: u32 = 1 << 2;
pub const PARAGRAPH_REQUEST_HAS_PREEDIT: u32 = 1 << 3;

pub const DIRECTION_LTR: u32 = 0;
pub const DIRECTION_RTL: u32 = 1;
pub const AFFINITY_UPSTREAM: u32 = 0;
pub const AFFINITY_DOWNSTREAM: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Utf8Slice {
    pub data: *const c_uchar,
    pub length: usize,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: u64,
    pub end: u64,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rgba8 {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FontVariation {
    pub tag: u32,
    pub value: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FontFeature {
    pub tag: u32,
    pub value: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TextStyleRun {
    pub struct_size: u32,
    pub flags: u32,
    pub range: TextRange,
    pub font_size: f32,
    pub color: Rgba8,
    pub font_family: Utf8Slice,
    pub locale: Utf8Slice,
    pub font_weight: u16,
    pub font_slant: u16,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub background_color: Rgba8,
    pub font_width: f32,
    pub word_spacing: f32,
    pub variations: *const FontVariation,
    pub variation_count: usize,
    pub features: *const FontFeature,
    pub feature_count: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ParagraphStyle {
    pub struct_size: u32,
    pub flags: u32,
    pub text_align: u32,
    pub overflow: u32,
    pub text_direction: u32,
    pub text_width_basis: u32,
    pub max_lines: u64,
    pub strut_line_height: f32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InlineObject {
    pub struct_size: u32,
    pub reserved: u32,
    pub id: u64,
    pub range: TextRange,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub reserved_scalar: f32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Preedit {
    pub range: TextRange,
    pub selection: TextRange,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ParagraphRequest {
    pub struct_size: u32,
    pub flags: u32,
    pub text: Utf8Slice,
    pub style_runs: *const TextStyleRun,
    pub style_run_count: usize,
    pub paragraph_style: ParagraphStyle,
    pub width_constraint: f32,
    pub reserved: u32,
    pub locale: Utf8Slice,
    pub inline_objects: *const InlineObject,
    pub inline_object_count: usize,
    pub selection: TextRange,
    pub preedit: Preedit,
    pub font_catalog_generation: u64,
    pub fallback_families: *const Utf8Slice,
    pub fallback_family_count: usize,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ParagraphSize {
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ParagraphRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ParagraphLine {
    pub range: TextRange,
    pub rect: ParagraphRect,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    pub hard_break: u32,
    pub direction: u32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ParagraphCluster {
    pub range: TextRange,
    pub rect: ParagraphRect,
    pub line_index: u64,
    pub direction: u32,
    pub starts_grapheme: u32,
    pub starts_word: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ParagraphCaret {
    pub index: u64,
    pub affinity: u32,
    pub reserved: u32,
    pub rect: ParagraphRect,
    pub line_index: u64,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ParagraphHitRegion {
    pub rect: ParagraphRect,
    pub index: u64,
    pub affinity: u32,
    pub reserved: u32,
    pub line_index: u64,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ParagraphInlineBox {
    pub id: u64,
    pub range: TextRange,
    pub rect: ParagraphRect,
    pub baseline: f32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UnresolvedGlyph {
    pub range: TextRange,
    pub codepoint_start: u64,
    pub codepoint_count: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ParagraphResultView {
    pub struct_size: u32,
    pub index_encoding: u32,
    pub capabilities: u64,
    pub size: ParagraphSize,
    pub min_intrinsic_width: f32,
    pub max_intrinsic_width: f32,
    pub first_baseline: f32,
    pub last_baseline: f32,
    pub has_first_baseline: u32,
    pub has_last_baseline: u32,
    pub lines: *const ParagraphLine,
    pub line_count: usize,
    pub clusters: *const ParagraphCluster,
    pub cluster_count: usize,
    pub carets: *const ParagraphCaret,
    pub caret_count: usize,
    pub hit_regions: *const ParagraphHitRegion,
    pub hit_region_count: usize,
    pub inline_boxes: *const ParagraphInlineBox,
    pub inline_box_count: usize,
    pub unresolved_glyphs: *const UnresolvedGlyph,
    pub unresolved_glyph_count: usize,
    pub unresolved_codepoints: *const u32,
    pub unresolved_codepoint_count: usize,
}

impl Default for ParagraphResultView {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            index_encoding: INDEX_UTF8,
            capabilities: 0,
            size: ParagraphSize::default(),
            min_intrinsic_width: 0.0,
            max_intrinsic_width: 0.0,
            first_baseline: 0.0,
            last_baseline: 0.0,
            has_first_baseline: 0,
            has_last_baseline: 0,
            lines: std::ptr::null(),
            line_count: 0,
            clusters: std::ptr::null(),
            cluster_count: 0,
            carets: std::ptr::null(),
            caret_count: 0,
            hit_regions: std::ptr::null(),
            hit_region_count: 0,
            inline_boxes: std::ptr::null(),
            inline_box_count: 0,
            unresolved_glyphs: std::ptr::null(),
            unresolved_glyph_count: 0,
            unresolved_codepoints: std::ptr::null(),
            unresolved_codepoint_count: 0,
        }
    }
}

extern "C" {
    pub fn fission_skia_paragraph_capabilities(
        out_capabilities: *mut u64,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_paragraph_layout(
        request: *const ParagraphRequest,
        out_result: *mut ParagraphResultHandle,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_paragraph_result_get_view(
        result: ParagraphResultHandle,
        out_view: *mut ParagraphResultView,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_paragraph_result_get_approximate_bytes(
        result: ParagraphResultHandle,
        out_approximate_bytes: *mut usize,
        out_error: *mut Error,
    ) -> Status;
    pub fn fission_skia_paragraph_result_destroy(
        result: ParagraphResultHandle,
        out_error: *mut Error,
    ) -> Status;
}
