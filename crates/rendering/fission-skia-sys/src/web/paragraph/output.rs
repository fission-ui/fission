use crate::ffi;
use crate::{
    ParagraphCapabilities, ParagraphCaret, ParagraphCluster, ParagraphHitRegion,
    ParagraphInlineBox, ParagraphLine, ParagraphOutput, ParagraphRect, ParagraphSize,
    UnresolvedGlyph,
};

use super::wire::{
    checked_u32, finite, non_negative, put_f32, put_u16, put_u32, put_u64, require_limit,
    ParagraphWireError, Reader, MAX_GEOMETRY_RECORDS, MAX_PARAGRAPH_PACKET_BYTES,
    MAX_UNRESOLVED_CODEPOINTS, PARAGRAPH_WIRE_VERSION,
};
use crate::web::ResourceHandle;

const MAGIC: [u8; 4] = *b"FSPR";
const HEADER_LEN: usize = 128;
const LINE_LEN: usize = 56;
const CLUSTER_LEN: usize = 48;
const CARET_LEN: usize = 40;
const HIT_REGION_LEN: usize = 40;
const INLINE_BOX_LEN: usize = 48;
const UNRESOLVED_GLYPH_LEN: usize = 32;

const FLAG_FIRST_BASELINE: u32 = 1 << 0;
const FLAG_LAST_BASELINE: u32 = 1 << 1;
const KNOWN_FLAGS: u32 = FLAG_FIRST_BASELINE | FLAG_LAST_BASELINE;

#[derive(Debug, Clone, PartialEq)]
pub struct WebParagraphResponse {
    pub handle: ResourceHandle,
    pub approximate_bytes: u64,
    pub output: ParagraphOutput,
}

pub fn encode_paragraph_response(
    response: &WebParagraphResponse,
) -> Result<Vec<u8>, ParagraphWireError> {
    validate_response(response)?;
    let output = &response.output;
    let encoded_len = encoded_response_len(output)?;
    let mut flags = 0;
    flags |= u32::from(output.first_baseline.is_some()) * FLAG_FIRST_BASELINE;
    flags |= u32::from(output.last_baseline.is_some()) * FLAG_LAST_BASELINE;

    let mut bytes = Vec::with_capacity(encoded_len);
    bytes.extend_from_slice(&MAGIC);
    put_u16(&mut bytes, PARAGRAPH_WIRE_VERSION);
    put_u16(&mut bytes, 0);
    put_u32(&mut bytes, 0);
    put_u32(&mut bytes, flags);
    put_u32(&mut bytes, response.handle.slot);
    put_u32(&mut bytes, response.handle.generation);
    put_u64(&mut bytes, output.capabilities.bits());
    put_u32(&mut bytes, output.index_encoding);
    put_u32(
        &mut bytes,
        checked_u32(output.lines.len(), "paragraph lines")?,
    );
    put_u32(
        &mut bytes,
        checked_u32(output.clusters.len(), "paragraph clusters")?,
    );
    put_u32(
        &mut bytes,
        checked_u32(output.carets.len(), "paragraph carets")?,
    );
    put_u32(
        &mut bytes,
        checked_u32(output.hit_regions.len(), "paragraph hit regions")?,
    );
    put_u32(
        &mut bytes,
        checked_u32(output.inline_boxes.len(), "paragraph inline boxes")?,
    );
    put_u32(
        &mut bytes,
        checked_u32(
            output.unresolved_glyphs.len(),
            "paragraph unresolved glyphs",
        )?,
    );
    put_u32(
        &mut bytes,
        checked_u32(
            output.unresolved_codepoints.len(),
            "paragraph unresolved codepoints",
        )?,
    );
    put_f32(&mut bytes, output.size.width);
    put_f32(&mut bytes, output.size.height);
    put_f32(&mut bytes, output.min_intrinsic_width);
    put_f32(&mut bytes, output.max_intrinsic_width);
    put_f32(&mut bytes, output.first_baseline.unwrap_or(0.0));
    put_f32(&mut bytes, output.last_baseline.unwrap_or(0.0));
    put_u64(&mut bytes, response.approximate_bytes);
    bytes.extend_from_slice(&[0; 32]);
    debug_assert_eq!(bytes.len(), HEADER_LEN);

    for line in &output.lines {
        encode_line(&mut bytes, *line);
    }
    for cluster in &output.clusters {
        encode_cluster(&mut bytes, *cluster);
    }
    for caret in &output.carets {
        encode_caret(&mut bytes, *caret);
    }
    for hit in &output.hit_regions {
        encode_hit_region(&mut bytes, *hit);
    }
    for inline in &output.inline_boxes {
        encode_inline_box(&mut bytes, *inline);
    }
    for glyph in &output.unresolved_glyphs {
        encode_unresolved_glyph(&mut bytes, *glyph);
    }
    for codepoint in &output.unresolved_codepoints {
        put_u32(&mut bytes, *codepoint);
    }

    debug_assert_eq!(bytes.len(), encoded_len);
    let total = checked_u32(bytes.len(), "paragraph response length")?;
    bytes[8..12].copy_from_slice(&total.to_le_bytes());
    Ok(bytes)
}

pub fn decode_paragraph_response(bytes: &[u8]) -> Result<WebParagraphResponse, ParagraphWireError> {
    require_limit(
        "paragraph response bytes",
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
    let handle = ResourceHandle {
        slot: reader.u32()?,
        generation: reader.u32()?,
    };
    if handle.slot == 0 || handle.generation == 0 {
        return Err(ParagraphWireError::InvalidValue(
            "paragraph resource handle",
        ));
    }
    let capability_bits = reader.u64()?;
    let capabilities = ParagraphCapabilities::from_known_bits(capability_bits)
        .ok_or(ParagraphWireError::InvalidValue("paragraph capabilities"))?;
    let index_encoding = reader.u32()?;
    if !matches!(index_encoding, ffi::INDEX_UTF8 | ffi::INDEX_UTF16) {
        return Err(ParagraphWireError::InvalidEnum("paragraph index encoding"));
    }
    let line_count = bounded_count(reader.u32()?, "paragraph lines")?;
    let cluster_count = bounded_count(reader.u32()?, "paragraph clusters")?;
    let caret_count = bounded_count(reader.u32()?, "paragraph carets")?;
    let hit_count = bounded_count(reader.u32()?, "paragraph hit regions")?;
    let inline_count = bounded_count(reader.u32()?, "paragraph inline boxes")?;
    let unresolved_count = bounded_count(reader.u32()?, "paragraph unresolved glyphs")?;
    let codepoint_count = reader.u32()? as usize;
    require_limit(
        "paragraph unresolved codepoints",
        codepoint_count,
        MAX_UNRESOLVED_CODEPOINTS,
    )?;
    let size = ParagraphSize {
        width: non_negative(reader.f32()?, "paragraph width")?,
        height: non_negative(reader.f32()?, "paragraph height")?,
    };
    let min_intrinsic_width = non_negative(reader.f32()?, "paragraph minimum intrinsic width")?;
    let max_intrinsic_width = non_negative(reader.f32()?, "paragraph maximum intrinsic width")?;
    let first_value = reader.f32()?;
    let last_value = reader.f32()?;
    let first_baseline = optional_finite(
        flags,
        FLAG_FIRST_BASELINE,
        first_value,
        "paragraph first baseline",
    )?;
    let last_baseline = optional_finite(
        flags,
        FLAG_LAST_BASELINE,
        last_value,
        "paragraph last baseline",
    )?;
    let approximate_bytes = reader.u64()?;
    if reader.take(32)?.iter().any(|byte| *byte != 0) {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    require_remaining(&reader, line_count, LINE_LEN)?;
    let mut lines = Vec::with_capacity(line_count);
    for _ in 0..line_count {
        lines.push(decode_line(&mut reader)?);
    }
    require_remaining(&reader, cluster_count, CLUSTER_LEN)?;
    let mut clusters = Vec::with_capacity(cluster_count);
    for _ in 0..cluster_count {
        clusters.push(decode_cluster(&mut reader)?);
    }
    require_remaining(&reader, caret_count, CARET_LEN)?;
    let mut carets = Vec::with_capacity(caret_count);
    for _ in 0..caret_count {
        carets.push(decode_caret(&mut reader)?);
    }
    require_remaining(&reader, hit_count, HIT_REGION_LEN)?;
    let mut hit_regions = Vec::with_capacity(hit_count);
    for _ in 0..hit_count {
        hit_regions.push(decode_hit_region(&mut reader)?);
    }
    require_remaining(&reader, inline_count, INLINE_BOX_LEN)?;
    let mut inline_boxes = Vec::with_capacity(inline_count);
    for _ in 0..inline_count {
        inline_boxes.push(decode_inline_box(&mut reader)?);
    }
    require_remaining(&reader, unresolved_count, UNRESOLVED_GLYPH_LEN)?;
    let mut unresolved_glyphs = Vec::with_capacity(unresolved_count);
    for _ in 0..unresolved_count {
        unresolved_glyphs.push(decode_unresolved_glyph(&mut reader)?);
    }
    require_remaining(&reader, codepoint_count, 4)?;
    let mut unresolved_codepoints = Vec::with_capacity(codepoint_count);
    for _ in 0..codepoint_count {
        let codepoint = reader.u32()?;
        if char::from_u32(codepoint).is_none() {
            return Err(ParagraphWireError::InvalidValue(
                "paragraph unresolved codepoint",
            ));
        }
        unresolved_codepoints.push(codepoint);
    }
    reader.finish()?;

    let response = WebParagraphResponse {
        handle,
        approximate_bytes,
        output: ParagraphOutput {
            index_encoding,
            capabilities,
            size,
            min_intrinsic_width,
            max_intrinsic_width,
            first_baseline,
            last_baseline,
            lines: lines.into_boxed_slice(),
            clusters: clusters.into_boxed_slice(),
            carets: carets.into_boxed_slice(),
            hit_regions: hit_regions.into_boxed_slice(),
            inline_boxes: inline_boxes.into_boxed_slice(),
            unresolved_glyphs: unresolved_glyphs.into_boxed_slice(),
            unresolved_codepoints: unresolved_codepoints.into_boxed_slice(),
        },
    };
    validate_response(&response)?;
    Ok(response)
}

fn encode_line(bytes: &mut Vec<u8>, value: ParagraphLine) {
    encode_range(bytes, value.range);
    encode_rect(bytes, value.rect);
    put_f32(bytes, value.baseline);
    put_f32(bytes, value.ascent);
    put_f32(bytes, value.descent);
    put_f32(bytes, value.leading);
    bytes.push(value.hard_break as u8);
    bytes.push(value.direction as u8);
    bytes.extend_from_slice(&[0; 6]);
}

fn decode_line(reader: &mut Reader<'_>) -> Result<ParagraphLine, ParagraphWireError> {
    let range = decode_range(reader)?;
    let rect = decode_rect(reader)?;
    let baseline = finite(reader.f32()?, "paragraph line baseline")?;
    let ascent = finite(reader.f32()?, "paragraph line ascent")?;
    let descent = finite(reader.f32()?, "paragraph line descent")?;
    let leading = finite(reader.f32()?, "paragraph line leading")?;
    let hard_break = decode_bool(reader.u8()?, "paragraph line hard break")?;
    let direction = decode_direction(reader.u8()?)?;
    if reader.take(6)?.iter().any(|byte| *byte != 0) {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    Ok(ParagraphLine {
        range,
        rect,
        baseline,
        ascent,
        descent,
        leading,
        hard_break,
        direction,
    })
}

fn encode_cluster(bytes: &mut Vec<u8>, value: ParagraphCluster) {
    encode_range(bytes, value.range);
    encode_rect(bytes, value.rect);
    put_u64(bytes, value.line_index);
    bytes.push(value.direction as u8);
    bytes.push(value.starts_grapheme as u8);
    bytes.push(value.starts_word as u8);
    bytes.extend_from_slice(&[0; 5]);
}

fn decode_cluster(reader: &mut Reader<'_>) -> Result<ParagraphCluster, ParagraphWireError> {
    let value = ParagraphCluster {
        range: decode_range(reader)?,
        rect: decode_rect(reader)?,
        line_index: reader.u64()?,
        direction: decode_direction(reader.u8()?)?,
        starts_grapheme: decode_bool(reader.u8()?, "paragraph cluster grapheme")?,
        starts_word: decode_bool(reader.u8()?, "paragraph cluster word")?,
        reserved: 0,
    };
    if reader.take(5)?.iter().any(|byte| *byte != 0) {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    Ok(value)
}

fn encode_caret(bytes: &mut Vec<u8>, value: ParagraphCaret) {
    put_u64(bytes, value.index);
    bytes.push(value.affinity as u8);
    bytes.extend_from_slice(&[0; 7]);
    encode_rect(bytes, value.rect);
    put_u64(bytes, value.line_index);
}

fn decode_caret(reader: &mut Reader<'_>) -> Result<ParagraphCaret, ParagraphWireError> {
    let index = reader.u64()?;
    let affinity = decode_affinity(reader.u8()?)?;
    if reader.take(7)?.iter().any(|byte| *byte != 0) {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    Ok(ParagraphCaret {
        index,
        affinity,
        reserved: 0,
        rect: decode_rect(reader)?,
        line_index: reader.u64()?,
    })
}

fn encode_hit_region(bytes: &mut Vec<u8>, value: ParagraphHitRegion) {
    encode_rect(bytes, value.rect);
    put_u64(bytes, value.index);
    bytes.push(value.affinity as u8);
    bytes.extend_from_slice(&[0; 7]);
    put_u64(bytes, value.line_index);
}

fn decode_hit_region(reader: &mut Reader<'_>) -> Result<ParagraphHitRegion, ParagraphWireError> {
    let rect = decode_rect(reader)?;
    let index = reader.u64()?;
    let affinity = decode_affinity(reader.u8()?)?;
    if reader.take(7)?.iter().any(|byte| *byte != 0) {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    Ok(ParagraphHitRegion {
        rect,
        index,
        affinity,
        reserved: 0,
        line_index: reader.u64()?,
    })
}

fn encode_inline_box(bytes: &mut Vec<u8>, value: ParagraphInlineBox) {
    put_u64(bytes, value.id);
    encode_range(bytes, value.range);
    encode_rect(bytes, value.rect);
    put_f32(bytes, value.baseline);
    put_u32(bytes, 0);
}

fn decode_inline_box(reader: &mut Reader<'_>) -> Result<ParagraphInlineBox, ParagraphWireError> {
    let value = ParagraphInlineBox {
        id: reader.u64()?,
        range: decode_range(reader)?,
        rect: decode_rect(reader)?,
        baseline: finite(reader.f32()?, "paragraph inline baseline")?,
        reserved: 0,
    };
    if reader.u32()? != 0 {
        return Err(ParagraphWireError::NonZeroReserved);
    }
    Ok(value)
}

fn encode_unresolved_glyph(bytes: &mut Vec<u8>, value: UnresolvedGlyph) {
    encode_range(bytes, value.range);
    put_u64(bytes, value.codepoint_start);
    put_u64(bytes, value.codepoint_count);
}

fn decode_unresolved_glyph(reader: &mut Reader<'_>) -> Result<UnresolvedGlyph, ParagraphWireError> {
    Ok(UnresolvedGlyph {
        range: decode_range(reader)?,
        codepoint_start: reader.u64()?,
        codepoint_count: reader.u64()?,
    })
}

fn validate_response(response: &WebParagraphResponse) -> Result<(), ParagraphWireError> {
    if response.handle.slot == 0 || response.handle.generation == 0 {
        return Err(ParagraphWireError::InvalidValue(
            "paragraph resource handle",
        ));
    }
    let output = &response.output;
    if !matches!(output.index_encoding, ffi::INDEX_UTF8 | ffi::INDEX_UTF16) {
        return Err(ParagraphWireError::InvalidEnum("paragraph index encoding"));
    }
    non_negative(output.size.width, "paragraph width")?;
    non_negative(output.size.height, "paragraph height")?;
    non_negative(
        output.min_intrinsic_width,
        "paragraph minimum intrinsic width",
    )?;
    non_negative(
        output.max_intrinsic_width,
        "paragraph maximum intrinsic width",
    )?;
    if output.min_intrinsic_width > output.max_intrinsic_width {
        return Err(ParagraphWireError::InvalidValue(
            "paragraph intrinsic widths",
        ));
    }
    if let Some(value) = output.first_baseline {
        finite(value, "paragraph first baseline")?;
    }
    if let Some(value) = output.last_baseline {
        finite(value, "paragraph last baseline")?;
    }
    for (field, count) in [
        ("paragraph lines", output.lines.len()),
        ("paragraph clusters", output.clusters.len()),
        ("paragraph carets", output.carets.len()),
        ("paragraph hit regions", output.hit_regions.len()),
        ("paragraph inline boxes", output.inline_boxes.len()),
        (
            "paragraph unresolved glyphs",
            output.unresolved_glyphs.len(),
        ),
    ] {
        require_limit(field, count, MAX_GEOMETRY_RECORDS)?;
    }
    require_limit(
        "paragraph unresolved codepoints",
        output.unresolved_codepoints.len(),
        MAX_UNRESOLVED_CODEPOINTS,
    )?;
    for line in &output.lines {
        validate_range(line.range.start, line.range.end, "paragraph line range")?;
        validate_rect(line.rect, "paragraph line rect")?;
        finite(line.baseline, "paragraph line baseline")?;
        finite(line.ascent, "paragraph line ascent")?;
        finite(line.descent, "paragraph line descent")?;
        finite(line.leading, "paragraph line leading")?;
        validate_bool(line.hard_break, "paragraph line hard break")?;
        validate_direction(line.direction)?;
    }
    for cluster in &output.clusters {
        validate_range(
            cluster.range.start,
            cluster.range.end,
            "paragraph cluster range",
        )?;
        validate_rect(cluster.rect, "paragraph cluster rect")?;
        validate_line_index(cluster.line_index, output.lines.len())?;
        validate_direction(cluster.direction)?;
        validate_bool(cluster.starts_grapheme, "paragraph cluster grapheme")?;
        validate_bool(cluster.starts_word, "paragraph cluster word")?;
        if cluster.reserved != 0 {
            return Err(ParagraphWireError::NonZeroReserved);
        }
    }
    for caret in &output.carets {
        validate_rect(caret.rect, "paragraph caret rect")?;
        validate_line_index(caret.line_index, output.lines.len())?;
        validate_affinity(caret.affinity)?;
        if caret.reserved != 0 {
            return Err(ParagraphWireError::NonZeroReserved);
        }
    }
    for hit in &output.hit_regions {
        validate_rect(hit.rect, "paragraph hit rect")?;
        validate_line_index(hit.line_index, output.lines.len())?;
        validate_affinity(hit.affinity)?;
        if hit.reserved != 0 {
            return Err(ParagraphWireError::NonZeroReserved);
        }
    }
    for inline in &output.inline_boxes {
        validate_range(
            inline.range.start,
            inline.range.end,
            "paragraph inline range",
        )?;
        validate_rect(inline.rect, "paragraph inline rect")?;
        finite(inline.baseline, "paragraph inline baseline")?;
        if inline.reserved != 0 {
            return Err(ParagraphWireError::NonZeroReserved);
        }
    }
    for glyph in &output.unresolved_glyphs {
        validate_range(
            glyph.range.start,
            glyph.range.end,
            "paragraph unresolved glyph range",
        )?;
        let start = usize::try_from(glyph.codepoint_start).map_err(|_| {
            ParagraphWireError::InvalidValue("paragraph unresolved glyph codepoint span")
        })?;
        let count = usize::try_from(glyph.codepoint_count).map_err(|_| {
            ParagraphWireError::InvalidValue("paragraph unresolved glyph codepoint span")
        })?;
        if start
            .checked_add(count)
            .is_none_or(|end| end > output.unresolved_codepoints.len())
        {
            return Err(ParagraphWireError::InvalidValue(
                "paragraph unresolved glyph codepoint span",
            ));
        }
    }
    if output
        .unresolved_codepoints
        .iter()
        .any(|value| char::from_u32(*value).is_none())
    {
        return Err(ParagraphWireError::InvalidValue(
            "paragraph unresolved codepoint",
        ));
    }
    Ok(())
}

fn bounded_count(value: u32, field: &'static str) -> Result<usize, ParagraphWireError> {
    let value = value as usize;
    require_limit(field, value, MAX_GEOMETRY_RECORDS)?;
    Ok(value)
}

fn require_remaining(
    reader: &Reader<'_>,
    count: usize,
    record_len: usize,
) -> Result<(), ParagraphWireError> {
    let needed = count
        .checked_mul(record_len)
        .ok_or(ParagraphWireError::LengthMismatch)?;
    if needed > reader.remaining() {
        Err(ParagraphWireError::Truncated)
    } else {
        Ok(())
    }
}

fn encode_range(bytes: &mut Vec<u8>, range: ffi::TextRange) {
    put_u64(bytes, range.start);
    put_u64(bytes, range.end);
}

fn decode_range(reader: &mut Reader<'_>) -> Result<ffi::TextRange, ParagraphWireError> {
    Ok(ffi::TextRange {
        start: reader.u64()?,
        end: reader.u64()?,
    })
}

fn encode_rect(bytes: &mut Vec<u8>, rect: ParagraphRect) {
    put_f32(bytes, rect.x);
    put_f32(bytes, rect.y);
    put_f32(bytes, rect.width);
    put_f32(bytes, rect.height);
}

fn decode_rect(reader: &mut Reader<'_>) -> Result<ParagraphRect, ParagraphWireError> {
    let rect = ParagraphRect {
        x: reader.f32()?,
        y: reader.f32()?,
        width: reader.f32()?,
        height: reader.f32()?,
    };
    validate_rect(rect, "paragraph rect")?;
    Ok(rect)
}

fn validate_rect(rect: ParagraphRect, field: &'static str) -> Result<(), ParagraphWireError> {
    finite(rect.x, field)?;
    finite(rect.y, field)?;
    non_negative(rect.width, field)?;
    non_negative(rect.height, field)?;
    if !(rect.x + rect.width).is_finite() || !(rect.y + rect.height).is_finite() {
        Err(ParagraphWireError::InvalidValue(field))
    } else {
        Ok(())
    }
}

fn validate_range(start: u64, end: u64, field: &'static str) -> Result<(), ParagraphWireError> {
    if start <= end {
        Ok(())
    } else {
        Err(ParagraphWireError::InvalidValue(field))
    }
}

fn validate_line_index(index: u64, lines: usize) -> Result<(), ParagraphWireError> {
    if usize::try_from(index).is_ok_and(|index| index < lines) {
        Ok(())
    } else {
        Err(ParagraphWireError::InvalidValue("paragraph line index"))
    }
}

fn optional_finite(
    flags: u32,
    flag: u32,
    value: f32,
    field: &'static str,
) -> Result<Option<f32>, ParagraphWireError> {
    if flags & flag != 0 {
        Ok(Some(finite(value, field)?))
    } else if value.to_bits() == 0 {
        Ok(None)
    } else {
        Err(ParagraphWireError::InvalidValue(field))
    }
}

fn encoded_response_len(output: &ParagraphOutput) -> Result<usize, ParagraphWireError> {
    let records = [
        (output.lines.len(), LINE_LEN),
        (output.clusters.len(), CLUSTER_LEN),
        (output.carets.len(), CARET_LEN),
        (output.hit_regions.len(), HIT_REGION_LEN),
        (output.inline_boxes.len(), INLINE_BOX_LEN),
        (output.unresolved_glyphs.len(), UNRESOLVED_GLYPH_LEN),
        (output.unresolved_codepoints.len(), 4),
    ];
    let mut length = HEADER_LEN;
    for (count, record_len) in records {
        let bytes = count
            .checked_mul(record_len)
            .ok_or(ParagraphWireError::LengthMismatch)?;
        length = length
            .checked_add(bytes)
            .ok_or(ParagraphWireError::LengthMismatch)?;
    }
    require_limit(
        "paragraph response bytes",
        length,
        MAX_PARAGRAPH_PACKET_BYTES,
    )?;
    Ok(length)
}

fn validate_bool(value: u32, field: &'static str) -> Result<(), ParagraphWireError> {
    if value <= 1 {
        Ok(())
    } else {
        Err(ParagraphWireError::InvalidValue(field))
    }
}

fn decode_bool(value: u8, field: &'static str) -> Result<u32, ParagraphWireError> {
    validate_bool(u32::from(value), field)?;
    Ok(u32::from(value))
}

fn validate_direction(value: u32) -> Result<(), ParagraphWireError> {
    if matches!(value, ffi::DIRECTION_LTR | ffi::DIRECTION_RTL) {
        Ok(())
    } else {
        Err(ParagraphWireError::InvalidEnum("paragraph direction"))
    }
}

fn decode_direction(value: u8) -> Result<u32, ParagraphWireError> {
    let value = u32::from(value);
    validate_direction(value)?;
    Ok(value)
}

fn validate_affinity(value: u32) -> Result<(), ParagraphWireError> {
    if matches!(value, ffi::AFFINITY_UPSTREAM | ffi::AFFINITY_DOWNSTREAM) {
        Ok(())
    } else {
        Err(ParagraphWireError::InvalidEnum("paragraph affinity"))
    }
}

fn decode_affinity(value: u8) -> Result<u32, ParagraphWireError> {
    let value = u32::from(value);
    validate_affinity(value)?;
    Ok(value)
}
