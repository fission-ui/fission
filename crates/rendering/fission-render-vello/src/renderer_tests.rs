use super::{
    map_fill_to_brush, paragraph_alignment, paragraph_fade, paragraph_line_trim,
    paragraph_line_visual_bounds, paragraph_y_offset, parse_svg_entry,
    text_background_segments_for_cluster_ranges, Painter, ParagraphFade, SvgShape,
    TextBackgroundSegment, TextClip, VelloRenderer, VelloTextMeasurer,
};
use fission_ir::op::{
    FontStyle, MouseCursor, RichTextAnnotation, TextAlign, TextDirection, TextHeightBehavior,
    TextOverflow, TextParagraphStyle, TextWidthBasis,
};
use fission_ir::{semantics::ActionTrigger, ActionEntry};
use fission_layout::TextMeasurer;
use fission_render::{
    Color as RenderColor, DisplayList, DisplayOp, Fill as RenderFill, LayoutPoint, LayoutRect,
    RenderScene, Renderer, TextStyle as RenderTextStyle,
};
use parley::FontContext;
use std::sync::{Arc, Mutex};
use vello_cpu::filter_effects::Filter;
use vello_cpu::kurbo::{Affine, BezPath, Point, Rect, Shape, Stroke};
use vello_cpu::peniko::{BlendMode, Color, Compose, FontData, GradientKind};
use vello_cpu::{Glyph, ImageSource, PaintType, Pixmap, Tint};

/// A painter that records what the encoder asked for instead of drawing it.
#[derive(Default)]
struct RecordingPainter {
    glyphs: usize,
    glyph_positions: Vec<(f32, f32)>,
    /// Distinct baselines glyphs were drawn on, one per painted line of text.
    glyph_rows: Vec<f32>,
    paths: usize,
    rects: usize,
    clip_layers: usize,
    layers: Vec<(Option<Rect>, BlendMode, f32)>,
    blurred_rects: Vec<(Rect, bool)>,
    backdrop_filters: Vec<Rect>,
    external_surfaces: Vec<(u64, u16, u16)>,
    resolve_external_surfaces: bool,
}

impl Painter for RecordingPainter {
    fn fill_path(&mut self, _: Affine, _: PaintType, _: &BezPath) {
        self.paths += 1;
    }
    fn fill_rect(&mut self, _: Affine, _: PaintType, _: &Rect) {
        self.rects += 1;
    }
    fn fill_image_rect(&mut self, _: Affine, _: PaintType, _: Option<Tint>, _: &Rect) {
        self.rects += 1;
    }
    fn stroke_path(&mut self, _: Affine, _: &Stroke, _: PaintType, _: &BezPath) {
        self.paths += 1;
    }
    fn fill_blurred_rounded_rect(
        &mut self,
        _: Affine,
        _: Color,
        rect: &Rect,
        _: f32,
        _: f32,
        invert: bool,
    ) {
        self.blurred_rects.push((*rect, invert));
    }
    fn push_layer(&mut self, _: Affine, clip: Option<&BezPath>, blend: BlendMode, opacity: f32) {
        if clip.is_some() {
            self.clip_layers += 1;
        }
        self.layers
            .push((clip.map(|clip| clip.bounding_box()), blend, opacity));
    }
    fn pop_layer(&mut self) {}
    fn apply_backdrop_filter(&mut self, _: Affine, clip: &BezPath, _: Filter) {
        self.backdrop_filters.push(clip.bounding_box());
    }
    fn fill_glyphs(
        &mut self,
        transform: Affine,
        _: PaintType,
        _: &FontData,
        _: f32,
        glyphs: &[Glyph],
    ) {
        self.glyphs += glyphs.len();
        for glyph in glyphs {
            let position = transform * Point::new(glyph.x as f64, glyph.y as f64);
            let x = position.x as f32;
            let y = position.y as f32;
            self.glyph_positions.push((x, y));
            if !self.glyph_rows.iter().any(|row| (row - y).abs() < 0.5) {
                self.glyph_rows.push(y);
            }
        }
    }
    fn image_source(&mut self, image: &Arc<Pixmap>) -> ImageSource {
        ImageSource::Pixmap(Arc::clone(image))
    }
    fn external_image_source(
        &mut self,
        texture_id: u64,
        width: u16,
        height: u16,
    ) -> Option<ImageSource> {
        self.external_surfaces.push((texture_id, width, height));
        self.resolve_external_surfaces.then(|| {
            ImageSource::external_texture(
                vello_gpu::TextureId(texture_id),
                vello_cpu::geometry::RectU16::new(0, 0, width, height),
                true,
            )
        })
    }
}

#[test]
fn normalized_gradient_geometry_maps_to_painted_bounds() {
    let brush = map_fill_to_brush(
        &RenderFill::LinearGradient {
            start: (0.0, 0.25),
            end: (1.0, 0.75),
            stops: vec![
                (
                    0.0,
                    RenderColor {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                ),
                (
                    1.0,
                    RenderColor {
                        r: 255,
                        g: 255,
                        b: 255,
                        a: 255,
                    },
                ),
            ],
            extend: Default::default(),
        },
        Rect::new(20.0, 40.0, 220.0, 140.0),
    );

    let PaintType::Gradient(gradient) = brush else {
        panic!("expected gradient brush");
    };
    let GradientKind::Linear(position) = gradient.kind else {
        panic!("expected linear gradient");
    };
    assert_eq!(position.start, Point::new(20.0, 65.0));
    assert_eq!(position.end, Point::new(220.0, 115.0));
}

#[test]
fn svg_parser_skips_fill_none_rect_placeholders() {
    let svg = r#"<svg viewBox="0 0 24 24">
        <rect fill="none" width="24" height="24"/>
        <path d="M0 0h10v10H0z"/>
    </svg>"#;
    let entry = parse_svg_entry(svg);
    assert_eq!(entry.shapes.len(), 1);
    assert!(matches!(entry.shapes[0], SvgShape::Path(_)));
}

#[test]
fn paragraph_fade_prefers_bottom_when_extra_lines_are_clipped() {
    assert_eq!(
        paragraph_fade(
            TextParagraphStyle {
                text_align: TextAlign::Start,
                max_lines: Some(1),
                overflow: TextOverflow::Fade,
                ..Default::default()
            },
            LayoutRect::new(0.0, 0.0, 120.0, 20.0),
            18.0,
            90.0,
            true,
            true,
            false,
        ),
        Some(ParagraphFade::Bottom {
            start: 9.0,
            end: 18.0,
        })
    );
}

fn test_renderer(painter: &mut RecordingPainter) -> VelloRenderer<'_> {
    let measurer = Arc::new(VelloTextMeasurer::new(Arc::new(Mutex::new(
        FontContext::new(),
    ))));
    VelloRenderer::new(painter, measurer, 1.0)
}

fn test_style() -> RenderTextStyle {
    RenderTextStyle {
        font_size: 16.0,
        color: RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        underline: false,
        font_family: None,
        locale: None,
        font_weight: 400,
        font_style: FontStyle::Normal,
        line_height: None,
        letter_spacing: 0.0,
        background_color: None,
        typography: Default::default(),
    }
}

#[test]
fn text_background_segments_match_selected_clusters_only() {
    let clusters = vec![
        (0..1, 0.0, 10.0),
        (1..2, 10.0, 20.0),
        (2..3, 20.0, 30.0),
        (3..4, 30.0, 40.0),
    ];

    let segments = text_background_segments_for_cluster_ranges(clusters, &(1..2), None);

    assert_eq!(
        segments,
        vec![TextBackgroundSegment {
            left: 10.0,
            right: 20.0
        }]
    );
}

#[test]
fn text_background_segments_clip_and_merge_adjacent_clusters() {
    let clusters = vec![
        (0..1, 0.0, 10.0),
        (1..2, 10.0, 20.0),
        (2..3, 20.0, 30.0),
        (3..4, 30.0, 40.0),
    ];

    let segments = text_background_segments_for_cluster_ranges(
        clusters,
        &(1..3),
        Some(TextClip {
            left: 12.0,
            right: 27.0,
            top: 0.0,
            bottom: 40.0,
        }),
    );

    assert_eq!(
        segments,
        vec![TextBackgroundSegment {
            left: 12.0,
            right: 27.0
        }]
    );
}

#[test]
fn selected_character_background_does_not_span_full_line() {
    let mut painter = RecordingPainter::default();
    let renderer = test_renderer(&mut painter);
    let base_style = test_style();
    let mut selected_style = test_style();
    selected_style.background_color = Some(RenderColor {
        r: 0,
        g: 80,
        b: 255,
        a: 120,
    });
    let text = "abcdef";
    let styles = vec![(0..text.len(), base_style.clone()), (2..3, selected_style)];
    let layout = renderer.paragraph_layout(
        text,
        &base_style,
        false,
        LayoutRect::new(0.0, 0.0, 240.0, 40.0),
        TextParagraphStyle::default(),
        &[],
        &styles,
    );
    let line = layout.lines().next().expect("single test line");
    let line_bounds = paragraph_line_visual_bounds(&line).expect("line visual bounds");
    let mut segments = Vec::new();
    for run in line.runs() {
        segments.extend(text_background_segments_for_cluster_ranges(
            run.visual_clusters().filter_map(|cluster| {
                let left = cluster.visual_offset()?;
                Some((cluster.text_range(), left, left + cluster.advance()))
            }),
            &(2..3),
            None,
        ));
    }

    assert_eq!(segments.len(), 1);
    let selected = segments[0];
    assert!(selected.left > line_bounds.left + 1.0);
    assert!(selected.right < line_bounds.right - 1.0);
    assert!(
        selected.right - selected.left < (line_bounds.right - line_bounds.left) * 0.5,
        "single selected character should not highlight the full line"
    );
}

#[test]
fn justify_alignment_stretches_non_terminal_lines() {
    let mut painter = RecordingPainter::default();
    let renderer = test_renderer(&mut painter);
    let style = test_style();
    let text = "one two three four five six seven eight";
    let bounds = LayoutRect::new(0.0, 0.0, 90.0, 200.0);
    let styles = vec![(0..text.len(), style.clone())];

    let start_layout = renderer.paragraph_layout(
        text,
        &style,
        true,
        bounds,
        TextParagraphStyle {
            text_align: TextAlign::Start,
            max_lines: None,
            overflow: TextOverflow::Visible,
            ..Default::default()
        },
        &[],
        &styles,
    );
    let justify_layout = renderer.paragraph_layout(
        text,
        &style,
        true,
        bounds,
        TextParagraphStyle {
            text_align: TextAlign::Justify,
            max_lines: None,
            overflow: TextOverflow::Visible,
            ..Default::default()
        },
        &[],
        &styles,
    );

    let start_lines: Vec<_> = start_layout.lines().collect();
    let justify_lines: Vec<_> = justify_layout.lines().collect();
    assert!(start_lines.len() > 1, "expected the sample text to wrap");
    assert_eq!(start_lines.len(), justify_lines.len());

    let start_first = paragraph_line_visual_bounds(&start_lines[0]).unwrap();
    let start_last = paragraph_line_visual_bounds(start_lines.last().unwrap()).unwrap();
    let justify_first = paragraph_line_visual_bounds(&justify_lines[0]).unwrap();
    let justify_last = paragraph_line_visual_bounds(justify_lines.last().unwrap()).unwrap();

    assert!(justify_first.right > start_first.right + 1.0);
    assert!(justify_first.right - justify_first.left > start_first.right - start_first.left);
    assert!(justify_last.right - justify_last.left <= start_last.right - start_last.left + 0.5);
    assert_eq!(
        paragraph_alignment(TextAlign::Justify),
        super::ParleyAlignment::Justify
    );
}

#[test]
fn wrapped_centred_text_aligns_within_its_own_box() {
    let mut painter = RecordingPainter::default();
    let renderer = test_renderer(&mut painter);
    let style = test_style();
    let text = "No emails here";
    let styles = vec![(0..text.len(), style.clone())];
    let paragraph = TextParagraphStyle {
        text_align: TextAlign::Center,
        text_width_basis: TextWidthBasis::Parent,
        ..Default::default()
    };
    let natural = renderer.paragraph_layout(
        text,
        &style,
        false,
        LayoutRect::new(0.0, 0.0, 1000.0, 40.0),
        TextParagraphStyle::default(),
        &[],
        &styles,
    );
    // Layout sized the box to the text and centred the box; the paragraph was broken against
    // the wider width layout offered.
    let own_box = LayoutRect::new(0.0, 0.0, natural.width(), 40.0);
    let wrap_bounds = LayoutRect::new(0.0, 0.0, 400.0, 40.0);
    let prepared = super::prepare_paragraph_layout(text, &style, paragraph, &[], &styles, None);
    let layout =
        renderer.paragraph_layout_from_prepared(&prepared, true, wrap_bounds, own_box, paragraph);
    let first = paragraph_line_visual_bounds(&layout.lines().next().unwrap()).unwrap();
    assert!(
        first.left.abs() < 1.0,
        "text already centred by layout starts at its box, not {} along the wrap width",
        first.left
    );
}

#[test]
fn text_wrapped_by_layout_is_painted_wrapped() {
    let color = RenderColor {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    // Layout wrapped this heading onto two lines at the width it was offered, even though
    // the draw op itself does not ask to wrap.
    let resolved = fission_render::ResolvedParagraphLayout {
        constraint_width: Some(150.0),
        size: fission_render::LayoutSize::new(113.34375, 86.5793),
        lines: Vec::new(),
        inline_boxes: Vec::new(),
        clusters: Vec::new(),
        glyphs: Vec::new(),
        caret_stops: Vec::new(),
        selection_boxes: Vec::new(),
        display_breaks: Vec::new(),
    };
    let mut painter = RecordingPainter::default();
    {
        let mut renderer = test_renderer(&mut painter);
        renderer.render_text(
            "Fission Inbox",
            36.0,
            color,
            false,
            false,
            LayoutPoint::new(0.0, 0.0),
            LayoutRect::new(0.0, 0.0, 113.34375, 86.5793),
            Some(&resolved),
            None,
            None,
            None,
            None,
            None,
            None,
            &[],
            &[],
        );
    }
    assert_eq!(
        painter.glyph_rows.len(),
        2,
        "text layout wrapped onto two lines is painted on two lines, rows at {:?}",
        painter.glyph_rows
    );
}

#[test]
fn rich_text_display_break_paints_the_next_segment_after_its_gap() {
    let gap = 28.0;
    let text = "abécd";
    let painter = render_ops(vec![DisplayOp::DrawRichText {
        runs: vec![fission_render::TextRun {
            text: text.into(),
            style: test_style(),
        }],
        display_breaks: vec![fission_ir::op::TextDisplayBreak {
            text_offset: "abé".len(),
            gap_after: gap,
        }],
        paragraph_styles: Vec::new(),
        position: LayoutPoint::new(0.0, 0.0),
        bounds: LayoutRect::new(0.0, 0.0, 200.0, 120.0),
        node_id: None,
        wrap: false,
        caret_index: None,
        caret_color: None,
        caret_width: None,
        caret_height: None,
        caret_radius: None,
        paragraph_style: None,
        annotations: Vec::new(),
        resolved_layout: None,
    }]);
    let mut rows = painter.glyph_rows;
    rows.sort_by(f32::total_cmp);

    assert_eq!(
        rows.len(),
        2,
        "one baseline should be painted for each segment"
    );
    assert!(rows[1] - rows[0] > gap);
}

#[test]
fn rich_text_mixed_paragraphs_render_wrapped_indentation_and_alignment() {
    // Keep each word narrower than the 70px content measure. An overflowing
    // unbreakable word is intentionally allowed to extend left when end-aligned.
    let text = "title\nshort words wrap in this inset end";
    let second_paragraph = "title\n".len();
    let painter = render_ops(vec![DisplayOp::DrawRichText {
        runs: vec![fission_render::TextRun {
            text: text.into(),
            style: test_style(),
        }],
        display_breaks: Vec::new(),
        paragraph_styles: vec![
            fission_ir::op::TextParagraphStyleSpan {
                text_offset: 0,
                style: fission_ir::op::TextParagraphLayoutStyle {
                    indent: 0.0,
                    text_align: TextAlign::Start,
                    text_direction: TextDirection::Ltr,
                },
            },
            fission_ir::op::TextParagraphStyleSpan {
                text_offset: second_paragraph,
                style: fission_ir::op::TextParagraphLayoutStyle {
                    indent: 20.0,
                    text_align: TextAlign::End,
                    text_direction: TextDirection::Ltr,
                },
            },
        ],
        position: LayoutPoint::new(0.0, 0.0),
        bounds: LayoutRect::new(0.0, 0.0, 90.0, 160.0),
        node_id: None,
        wrap: true,
        caret_index: None,
        caret_color: None,
        caret_width: None,
        caret_height: None,
        caret_radius: None,
        paragraph_style: None,
        annotations: Vec::new(),
        resolved_layout: None,
    }]);

    let mut row_min_x = Vec::<(f32, f32)>::new();
    for (x, y) in painter.glyph_positions {
        if let Some((_, min_x)) = row_min_x
            .iter_mut()
            .find(|(row_y, _)| (*row_y - y).abs() < 0.5)
        {
            *min_x = min_x.min(x);
        } else {
            row_min_x.push((y, x));
        }
    }
    row_min_x.sort_by(|left, right| left.0.total_cmp(&right.0));

    assert!(row_min_x.len() >= 3, "the indented paragraph wraps");
    assert!(row_min_x[0].1 < 20.0, "the first paragraph has no indent");
    assert!(
        row_min_x[1..].iter().all(|(_, x)| *x >= 20.0),
        "all wrapped lines in the second paragraph respect its leading inset: {row_min_x:?}"
    );
    assert!(
        row_min_x.last().unwrap().1 > row_min_x[1].1,
        "the shorter final line is aligned to the paragraph end"
    );
}

#[test]
fn longest_line_width_basis_aligns_against_content_width() {
    let mut painter = RecordingPainter::default();
    let renderer = test_renderer(&mut painter);
    let style = test_style();
    let text = "paragraph width\nshort";
    let bounds = LayoutRect::new(0.0, 0.0, 220.0, 80.0);
    let styles = vec![(0..text.len(), style.clone())];

    let parent_layout = renderer.paragraph_layout(
        text,
        &style,
        false,
        bounds,
        TextParagraphStyle {
            text_align: TextAlign::Center,
            text_width_basis: TextWidthBasis::Parent,
            ..Default::default()
        },
        &[],
        &styles,
    );
    let longest_line_layout = renderer.paragraph_layout(
        text,
        &style,
        false,
        bounds,
        TextParagraphStyle {
            text_align: TextAlign::Center,
            text_width_basis: TextWidthBasis::LongestLine,
            ..Default::default()
        },
        &[],
        &styles,
    );

    let parent_lines: Vec<_> = parent_layout.lines().collect();
    let longest_line_lines: Vec<_> = longest_line_layout.lines().collect();
    let parent_first = paragraph_line_visual_bounds(&parent_lines[0]).unwrap();
    let parent_second = paragraph_line_visual_bounds(&parent_lines[1]).unwrap();
    let longest_first = paragraph_line_visual_bounds(&longest_line_lines[0]).unwrap();
    let longest_second = paragraph_line_visual_bounds(&longest_line_lines[1]).unwrap();

    assert!(parent_first.left > longest_first.left + 5.0);
    assert!(parent_second.left > longest_second.left + 5.0);
    assert!((longest_first.left - bounds.x()).abs() < 1.0);
}

#[test]
fn fade_overflow_adds_renderer_side_clips() {
    let mut painter = RecordingPainter::default();
    let mut renderer = test_renderer(&mut painter);
    let style = test_style();
    let text = "this line should visibly fade instead of only clipping";

    renderer.render_paragraph_text(
        text,
        &style,
        false,
        LayoutPoint::new(0.0, 0.0),
        LayoutRect::new(0.0, 0.0, 80.0, 24.0),
        LayoutRect::new(0.0, 0.0, 80.0, 24.0),
        TextParagraphStyle {
            text_align: TextAlign::Start,
            max_lines: None,
            overflow: TextOverflow::Fade,
            ..Default::default()
        },
        &[],
        &[(0..text.len(), style.clone())],
        None,
        None,
        None,
        None,
        None,
        None,
    );
    drop(renderer);

    assert!(
        painter.clip_layers > 0,
        "fade overflow should add internal clip layers"
    );
}

#[test]
fn simple_text_rendering_culls_glyphs_outside_bounds() {
    let mut painter = RecordingPainter::default();
    let mut renderer = test_renderer(&mut painter);
    let text = "M".repeat(20_000);

    renderer.render_text(
        &text,
        16.0,
        RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        false,
        false,
        LayoutPoint::new(0.0, 0.0),
        LayoutRect::new(0.0, 0.0, 120.0, 32.0),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        &[],
        &[],
    );
    drop(renderer);

    let glyphs = painter.glyphs;
    assert!(glyphs > 0, "visible glyphs should still be encoded");
    assert!(
        glyphs < 256,
        "renderer should not encode the full off-bounds text run; glyphs={glyphs}"
    );
}

#[test]
fn rich_text_effects_are_encoded_through_the_normal_vello_path() {
    let text = "effects";
    let mut style = test_style();
    style.typography.shadows.push(fission_ir::op::TextShadow {
        color: fission_ir::op::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 160,
        },
        offset: (2.0, 3.0),
        blur_radius: 6.0,
    });
    style.typography.decoration.lines.underline = true;
    style.typography.decoration.style = fission_ir::op::TextDecorationStyle::Wavy;
    let styles = vec![(0..text.len(), style.clone())];
    let mut painter = RecordingPainter::default();
    let mut renderer = test_renderer(&mut painter);

    renderer.render_text(
        text,
        style.font_size,
        style.color,
        false,
        false,
        LayoutPoint::new(0.0, 0.0),
        LayoutRect::new(0.0, 0.0, 180.0, 48.0),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        &[],
        &styles,
    );
    drop(renderer);

    assert!(
        painter.glyphs > text.chars().count(),
        "blurred shadow samples must encode additional glyph draws"
    );
    assert!(
        painter.paths > 0,
        "wavy decoration must contribute path geometry"
    );
}

#[test]
fn rich_multiline_text_encodes_only_lines_inside_the_visible_bounds() {
    let text = "version = 4\n".repeat(10_000);
    let style = test_style();
    let styles = vec![(0..text.len(), style.clone())];
    let mut painter = RecordingPainter::default();
    let mut renderer = test_renderer(&mut painter);
    let viewport = Rect::new(0.0, 0.0, 400.0, 80.0);
    let transform = renderer.current_transform;
    renderer.painter.push_layer(
        transform,
        Some(&viewport.to_path(0.1)),
        BlendMode::default(),
        1.0,
    );
    renderer.push_clip_bounds(viewport);

    renderer.render_text(
        &text,
        style.font_size,
        style.color,
        false,
        false,
        LayoutPoint::new(0.0, 0.0),
        LayoutRect::new(0.0, 0.0, 400.0, 200_000.0),
        None,
        Some(0),
        Some(style.color),
        Some(2.0),
        None,
        None,
        None,
        &[],
        &styles,
    );
    renderer.painter.pop_layer();
    drop(renderer);

    let glyphs = painter.glyphs;
    assert!(
        glyphs < 256,
        "renderer should not encode offscreen rich-text lines; glyphs={glyphs}"
    );
}

#[test]
fn explicit_text_direction_realigns_neutral_content() {
    let mut painter = RecordingPainter::default();
    let renderer = test_renderer(&mut painter);
    let style = test_style();
    let text = "12345";
    let bounds = LayoutRect::new(0.0, 0.0, 120.0, 40.0);
    let styles = vec![(0..text.len(), style.clone())];

    let ltr_layout = renderer.paragraph_layout(
        text,
        &style,
        false,
        bounds,
        TextParagraphStyle {
            text_align: TextAlign::Start,
            text_direction: TextDirection::Ltr,
            ..Default::default()
        },
        &[],
        &styles,
    );
    let rtl_layout = renderer.paragraph_layout(
        text,
        &style,
        false,
        bounds,
        TextParagraphStyle {
            text_align: TextAlign::Start,
            text_direction: TextDirection::Rtl,
            ..Default::default()
        },
        &[],
        &styles,
    );

    let ltr_bounds = paragraph_line_visual_bounds(&ltr_layout.lines().next().unwrap()).unwrap();
    let rtl_bounds = paragraph_line_visual_bounds(&rtl_layout.lines().next().unwrap()).unwrap();

    assert!(rtl_bounds.left > ltr_bounds.left + 5.0);
}

#[test]
fn paragraph_strut_height_raises_line_metrics() {
    let mut painter = RecordingPainter::default();
    let renderer = test_renderer(&mut painter);
    let style = test_style();
    let text = "line";
    let styles = vec![(0..text.len(), style.clone())];

    let default_layout = renderer.paragraph_layout(
        text,
        &style,
        false,
        LayoutRect::new(0.0, 0.0, 80.0, 40.0),
        TextParagraphStyle::default(),
        &[],
        &styles,
    );
    let strut_layout = renderer.paragraph_layout(
        text,
        &style,
        false,
        LayoutRect::new(0.0, 0.0, 80.0, 40.0),
        TextParagraphStyle {
            strut_line_height: Some(28.0),
            ..Default::default()
        },
        &[],
        &styles,
    );

    let default_height = default_layout.lines().next().unwrap().metrics().line_height;
    let strut_height = strut_layout.lines().next().unwrap().metrics().line_height;

    assert!(strut_height > default_height + 5.0);
}

#[test]
fn text_height_behavior_can_trim_first_line_leading() {
    let mut painter = RecordingPainter::default();
    let renderer = test_renderer(&mut painter);
    let mut style = test_style();
    style.line_height = Some(30.0);
    let text = "trimmed";
    let styles = vec![(0..text.len(), style.clone())];
    let behavior = TextHeightBehavior {
        apply_height_to_first_ascent: false,
        apply_height_to_last_descent: true,
    };
    let layout = renderer.paragraph_layout(
        text,
        &style,
        false,
        LayoutRect::new(0.0, 0.0, 120.0, 60.0),
        TextParagraphStyle {
            text_height_behavior: behavior,
            ..Default::default()
        },
        &[],
        &styles,
    );
    let lines: Vec<_> = layout.lines().collect();
    let (top_trim, bottom_trim) = paragraph_line_trim(&lines[0], behavior, true, true);

    assert!(top_trim > 0.0);
    assert_eq!(bottom_trim, 0.0);
    assert!(paragraph_y_offset(lines.first(), behavior, true) < 0.0);
}

#[test]
fn rich_text_annotation_hit_testing_prefers_nested_span_metadata() {
    let mut painter = RecordingPainter::default();
    let renderer = test_renderer(&mut painter);
    let style = test_style();
    let text = "Read docs now";
    let styles = vec![(0..text.len(), style.clone())];
    let bounds = LayoutRect::new(0.0, 0.0, 160.0, 40.0);
    let annotations = vec![
        RichTextAnnotation {
            range: 0..13,
            semantics_label: None,
            semantics_identifier: None,
            spell_out: None,
            mouse_cursor: Some(MouseCursor::Pointer),
            actions: vec![ActionEntry {
                trigger: ActionTrigger::Default,
                action_id: 1,
                payload_data: Some(vec![1]),
            }],
            hyperlink: None,
        },
        RichTextAnnotation {
            range: 5..9,
            semantics_label: Some("documentation".into()),
            semantics_identifier: Some("docs-link".into()),
            spell_out: Some(true),
            mouse_cursor: None,
            actions: vec![ActionEntry {
                trigger: ActionTrigger::HoverEnter,
                action_id: 2,
                payload_data: Some(vec![2]),
            }],
            hyperlink: None,
        },
    ];
    let layout = renderer.paragraph_layout(
        text,
        &style,
        false,
        bounds,
        TextParagraphStyle::default(),
        &[],
        &styles,
    );
    let line = layout.lines().next().unwrap();
    let x_start = renderer
        .measurer
        .get_caret_position(text, style.font_size, None, 5)
        .0;
    let x_end = renderer
        .measurer
        .get_caret_position(text, style.font_size, None, 9)
        .0;
    let y = line.metrics().baseline - (line.metrics().ascent * 0.5);

    let resolved = renderer
        .paragraph_annotation_at_point(
            text,
            &style,
            false,
            bounds,
            TextParagraphStyle::default(),
            &[],
            &styles,
            &annotations,
            (x_start + x_end) * 0.5,
            y,
        )
        .expect("nested annotation hit");

    assert_eq!(resolved.range, 5..9);
    assert_eq!(resolved.semantics_label.as_deref(), Some("documentation"));
    assert_eq!(resolved.semantics_identifier.as_deref(), Some("docs-link"));
    assert_eq!(resolved.mouse_cursor, Some(MouseCursor::Pointer));
    assert!(resolved
        .actions
        .iter()
        .any(|action| { action.trigger == ActionTrigger::Default && action.action_id == 1 }));
    assert!(resolved
        .actions
        .iter()
        .any(|action| { action.trigger == ActionTrigger::HoverEnter && action.action_id == 2 }));
}

fn black() -> RenderColor {
    RenderColor {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    }
}

fn solid_stroke(width: f32) -> fission_render::Stroke {
    fission_render::Stroke {
        fill: RenderFill::Solid(black()),
        width,
        dash_array: None,
        dash_offset: 0.0,
        trim: None,
        line_cap: fission_render::LineCap::Butt,
        line_join: fission_render::LineJoin::Miter,
    }
}

fn render_ops(ops: Vec<DisplayOp>) -> RecordingPainter {
    render_ops_with_backdrop_filters(ops, true)
}

fn render_ops_with_backdrop_filters(
    ops: Vec<DisplayOp>,
    backdrop_filters_enabled: bool,
) -> RecordingPainter {
    let bounds = LayoutRect::new(0.0, 0.0, 200.0, 200.0);
    let mut list = DisplayList::new(bounds);
    for op in ops {
        list.push(op);
    }
    let mut painter = RecordingPainter::default();
    test_renderer(&mut painter)
        .with_backdrop_filters(backdrop_filters_enabled)
        .render_scene(&RenderScene::from_display_list(list))
        .expect("render test scene");
    painter
}

#[test]
fn gpu_surface_paints_the_bound_external_texture_at_device_resolution() {
    let rect = LayoutRect::new(10.0, 20.0, 120.0, 80.0);
    let mut list = DisplayList::new(LayoutRect::new(0.0, 0.0, 200.0, 200.0));
    list.push(DisplayOp::DrawSurface {
        rect,
        surface_id: 42,
        position: 0,
        bounds: rect,
        node_id: None,
    });
    let mut painter = RecordingPainter {
        resolve_external_surfaces: true,
        ..RecordingPainter::default()
    };
    let measurer = Arc::new(VelloTextMeasurer::new(Arc::new(Mutex::new(
        FontContext::new(),
    ))));
    VelloRenderer::new(&mut painter, measurer, 2.0)
        .render_scene(&RenderScene::from_display_list(list))
        .expect("render external surface");

    assert_eq!(painter.external_surfaces, [(42, 240, 160)]);
    assert_eq!(painter.rects, 1);
}

fn draw_rect(
    stroke: Option<fission_render::Stroke>,
    border_sides: Option<fission_render::BorderSides>,
    shadow: Option<fission_render::BoxShadow>,
) -> DisplayOp {
    let rect = LayoutRect::new(10.0, 10.0, 100.0, 60.0);
    DisplayOp::DrawRect {
        rect,
        fill: Some(RenderFill::Solid(black())),
        stroke,
        corner_radius: 8.0,
        shadow,
        bounds: rect,
        node_id: None,
        corner_radii: None,
        border_sides,
    }
}

#[test]
fn backdrop_filters_reach_the_painter_clipped_to_the_rounded_box() {
    let rect = LayoutRect::new(20.0, 30.0, 80.0, 40.0);
    let painter = render_ops(vec![DisplayOp::BackdropFilter {
        rect,
        filter: fission_ir::op::BackdropFilter::Chain(vec![
            fission_ir::op::BackdropFilter::Blur(12.0),
            fission_ir::op::BackdropFilter::Saturate(1.8),
        ]),
        corner_radius: 10.0,
        bounds: rect,
        node_id: None,
        corner_radii: None,
    }]);
    assert_eq!(painter.backdrop_filters.len(), 1);
    let clip = painter.backdrop_filters[0];
    assert!((clip.x0 - 20.0).abs() < 0.01 && (clip.x1 - 100.0).abs() < 0.01);
}

#[test]
fn software_fallback_can_omit_backdrop_filters() {
    let rect = LayoutRect::new(0.0, 0.0, 200.0, 200.0);
    let painter = render_ops_with_backdrop_filters(
        vec![
            DisplayOp::BackdropFilter {
                rect,
                filter: fission_ir::op::BackdropFilter::Blur(4.0),
                corner_radius: 0.0,
                bounds: rect,
                node_id: None,
                corner_radii: None,
            },
            draw_rect(None, None, None),
        ],
        false,
    );
    assert!(painter.backdrop_filters.is_empty());
    assert_eq!(painter.paths, 1, "the modal scrim still paints");
}

#[test]
fn a_no_op_backdrop_filter_does_not_cost_a_layer() {
    let rect = LayoutRect::new(20.0, 30.0, 80.0, 40.0);
    let painter = render_ops(vec![DisplayOp::BackdropFilter {
        rect,
        filter: fission_ir::op::BackdropFilter::Blur(0.0),
        corner_radius: 0.0,
        bounds: rect,
        node_id: None,
        corner_radii: None,
    }]);
    assert!(painter.backdrop_filters.is_empty());
}

#[test]
fn a_uniform_border_fills_a_single_ring_without_wedges() {
    let painter = render_ops(vec![draw_rect(Some(solid_stroke(2.0)), None, None)]);
    // Background plus one ring; no per-edge clip layers.
    assert_eq!(painter.paths, 2);
    assert_eq!(painter.clip_layers, 0);
}

#[test]
fn differing_edges_are_painted_through_corner_wedges() {
    let sides = fission_render::BorderSides {
        top: Some(solid_stroke(1.0)),
        right: None,
        bottom: Some(solid_stroke(4.0)),
        left: None,
    };
    let painter = render_ops(vec![draw_rect(None, Some(sides), None)]);
    assert_eq!(
        painter.clip_layers, 2,
        "each stroked edge is clipped to its own wedge"
    );
    assert_eq!(painter.paths, 3, "background plus one ring fill per edge");
}

#[test]
fn outer_shadows_are_clipped_to_outside_the_box() {
    let shadow = fission_render::BoxShadow {
        color: black(),
        blur_radius: 0.0,
        spread_radius: 3.0,
        offset: (0.0, 0.0),
        inset: false,
    };
    let painter = render_ops(vec![draw_rect(None, None, Some(shadow))]);
    assert_eq!(painter.blurred_rects.len(), 1);
    assert_eq!(
        painter.clip_layers, 1,
        "an outer shadow is clipped so it does not fill the box it surrounds"
    );
}

#[test]
fn inset_shadows_paint_an_inverted_blur_inside_the_box() {
    let shadow = fission_render::BoxShadow {
        color: black(),
        blur_radius: 6.0,
        spread_radius: 2.0,
        offset: (0.0, 0.0),
        inset: true,
    };
    let painter = render_ops(vec![draw_rect(None, None, Some(shadow))]);
    assert_eq!(painter.blurred_rects.len(), 1);
    let (rect, invert) = painter.blurred_rects[0];
    assert!(
        invert,
        "an inset shadow is the inverse of a blurred rectangle"
    );
    assert!(rect.x0 > 10.0 && rect.x1 < 110.0, "shrunk by the spread");
    assert_eq!(painter.clip_layers, 1, "and confined to the box");
}

#[test]
fn additive_blend_maps_to_a_real_compose_mode() {
    let blend = super::painter::blend_mode(fission_ir::BlendMode::Plus);
    assert_eq!(blend.compose, Compose::Plus);
}
