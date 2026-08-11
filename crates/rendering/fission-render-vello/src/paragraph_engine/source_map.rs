use std::ops::Range;

use fission_layout::{ParagraphDescription, ParagraphError, ParagraphInlineObject, Utf8Range};

/// Maps the source paragraph to the string Parley shapes.
///
/// Fission inline objects own a non-empty source placeholder range. Parley's
/// inline boxes are inserted at an index and do not consume source text, so the
/// adapter removes placeholder ranges before shaping and records both sides of
/// every collapsed boundary. This keeps every public result index in the
/// original UTF-8 source coordinate space.
#[derive(Debug)]
pub(super) struct ParagraphSourceMap {
    shaped_text: String,
    removals: Vec<Removal>,
}

#[derive(Debug, Clone, Copy)]
struct Removal {
    source: Utf8Range,
    shaped_index: usize,
}

impl ParagraphSourceMap {
    pub(super) fn new(description: &ParagraphDescription) -> Result<Self, ParagraphError> {
        validate_inline_object_order(&description.inline_objects)?;

        let mut shaped_text = String::with_capacity(description.text.len());
        let mut removals = Vec::with_capacity(description.inline_objects.len());
        let mut source_cursor = 0;

        for inline in &description.inline_objects {
            let start = inline.range.start().byte_offset();
            let end = inline.range.end().byte_offset();
            shaped_text.push_str(&description.text[source_cursor..start]);
            removals.push(Removal {
                source: inline.range,
                shaped_index: shaped_text.len(),
            });
            source_cursor = end;
        }
        shaped_text.push_str(&description.text[source_cursor..]);

        Ok(Self {
            shaped_text,
            removals,
        })
    }

    pub(super) fn text(&self) -> &str {
        &self.shaped_text
    }

    pub(super) fn len(&self) -> usize {
        self.shaped_text.len()
    }

    /// Maps a source boundary to the collapsed shaping boundary. Both ends of
    /// an inline placeholder intentionally map to the same position.
    pub(super) fn source_to_shaped(&self, source_index: usize) -> usize {
        let mut removed_before = 0;
        for removal in &self.removals {
            let start = removal.source.start().byte_offset();
            let end = removal.source.end().byte_offset();
            if source_index < start {
                break;
            }
            if source_index <= end {
                return removal.shaped_index;
            }
            removed_before += end - start;
        }
        source_index.saturating_sub(removed_before)
    }

    pub(super) fn source_range_to_shaped(&self, range: Utf8Range) -> Range<usize> {
        self.source_to_shaped(range.start().byte_offset())
            ..self.source_to_shaped(range.end().byte_offset())
    }

    /// Maps a shaping boundary to the logical source position on its
    /// downstream side (after any inline placeholders collapsed there).
    pub(super) fn shaped_to_source_downstream(&self, shaped_index: usize) -> usize {
        let mut source_index = shaped_index;
        for removal in &self.removals {
            if removal.shaped_index > shaped_index {
                break;
            }
            source_index += removal.source.len();
        }
        source_index
    }

    /// Maps a shaping boundary to the logical source position on its upstream
    /// side (before any inline placeholders collapsed there).
    pub(super) fn shaped_to_source_upstream(&self, shaped_index: usize) -> usize {
        let mut source_index = shaped_index;
        for removal in &self.removals {
            if removal.shaped_index >= shaped_index {
                break;
            }
            source_index += removal.source.len();
        }
        source_index
    }

    pub(super) fn shaped_range_to_source(&self, range: Range<usize>) -> Utf8Range {
        let shaped_start = range.start.min(self.len());
        let shaped_end = range.end.min(self.len());
        if shaped_start == shaped_end {
            let index = self.shaped_to_source_upstream(shaped_start);
            return Utf8Range::empty(index.into());
        }
        let start = self.shaped_to_source_downstream(shaped_start);
        let end = self.shaped_to_source_upstream(shaped_end);
        Utf8Range::from_byte_offsets(start, end).expect("ordered mapped source range")
    }
}

fn validate_inline_object_order(
    inline_objects: &[ParagraphInlineObject],
) -> Result<(), ParagraphError> {
    let mut previous_end = 0;
    for (index, inline) in inline_objects.iter().enumerate() {
        let start = inline.range.start().byte_offset();
        if start < previous_end {
            return Err(ParagraphError::invalid_description(
                "inline_objects.range",
                format!(
                    "inline object {index} overlaps an earlier object or is not in source order"
                ),
            ));
        }
        previous_end = inline.range.end().byte_offset();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use fission_ir::op::{Color, TextParagraphStyle, TextStyle};
    use fission_layout::{
        LayoutSize, ParagraphDescription, ParagraphInlineObject, ParagraphStyleRun, Utf8Range,
    };

    use super::ParagraphSourceMap;

    fn range(start: usize, end: usize) -> Utf8Range {
        Utf8Range::from_byte_offsets(start, end).unwrap()
    }

    fn description() -> ParagraphDescription {
        let text = "A\u{fffc}B";
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
        let mut description = ParagraphDescription::new(
            text,
            vec![ParagraphStyleRun::new(range(0, text.len()), style)],
            TextParagraphStyle::default(),
            Some(100.0),
        );
        description.inline_objects.push(ParagraphInlineObject {
            id: 7,
            range: range(1, 4),
            size: LayoutSize::new(12.0, 10.0),
            baseline: 10.0,
        });
        description
    }

    #[test]
    fn placeholder_ranges_collapse_without_losing_source_sides() {
        let map = ParagraphSourceMap::new(&description()).unwrap();

        assert_eq!(map.text(), "AB");
        assert_eq!(map.source_to_shaped(1), 1);
        assert_eq!(map.source_to_shaped(4), 1);
        assert_eq!(map.shaped_to_source_upstream(1), 1);
        assert_eq!(map.shaped_to_source_downstream(1), 4);
        assert_eq!(map.shaped_range_to_source(0..1), range(0, 1));
        assert_eq!(map.shaped_range_to_source(1..2), range(4, 5));
    }
}
