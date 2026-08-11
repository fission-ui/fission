use fission_layout::{
    LayoutRect, LayoutSize, ParagraphAffinity, ParagraphCaret, ParagraphCluster,
    ParagraphDirection, ParagraphError, ParagraphGeometry, ParagraphHitRegion, ParagraphInlineBox,
    ParagraphLine, ParagraphUnresolvedGlyph, Utf8Index, Utf8Range,
};

use super::request::PackedRange;

pub(super) const INDEX_ENCODING_UTF8: u8 = 0;
pub(super) const INDEX_ENCODING_UTF16: u8 = 1;
pub(super) const DIRECTION_LEFT_TO_RIGHT: u8 = 0;
pub(super) const DIRECTION_RIGHT_TO_LEFT: u8 = 1;
pub(super) const AFFINITY_UPSTREAM: u8 = 0;
pub(super) const AFFINITY_DOWNSTREAM: u8 = 1;

/// All geometry returned by one native paragraph-layout call.
///
/// The arrays own their contents and contain only scalar records. No record
/// retains a pointer to a Skia object or requires a callback into native code.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PackedParagraphOutput {
    pub(super) index_encoding: u8,
    pub(super) size: PackedSize,
    pub(super) min_intrinsic_width: f32,
    pub(super) max_intrinsic_width: f32,
    pub(super) first_baseline: Option<f32>,
    pub(super) last_baseline: Option<f32>,
    pub(super) lines: Box<[PackedLine]>,
    pub(super) clusters: Box<[PackedCluster]>,
    pub(super) carets: Box<[PackedCaret]>,
    pub(super) hit_regions: Box<[PackedHitRegion]>,
    pub(super) inline_boxes: Box<[PackedInlineBox]>,
    pub(super) unresolved_glyphs: Box<[PackedUnresolvedGlyph]>,
    pub(super) unresolved_codepoints: Box<[u32]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedSize {
    pub(super) width: f32,
    pub(super) height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedRect {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedLine {
    pub(super) range: PackedRange,
    pub(super) rect: PackedRect,
    pub(super) baseline: f32,
    pub(super) ascent: f32,
    pub(super) descent: f32,
    pub(super) leading: f32,
    pub(super) hard_break: bool,
    pub(super) direction: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedCluster {
    pub(super) range: PackedRange,
    pub(super) rect: PackedRect,
    pub(super) line_index: u64,
    pub(super) direction: u8,
    pub(super) starts_grapheme: bool,
    pub(super) starts_word: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedCaret {
    pub(super) index: u64,
    pub(super) affinity: u8,
    pub(super) rect: PackedRect,
    pub(super) line_index: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedHitRegion {
    pub(super) rect: PackedRect,
    pub(super) index: u64,
    pub(super) affinity: u8,
    pub(super) line_index: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PackedInlineBox {
    pub(super) id: u64,
    pub(super) range: PackedRange,
    pub(super) rect: PackedRect,
    pub(super) baseline: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PackedUnresolvedGlyph {
    pub(super) range: PackedRange,
    pub(super) codepoint_start: u64,
    pub(super) codepoint_count: u64,
}

pub(super) struct DecodedParagraph {
    pub(super) geometry: ParagraphGeometry,
    pub(super) unresolved_glyphs: Vec<ParagraphUnresolvedGlyph>,
}

impl PackedParagraphOutput {
    pub(super) fn decode(self, text: &str) -> Result<DecodedParagraph, ParagraphError> {
        let indices = IndexNormalizer::new(text, self.index_encoding)?;

        let lines = self
            .lines
            .into_vec()
            .into_iter()
            .map(|line| {
                Ok(ParagraphLine {
                    range: indices.range(line.range, "lines.range")?,
                    rect: rect(line.rect),
                    baseline: line.baseline,
                    ascent: line.ascent,
                    descent: line.descent,
                    leading: line.leading,
                    hard_break: line.hard_break,
                    direction: direction(line.direction, "lines.direction")?,
                })
            })
            .collect::<Result<Vec<_>, ParagraphError>>()?;

        let clusters = self
            .clusters
            .into_vec()
            .into_iter()
            .map(|cluster| {
                Ok(ParagraphCluster {
                    range: indices.range(cluster.range, "clusters.range")?,
                    rect: rect(cluster.rect),
                    line_index: line_index(cluster.line_index, "clusters.line_index")?,
                    direction: direction(cluster.direction, "clusters.direction")?,
                    starts_grapheme: cluster.starts_grapheme,
                    starts_word: cluster.starts_word,
                })
            })
            .collect::<Result<Vec<_>, ParagraphError>>()?;

        let carets = self
            .carets
            .into_vec()
            .into_iter()
            .map(|caret| {
                Ok(ParagraphCaret {
                    index: indices.index(caret.index, "carets.index")?,
                    affinity: affinity(caret.affinity, "carets.affinity")?,
                    rect: rect(caret.rect),
                    line_index: line_index(caret.line_index, "carets.line_index")?,
                })
            })
            .collect::<Result<Vec<_>, ParagraphError>>()?;

        let hit_regions = self
            .hit_regions
            .into_vec()
            .into_iter()
            .map(|hit| {
                Ok(ParagraphHitRegion {
                    rect: rect(hit.rect),
                    index: indices.index(hit.index, "hit_regions.index")?,
                    affinity: affinity(hit.affinity, "hit_regions.affinity")?,
                    line_index: line_index(hit.line_index, "hit_regions.line_index")?,
                })
            })
            .collect::<Result<Vec<_>, ParagraphError>>()?;

        let inline_boxes = self
            .inline_boxes
            .into_vec()
            .into_iter()
            .map(|inline| {
                Ok(ParagraphInlineBox {
                    id: inline.id,
                    range: indices.range(inline.range, "inline_boxes.range")?,
                    rect: rect(inline.rect),
                    baseline: inline.baseline,
                })
            })
            .collect::<Result<Vec<_>, ParagraphError>>()?;

        let unresolved_glyphs = self
            .unresolved_glyphs
            .into_vec()
            .into_iter()
            .map(|glyph| {
                let start = line_index(glyph.codepoint_start, "unresolved_glyphs.codepoint_start")?;
                let count = line_index(glyph.codepoint_count, "unresolved_glyphs.codepoint_count")?;
                let end = start.checked_add(count).ok_or_else(|| {
                    ParagraphError::invalid_result(
                        "unresolved_glyphs.codepoints",
                        "codepoint span overflows usize",
                    )
                })?;
                let codepoints = self
                    .unresolved_codepoints
                    .get(start..end)
                    .ok_or_else(|| {
                        ParagraphError::invalid_result(
                            "unresolved_glyphs.codepoints",
                            format!(
                                "codepoint span {start}..{end} exceeds {} values",
                                self.unresolved_codepoints.len()
                            ),
                        )
                    })?
                    .to_vec();
                if codepoints
                    .iter()
                    .any(|codepoint| char::from_u32(*codepoint).is_none())
                {
                    return Err(ParagraphError::invalid_result(
                        "unresolved_glyphs.codepoints",
                        "diagnostic contains a value that is not a Unicode scalar",
                    ));
                }
                Ok(ParagraphUnresolvedGlyph {
                    range: indices.range(glyph.range, "unresolved_glyphs.range")?,
                    codepoints,
                })
            })
            .collect::<Result<Vec<_>, ParagraphError>>()?;

        let geometry = ParagraphGeometry::new(LayoutSize::new(self.size.width, self.size.height))
            .with_intrinsic_widths(self.min_intrinsic_width, self.max_intrinsic_width)
            .with_baselines(self.first_baseline, self.last_baseline)
            .with_lines(lines)
            .with_clusters(clusters)
            .with_carets(carets)
            .with_hit_regions(hit_regions)
            .with_inline_boxes(inline_boxes);

        Ok(DecodedParagraph {
            geometry,
            unresolved_glyphs,
        })
    }
}

struct IndexNormalizer<'a> {
    text: &'a str,
    encoding: IndexEncoding,
    utf16_boundaries: Vec<(u64, usize)>,
}

#[derive(Debug, Clone, Copy)]
enum IndexEncoding {
    Utf8,
    Utf16,
}

impl<'a> IndexNormalizer<'a> {
    fn new(text: &'a str, encoding: u8) -> Result<Self, ParagraphError> {
        let encoding = match encoding {
            INDEX_ENCODING_UTF8 => IndexEncoding::Utf8,
            INDEX_ENCODING_UTF16 => IndexEncoding::Utf16,
            other => {
                return Err(ParagraphError::invalid_result(
                    "index_encoding",
                    format!("unknown native text-index encoding {other}"),
                ));
            }
        };
        let mut utf16_boundaries = Vec::with_capacity(text.chars().count() + 1);
        utf16_boundaries.push((0, 0));
        let mut utf16_offset = 0_u64;
        for (byte_offset, character) in text.char_indices() {
            utf16_offset = utf16_offset
                .checked_add(character.len_utf16() as u64)
                .ok_or_else(|| {
                    ParagraphError::invalid_result(
                        "index_encoding",
                        "UTF-16 source length exceeds the native u64 contract",
                    )
                })?;
            utf16_boundaries.push((utf16_offset, byte_offset + character.len_utf8()));
        }
        Ok(Self {
            text,
            encoding,
            utf16_boundaries,
        })
    }

    fn index(&self, raw: u64, field: &'static str) -> Result<Utf8Index, ParagraphError> {
        let byte_offset = match self.encoding {
            IndexEncoding::Utf8 => usize::try_from(raw).map_err(|_| {
                ParagraphError::invalid_result(
                    field,
                    format!("native UTF-8 offset {raw} does not fit usize"),
                )
            })?,
            IndexEncoding::Utf16 => self
                .utf16_boundaries
                .binary_search_by_key(&raw, |(offset, _)| *offset)
                .map(|position| self.utf16_boundaries[position].1)
                .map_err(|_| {
                    ParagraphError::invalid_result(
                        field,
                        format!("native UTF-16 offset {raw} splits a Unicode scalar"),
                    )
                })?,
        };
        if byte_offset > self.text.len() || !self.text.is_char_boundary(byte_offset) {
            return Err(ParagraphError::invalid_result(
                field,
                format!(
                    "native offset {raw} resolves to byte {byte_offset}, which is not a UTF-8 boundary"
                ),
            ));
        }
        Ok(Utf8Index::new(byte_offset))
    }

    fn range(&self, raw: PackedRange, field: &'static str) -> Result<Utf8Range, ParagraphError> {
        if raw.start > raw.end {
            return Err(ParagraphError::invalid_result(
                field,
                format!("native range {}..{} is inverted", raw.start, raw.end),
            ));
        }
        Utf8Range::new(self.index(raw.start, field)?, self.index(raw.end, field)?).ok_or_else(
            || ParagraphError::invalid_result(field, "normalized UTF-8 range is inverted"),
        )
    }
}

fn line_index(raw: u64, field: &'static str) -> Result<usize, ParagraphError> {
    usize::try_from(raw).map_err(|_| {
        ParagraphError::invalid_result(field, format!("native index {raw} does not fit usize"))
    })
}

fn direction(raw: u8, field: &'static str) -> Result<ParagraphDirection, ParagraphError> {
    match raw {
        DIRECTION_LEFT_TO_RIGHT => Ok(ParagraphDirection::LeftToRight),
        DIRECTION_RIGHT_TO_LEFT => Ok(ParagraphDirection::RightToLeft),
        other => Err(ParagraphError::invalid_result(
            field,
            format!("unknown native paragraph direction {other}"),
        )),
    }
}

fn affinity(raw: u8, field: &'static str) -> Result<ParagraphAffinity, ParagraphError> {
    match raw {
        AFFINITY_UPSTREAM => Ok(ParagraphAffinity::Upstream),
        AFFINITY_DOWNSTREAM => Ok(ParagraphAffinity::Downstream),
        other => Err(ParagraphError::invalid_result(
            field,
            format!("unknown native caret affinity {other}"),
        )),
    }
}

fn rect(value: PackedRect) -> LayoutRect {
    LayoutRect::new(value.x, value.y, value.width, value.height)
}
