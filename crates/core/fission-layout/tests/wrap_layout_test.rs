use fission_ir::op::{AlignItems, Color, FlexWrap, JustifyContent, TextRun, TextStyle};
use fission_ir::{FlexDirection, LayoutOp, WidgetId};
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutSize, TextMeasurer};
use std::sync::Arc;

struct FixedTextMeasurer;

impl TextMeasurer for FixedTextMeasurer {
    fn measure(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let width = text.len() as f32 * 8.0;
        (
            available_width.map_or(width, |limit| width.min(limit)),
            20.0,
        )
    }

    fn measure_rich_text(&self, runs: &[TextRun], available_width: Option<f32>) -> (f32, f32) {
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        self.measure(&text, 16.0, available_width)
    }
}

fn box_node(
    id: u128,
    parent: Option<u128>,
    children: Vec<u128>,
    padding: [f32; 4],
) -> LayoutInputNode {
    LayoutInputNode {
        id: WidgetId::from_u128(id),
        parent_id: parent.map(WidgetId::from_u128),
        op: LayoutOp::Box {
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        },
        children_ids: children.into_iter().map(WidgetId::from_u128).collect(),
        debug_name: format!("box-{id}"),
        width: None,
        height: None,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: None,
    }
}

fn text_node(id: u128, parent: u128, text: &str) -> LayoutInputNode {
    let mut node = box_node(id, Some(parent), Vec::new(), [0.0; 4]);
    node.debug_name = format!("text-{id}");
    node.rich_text = Some(vec![TextRun {
        text: text.into(),
        style: TextStyle {
            font_size: 16.0,
            color: Color::BLACK,
            underline: false,
            font_family: None,
            locale: None,
            font_weight: 400,
            font_style: fission_ir::op::FontStyle::Normal,
            line_height: None,
            letter_spacing: 0.0,
            background_color: None,
        },
    }]);
    node
}

#[test]
fn wrapped_auto_sized_controls_keep_intrinsic_width() {
    let root_id = WidgetId::from_u128(1);
    let root = LayoutInputNode {
        id: root_id,
        parent_id: None,
        op: LayoutOp::Flex {
            direction: FlexDirection::Row,
            wrap: FlexWrap::Wrap,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            padding: [0.0; 4],
            gap: Some(8.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Start,
        },
        children_ids: vec![WidgetId::from_u128(2), WidgetId::from_u128(5)],
        debug_name: "wrap".into(),
        width: Some(300.0),
        height: None,
        flex_grow: 1.0,
        flex_shrink: 1.0,
        rich_text: None,
    };
    let nodes = vec![
        root,
        box_node(2, Some(1), vec![3], [0.0; 4]),
        box_node(3, Some(2), vec![4], [12.0, 12.0, 8.0, 8.0]),
        text_node(4, 3, "First"),
        box_node(5, Some(1), vec![6], [0.0; 4]),
        box_node(6, Some(5), vec![7], [12.0, 12.0, 8.0, 8.0]),
        text_node(7, 6, "Second"),
    ];

    let mut engine = LayoutEngine::new().with_measurer(Arc::new(FixedTextMeasurer));
    let snapshot = engine
        .compute_layout(&nodes, root_id, LayoutSize::new(300.0, 200.0), &|_| 0.0)
        .expect("wrapped layout");
    let first = snapshot
        .get_node_geometry(WidgetId::from_u128(2))
        .expect("first control")
        .rect;
    let second = snapshot
        .get_node_geometry(WidgetId::from_u128(5))
        .expect("second control")
        .rect;

    assert!(first.width() < 100.0, "first control expanded: {first:?}");
    assert!(
        second.width() < 100.0,
        "second control expanded: {second:?}"
    );
    assert_eq!(first.y(), second.y(), "controls should share a wrap line");
    assert!(
        second.x() > first.x() + first.width(),
        "second control should follow first"
    );
}
