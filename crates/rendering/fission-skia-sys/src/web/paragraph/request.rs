use std::collections::HashSet;

use crate::{
    ParagraphColor, ParagraphFontFeature, ParagraphFontSlant, ParagraphFontVariation,
    ParagraphInlineObject, ParagraphOverflow, ParagraphPreedit, ParagraphRange, ParagraphRequest,
    ParagraphStyle, ParagraphTextAlign, ParagraphTextDirection, ParagraphTextStyleRun,
    ParagraphTextWidthBasis,
};

use super::wire::{
    checked_u32, finite, non_negative, positive, put_f32, put_u16, put_u32, put_u64, require_limit,
    ParagraphWireError, Reader, MAX_FALLBACK_FAMILIES, MAX_FEATURES, MAX_FONT_RESOURCES,
    MAX_INLINE_OBJECTS, MAX_PARAGRAPH_PACKET_BYTES, MAX_STRING_BYTES, MAX_STYLE_RUNS,
    MAX_TEXT_BYTES, MAX_VARIATIONS, PARAGRAPH_WIRE_VERSION,
};
use crate::web::ResourceHandle;

const MAGIC: [u8; 4] = *b"FSPQ";
const HEADER_LEN: usize = 144;
const STYLE_HEADER_LEN: usize = 72;
const INLINE_LEN: usize = 40;
const FONT_HEADER_LEN: usize = 16;

const FLAG_WRAP: u32 = 1 << 0;
const FLAG_WIDTH: u32 = 1 << 1;
const FLAG_LOCALE: u32 = 1 << 2;
const FLAG_SELECTION: u32 = 1 << 3;
const FLAG_PREEDIT: u32 = 1 << 4;
const FLAG_MAX_LINES: u32 = 1 << 5;
const FLAG_STRUT: u32 = 1 << 6;
const FLAG_FIRST_ASCENT: u32 = 1 << 7;
const FLAG_LAST_DESCENT: u32 = 1 << 8;
const KNOWN_FLAGS: u32 = (1 << 9) - 1;

const STYLE_UNDERLINE: u32 = 1 << 0;
const STYLE_FAMILY: u32 = 1 << 1;
const STYLE_LOCALE: u32 = 1 << 2;
const STYLE_LINE_HEIGHT: u32 = 1 << 3;
const STYLE_BACKGROUND: u32 = 1 << 4;
const KNOWN_STYLE_FLAGS: u32 = (1 << 5) - 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebParagraphFont {
    pub handle: ResourceHandle,
    pub family: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WebParagraphRequest {
    pub paragraph: ParagraphRequest,
    pub fonts: Vec<WebParagraphFont>,
}

pub fn encode_paragraph_request(
    request: &WebParagraphRequest,
) -> Result<Vec<u8>, ParagraphWireError> {
    validate_request(request)?;
    let paragraph = &request.paragraph;
    let encoded_len = encoded_request_len(request)?;
    let mut flags = 0;
    flags |= u32::from(paragraph.wrap) * FLAG_WRAP;
    flags |= u32::from(paragraph.width_constraint.is_some()) * FLAG_WIDTH;
    flags |= u32::from(paragraph.locale.is_some()) * FLAG_LOCALE;
    flags |= u32::from(paragraph.selection.is_some()) * FLAG_SELECTION;
    flags |= u32::from(paragraph.preedit.is_some()) * FLAG_PREEDIT;
    flags |= u32::from(paragraph.paragraph_style.max_lines.is_some()) * FLAG_MAX_LINES;
    flags |= u32::from(paragraph.paragraph_style.strut_line_height.is_some()) * FLAG_STRUT;
    flags |= u32::from(paragraph.paragraph_style.apply_height_to_first_ascent) * FLAG_FIRST_ASCENT;
    flags |= u32::from(paragraph.paragraph_style.apply_height_to_last_descent) * FLAG_LAST_DESCENT;

    let mut bytes = Vec::with_capacity(encoded_len);
    bytes.extend_from_slice(&MAGIC);
    put_u16(&mut bytes, PARAGRAPH_WIRE_VERSION);
    put_u16(&mut bytes, 0);
    put_u32(&mut bytes, 0);
    put_u32(&mut bytes, flags);
    put_u32(
        &mut bytes,
        checked_u32(paragraph.text.len(), "paragraph text length")?,
    );
    put_u32(
        &mut bytes,
        checked_u32(paragraph.style_runs.len(), "paragraph style run count")?,
    );
    put_u32(
        &mut bytes,
        checked_u32(
            paragraph.inline_objects.len(),
            "paragraph inline object count",
        )?,
    );
    put_u32(
        &mut bytes,
        checked_u32(
            paragraph.fallback_families.len(),
            "paragraph fallback family count",
        )?,
    );
    put_u64(&mut bytes, paragraph.font_catalog_generation);
    put_f32(&mut bytes, paragraph.width_constraint.unwrap_or(0.0));
    bytes.push(text_align(paragraph.paragraph_style.text_align));
    bytes.push(overflow(paragraph.paragraph_style.overflow));
    bytes.push(text_direction(paragraph.paragraph_style.text_direction));
    bytes.push(text_width_basis(paragraph.paragraph_style.text_width_basis));
    put_u64(&mut bytes, paragraph.paragraph_style.max_lines.unwrap_or(0));
    put_f32(
        &mut bytes,
        paragraph.paragraph_style.strut_line_height.unwrap_or(0.0),
    );
    put_u32(
        &mut bytes,
        checked_u32(
            paragraph.locale.as_deref().map_or(0, str::len),
            "paragraph locale length",
        )?,
    );
    put_u32(
        &mut bytes,
        checked_u32(request.fonts.len(), "paragraph font count")?,
    );
    put_u32(&mut bytes, 0);
    encode_range(
        &mut bytes,
        paragraph.selection.unwrap_or(ParagraphRange::new(0, 0)),
    );
    let preedit = paragraph.preedit.unwrap_or(ParagraphPreedit {
        range: ParagraphRange::new(0, 0),
        selection: ParagraphRange::new(0, 0),
    });
    encode_range(&mut bytes, preedit.range);
    encode_range(&mut bytes, preedit.selection);
    bytes.extend_from_slice(&[0; 24]);
    debug_assert_eq!(bytes.len(), HEADER_LEN);

    bytes.extend_from_slice(paragraph.text.as_bytes());
    if let Some(locale) = &paragraph.locale {
        bytes.extend_from_slice(locale.as_bytes());
    }
    for style in &paragraph.style_runs {
        encode_style(&mut bytes, style)?;
    }
    for inline in &paragraph.inline_objects {
        encode_inline(&mut bytes, *inline);
    }
    for family in &paragraph.fallback_families {
        encode_length_prefixed_string(&mut bytes, family)?;
    }
    for font in &request.fonts {
        put_u32(&mut bytes, font.handle.slot);
        put_u32(&mut bytes, font.handle.generation);
        put_u32(
            &mut bytes,
            checked_u32(font.family.len(), "paragraph font family length")?,
        );
        put_u32(&mut bytes, 0);
        bytes.extend_from_slice(font.family.as_bytes());
    }
    debug_assert_eq!(bytes.len(), encoded_len);
    let total = checked_u32(bytes.len(), "paragraph packet length")?;
    bytes[8..12].copy_from_slice(&total.to_le_bytes());
    Ok(bytes)
}

pub fn decode_paragraph_request(bytes: &[u8]) -> Result<WebParagraphRequest, ParagraphWireError> {
    require_limit(
        "paragraph packet bytes",
        bytes.len(),
        MAX_PARAGRAPH_PACKET_BYTES,
    )?;
    if bytes.len() < HEADER_LEN {
        return Err(ParagraphWireError::Truncated);
    }
    let mut reader = Reader::new(bytes);
    if reader.take(4)? != MAGIC {
        return Err(ParagraphWireError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != PARAGRAPH_WIRE_VERSION {
        return Err(ParagraphWireError::UnsupportedVersion(version));
    }
    if reader.u16()? != 0 {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    if reader.u32()? as usize != bytes.len() {
        return Err(ParagraphWireError::LengthMismatch);
    }
    let flags = reader.u32()?;
    if flags & !KNOWN_FLAGS != 0 {
        return Err(ParagraphWireError::InvalidFlags);
    }
    let text_len = reader.u32()? as usize;
    let style_count = reader.u32()? as usize;
    let inline_count = reader.u32()? as usize;
    let fallback_count = reader.u32()? as usize;
    let font_catalog_generation = reader.u64()?;
    let width_value = reader.f32()?;
    let paragraph_style = ParagraphStyle {
        text_align: decode_text_align(reader.u8()?)?,
        overflow: decode_overflow(reader.u8()?)?,
        text_direction: decode_text_direction(reader.u8()?)?,
        text_width_basis: decode_text_width_basis(reader.u8()?)?,
        max_lines: option_u64(flags, FLAG_MAX_LINES, reader.u64()?, "paragraph max lines")?,
        strut_line_height: option_positive_f32(
            flags,
            FLAG_STRUT,
            reader.f32()?,
            "paragraph strut line height",
        )?,
        apply_height_to_first_ascent: flags & FLAG_FIRST_ASCENT != 0,
        apply_height_to_last_descent: flags & FLAG_LAST_DESCENT != 0,
    };
    let locale_len = reader.u32()? as usize;
    let font_count = reader.u32()? as usize;
    if reader.u32()? != 0 {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    let selection_value = decode_range(&mut reader)?;
    let preedit_range = decode_range(&mut reader)?;
    let preedit_selection = decode_range(&mut reader)?;
    if reader.take(24)?.iter().any(|byte| *byte != 0) {
        return Err(ParagraphWireError::NonZeroReserved);
    }

    require_limit("paragraph text bytes", text_len, MAX_TEXT_BYTES)?;
    require_limit("paragraph style runs", style_count, MAX_STYLE_RUNS)?;
    require_limit("paragraph inline objects", inline_count, MAX_INLINE_OBJECTS)?;
    require_limit(
        "paragraph fallback families",
        fallback_count,
        MAX_FALLBACK_FAMILIES,
    )?;
    require_limit("paragraph fonts", font_count, MAX_FONT_RESOURCES)?;
    require_limit("paragraph locale bytes", locale_len, MAX_STRING_BYTES)?;

    let text = decode_utf8(reader.take(text_len)?, "paragraph text")?;
    let locale_value = decode_utf8(reader.take(locale_len)?, "paragraph locale")?;
    let locale = option_string(flags, FLAG_LOCALE, locale_value, "paragraph locale")?;
    let width_constraint =
        option_non_negative_f32(flags, FLAG_WIDTH, width_value, "paragraph width constraint")?;

    require_remaining_records(&reader, style_count, STYLE_HEADER_LEN)?;
    let mut style_runs = Vec::with_capacity(style_count);
    let mut variation_total = 0usize;
    let mut feature_total = 0usize;
    for _ in 0..style_count {
        style_runs.push(decode_style(
            &mut reader,
            &mut variation_total,
            &mut feature_total,
        )?);
    }
    require_remaining_records(&reader, inline_count, INLINE_LEN)?;
    let mut inline_objects = Vec::with_capacity(inline_count);
    for _ in 0..inline_count {
        inline_objects.push(decode_inline(&mut reader)?);
    }
    require_remaining_records(&reader, fallback_count, 4)?;
    let mut fallback_families = Vec::with_capacity(fallback_count);
    for _ in 0..fallback_count {
        fallback_families.push(decode_length_prefixed_string(
            &mut reader,
            "paragraph fallback family",
        )?);
    }
    require_remaining_records(&reader, font_count, FONT_HEADER_LEN)?;
    let mut fonts = Vec::with_capacity(font_count);
    for _ in 0..font_count {
        let handle = ResourceHandle {
            slot: reader.u32()?,
            generation: reader.u32()?,
        };
        if handle.slot == 0 || handle.generation == 0 {
            return Err(ParagraphWireError::InvalidValue("paragraph font handle"));
        }
        let family_len = reader.u32()? as usize;
        if reader.u32()? != 0 {
            return Err(ParagraphWireError::NonZeroReserved);
        }
        require_limit("paragraph font family bytes", family_len, MAX_STRING_BYTES)?;
        let family = decode_utf8(reader.take(family_len)?, "paragraph font family")?;
        if family.is_empty() {
            return Err(ParagraphWireError::InvalidValue("paragraph font family"));
        }
        fonts.push(WebParagraphFont { handle, family });
    }
    reader.finish()?;

    let paragraph = ParagraphRequest {
        text,
        style_runs,
        paragraph_style,
        width_constraint,
        wrap: flags & FLAG_WRAP != 0,
        locale,
        inline_objects,
        selection: option_range(
            flags,
            FLAG_SELECTION,
            selection_value,
            "paragraph selection",
        )?,
        preedit: option_preedit(flags, preedit_range, preedit_selection)?,
        font_catalog_generation,
        fallback_families,
    };
    let request = WebParagraphRequest { paragraph, fonts };
    validate_request(&request)?;
    Ok(request)
}

fn encode_style(
    bytes: &mut Vec<u8>,
    style: &ParagraphTextStyleRun,
) -> Result<(), ParagraphWireError> {
    let start = bytes.len();
    put_u32(bytes, 0);
    let mut flags = 0;
    flags |= u32::from(style.underline) * STYLE_UNDERLINE;
    flags |= u32::from(style.font_family.is_some()) * STYLE_FAMILY;
    flags |= u32::from(style.locale.is_some()) * STYLE_LOCALE;
    flags |= u32::from(style.line_height.is_some()) * STYLE_LINE_HEIGHT;
    flags |= u32::from(style.background_color.is_some()) * STYLE_BACKGROUND;
    put_u32(bytes, flags);
    encode_range(bytes, style.range);
    put_f32(bytes, style.font_size);
    encode_color(bytes, style.color);
    put_u16(bytes, style.font_weight);
    bytes.push(match style.font_slant {
        ParagraphFontSlant::Normal => 0,
        ParagraphFontSlant::Italic => 1,
    });
    bytes.push(0);
    put_f32(bytes, style.line_height.unwrap_or(0.0));
    put_f32(bytes, style.letter_spacing);
    encode_color(bytes, style.background_color.unwrap_or_default());
    put_f32(bytes, style.font_width);
    put_f32(bytes, style.word_spacing);
    put_u32(
        bytes,
        checked_u32(
            style.font_family.as_deref().map_or(0, str::len),
            "style font family length",
        )?,
    );
    put_u32(
        bytes,
        checked_u32(
            style.locale.as_deref().map_or(0, str::len),
            "style locale length",
        )?,
    );
    put_u32(
        bytes,
        checked_u32(style.variations.len(), "style variation count")?,
    );
    put_u32(
        bytes,
        checked_u32(style.features.len(), "style feature count")?,
    );
    debug_assert_eq!(bytes.len() - start, STYLE_HEADER_LEN);
    if let Some(family) = &style.font_family {
        bytes.extend_from_slice(family.as_bytes());
    }
    if let Some(locale) = &style.locale {
        bytes.extend_from_slice(locale.as_bytes());
    }
    for variation in &style.variations {
        put_u32(bytes, variation.tag);
        put_f32(bytes, variation.value);
    }
    for feature in &style.features {
        put_u32(bytes, feature.tag);
        put_u32(bytes, feature.value);
    }
    let length = checked_u32(bytes.len() - start, "style entry length")?;
    bytes[start..start + 4].copy_from_slice(&length.to_le_bytes());
    Ok(())
}

fn decode_style(
    reader: &mut Reader<'_>,
    variation_total: &mut usize,
    feature_total: &mut usize,
) -> Result<ParagraphTextStyleRun, ParagraphWireError> {
    let entry_len = reader.u32()? as usize;
    if entry_len < STYLE_HEADER_LEN || entry_len - 4 > reader.remaining() {
        return Err(ParagraphWireError::LengthMismatch);
    }
    let mut entry = Reader::new(reader.take(entry_len - 4)?);
    let flags = entry.u32()?;
    if flags & !KNOWN_STYLE_FLAGS != 0 {
        return Err(ParagraphWireError::InvalidFlags);
    }
    let range = decode_range(&mut entry)?;
    let font_size = positive(entry.f32()?, "style font size")?;
    let color = decode_color(&mut entry)?;
    let font_weight = entry.u16()?;
    let font_slant = match entry.u8()? {
        0 => ParagraphFontSlant::Normal,
        1 => ParagraphFontSlant::Italic,
        _ => return Err(ParagraphWireError::InvalidEnum("style font slant")),
    };
    if entry.u8()? != 0 {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    let line_height_value = entry.f32()?;
    let letter_spacing = finite(entry.f32()?, "style letter spacing")?;
    let background_value = decode_color(&mut entry)?;
    let font_width = positive(entry.f32()?, "style font width")?;
    let word_spacing = finite(entry.f32()?, "style word spacing")?;
    let family_len = entry.u32()? as usize;
    let locale_len = entry.u32()? as usize;
    let variation_count = entry.u32()? as usize;
    let feature_count = entry.u32()? as usize;
    require_limit("style font family bytes", family_len, MAX_STRING_BYTES)?;
    require_limit("style locale bytes", locale_len, MAX_STRING_BYTES)?;
    *variation_total = variation_total
        .checked_add(variation_count)
        .ok_or(ParagraphWireError::InvalidValue("style variation count"))?;
    *feature_total = feature_total
        .checked_add(feature_count)
        .ok_or(ParagraphWireError::InvalidValue("style feature count"))?;
    require_limit("style variations", *variation_total, MAX_VARIATIONS)?;
    require_limit("style features", *feature_total, MAX_FEATURES)?;
    let family_value = decode_utf8(entry.take(family_len)?, "style font family")?;
    let locale_value = decode_utf8(entry.take(locale_len)?, "style locale")?;
    let font_family = option_string(flags, STYLE_FAMILY, family_value, "style font family")?;
    let locale = option_string(flags, STYLE_LOCALE, locale_value, "style locale")?;
    let mut variations = Vec::with_capacity(variation_count);
    for _ in 0..variation_count {
        variations.push(ParagraphFontVariation {
            tag: entry.u32()?,
            value: finite(entry.f32()?, "style variation value")?,
        });
    }
    let mut features = Vec::with_capacity(feature_count);
    for _ in 0..feature_count {
        features.push(ParagraphFontFeature {
            tag: entry.u32()?,
            value: entry.u32()?,
        });
    }
    entry.finish()?;
    Ok(ParagraphTextStyleRun {
        range,
        font_size,
        color,
        underline: flags & STYLE_UNDERLINE != 0,
        font_family,
        locale,
        font_weight,
        font_slant,
        line_height: option_positive_f32(
            flags,
            STYLE_LINE_HEIGHT,
            line_height_value,
            "style line height",
        )?,
        letter_spacing,
        background_color: option_color(flags, background_value)?,
        font_width,
        word_spacing,
        variations,
        features,
    })
}

fn encode_inline(bytes: &mut Vec<u8>, value: ParagraphInlineObject) {
    put_u64(bytes, value.id);
    encode_range(bytes, value.range);
    put_f32(bytes, value.width);
    put_f32(bytes, value.height);
    put_f32(bytes, value.baseline);
    put_u32(bytes, 0);
}

fn decode_inline(reader: &mut Reader<'_>) -> Result<ParagraphInlineObject, ParagraphWireError> {
    let value = ParagraphInlineObject {
        id: reader.u64()?,
        range: decode_range(reader)?,
        width: non_negative(reader.f32()?, "inline object width")?,
        height: non_negative(reader.f32()?, "inline object height")?,
        baseline: finite(reader.f32()?, "inline object baseline")?,
    };
    if reader.u32()? != 0 {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    Ok(value)
}

fn validate_request(request: &WebParagraphRequest) -> Result<(), ParagraphWireError> {
    let paragraph = &request.paragraph;
    require_limit("paragraph text bytes", paragraph.text.len(), MAX_TEXT_BYTES)?;
    require_limit(
        "paragraph style runs",
        paragraph.style_runs.len(),
        MAX_STYLE_RUNS,
    )?;
    require_limit(
        "paragraph inline objects",
        paragraph.inline_objects.len(),
        MAX_INLINE_OBJECTS,
    )?;
    require_limit(
        "paragraph fallback families",
        paragraph.fallback_families.len(),
        MAX_FALLBACK_FAMILIES,
    )?;
    require_limit("paragraph fonts", request.fonts.len(), MAX_FONT_RESOURCES)?;
    if let Some(width) = paragraph.width_constraint {
        non_negative(width, "paragraph width constraint")?;
    }
    if let Some(height) = paragraph.paragraph_style.strut_line_height {
        positive(height, "paragraph strut line height")?;
    }
    if paragraph.paragraph_style.max_lines == Some(0) {
        return Err(ParagraphWireError::InvalidValue("paragraph max lines"));
    }
    validate_optional_string(paragraph.locale.as_deref(), "paragraph locale")?;
    validate_ranges(
        &paragraph.text,
        &paragraph.style_runs,
        &paragraph.inline_objects,
    )?;
    if let Some(range) = paragraph.selection {
        validate_range(&paragraph.text, range, "paragraph selection")?;
    }
    if let Some(preedit) = paragraph.preedit {
        validate_range(&paragraph.text, preedit.range, "paragraph preedit")?;
        validate_range(
            &paragraph.text,
            preedit.selection,
            "paragraph preedit selection",
        )?;
        if preedit.selection.start < preedit.range.start
            || preedit.selection.end > preedit.range.end
        {
            return Err(ParagraphWireError::InvalidValue(
                "paragraph preedit selection",
            ));
        }
    }
    let mut variation_count = 0usize;
    let mut feature_count = 0usize;
    for style in &paragraph.style_runs {
        positive(style.font_size, "style font size")?;
        positive(style.font_width, "style font width")?;
        finite(style.letter_spacing, "style letter spacing")?;
        finite(style.word_spacing, "style word spacing")?;
        if !(1..=1000).contains(&style.font_weight) {
            return Err(ParagraphWireError::InvalidValue("style font weight"));
        }
        if let Some(height) = style.line_height {
            positive(height, "style line height")?;
        }
        validate_optional_string(style.font_family.as_deref(), "style font family")?;
        validate_optional_string(style.locale.as_deref(), "style locale")?;
        variation_count = variation_count
            .checked_add(style.variations.len())
            .ok_or(ParagraphWireError::InvalidValue("style variation count"))?;
        feature_count = feature_count
            .checked_add(style.features.len())
            .ok_or(ParagraphWireError::InvalidValue("style feature count"))?;
        for variation in &style.variations {
            finite(variation.value, "style variation value")?;
        }
    }
    require_limit("style variations", variation_count, MAX_VARIATIONS)?;
    require_limit("style features", feature_count, MAX_FEATURES)?;
    let mut inline_ids = HashSet::with_capacity(paragraph.inline_objects.len());
    for inline in &paragraph.inline_objects {
        non_negative(inline.width, "inline object width")?;
        non_negative(inline.height, "inline object height")?;
        let baseline = non_negative(inline.baseline, "inline object baseline")?;
        if baseline > inline.height {
            return Err(ParagraphWireError::InvalidValue("inline object baseline"));
        }
        if !inline_ids.insert(inline.id) {
            return Err(ParagraphWireError::InvalidValue("inline object id"));
        }
    }
    for family in &paragraph.fallback_families {
        validate_required_string(family, "paragraph fallback family")?;
    }
    let mut font_handles = HashSet::with_capacity(request.fonts.len());
    for font in &request.fonts {
        if font.handle.slot == 0 || font.handle.generation == 0 {
            return Err(ParagraphWireError::InvalidValue("paragraph font handle"));
        }
        validate_required_string(&font.family, "paragraph font family")?;
        if !font_handles.insert((font.handle.slot, font.handle.generation)) {
            return Err(ParagraphWireError::InvalidValue("paragraph font handle"));
        }
    }
    Ok(())
}

fn validate_ranges(
    text: &str,
    styles: &[ParagraphTextStyleRun],
    inline: &[ParagraphInlineObject],
) -> Result<(), ParagraphWireError> {
    let mut covered_until = 0;
    if !text.is_empty() && styles.is_empty() {
        return Err(ParagraphWireError::InvalidValue("style range coverage"));
    }
    if text.is_empty() && styles.len() > 1 {
        return Err(ParagraphWireError::InvalidValue("style range coverage"));
    }
    for style in styles {
        validate_range(text, style.range, "style range")?;
        if style.range.start != covered_until
            || (!text.is_empty() && style.range.start == style.range.end)
        {
            return Err(ParagraphWireError::InvalidValue("style range coverage"));
        }
        covered_until = style.range.end;
    }
    if covered_until != text.len() as u64 {
        return Err(ParagraphWireError::InvalidValue("style range coverage"));
    }
    let mut previous_inline_end = 0;
    for object in inline {
        validate_range(text, object.range, "inline object range")?;
        if object.range.start == object.range.end || object.range.start < previous_inline_end {
            return Err(ParagraphWireError::InvalidValue("inline object range"));
        }
        let start = usize::try_from(object.range.start)
            .map_err(|_| ParagraphWireError::InvalidValue("inline object range"))?;
        let end = usize::try_from(object.range.end)
            .map_err(|_| ParagraphWireError::InvalidValue("inline object range"))?;
        if text.get(start..end) != Some("\u{fffc}")
            || !styles.iter().any(|style| {
                style.range.start <= object.range.start && object.range.end <= style.range.end
            })
        {
            return Err(ParagraphWireError::InvalidValue("inline object range"));
        }
        previous_inline_end = object.range.end;
    }
    Ok(())
}

fn validate_range(
    text: &str,
    range: ParagraphRange,
    field: &'static str,
) -> Result<(), ParagraphWireError> {
    let start =
        usize::try_from(range.start).map_err(|_| ParagraphWireError::InvalidValue(field))?;
    let end = usize::try_from(range.end).map_err(|_| ParagraphWireError::InvalidValue(field))?;
    if start > end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        Err(ParagraphWireError::InvalidValue(field))
    } else {
        Ok(())
    }
}

fn validate_optional_string(
    value: Option<&str>,
    field: &'static str,
) -> Result<(), ParagraphWireError> {
    if let Some(value) = value {
        validate_required_string(value, field)?;
    }
    Ok(())
}

fn validate_required_string(value: &str, field: &'static str) -> Result<(), ParagraphWireError> {
    require_limit(field, value.len(), MAX_STRING_BYTES)?;
    if value.is_empty() || value.contains('\0') {
        Err(ParagraphWireError::InvalidValue(field))
    } else {
        Ok(())
    }
}

fn encode_range(bytes: &mut Vec<u8>, range: ParagraphRange) {
    put_u64(bytes, range.start);
    put_u64(bytes, range.end);
}

fn decode_range(reader: &mut Reader<'_>) -> Result<ParagraphRange, ParagraphWireError> {
    Ok(ParagraphRange::new(reader.u64()?, reader.u64()?))
}

fn encode_color(bytes: &mut Vec<u8>, color: ParagraphColor) {
    bytes.extend_from_slice(&[color.red, color.green, color.blue, color.alpha]);
}

fn decode_color(reader: &mut Reader<'_>) -> Result<ParagraphColor, ParagraphWireError> {
    Ok(ParagraphColor::rgba(
        reader.u8()?,
        reader.u8()?,
        reader.u8()?,
        reader.u8()?,
    ))
}

fn encode_length_prefixed_string(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), ParagraphWireError> {
    put_u32(bytes, checked_u32(value.len(), "paragraph string length")?);
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn decode_length_prefixed_string(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<String, ParagraphWireError> {
    let length = reader.u32()? as usize;
    require_limit(field, length, MAX_STRING_BYTES)?;
    let value = decode_utf8(reader.take(length)?, field)?;
    if value.is_empty() {
        return Err(ParagraphWireError::InvalidValue(field));
    }
    Ok(value)
}

fn decode_utf8(bytes: &[u8], field: &'static str) -> Result<String, ParagraphWireError> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| ParagraphWireError::InvalidUtf8(field))
}

fn option_string(
    flags: u32,
    flag: u32,
    value: String,
    field: &'static str,
) -> Result<Option<String>, ParagraphWireError> {
    if flags & flag != 0 {
        if value.is_empty() {
            Err(ParagraphWireError::InvalidValue(field))
        } else {
            Ok(Some(value))
        }
    } else if value.is_empty() {
        Ok(None)
    } else {
        Err(ParagraphWireError::InvalidValue(field))
    }
}

fn option_u64(
    flags: u32,
    flag: u32,
    value: u64,
    field: &'static str,
) -> Result<Option<u64>, ParagraphWireError> {
    if flags & flag != 0 {
        if value == 0 {
            Err(ParagraphWireError::InvalidValue(field))
        } else {
            Ok(Some(value))
        }
    } else if value == 0 {
        Ok(None)
    } else {
        Err(ParagraphWireError::InvalidValue(field))
    }
}

fn option_non_negative_f32(
    flags: u32,
    flag: u32,
    value: f32,
    field: &'static str,
) -> Result<Option<f32>, ParagraphWireError> {
    if flags & flag != 0 {
        Ok(Some(non_negative(value, field)?))
    } else if value.to_bits() == 0 {
        Ok(None)
    } else {
        Err(ParagraphWireError::InvalidValue(field))
    }
}

fn option_positive_f32(
    flags: u32,
    flag: u32,
    value: f32,
    field: &'static str,
) -> Result<Option<f32>, ParagraphWireError> {
    if flags & flag != 0 {
        Ok(Some(positive(value, field)?))
    } else if value.to_bits() == 0 {
        Ok(None)
    } else {
        Err(ParagraphWireError::InvalidValue(field))
    }
}

fn encoded_request_len(request: &WebParagraphRequest) -> Result<usize, ParagraphWireError> {
    let paragraph = &request.paragraph;
    let mut length = HEADER_LEN;
    add_length(&mut length, paragraph.text.len())?;
    add_length(&mut length, paragraph.locale.as_deref().map_or(0, str::len))?;
    for style in &paragraph.style_runs {
        add_length(&mut length, STYLE_HEADER_LEN)?;
        add_length(
            &mut length,
            style.font_family.as_deref().map_or(0, str::len),
        )?;
        add_length(&mut length, style.locale.as_deref().map_or(0, str::len))?;
        add_length(
            &mut length,
            style
                .variations
                .len()
                .checked_mul(8)
                .ok_or(ParagraphWireError::LengthMismatch)?,
        )?;
        add_length(
            &mut length,
            style
                .features
                .len()
                .checked_mul(8)
                .ok_or(ParagraphWireError::LengthMismatch)?,
        )?;
    }
    add_length(
        &mut length,
        paragraph
            .inline_objects
            .len()
            .checked_mul(INLINE_LEN)
            .ok_or(ParagraphWireError::LengthMismatch)?,
    )?;
    for family in &paragraph.fallback_families {
        add_length(&mut length, 4)?;
        add_length(&mut length, family.len())?;
    }
    for font in &request.fonts {
        add_length(&mut length, FONT_HEADER_LEN)?;
        add_length(&mut length, font.family.len())?;
    }
    require_limit("paragraph packet bytes", length, MAX_PARAGRAPH_PACKET_BYTES)?;
    Ok(length)
}

fn add_length(total: &mut usize, value: usize) -> Result<(), ParagraphWireError> {
    *total = total
        .checked_add(value)
        .ok_or(ParagraphWireError::LengthMismatch)?;
    Ok(())
}

fn require_remaining_records(
    reader: &Reader<'_>,
    count: usize,
    minimum_record_len: usize,
) -> Result<(), ParagraphWireError> {
    let minimum = count
        .checked_mul(minimum_record_len)
        .ok_or(ParagraphWireError::LengthMismatch)?;
    if minimum > reader.remaining() {
        Err(ParagraphWireError::Truncated)
    } else {
        Ok(())
    }
}

fn option_range(
    flags: u32,
    flag: u32,
    value: ParagraphRange,
    field: &'static str,
) -> Result<Option<ParagraphRange>, ParagraphWireError> {
    if flags & flag != 0 {
        Ok(Some(value))
    } else if value == ParagraphRange::new(0, 0) {
        Ok(None)
    } else {
        Err(ParagraphWireError::InvalidValue(field))
    }
}

fn option_preedit(
    flags: u32,
    range: ParagraphRange,
    selection: ParagraphRange,
) -> Result<Option<ParagraphPreedit>, ParagraphWireError> {
    if flags & FLAG_PREEDIT != 0 {
        Ok(Some(ParagraphPreedit { range, selection }))
    } else if range == ParagraphRange::new(0, 0) && selection == ParagraphRange::new(0, 0) {
        Ok(None)
    } else {
        Err(ParagraphWireError::InvalidValue("paragraph preedit"))
    }
}

fn option_color(
    flags: u32,
    value: ParagraphColor,
) -> Result<Option<ParagraphColor>, ParagraphWireError> {
    if flags & STYLE_BACKGROUND != 0 {
        Ok(Some(value))
    } else if value == ParagraphColor::default() {
        Ok(None)
    } else {
        Err(ParagraphWireError::InvalidValue("style background color"))
    }
}

fn text_align(value: ParagraphTextAlign) -> u8 {
    match value {
        ParagraphTextAlign::Left => 0,
        ParagraphTextAlign::Right => 1,
        ParagraphTextAlign::Center => 2,
        ParagraphTextAlign::Justify => 3,
        ParagraphTextAlign::Start => 4,
        ParagraphTextAlign::End => 5,
    }
}

fn decode_text_align(value: u8) -> Result<ParagraphTextAlign, ParagraphWireError> {
    Ok(match value {
        0 => ParagraphTextAlign::Left,
        1 => ParagraphTextAlign::Right,
        2 => ParagraphTextAlign::Center,
        3 => ParagraphTextAlign::Justify,
        4 => ParagraphTextAlign::Start,
        5 => ParagraphTextAlign::End,
        _ => return Err(ParagraphWireError::InvalidEnum("paragraph text align")),
    })
}

fn overflow(value: ParagraphOverflow) -> u8 {
    match value {
        ParagraphOverflow::Clip => 0,
        ParagraphOverflow::Ellipsis => 1,
        ParagraphOverflow::Fade => 2,
        ParagraphOverflow::Visible => 3,
    }
}

fn decode_overflow(value: u8) -> Result<ParagraphOverflow, ParagraphWireError> {
    Ok(match value {
        0 => ParagraphOverflow::Clip,
        1 => ParagraphOverflow::Ellipsis,
        2 => ParagraphOverflow::Fade,
        3 => ParagraphOverflow::Visible,
        _ => return Err(ParagraphWireError::InvalidEnum("paragraph overflow")),
    })
}

fn text_direction(value: ParagraphTextDirection) -> u8 {
    match value {
        ParagraphTextDirection::Auto => 0,
        ParagraphTextDirection::LeftToRight => 1,
        ParagraphTextDirection::RightToLeft => 2,
    }
}

fn decode_text_direction(value: u8) -> Result<ParagraphTextDirection, ParagraphWireError> {
    Ok(match value {
        0 => ParagraphTextDirection::Auto,
        1 => ParagraphTextDirection::LeftToRight,
        2 => ParagraphTextDirection::RightToLeft,
        _ => return Err(ParagraphWireError::InvalidEnum("paragraph text direction")),
    })
}

fn text_width_basis(value: ParagraphTextWidthBasis) -> u8 {
    match value {
        ParagraphTextWidthBasis::Parent => 0,
        ParagraphTextWidthBasis::LongestLine => 1,
    }
}

fn decode_text_width_basis(value: u8) -> Result<ParagraphTextWidthBasis, ParagraphWireError> {
    Ok(match value {
        0 => ParagraphTextWidthBasis::Parent,
        1 => ParagraphTextWidthBasis::LongestLine,
        _ => {
            return Err(ParagraphWireError::InvalidEnum(
                "paragraph text width basis",
            ))
        }
    })
}
