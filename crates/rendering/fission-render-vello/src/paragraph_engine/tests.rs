use std::sync::{Arc, Mutex};

use fission_ir::op::{Color, TextDirection, TextParagraphStyle, TextStyle};
use fission_layout::{
    LayoutPoint, LayoutSize, ParagraphAffinity, ParagraphCapability, ParagraphDescription,
    ParagraphEngine, ParagraphError, ParagraphFontFeature, ParagraphInlineObject,
    ParagraphStyleRun, Utf8Index, Utf8Range,
};
use parley::FontContext;

use crate::text::RichShapingStyle;
use crate::VelloTextMeasurer;

fn range(start: usize, end: usize) -> Utf8Range {
    Utf8Range::from_byte_offsets(start, end).unwrap()
}

fn style() -> TextStyle {
    TextStyle {
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
    }
}

fn description(text: &str, width: Option<f32>) -> ParagraphDescription {
    ParagraphDescription::new(
        text,
        if text.is_empty() {
            Vec::new()
        } else {
            vec![ParagraphStyleRun::new(range(0, text.len()), style())]
        },
        TextParagraphStyle::default(),
        width,
    )
}

fn measurer() -> VelloTextMeasurer {
    VelloTextMeasurer::new(Arc::new(Mutex::new(FontContext::new())))
}

#[test]
fn adapter_reports_only_geometry_it_can_produce() {
    let capabilities = ParagraphEngine::capabilities(&measurer());

    for capability in [
        ParagraphCapability::BidirectionalText,
        ParagraphCapability::VariableFonts,
        ParagraphCapability::FontFeatures,
        ParagraphCapability::ClusterMapping,
        ParagraphCapability::HitTesting,
        ParagraphCapability::CaretGeometry,
        ParagraphCapability::SelectionGeometry,
    ] {
        assert!(capabilities.supports(capability));
    }
    assert!(!capabilities.supports(ParagraphCapability::InlineObjects));
    assert!(!capabilities.supports(ParagraphCapability::UnresolvedGlyphDiagnostics));
}

#[test]
fn legacy_and_paragraph_calls_share_the_same_parley_cache() {
    let measurer = measurer();
    let render_style = fission_render::TextStyle {
        font_size: 16.0,
        color: fission_render::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        underline: false,
        font_family: None,
        locale: None,
        font_weight: 400,
        font_style: Default::default(),
        line_height: None,
        letter_spacing: 0.0,
        background_color: None,
    };
    let ranges = vec![(0..6, render_style.clone())];
    let legacy = measurer.layout_rich(
        "shared",
        16.0,
        render_style.color,
        &ranges,
        &[],
        Some(200.0),
    );
    let paragraph = measurer.layout_rich_with_shaping(
        "shared",
        16.0,
        render_style.color,
        &[RichShapingStyle {
            range: 0..6,
            style: render_style,
            font_width: 1.0,
            word_spacing: 0.0,
            variations: Vec::new(),
            features: Vec::new(),
        }],
        &[],
        Some(200.0),
    );

    assert!(Arc::ptr_eq(&legacy, &paragraph));
}

#[test]
fn one_parley_result_serves_layout_hit_caret_and_selection() {
    let text = "Hello café";
    let description = description(text, Some(300.0));
    let result = ParagraphEngine::layout(&measurer(), &description).unwrap();
    let geometry = result.geometry();

    assert!(!geometry.lines().is_empty());
    assert!(!geometry.clusters().is_empty());
    assert!(geometry
        .clusters()
        .iter()
        .all(|cluster| cluster.range.end().byte_offset() <= text.len()));
    assert!(result
        .caret(Utf8Index::new(0), ParagraphAffinity::Downstream)
        .unwrap()
        .is_some());
    assert!(!result
        .selection_boxes(range(0, text.len()))
        .unwrap()
        .is_empty());

    let hit = result.hit_test(LayoutPoint::new(1.0, 1.0)).unwrap();
    assert!(hit.index.byte_offset() <= text.len());
}

#[test]
fn explicit_direction_uses_source_indices_not_direction_mark_indices() {
    let text = "مرحبا";
    let mut description = description(text, Some(300.0));
    description.paragraph_style.text_direction = TextDirection::Rtl;
    let result = ParagraphEngine::layout(&measurer(), &description).unwrap();

    assert!(result
        .geometry()
        .lines()
        .iter()
        .all(|line| line.range.end().byte_offset() <= text.len()));
    assert!(result
        .geometry()
        .clusters()
        .iter()
        .all(|cluster| cluster.range.end().byte_offset() <= text.len()));
}

#[test]
fn inline_objects_are_rejected_until_parley_can_honor_the_full_contract() {
    let text = "A\u{fffc}B";
    let mut description = description(text, Some(300.0));
    description.inline_objects.push(ParagraphInlineObject {
        id: 42,
        range: range(1, 4),
        size: LayoutSize::new(18.0, 12.0),
        baseline: 12.0,
    });
    assert!(matches!(
        ParagraphEngine::layout(&measurer(), &description),
        Err(ParagraphError::UnsupportedCapability(error))
            if error.capability == ParagraphCapability::InlineObjects
    ));
}

#[test]
fn font_features_use_the_shared_rich_layout_path() {
    let text = "office";
    let mut description = description(text, Some(300.0));
    description.style_runs[0]
        .features
        .push(ParagraphFontFeature {
            tag: u32::from_be_bytes(*b"liga"),
            value: 0,
        });

    let result = ParagraphEngine::layout(&measurer(), &description).unwrap();
    assert!(!result.geometry().clusters().is_empty());
}

#[test]
fn parley_rejects_feature_values_it_cannot_represent() {
    let text = "office";
    let mut description = description(text, Some(300.0));
    description.style_runs[0]
        .features
        .push(ParagraphFontFeature {
            tag: u32::from_be_bytes(*b"liga"),
            value: u32::from(u16::MAX) + 1,
        });

    assert!(matches!(
        ParagraphEngine::layout(&measurer(), &description),
        Err(ParagraphError::InvalidDescription {
            field: "style_runs.features.value",
            ..
        })
    ));
}

#[test]
fn max_lines_limits_all_exposed_interaction_geometry() {
    let text = "one two three four five six seven eight";
    let mut description = description(text, Some(50.0));
    description.paragraph_style.max_lines = Some(1);
    let result = ParagraphEngine::layout(&measurer(), &description).unwrap();

    assert_eq!(result.geometry().lines().len(), 1);
    assert!(result
        .geometry()
        .clusters()
        .iter()
        .all(|cluster| cluster.line_index == 0));
    assert!(result
        .geometry()
        .carets()
        .iter()
        .all(|caret| caret.line_index == 0));
    assert!(result
        .geometry()
        .hit_regions()
        .iter()
        .all(|region| region.line_index == 0));
}

#[test]
fn every_visible_line_has_caret_and_hit_geometry() {
    let result = ParagraphEngine::layout(&measurer(), &description("a\n\nb", Some(300.0)))
        .expect("blank lines retain interaction geometry");
    let geometry = result.geometry();

    for line_index in 0..geometry.lines().len() {
        assert!(geometry
            .carets()
            .iter()
            .any(|caret| caret.line_index == line_index));
        assert!(geometry
            .hit_regions()
            .iter()
            .any(|region| region.line_index == line_index));
    }
}

#[test]
fn zero_width_is_a_real_parent_constraint_not_an_unbounded_sentinel() {
    let result = ParagraphEngine::layout(&measurer(), &description("zero width", Some(0.0)))
        .expect("zero is a valid finite paragraph constraint");

    assert_eq!(result.geometry().size().width, 0.0);
}
