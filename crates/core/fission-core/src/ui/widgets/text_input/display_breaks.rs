use fission_ir::op::TextDisplayBreak;

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

#[cfg(test)]
mod tests {
    use super::map_display_breaks_through_preedit;
    use fission_ir::op::TextDisplayBreak;

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
}
