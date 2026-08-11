use fission_ir::op::{RichTextAnnotation, TextParagraphStyle, TextRun};

/// Per-line metrics returned by text measurement.
///
/// When the layout engine or hit-testing code needs to know about individual lines
/// of text (e.g., for cursor positioning in a multi-line text field), it calls
/// [`TextMeasurer::get_line_metrics`] and receives a `Vec<LineMetric>`.
pub struct LineMetric {
    /// Byte index where this line starts in the source string.
    pub start_index: usize,
    /// Byte index where this line ends in the source string (exclusive).
    pub end_index: usize,
    /// Distance from the top of the line to its alphabetic baseline, in logical pixels.
    pub baseline: f32,
    /// Total height of the line (ascent + descent + leading), in logical pixels.
    pub height: f32,
    /// Measured width of the line's content, in logical pixels.
    pub width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RichTextInlineBox {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichTextLayoutInfo {
    pub width: f32,
    pub height: f32,
    pub inline_boxes: Vec<RichTextInlineBox>,
}

/// A platform-provided text measurement backend.
///
/// The layout engine does not shape or measure text itself. Instead, platform
/// backends implement `TextMeasurer` to wrap their native text engine (CoreText
/// on macOS, DirectWrite on Windows, HarfBuzz + FreeType on Linux, etc.).
///
/// All methods have default implementations that return zero-sized results, so
/// you only need to override the methods your backend supports.
///
/// # Required
///
/// * [`measure`](TextMeasurer::measure) -- must be implemented to get correct text layout.
///
/// # Optional
///
/// * [`hit_test`](TextMeasurer::hit_test) -- needed for click-to-cursor in text fields.
/// * [`get_line_metrics`](TextMeasurer::get_line_metrics) -- needed for multi-line cursor navigation.
/// * [`get_caret_position`](TextMeasurer::get_caret_position) -- needed for drawing the text cursor.
/// * [`measure_rich_text`](TextMeasurer::measure_rich_text) -- needed for mixed-style text.
pub(crate) const DEFAULT_RICH_TEXT_HIT_TEST_FONT_SIZE: f32 = 14.0;

pub trait TextMeasurer: Send + Sync {
    /// Measures single-style text and returns `(width, height)` in logical pixels.
    ///
    /// If `available_width` is `Some`, the text should be wrapped at that width.
    /// If `None`, the text is measured as a single unwrapped line.
    fn measure(&self, text: &str, font_size: f32, available_width: Option<f32>) -> (f32, f32);

    /// Returns the byte index of the character closest to the point `(x, y)`,
    /// relative to the text's origin. Used for click-to-cursor in text fields.
    ///
    /// The default implementation returns `0`.
    fn hit_test(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        _x: f32,
        _y: f32,
    ) -> usize {
        0
    }

    /// Returns per-line metrics for the given text. Used for multi-line text fields
    /// and line-based cursor navigation.
    ///
    /// The default implementation returns an empty vec.
    fn get_line_metrics(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        vec![]
    }

    /// Returns the `(x, y)` position of the text cursor at `caret_index` (byte offset),
    /// relative to the text's origin.
    ///
    /// The default implementation returns `(0.0, 0.0)`.
    fn get_caret_position(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        _caret_index: usize,
    ) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// Measures multi-style (rich) text and returns `(width, height)` in logical pixels.
    ///
    /// The default implementation returns `(0.0, 0.0)`.
    fn measure_rich_text(&self, _runs: &[TextRun], _available_width: Option<f32>) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// Measures rich text and returns positioned inline-widget boxes, if any.
    ///
    /// Backends that understand inline rich-text widget markers should override
    /// this so layout can place the child widgets at the same coordinates used
    /// by text shaping.
    fn layout_rich_text(
        &self,
        runs: &[TextRun],
        available_width: Option<f32>,
    ) -> RichTextLayoutInfo {
        let (width, height) = if runs.len() == 1 {
            let run = &runs[0];
            self.measure(&run.text, run.style.font_size, available_width)
        } else {
            self.measure_rich_text(runs, available_width)
        };
        RichTextLayoutInfo {
            width,
            height,
            inline_boxes: Vec::new(),
        }
    }

    /// Hit-test rich text (styled runs) at the given (x, y) position.
    /// Returns the byte offset into the concatenated text of all runs.
    /// Default falls back to plain hit_test using the first run's font size.
    fn hit_test_rich(
        &self,
        runs: &[TextRun],
        _available_width: Option<f32>,
        x: f32,
        y: f32,
    ) -> usize {
        // Preserve the normal body-text fallback when no run is available, so
        // fallback hit testing never asks a backend to shape zero-sized text.
        let text: String = runs.iter().map(|r| r.text.as_str()).collect();
        let font_size = runs
            .first()
            .map(|r| r.style.font_size)
            .unwrap_or(DEFAULT_RICH_TEXT_HIT_TEST_FONT_SIZE);
        self.hit_test(&text, font_size, None, x, y)
    }

    /// Resolves the rich-text annotation at the given point, if any.
    ///
    /// This is used for interactive rich-text spans that need hit testing
    /// against shaped rich text rather than box nodes.
    fn resolve_rich_text_annotation_at_point(
        &self,
        _runs: &[TextRun],
        _available_width: Option<f32>,
        _x: f32,
        _y: f32,
        _paragraph_style: TextParagraphStyle,
        _annotations: &[RichTextAnnotation],
    ) -> Option<RichTextAnnotation> {
        None
    }
}
