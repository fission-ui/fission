use anyhow::Result;
use fission_ir::op::{
    TextAlign, TextDirection, TextHeightBehavior, TextOverflow, TextParagraphStyle,
};
use fission_render::{
    Color, DisplayList, DisplayOp, LayoutPoint, LayoutRect, RenderScene, Renderer, TextRun,
    TextStyle,
};

// A mock renderer that captures what it was asked to render.
#[derive(Default)]
struct MockRenderer {
    captured_list: Option<DisplayList>,
}

impl Renderer for MockRenderer {
    fn render_scene(&mut self, scene: &RenderScene) -> Result<()> {
        self.captured_list = Some(scene.flatten());
        Ok(())
    }
}

#[test]
fn test_display_list_serialization() {
    let rect = LayoutRect::new(0.0, 0.0, 100.0, 100.0);
    let op1 = DisplayOp::DrawRect {
        rect,
        fill: None,
        stroke: None,
        corner_radius: 0.0,
        shadow: None, // Added
        bounds: rect,
        node_id: None,
    };

    let mut list = DisplayList::new(rect);
    list.push(op1.clone());

    // Serialize
    let serialized = serde_json::to_string(&list).expect("Failed to serialize display list");

    // Deserialize
    let deserialized: DisplayList =
        serde_json::from_str(&serialized).expect("Failed to deserialize display list");

    assert_eq!(list, deserialized);
    assert_eq!(deserialized.ops.len(), 1);
    assert_eq!(deserialized.ops[0], op1);
}

#[test]
fn test_renderer_consumes_display_list() {
    let mut renderer = MockRenderer::default();
    let bounds = LayoutRect::new(0.0, 0.0, 800.0, 600.0);
    let mut dl = DisplayList::new(bounds);

    dl.push(DisplayOp::DrawRect {
        rect: LayoutRect::new(10.0, 10.0, 50.0, 50.0),
        fill: None,
        stroke: None,
        corner_radius: 0.0,
        shadow: None, // Added
        bounds: LayoutRect::new(10.0, 10.0, 50.0, 50.0),
        node_id: None,
    });

    renderer.render(&dl).expect("Render failed");

    assert!(renderer.captured_list.is_some());
    let captured = renderer.captured_list.unwrap();
    assert_eq!(captured.ops.len(), 1);
}

#[test]
fn test_rich_text_display_ops_preserve_caret_metadata() {
    let bounds = LayoutRect::new(0.0, 0.0, 200.0, 48.0);
    let op = DisplayOp::DrawRichText {
        runs: vec![TextRun {
            text: "Paragraph".into(),
            style: TextStyle {
                font_size: 16.0,
                color: Color {
                    r: 10,
                    g: 20,
                    b: 30,
                    a: 255,
                },
                underline: false,
                font_family: Some("Inter".into()),
                locale: None,
                font_weight: 500,
                font_style: fission_ir::op::FontStyle::Normal,
                line_height: Some(20.0),
                letter_spacing: 0.25,
                background_color: None,
            },
        }],
        position: LayoutPoint::new(8.0, 12.0),
        bounds,
        node_id: None,
        wrap: true,
        caret_index: Some(4),
        caret_color: Some(Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        }),
        caret_width: Some(2.0),
        caret_height: Some(18.0),
        caret_radius: Some(1.5),
        paragraph_style: Some(TextParagraphStyle {
            text_align: TextAlign::Center,
            max_lines: Some(2),
            overflow: TextOverflow::Ellipsis,
            text_direction: TextDirection::Rtl,
            strut_line_height: Some(24.0),
            text_height_behavior: TextHeightBehavior {
                apply_height_to_first_ascent: false,
                apply_height_to_last_descent: true,
            },
        }),
    };

    let mut list = DisplayList::new(bounds);
    list.push(op.clone());

    let serialized = serde_json::to_string(&list).expect("Failed to serialize rich text list");
    let deserialized: DisplayList =
        serde_json::from_str(&serialized).expect("Failed to deserialize rich text list");

    assert_eq!(deserialized.ops[0], op);
}
