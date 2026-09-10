use crate::op::{
    decode_inline_widget_marker, decode_text_paragraph_style, encode_inline_widget_marker,
    encode_text_paragraph_style, AlignItems, BoxAlignment, FlexDirection, FlexWrap, HttpHeader,
    ImageCachePolicy, ImageRequest, ImageSource, InlineWidgetMarker, JustifyContent,
    LayoutDirection, LayoutOp, TextAlign, TextDirection, TextHeightBehavior, TextOverflow,
    TextParagraphStyle, TextWidthBasis, TEXT_PARAGRAPH_MAX_ENCODED_LINES,
};
use crate::{CoreIR, FlyoutAlignment};

#[test]
fn paragraph_style_round_trips_alignment_overflow_and_line_cap() {
    let style = TextParagraphStyle {
        text_align: TextAlign::Justify,
        max_lines: Some(3),
        overflow: TextOverflow::Fade,
        text_direction: TextDirection::Auto,
        text_width_basis: TextWidthBasis::Parent,
        strut_line_height: None,
        text_height_behavior: TextHeightBehavior::default(),
    };

    let encoded = encode_text_paragraph_style(style);
    assert_eq!(decode_text_paragraph_style(encoded), Some(style));
}

#[test]
fn paragraph_style_clamps_line_count_to_precise_encoding_budget() {
    let encoded = encode_text_paragraph_style(TextParagraphStyle {
        text_align: TextAlign::End,
        max_lines: Some(TEXT_PARAGRAPH_MAX_ENCODED_LINES + 99),
        overflow: TextOverflow::Ellipsis,
        text_direction: TextDirection::Auto,
        text_width_basis: TextWidthBasis::Parent,
        strut_line_height: None,
        text_height_behavior: TextHeightBehavior::default(),
    });

    assert_eq!(
        decode_text_paragraph_style(encoded),
        Some(TextParagraphStyle {
            text_align: TextAlign::End,
            max_lines: Some(TEXT_PARAGRAPH_MAX_ENCODED_LINES),
            overflow: TextOverflow::Ellipsis,
            text_direction: TextDirection::Auto,
            text_width_basis: TextWidthBasis::Parent,
            strut_line_height: None,
            text_height_behavior: TextHeightBehavior::default(),
        })
    );
}

#[test]
fn image_request_cache_key_is_stable_and_dimension_sensitive() {
    let request = ImageRequest {
        source: ImageSource::Network {
            url: "https://cdn.example.com/image.webp".into(),
            headers: vec![HttpHeader {
                name: "Accept".into(),
                value: "image/webp".into(),
            }],
            cache_policy: ImageCachePolicy::Default,
        },
        cache_width: Some(320),
        cache_height: Some(180),
        ..Default::default()
    };

    let same = request.clone();
    let mut resized = request.clone();
    resized.cache_width = Some(640);

    assert_eq!(request.stable_cache_key(), same.stable_cache_key());
    assert_ne!(request.stable_cache_key(), resized.stable_cache_key());
}

#[test]
fn image_source_helpers_report_path_and_network_sources() {
    assert_eq!(
        ImageSource::Asset {
            path: "assets/logo.png".into()
        }
        .local_path(),
        Some("assets/logo.png")
    );
    assert_eq!(
        ImageSource::Network {
            url: "https://example.com/logo.png".into(),
            headers: Vec::new(),
            cache_policy: ImageCachePolicy::Default,
        }
        .network_url(),
        Some("https://example.com/logo.png")
    );
}

#[test]
fn paragraph_style_compact_encoding_rejects_extended_fields() {
    assert_eq!(
        encode_text_paragraph_style(TextParagraphStyle {
            text_align: TextAlign::Start,
            max_lines: Some(2),
            overflow: TextOverflow::Visible,
            text_direction: TextDirection::Rtl,
            text_width_basis: TextWidthBasis::LongestLine,
            strut_line_height: Some(24.0),
            text_height_behavior: TextHeightBehavior {
                apply_height_to_first_ascent: false,
                apply_height_to_last_descent: true,
            },
        }),
        None
    );
}

#[test]
fn inline_widget_marker_round_trips() {
    let encoded = encode_inline_widget_marker(7, 24.5, 12.0);
    assert_eq!(
        decode_inline_widget_marker(Some(encoded.as_str())),
        Some(InlineWidgetMarker {
            id: 7,
            width: 24.5,
            height: 12.0,
        })
    );
}

#[test]
fn layout_direction_resolves_logical_horizontal_edges() {
    assert_eq!(
        LayoutDirection::LeftToRight.resolve_horizontal_justification(JustifyContent::Start),
        JustifyContent::Start
    );
    assert_eq!(
        LayoutDirection::RightToLeft.resolve_horizontal_justification(JustifyContent::Start),
        JustifyContent::End
    );
    assert_eq!(
        LayoutDirection::RightToLeft.resolve_flyout_alignment(FlyoutAlignment::Start),
        FlyoutAlignment::End
    );
    assert_eq!(
        LayoutDirection::RightToLeft.resolve_flyout_alignment(FlyoutAlignment::End),
        FlyoutAlignment::Start
    );
}

#[test]
fn core_ir_direction_is_backward_compatible_and_omits_ltr_default() {
    let encoded = serde_json::to_value(CoreIR::default()).expect("serialize default IR");
    assert!(encoded.get("layout_direction").is_none());

    let decoded: CoreIR = serde_json::from_value(encoded).expect("deserialize legacy-shaped IR");
    assert_eq!(decoded.layout_direction, LayoutDirection::LeftToRight);

    let mut rtl = CoreIR::default();
    rtl.layout_direction = LayoutDirection::RightToLeft;
    let encoded = serde_json::to_value(&rtl).expect("serialize RTL IR");
    assert_eq!(
        encoded.get("layout_direction"),
        Some(&serde_json::json!("RightToLeft"))
    );
    let decoded: CoreIR = serde_json::from_value(encoded).expect("deserialize RTL IR");
    assert_eq!(decoded.layout_direction, LayoutDirection::RightToLeft);
}

#[test]
fn flex_line_gap_defaults_when_reading_legacy_ir() {
    let op = LayoutOp::Flex {
        direction: FlexDirection::Row,
        wrap: FlexWrap::Wrap,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        padding: [0.0; 4],
        gap: Some(8.0),
        line_gap: Some(16.0),
        align_items: AlignItems::Start,
        justify_content: JustifyContent::Start,
    };
    let mut encoded = serde_json::to_value(op).expect("serialize flex");
    encoded
        .get_mut("Flex")
        .and_then(serde_json::Value::as_object_mut)
        .expect("flex object")
        .remove("line_gap");

    let decoded: LayoutOp = serde_json::from_value(encoded).expect("legacy flex IR");
    assert!(matches!(decoded, LayoutOp::Flex { line_gap: None, .. }));
}

#[test]
fn parameterized_alignment_round_trips_without_changing_legacy_align() {
    let legacy = serde_json::to_value(LayoutOp::Align).expect("serialize legacy align");
    assert_eq!(legacy, serde_json::json!("Align"));

    let op = LayoutOp::Aligned {
        horizontal: BoxAlignment::End,
        vertical: BoxAlignment::Start,
    };
    let encoded = serde_json::to_value(&op).expect("serialize aligned");
    let decoded: LayoutOp = serde_json::from_value(encoded).expect("deserialize aligned");
    assert_eq!(decoded, op);
}
