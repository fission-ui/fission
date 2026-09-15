//! Where text decorations sit relative to the baseline.

/// Top edge, in layout coordinates, of a text decoration drawn `offset` from
/// `baseline`.
///
/// Font metrics measure decoration offsets upward from the baseline: an
/// underline's offset is negative (below the baseline) and a strikethrough's
/// is positive (through the lowercase letters). Layout y grows downward, so
/// the offset is subtracted. Adding it drew underlines through the bottom of
/// the letters and strikethroughs under the text.
pub(crate) fn decoration_top(baseline: f32, offset: f32) -> f32 {
    baseline - offset
}

#[cfg(test)]
mod decoration_position_tests {
    use super::decoration_top;

    #[test]
    fn an_underline_sits_below_the_baseline() {
        // A typical font puts the underline's top about 1.5px under the baseline.
        assert!(decoration_top(20.0, -1.5) > 20.0);
    }

    #[test]
    fn a_strikethrough_sits_above_the_baseline() {
        // And the strikethrough about 4px above it, through the x-height.
        assert!(decoration_top(20.0, 4.0) < 20.0);
    }
}
