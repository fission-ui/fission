//! Caret paint derived from the authoritative paragraph geometry.

use fission_layout::{ParagraphAffinity, ParagraphError, ParagraphResult, Utf8Index};
use fission_render::{Color, LayoutPoint, LayoutRect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ParagraphCaretPaint {
    pub(crate) rect: LayoutRect,
    pub(crate) color: Color,
    pub(crate) radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ParagraphCaretStyle {
    pub(crate) color: Color,
    pub(crate) width: Option<f32>,
    pub(crate) height: Option<f32>,
    pub(crate) radius: Option<f32>,
}

/// Resolves the existing display-list caret options against the same geometry
/// used by layout and input. No width or glyph position is inferred from text.
pub(crate) fn paragraph_caret_paint(
    result: &ParagraphResult,
    index: Option<usize>,
    paragraph_origin: LayoutPoint,
    style: ParagraphCaretStyle,
) -> Result<Option<ParagraphCaretPaint>, ParagraphError> {
    let Some(index) = index else {
        return Ok(None);
    };
    let index = Utf8Index::new(index);
    let caret = result
        .caret(index, ParagraphAffinity::Downstream)?
        .ok_or_else(|| ParagraphError::missing_geometry("caret paint"))?;
    let available_height = caret.rect.height().max(1.0);
    let height = style
        .height
        .unwrap_or(available_height)
        .clamp(1.0, available_height);
    let y = paragraph_origin.y + caret.rect.y() + (available_height - height) * 0.5;
    let rect = LayoutRect::new(
        paragraph_origin.x + caret.rect.x(),
        y,
        style.width.unwrap_or(2.0),
        height,
    );
    Ok(Some(ParagraphCaretPaint {
        rect,
        color: style.color,
        radius: style.radius.unwrap_or(0.0).max(0.0),
    }))
}

#[cfg(test)]
mod tests {
    use fission_ir::op::{Color as IrColor, TextDirection, TextParagraphStyle, TextStyle};
    use fission_layout::{
        LayoutSize, ParagraphCacheKey, ParagraphCapabilities, ParagraphCapability, ParagraphCaret,
        ParagraphDescription, ParagraphDirection, ParagraphGeometry, ParagraphLine,
        ParagraphStyleRun, Utf8Range,
    };

    use super::*;

    fn range(start: usize, end: usize) -> Utf8Range {
        Utf8Range::from_byte_offsets(start, end).unwrap()
    }

    fn paragraph() -> ParagraphResult {
        let text = "text";
        let mut description = ParagraphDescription::new(
            text,
            vec![ParagraphStyleRun::new(
                range(0, text.len()),
                TextStyle {
                    font_size: 14.0,
                    color: IrColor::BLACK,
                    underline: false,
                    font_family: None,
                    locale: None,
                    font_weight: 400,
                    font_style: Default::default(),
                    line_height: None,
                    letter_spacing: 0.0,
                    background_color: None,
                },
            )],
            TextParagraphStyle::default(),
            Some(100.0),
        );
        description.paragraph_style.text_direction = TextDirection::Ltr;
        let geometry = ParagraphGeometry::new(LayoutSize::new(40.0, 20.0))
            .with_lines(vec![ParagraphLine {
                range: range(0, text.len()),
                rect: LayoutRect::new(0.0, 0.0, 40.0, 20.0),
                baseline: 15.0,
                ascent: 12.0,
                descent: 4.0,
                leading: 4.0,
                hard_break: false,
                direction: ParagraphDirection::LeftToRight,
            }])
            .with_carets(vec![ParagraphCaret {
                index: Utf8Index::new(2),
                affinity: ParagraphAffinity::Downstream,
                rect: LayoutRect::new(12.0, 2.0, 1.0, 16.0),
                line_index: 0,
            }]);
        ParagraphResult::new(
            &description,
            ParagraphCacheKey::new(1),
            ParagraphCapabilities::NONE.with(ParagraphCapability::CaretGeometry),
            geometry,
            Vec::new(),
        )
        .unwrap()
    }

    fn black() -> Color {
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    #[test]
    fn caret_uses_authoritative_position_and_existing_style_overrides() {
        let color = Color {
            r: 10,
            g: 20,
            b: 30,
            a: 255,
        };
        let paint = paragraph_caret_paint(
            &paragraph(),
            Some(2),
            LayoutPoint::new(100.0, 50.0),
            ParagraphCaretStyle {
                color,
                width: Some(3.0),
                height: Some(8.0),
                radius: Some(2.0),
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(paint.rect, LayoutRect::new(112.0, 56.0, 3.0, 8.0));
        assert_eq!(paint.color, color);
        assert_eq!(paint.radius, 2.0);
    }

    #[test]
    fn absent_caret_option_produces_no_extra_paint() {
        assert_eq!(
            paragraph_caret_paint(
                &paragraph(),
                None,
                LayoutPoint::new(0.0, 0.0),
                ParagraphCaretStyle {
                    color: black(),
                    width: None,
                    height: None,
                    radius: None,
                },
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn invalid_or_unavailable_caret_geometry_fails_instead_of_disappearing() {
        assert!(matches!(
            paragraph_caret_paint(
                &paragraph(),
                Some(1),
                LayoutPoint::new(0.0, 0.0),
                ParagraphCaretStyle {
                    color: black(),
                    width: None,
                    height: None,
                    radius: None,
                },
            ),
            Err(ParagraphError::MissingGeometry { .. })
        ));
        assert!(matches!(
            paragraph_caret_paint(
                &paragraph(),
                Some(usize::MAX),
                LayoutPoint::new(0.0, 0.0),
                ParagraphCaretStyle {
                    color: black(),
                    width: None,
                    height: None,
                    radius: None,
                },
            ),
            Err(ParagraphError::InvalidQuery { .. })
        ));
    }
}
