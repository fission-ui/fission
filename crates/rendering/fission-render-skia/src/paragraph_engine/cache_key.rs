use fission_layout::ParagraphCacheKey;

use super::request::{PackedColor, PackedParagraphRequest, PackedRange};

// FNV-1a 128 is deterministic and sufficient for an in-process shaping-cache
// identity. It is not used as a security boundary or a persistent file digest.
const FNV_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

pub(super) fn paragraph_cache_key(request: &PackedParagraphRequest) -> ParagraphCacheKey {
    let mut key = StableKey::new();
    key.bytes(b"fission-skparagraph-owned-v1");
    key.bytes(&request.text);
    key.u64(request.style_runs.len() as u64);
    for run in &request.style_runs {
        key.range(run.range);
        key.f32(run.font_size);
        key.color(run.color);
        key.bool(run.underline);
        key.option_string(run.font_family.as_deref());
        key.option_string(run.locale.as_deref());
        key.u16(run.font_weight);
        key.u8(run.font_style);
        key.option_f32(run.line_height);
        key.f32(run.letter_spacing);
        key.option_color(run.background_color);
        key.f32(run.font_width);
        key.f32(run.word_spacing);
        key.u64(run.variations.len() as u64);
        for variation in &run.variations {
            key.u32(variation.tag);
            key.f32(variation.value);
        }
        key.u64(run.features.len() as u64);
        for feature in &run.features {
            key.u32(feature.tag);
            key.u32(feature.value);
        }
    }

    let paragraph = request.paragraph_style;
    key.u8(paragraph.text_align);
    key.option_u64(paragraph.max_lines);
    key.u8(paragraph.overflow);
    key.u8(paragraph.text_direction);
    key.u8(paragraph.text_width_basis);
    key.option_f32(paragraph.strut_line_height);
    key.bool(paragraph.apply_height_to_first_ascent);
    key.bool(paragraph.apply_height_to_last_descent);
    key.option_f32(request.width_constraint);
    key.bool(request.wrap);
    key.option_string(request.locale.as_deref());

    key.u64(request.inline_objects.len() as u64);
    for inline in &request.inline_objects {
        key.u64(inline.id);
        key.range(inline.range);
        key.f32(inline.width);
        key.f32(inline.height);
        key.f32(inline.baseline);
    }
    key.option_range(request.selection);
    match request.preedit {
        Some(preedit) => {
            key.bool(true);
            key.range(preedit.range);
            key.range(preedit.selection);
        }
        None => key.bool(false),
    }
    key.u64(request.font_catalog_generation);
    key.u64(request.fallback_families.len() as u64);
    for family in &request.fallback_families {
        key.string(family);
    }

    ParagraphCacheKey::new(key.finish())
}

struct StableKey(u128);

impl StableKey {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    const fn finish(self) -> u128 {
        self.0
    }

    fn raw(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u128::from(*byte);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        self.raw(value);
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u8(&mut self, value: u8) {
        self.raw(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.raw(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.raw(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.raw(&value.to_le_bytes());
    }

    fn f32(&mut self, value: f32) {
        self.u32(value.to_bits());
    }

    fn range(&mut self, value: PackedRange) {
        self.u64(value.start);
        self.u64(value.end);
    }

    fn color(&mut self, value: PackedColor) {
        self.u8(value.red);
        self.u8(value.green);
        self.u8(value.blue);
        self.u8(value.alpha);
    }

    fn option_u64(&mut self, value: Option<u64>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.u64(value);
            }
            None => self.bool(false),
        }
    }

    fn option_f32(&mut self, value: Option<f32>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.f32(value);
            }
            None => self.bool(false),
        }
    }

    fn option_string(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.string(value);
            }
            None => self.bool(false),
        }
    }

    fn option_range(&mut self, value: Option<PackedRange>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.range(value);
            }
            None => self.bool(false),
        }
    }

    fn option_color(&mut self, value: Option<PackedColor>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.color(value);
            }
            None => self.bool(false),
        }
    }
}
