use fission_ir::op::TextRun;
use fission_layout::{LineMetric, TextMeasurer};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Debug, Default)]
pub struct TerminalTextMeasurer;

impl TerminalTextMeasurer {
    fn wrapped_lines(text: &str, available_width: Option<f32>) -> Vec<String> {
        let width = available_width
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(|value| value.floor().max(1.0) as usize);
        let Some(width) = width else {
            return text.split('\n').map(ToOwned::to_owned).collect();
        };

        let mut lines = Vec::new();
        for source_line in text.split('\n') {
            let mut current = String::new();
            let mut current_width = 0usize;
            for grapheme in UnicodeSegmentation::graphemes(source_line, true) {
                let grapheme_width = UnicodeWidthStr::width(grapheme).max(1);
                if current_width > 0 && current_width + grapheme_width > width {
                    lines.push(std::mem::take(&mut current));
                    current_width = 0;
                }
                current.push_str(grapheme);
                current_width += grapheme_width;
            }
            lines.push(current);
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }

    fn measure_text(text: &str, available_width: Option<f32>) -> (f32, f32) {
        let lines = Self::wrapped_lines(text, available_width);
        let width = lines
            .iter()
            .map(|line| UnicodeWidthStr::width(line.as_str()))
            .max()
            .unwrap_or(0) as f32;
        let height = lines.len().max(1) as f32;
        (width, height)
    }

    pub fn char_width(ch: char) -> usize {
        UnicodeWidthChar::width(ch).unwrap_or(1).max(1)
    }
}

impl TextMeasurer for TerminalTextMeasurer {
    fn measure(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        Self::measure_text(text, available_width)
    }

    fn get_line_metrics(
        &self,
        text: &str,
        _font_size: f32,
        available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        let mut offset = 0usize;
        Self::wrapped_lines(text, available_width)
            .into_iter()
            .map(|line| {
                let start = offset;
                offset = offset.saturating_add(line.len());
                LineMetric {
                    start_index: start,
                    end_index: offset,
                    baseline: 0.0,
                    height: 1.0,
                    width: UnicodeWidthStr::width(line.as_str()) as f32,
                }
            })
            .collect()
    }

    fn get_caret_position(
        &self,
        text: &str,
        _font_size: f32,
        available_width: Option<f32>,
        caret_index: usize,
    ) -> (f32, f32) {
        let mut consumed = 0usize;
        for (line_idx, line) in Self::wrapped_lines(text, available_width)
            .iter()
            .enumerate()
        {
            let next = consumed + line.len();
            if caret_index <= next {
                let local = caret_index.saturating_sub(consumed).min(line.len());
                let width = UnicodeWidthStr::width(&line[..local]);
                return (width as f32, line_idx as f32);
            }
            consumed = next;
        }
        (0.0, 0.0)
    }

    fn measure_rich_text(&self, runs: &[TextRun], available_width: Option<f32>) -> (f32, f32) {
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        Self::measure_text(&text, available_width)
    }
}
