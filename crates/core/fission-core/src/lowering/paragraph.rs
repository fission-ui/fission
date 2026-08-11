use std::collections::HashMap;

use fission_ir::op::{
    decode_inline_widget_marker, decode_text_paragraph_style, Color, FontStyle, PaintOp,
    TextParagraphStyle, TextRun, TextStyle,
};
use fission_ir::{CoreIR, Op, WidgetId};
use fission_layout::{
    LayoutSize, ParagraphDescription, ParagraphInlineObject, ParagraphStyleRun, Utf8Index,
    Utf8Range,
};

const INLINE_OBJECT_CHARACTER: char = '\u{fffc}';

pub fn build_paragraph_descriptions(ir: &CoreIR) -> HashMap<WidgetId, ParagraphDescription> {
    ir.nodes
        .iter()
        .filter_map(|(node_id, node)| {
            let Op::Paint(paint) = &node.op else {
                return None;
            };
            paragraph_description(paint).map(|description| (*node_id, description))
        })
        .collect()
}

fn paragraph_description(paint: &PaintOp) -> Option<ParagraphDescription> {
    match paint {
        PaintOp::DrawText {
            text,
            size,
            color,
            underline,
            wrap,
            caret_index,
            caret_width,
            paragraph_style,
            ..
        } => {
            let style = TextStyle {
                font_size: *size,
                color: *color,
                underline: *underline,
                font_family: None,
                locale: None,
                font_weight: 400,
                font_style: FontStyle::Normal,
                line_height: None,
                letter_spacing: 0.0,
                background_color: None,
            };
            let range = Utf8Range::from_byte_offsets(0, text.len()).ok()?;
            let mut description = ParagraphDescription::new(
                text,
                vec![ParagraphStyleRun::new(range, style)],
                resolved_paragraph_style(*paragraph_style, *caret_index, *caret_width),
                None,
            );
            description.wrap = *wrap;
            Some(description)
        }
        PaintOp::DrawRichText {
            runs,
            wrap,
            caret_index,
            caret_width,
            paragraph_style,
            ..
        } => {
            let (text, style_runs, inline_objects) = normalize_rich_runs(runs);
            let mut description = ParagraphDescription::new(
                text,
                style_runs,
                resolved_paragraph_style(*paragraph_style, *caret_index, *caret_width),
                None,
            );
            description.wrap = *wrap;
            description.inline_objects = inline_objects;
            Some(description)
        }
        _ => None,
    }
}

fn resolved_paragraph_style(
    paragraph_style: Option<TextParagraphStyle>,
    caret_index: Option<usize>,
    caret_width: Option<f32>,
) -> TextParagraphStyle {
    paragraph_style
        .or_else(|| {
            caret_index
                .is_none()
                .then(|| decode_text_paragraph_style(caret_width))
                .flatten()
        })
        .unwrap_or_default()
}

fn normalize_rich_runs(
    runs: &[TextRun],
) -> (String, Vec<ParagraphStyleRun>, Vec<ParagraphInlineObject>) {
    let mut text = String::new();
    let mut style_runs = Vec::new();
    let mut inline_objects = Vec::new();

    for run in runs {
        if run.text.is_empty() {
            let Some(marker) = decode_inline_widget_marker(run.style.font_family.as_deref()) else {
                continue;
            };
            let start = text.len();
            text.push(INLINE_OBJECT_CHARACTER);
            let range = Utf8Range::from_byte_offsets(start, text.len())
                .expect("the synthesized inline-object range is ordered");
            let mut style = run.style.clone();
            style.font_family = None;
            style_runs.push(ParagraphStyleRun::new(range, style));
            inline_objects.push(ParagraphInlineObject {
                id: marker.id,
                range,
                size: LayoutSize::new(marker.width.max(0.0), marker.height.max(0.0)),
                baseline: marker.height.max(0.0),
            });
            continue;
        }

        let start = text.len();
        text.push_str(&run.text);
        let range = Utf8Range::from_byte_offsets(start, text.len())
            .expect("a text run appended to the paragraph is ordered");
        style_runs.push(ParagraphStyleRun::new(range, run.style.clone()));
    }

    if text.is_empty() {
        let style = runs
            .first()
            .map(|run| {
                let mut style = run.style.clone();
                if decode_inline_widget_marker(style.font_family.as_deref()).is_some() {
                    style.font_family = None;
                }
                style
            })
            .unwrap_or_else(default_text_style);
        style_runs.push(ParagraphStyleRun::new(
            Utf8Range::empty(Utf8Index::new(0)),
            style,
        ));
    }

    (text, style_runs, inline_objects)
}

fn default_text_style() -> TextStyle {
    TextStyle {
        font_size: 14.0,
        color: Color::BLACK,
        underline: false,
        font_family: None,
        locale: None,
        font_weight: 400,
        font_style: FontStyle::Normal,
        line_height: None,
        letter_spacing: 0.0,
        background_color: None,
    }
}

#[cfg(test)]
mod tests {
    use fission_ir::op::{
        encode_inline_widget_marker, PaintOp, TextAlign, TextOverflow, TextParagraphStyle,
    };

    use super::*;

    fn style(size: f32) -> TextStyle {
        TextStyle {
            font_size: size,
            ..default_text_style()
        }
    }

    #[test]
    fn plain_text_keeps_paragraph_controls_and_wrap_policy() {
        let paragraph_style = TextParagraphStyle {
            text_align: TextAlign::Center,
            max_lines: Some(2),
            overflow: TextOverflow::Ellipsis,
            ..Default::default()
        };
        let paint = PaintOp::DrawText {
            text: "hello".into(),
            size: 18.0,
            color: Color::BLACK,
            underline: true,
            wrap: false,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: Some(paragraph_style),
        };

        let description = paragraph_description(&paint).unwrap();

        assert_eq!(description.text, "hello");
        assert!(!description.wrap);
        assert_eq!(description.paragraph_style, paragraph_style);
        assert_eq!(description.style_runs[0].style.font_size, 18.0);
        assert!(description.validate().is_ok());
    }

    #[test]
    fn rich_inline_markers_become_owned_source_placeholders() {
        let marker = TextRun {
            text: String::new(),
            style: TextStyle {
                font_family: Some(encode_inline_widget_marker(7, 24.0, 12.0)),
                ..style(16.0)
            },
        };
        let paint = PaintOp::DrawRichText {
            runs: vec![
                TextRun {
                    text: "A".into(),
                    style: style(16.0),
                },
                marker,
                TextRun {
                    text: "B".into(),
                    style: style(18.0),
                },
            ],
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        };

        let description = paragraph_description(&paint).unwrap();

        assert_eq!(description.text, "A\u{fffc}B");
        assert_eq!(description.inline_objects.len(), 1);
        assert_eq!(description.inline_objects[0].id, 7);
        assert_eq!(
            description.inline_objects[0].size,
            LayoutSize::new(24.0, 12.0)
        );
        assert_eq!(description.style_runs.len(), 3);
        assert!(description.validate().is_ok());
    }

    #[test]
    fn empty_rich_text_still_has_a_valid_normalized_style() {
        let paint = PaintOp::DrawRichText {
            runs: Vec::new(),
            wrap: true,
            caret_index: Some(0),
            caret_color: None,
            caret_width: Some(2.0),
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        };

        let description = paragraph_description(&paint).unwrap();

        assert!(description.text.is_empty());
        assert_eq!(description.style_runs.len(), 1);
        assert!(description.style_runs[0].range.is_empty());
        assert!(description.validate().is_ok());
    }

    #[test]
    fn descriptions_are_keyed_by_the_exact_paint_node() {
        let wrapper = WidgetId::explicit("wrapper");
        let text = WidgetId::explicit("text-paint");
        let mut ir = CoreIR::new();
        ir.add_node(
            text,
            Op::Paint(PaintOp::DrawText {
                text: "hello".into(),
                size: 14.0,
                color: Color::BLACK,
                underline: false,
                wrap: true,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style: None,
            }),
            Vec::new(),
        );
        ir.add_node(
            wrapper,
            Op::Layout(fission_ir::LayoutOp::AbsoluteFill),
            vec![text],
        );
        ir.set_root(wrapper);

        let descriptions = build_paragraph_descriptions(&ir);

        assert_eq!(descriptions.len(), 1);
        assert!(descriptions.contains_key(&text));
        assert!(!descriptions.contains_key(&wrapper));
    }
}
