use fission_ir::op::{TextDisplayBreak, TextParagraphStyleSpan};

/// Maps model-text byte offsets through an active IME replacement.
///
/// A boundary at the start of the replaced range stays before the preedit;
/// one at its end moves after the preedit. Boundaries inside replaced text are
/// temporarily omitted because that selection no longer has a stable visual
/// position until composition commits.
pub(super) fn map_display_breaks_through_preedit(
    breaks: &[TextDisplayBreak],
    replaced_range: (usize, usize),
    preedit_len_bytes: usize,
) -> Vec<TextDisplayBreak> {
    let (start, end) = replaced_range;
    if end < start {
        return Vec::new();
    }

    breaks
        .iter()
        .filter_map(|display_break| {
            let text_offset = if display_break.text_offset <= start {
                display_break.text_offset
            } else if display_break.text_offset >= end {
                display_break
                    .text_offset
                    .saturating_sub(end - start)
                    .saturating_add(preedit_len_bytes)
            } else {
                return None;
            };
            Some(TextDisplayBreak {
                text_offset,
                gap_after: display_break.gap_after,
            })
        })
        .collect()
}

pub(super) fn map_paragraph_styles_through_preedit(
    styles: &[TextParagraphStyleSpan],
    replaced_range: (usize, usize),
    preedit_len_bytes: usize,
) -> Vec<TextParagraphStyleSpan> {
    let (start, end) = replaced_range;
    if end < start {
        return Vec::new();
    }

    styles
        .iter()
        .filter_map(|style| {
            let text_offset = if style.text_offset <= start {
                style.text_offset
            } else if style.text_offset >= end {
                style
                    .text_offset
                    .saturating_sub(end - start)
                    .saturating_add(preedit_len_bytes)
            } else {
                return None;
            };
            Some(TextParagraphStyleSpan {
                text_offset,
                ..*style
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{map_display_breaks_through_preedit, map_paragraph_styles_through_preedit};
    use fission_ir::op::{
        TextAlign, TextDirection, TextDisplayBreak, TextParagraphLayoutStyle,
        TextParagraphStyleSpan,
    };

    #[test]
    fn preedit_mapping_uses_byte_offsets_and_omits_replaced_boundaries() {
        let breaks = [1, 2, 3, 4].map(|text_offset| TextDisplayBreak {
            text_offset,
            gap_after: 20.0,
        });
        let mapped = map_display_breaks_through_preedit(&breaks, (1, 3), "中".len());

        assert_eq!(
            mapped
                .iter()
                .map(|item| item.text_offset)
                .collect::<Vec<_>>(),
            vec![1, 4, 5]
        );
    }

    #[test]
    fn preedit_mapping_preserves_paragraph_styles_outside_replaced_text() {
        let styles = [
            TextParagraphStyleSpan {
                text_offset: 0,
                style: TextParagraphLayoutStyle {
                    indent: 0.0,
                    text_align: TextAlign::Start,
                    text_direction: TextDirection::Auto,
                },
            },
            TextParagraphStyleSpan {
                text_offset: 2,
                style: TextParagraphLayoutStyle {
                    indent: 16.0,
                    text_align: TextAlign::End,
                    text_direction: TextDirection::Ltr,
                },
            },
            TextParagraphStyleSpan {
                text_offset: 4,
                style: TextParagraphLayoutStyle {
                    indent: 24.0,
                    text_align: TextAlign::Center,
                    text_direction: TextDirection::Auto,
                },
            },
        ];

        let mapped = map_paragraph_styles_through_preedit(&styles, (1, 3), "中".len());

        assert_eq!(
            mapped
                .iter()
                .map(|style| style.text_offset)
                .collect::<Vec<_>>(),
            vec![0, 5]
        );
        assert_eq!(mapped[1].style.indent, 24.0);
        assert_eq!(mapped[1].style.text_align, TextAlign::Center);
    }
}
