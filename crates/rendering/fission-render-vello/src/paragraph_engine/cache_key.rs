use fission_ir::op::{FontStyle, TextAlign, TextDirection, TextOverflow, TextWidthBasis};
use fission_layout::{ParagraphCacheKey, ParagraphDescription, Utf8Range};

// FNV-1a 128 is small, deterministic, and sufficient for an in-process cache
// identity. This key is not a cryptographic content digest or a persistent file
// format; it only prevents geometry and draw data for different shaping inputs
// from being conflated.
const FNV_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

pub(super) fn paragraph_cache_key(
    description: &ParagraphDescription,
    default_family: &str,
) -> ParagraphCacheKey {
    let mut key = StableKey::new();
    key.bytes(b"fission-parley-paragraph-v1");
    key.string(default_family);
    key.string(&description.text);
    key.u64(description.style_runs.len() as u64);
    for run in &description.style_runs {
        key.range(run.range);
        key.f32(run.style.font_size);
        key.u8(run.style.color.r);
        key.u8(run.style.color.g);
        key.u8(run.style.color.b);
        key.u8(run.style.color.a);
        key.bool(run.style.underline);
        key.option_string(run.style.font_family.as_deref());
        key.option_string(run.style.locale.as_deref());
        key.u16(run.style.font_weight);
        key.u8(match run.style.font_style {
            FontStyle::Normal => 0,
            FontStyle::Italic => 1,
        });
        key.option_f32(run.style.line_height);
        key.f32(run.style.letter_spacing);
        match run.style.background_color {
            Some(color) => {
                key.bool(true);
                key.u8(color.r);
                key.u8(color.g);
                key.u8(color.b);
                key.u8(color.a);
            }
            None => key.bool(false),
        }
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

    let paragraph = description.paragraph_style;
    key.u8(match paragraph.text_align {
        TextAlign::Left => 0,
        TextAlign::Right => 1,
        TextAlign::Center => 2,
        TextAlign::Justify => 3,
        TextAlign::Start => 4,
        TextAlign::End => 5,
    });
    key.option_usize(paragraph.max_lines);
    key.u8(match paragraph.overflow {
        TextOverflow::Clip => 0,
        TextOverflow::Ellipsis => 1,
        TextOverflow::Fade => 2,
        TextOverflow::Visible => 3,
    });
    key.u8(match paragraph.text_direction {
        TextDirection::Auto => 0,
        TextDirection::Ltr => 1,
        TextDirection::Rtl => 2,
    });
    key.u8(match paragraph.text_width_basis {
        TextWidthBasis::Parent => 0,
        TextWidthBasis::LongestLine => 1,
    });
    key.option_f32(paragraph.strut_line_height);
    key.bool(paragraph.text_height_behavior.apply_height_to_first_ascent);
    key.bool(paragraph.text_height_behavior.apply_height_to_last_descent);

    key.option_f32(description.width_constraint);
    key.bool(description.wrap);
    key.option_string(description.locale.as_deref());
    key.u64(description.inline_objects.len() as u64);
    for inline in &description.inline_objects {
        key.u64(inline.id);
        key.range(inline.range);
        key.f32(inline.size.width);
        key.f32(inline.size.height);
        key.f32(inline.baseline);
    }
    key.option_range(description.selection);
    match description.preedit {
        Some(preedit) => {
            key.bool(true);
            key.range(preedit.range);
            key.range(preedit.selection);
        }
        None => key.bool(false),
    }
    key.u64(description.font_catalog_generation);
    key.u64(description.fallback_families.len() as u64);
    for family in &description.fallback_families {
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

    fn bytes(&mut self, bytes: &[u8]) {
        self.raw(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
        self.raw(bytes);
    }

    fn raw(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u128::from(*byte);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
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

    fn option_f32(&mut self, value: Option<f32>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.f32(value);
            }
            None => self.bool(false),
        }
    }

    fn option_usize(&mut self, value: Option<usize>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.u64(value as u64);
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

    fn range(&mut self, range: Utf8Range) {
        self.u64(range.start().byte_offset() as u64);
        self.u64(range.end().byte_offset() as u64);
    }

    fn option_range(&mut self, range: Option<Utf8Range>) {
        match range {
            Some(range) => {
                self.bool(true);
                self.range(range);
            }
            None => self.bool(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use fission_ir::op::{Color, TextParagraphStyle, TextStyle};
    use fission_layout::{
        ParagraphDescription, ParagraphFontFeature, ParagraphStyleRun, Utf8Range,
    };

    use super::paragraph_cache_key;

    fn description() -> ParagraphDescription {
        let text = "cache me";
        let style = TextStyle {
            font_size: 16.0,
            color: Color::BLACK,
            underline: false,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_style: Default::default(),
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
        };
        ParagraphDescription::new(
            text,
            vec![ParagraphStyleRun::new(
                Utf8Range::from_byte_offsets(0, text.len()).unwrap(),
                style,
            )],
            TextParagraphStyle::default(),
            Some(200.0),
        )
    }

    #[test]
    fn every_catalog_generation_gets_a_distinct_key() {
        let first = description();
        let mut second = first.clone();
        second.font_catalog_generation = 1;

        assert_ne!(
            paragraph_cache_key(&first, "system-ui"),
            paragraph_cache_key(&second, "system-ui")
        );
    }

    #[test]
    fn feature_settings_participate_in_the_key() {
        let first = description();
        let mut second = first.clone();
        second.style_runs[0].features.push(ParagraphFontFeature {
            tag: u32::from_be_bytes(*b"liga"),
            value: 0,
        });

        assert_ne!(
            paragraph_cache_key(&first, "system-ui"),
            paragraph_cache_key(&second, "system-ui")
        );
    }
}
