use super::{
    map_fill_to_brush, paragraph_alignment, paragraph_fade, paragraph_line_trim,
    paragraph_line_visual_bounds, paragraph_y_offset, parse_svg_entry,
    text_background_segments_for_cluster_ranges, workload_profile_for_encoded_scene,
    workload_profile_for_scene, ParagraphFade, RetainedSceneCache, SvgShape, TextBackgroundSegment,
    TextClip, VelloRenderer, VelloTextMeasurer,
};
use fission_ir::op::{
    FontStyle, MouseCursor, RichTextAnnotation, TextAlign, TextDirection, TextHeightBehavior,
    TextOverflow, TextParagraphStyle, TextWidthBasis,
};
use fission_ir::{semantics::ActionTrigger, ActionEntry, WidgetId};
use fission_layout::TextMeasurer;
use fission_render::{
    Color as RenderColor, DisplayList, DisplayOp, Fill as RenderFill, LayoutPoint, LayoutRect,
    RenderScene, Renderer, TextStyle as RenderTextStyle,
};
use parley::FontContext;
use std::sync::{Arc, Mutex};
use vello::kurbo::{Affine, Point, Rect};
use vello::peniko::{Brush, GradientKind, Mix};
use vello::Scene;

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
        },
        Rect::new(20.0, 40.0, 220.0, 140.0),
    );

    let Brush::Gradient(gradient) = brush else {
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
    assert_eq!(entry.error, None);
    assert_eq!(entry.shapes.len(), 1);
    assert!(matches!(entry.shapes[0], SvgShape::Path(_)));
}

#[test]
fn svg_parser_preserves_supported_rect_and_polygon_shapes() {
    let entry = parse_svg_entry(
        "<svg viewBox='0 0 24 24'><rect x='1' y='2' width='3' height='4'/><polygon points='0,0 8,0 4,8'/></svg>",
    );

    assert_eq!(entry.error, None);
    assert_eq!(entry.shapes.len(), 2);
    assert!(matches!(entry.shapes[0], SvgShape::Rect(_)));
    assert!(matches!(entry.shapes[1], SvgShape::Path(_)));
}

#[test]
fn svg_parser_retains_malformed_path_and_attribute_errors() {
    let malformed_path = parse_svg_entry("<svg><path d='not valid path data'/></svg>");
    assert!(malformed_path
        .error
        .as_deref()
        .is_some_and(|error| error.starts_with("path element contains invalid d data:")));

    let malformed_rect = parse_svg_entry("<svg><rect width='not-a-number' height='10'/></svg>");
    assert_eq!(
        malformed_rect.error.as_deref(),
        Some("rect element has a malformed attribute: width attribute must be a finite number")
    );
}

#[test]
fn svg_parser_rejects_unsupported_elements_and_group_transforms() {
    let unsupported_element = parse_svg_entry("<svg><circle cx='5' cy='5' r='4'/></svg>");
    assert_eq!(
        unsupported_element.error.as_deref(),
        Some("unsupported SVG element <circle>")
    );

    let transformed_group =
        parse_svg_entry("<svg><g transform='translate(4 6)'><path d='M0 0L1 1'/></g></svg>");
    assert_eq!(
        transformed_group.error.as_deref(),
        Some("transform attributes on <g> are not supported by the Vello SVG renderer")
    );
}

#[test]
fn svg_render_failure_includes_widget_provenance() {
    let bounds = LayoutRect::new(0.0, 0.0, 20.0, 20.0);
    let node_id = WidgetId::from_u128(0xfeed);
    let mut list = DisplayList::new(bounds);
    list.push(DisplayOp::DrawSvg {
        content: "<svg><circle cx='5' cy='5' r='4'/></svg>".into(),
        fill: None,
        stroke: None,
        bounds,
        node_id: Some(node_id),
    });
    let retained = RenderScene::from_display_list(list);
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let mut renderer = test_renderer(&mut scene, &mut cache);

    let error = renderer
        .render_scene(&retained)
        .expect_err("unsupported SVG must fail rendering")
        .to_string();

    assert!(error.contains("unsupported SVG element <circle>"));
    assert!(error.contains(&node_id.to_string()));
}

#[test]
fn path_render_failure_is_explicit_and_includes_widget_provenance() {
    let bounds = LayoutRect::new(0.0, 0.0, 20.0, 20.0);
    let node_id = WidgetId::from_u128(0xcafe);
    let mut list = DisplayList::new(bounds);
    list.push(DisplayOp::DrawPath {
        path: "this is not path data".into(),
        fill: Some(RenderFill::Solid(RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        })),
        stroke: None,
        bounds,
        node_id: Some(node_id),
    });
    let retained = RenderScene::from_display_list(list);
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let mut renderer = test_renderer(&mut scene, &mut cache);

    let error = renderer
        .render_scene(&retained)
        .expect_err("malformed path must fail rendering")
        .to_string();

    assert!(error.contains("Vello renderer rejected path"));
    assert!(error.contains(&node_id.to_string()));
}

#[test]
fn affine_mat4_conversion_preserves_two_dimensional_coefficients() {
    let matrix = [
        2.0, 0.5, 0.0, 0.0, -0.25, 3.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 8.0, 13.0, 0.0, 1.0,
    ];

    let affine = VelloRenderer::affine_from_mat4(&matrix).expect("valid 2D affine matrix");

    assert_eq!(affine, Affine::new([2.0, 0.5, -0.25, 3.0, 8.0, 13.0]));
}

#[test]
fn affine_mat4_conversion_rejects_non_affine_and_non_finite_values() {
    let mut perspective = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    perspective[3] = 0.25;
    let perspective_error = VelloRenderer::affine_from_mat4(&perspective)
        .expect_err("perspective must not be truncated")
        .to_string();
    assert!(perspective_error.contains("only two-dimensional affine transforms"));

    let mut non_finite = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    non_finite[12] = f32::NAN;
    let non_finite_error = VelloRenderer::affine_from_mat4(&non_finite)
        .expect_err("non-finite matrix must not enter Vello")
        .to_string();
    assert!(non_finite_error.contains("requires finite transform matrix values"));
}

#[test]
fn display_list_transform_propagates_non_affine_renderer_error() {
    let bounds = LayoutRect::new(0.0, 0.0, 20.0, 20.0);
    let mut perspective = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    perspective[3] = 0.25;
    let mut list = DisplayList::new(bounds);
    list.push(DisplayOp::Transform(perspective));
    let retained = RenderScene::from_display_list(list);
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let mut renderer = test_renderer(&mut scene, &mut cache);

    let error = renderer
        .render_scene(&retained)
        .expect_err("perspective display operation must fail rendering")
        .to_string();

    assert!(error.contains("only two-dimensional affine transforms"));
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

fn test_renderer<'a>(scene: &'a mut Scene, cache: &'a mut RetainedSceneCache) -> VelloRenderer<'a> {
    let measurer = Arc::new(VelloTextMeasurer::new(Arc::new(Mutex::new(
        FontContext::new(),
    ))));
    VelloRenderer::new(scene, measurer, cache, 1.0)
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
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let renderer = test_renderer(&mut scene, &mut cache);
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
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let renderer = test_renderer(&mut scene, &mut cache);
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
fn longest_line_width_basis_aligns_against_content_width() {
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let renderer = test_renderer(&mut scene, &mut cache);
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
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let mut renderer = test_renderer(&mut scene, &mut cache);
    let style = test_style();
    let text = "this line should visibly fade instead of only clipping";

    renderer.render_paragraph_text(
        text,
        &style,
        false,
        LayoutPoint::new(0.0, 0.0),
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
    );
    drop(renderer);

    assert!(
        scene.encoding().n_clips > 0,
        "fade overflow should add internal clip layers"
    );
}

#[test]
fn simple_text_rendering_culls_glyphs_outside_bounds() {
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let mut renderer = test_renderer(&mut scene, &mut cache);
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
        &[],
        &[],
    );
    drop(renderer);

    let glyphs = scene.encoding().resources.glyphs.len();
    assert!(glyphs > 0, "visible glyphs should still be encoded");
    assert!(
        glyphs < 256,
        "renderer should not encode the full off-bounds text run; glyphs={glyphs}"
    );
}

#[test]
fn encoded_workload_profile_counts_only_glyphs_contributing_to_the_frame() {
    let text = "M".repeat(20_000);
    let bounds = LayoutRect::new(0.0, 0.0, 120.0, 32.0);
    let mut list = DisplayList::new(bounds);
    list.push(DisplayOp::DrawText {
        text,
        position: LayoutPoint::new(0.0, 0.0),
        size: 16.0,
        color: RenderColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        bounds,
        node_id: None,
        underline: false,
        wrap: false,
        caret_index: None,
        caret_color: None,
        caret_width: None,
        caret_height: None,
        caret_radius: None,
        paragraph_style: None,
    });
    let retained = RenderScene::from_display_list(list);
    let raw_profile = workload_profile_for_scene(&retained, 120, 32, 1.0);
    assert_eq!(raw_profile.scene.glyphs, 20_000);

    let mut encoded = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let mut renderer = test_renderer(&mut encoded, &mut cache);
    renderer.render_scene(&retained).expect("encode test scene");
    drop(renderer);

    let profile = workload_profile_for_encoded_scene(&retained, &encoded, 120, 32, 1.0);
    assert_eq!(
        profile.scene.glyphs as usize,
        encoded.encoding().resources.glyphs.len()
    );
    assert_eq!(
        profile.scene.glyph_runs as usize,
        encoded.encoding().resources.glyph_runs.len()
    );
    assert!(
        profile.scene.glyphs < 256,
        "workload sizing should use the encoder's culled glyphs, not the retained document"
    );
}

#[test]
fn rich_multiline_text_encodes_only_lines_inside_the_visible_bounds() {
    let text = "version = 4\n".repeat(10_000);
    let style = test_style();
    let styles = vec![(0..text.len(), style.clone())];
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let mut renderer = test_renderer(&mut scene, &mut cache);
    let viewport = Rect::new(0.0, 0.0, 400.0, 80.0);
    renderer
        .scene
        .push_layer(Mix::Normal, 1.0, renderer.current_transform, &viewport);
    renderer.push_clip_bounds(viewport);

    renderer.render_text(
        &text,
        style.font_size,
        style.color,
        false,
        false,
        LayoutPoint::new(0.0, 0.0),
        LayoutRect::new(0.0, 0.0, 400.0, 200_000.0),
        Some(0),
        Some(style.color),
        Some(2.0),
        None,
        None,
        None,
        &[],
        &styles,
    );
    renderer.scene.pop_layer();
    drop(renderer);

    let glyphs = scene.encoding().resources.glyphs.len();
    assert!(
        glyphs < 256,
        "renderer should not encode offscreen rich-text lines; glyphs={glyphs}"
    );
}

#[test]
fn explicit_text_direction_realigns_neutral_content() {
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let renderer = test_renderer(&mut scene, &mut cache);
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
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let renderer = test_renderer(&mut scene, &mut cache);
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
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let renderer = test_renderer(&mut scene, &mut cache);
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
    let mut scene = Scene::new();
    let mut cache = RetainedSceneCache::default();
    let renderer = test_renderer(&mut scene, &mut cache);
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
