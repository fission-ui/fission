use fission_ir::op::{AlignItems, Color, JustifyContent, TextRun, TextStyle};
use fission_ir::{FlexDirection, FlexWrap, LayoutOp, WidgetId};
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutSize, TextMeasurer};
use std::sync::Arc;

struct FixedTextMeasurer;

impl TextMeasurer for FixedTextMeasurer {
    fn measure(&self, text: &str, _font_size: f32, _width: Option<f32>) -> (f32, f32) {
        (text.chars().count() as f32 * 10.0, 20.0)
    }

    fn measure_rich_text(&self, runs: &[TextRun], width: Option<f32>) -> (f32, f32) {
        let text: String = runs.iter().map(|run| run.text.as_str()).collect();
        self.measure(&text, 16.0, width)
    }
}

fn box_node(
    id: u128,
    parent: Option<u128>,
    children: Vec<u128>,
    width: Option<f32>,
    height: Option<f32>,
) -> LayoutInputNode {
    LayoutInputNode {
        id: WidgetId::from_u128(id),
        parent_id: parent.map(WidgetId::from_u128),
        op: LayoutOp::Box {
            width,
            height,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        },
        children_ids: children.into_iter().map(WidgetId::from_u128).collect(),
        debug_name: format!("box-{id}"),
        width,
        height,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: None,
    }
}

#[test]
fn align_centers_an_intrinsic_layout_wrapper_and_its_text() {
    let root = box_node(1, None, vec![2], Some(40.0), Some(40.0));
    let align = LayoutInputNode {
        id: WidgetId::from_u128(2),
        parent_id: Some(WidgetId::from_u128(1)),
        op: LayoutOp::Align,
        children_ids: vec![WidgetId::from_u128(3)],
        debug_name: "align".into(),
        width: None,
        height: None,
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    };
    let wrapper = box_node(3, Some(2), vec![4], None, None);
    let mut text = box_node(4, Some(3), Vec::new(), None, None);
    text.rich_text = Some(vec![TextRun {
        text: "JD".into(),
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
            typography: Default::default(),
        },
    }]);

    let nodes = vec![root, align, wrapper, text];
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(FixedTextMeasurer));
    engine.update(&nodes);
    let snapshot = engine
        .compute_layout(
            &nodes,
            WidgetId::from_u128(1),
            LayoutSize::new(800.0, 600.0),
            &|_| 0.0,
        )
        .expect("layout");

    let wrapper = snapshot
        .get_node_geometry(WidgetId::from_u128(3))
        .expect("wrapper geometry");
    let text = snapshot
        .get_node_geometry(WidgetId::from_u128(4))
        .expect("text geometry");
    assert_eq!(wrapper.rect.width(), 20.0);
    assert_eq!(text.rect.width(), 20.0);
    assert_eq!(text.rect.x(), 10.0);
    assert_eq!(text.rect.y(), 10.0);
}

#[test]
fn flex_stretch_preserves_an_explicit_cross_axis_size() {
    let root_id = WidgetId::from_u128(10);
    let child_id = WidgetId::from_u128(11);
    let root = LayoutInputNode {
        id: root_id,
        parent_id: None,
        op: LayoutOp::Flex {
            direction: FlexDirection::Column,
            wrap: FlexWrap::NoWrap,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            padding: [0.0; 4],
            gap: None,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
        },
        children_ids: vec![child_id],
        debug_name: "column".into(),
        width: Some(900.0),
        height: Some(3000.0),
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: None,
    };
    let child = box_node(11, Some(10), Vec::new(), Some(40.0), Some(40.0));

    let nodes = vec![root, child];
    let mut engine = LayoutEngine::new();
    engine.update(&nodes);
    let snapshot = engine
        .compute_layout(&nodes, root_id, LayoutSize::new(900.0, 3000.0), &|_| 0.0)
        .expect("layout");

    let child = snapshot
        .get_node_geometry(child_id)
        .expect("child geometry");
    assert_eq!(child.rect.width(), 40.0);
    assert_eq!(child.rect.height(), 40.0);
}

#[test]
fn intrinsic_box_does_not_expand_an_align_child_to_a_loose_cross_axis_maximum() {
    let root_id = WidgetId::from_u128(20);
    let button_id = WidgetId::from_u128(21);
    let align_id = WidgetId::from_u128(22);
    let text_id = WidgetId::from_u128(23);
    let root = LayoutInputNode {
        id: root_id,
        parent_id: None,
        op: LayoutOp::Flex {
            direction: FlexDirection::Row,
            wrap: FlexWrap::NoWrap,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            padding: [0.0; 4],
            gap: None,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Center,
        },
        children_ids: vec![button_id],
        debug_name: "toolbar".into(),
        width: Some(300.0),
        height: Some(80.0),
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: None,
    };
    let button = LayoutInputNode {
        id: button_id,
        parent_id: Some(root_id),
        op: LayoutOp::Box {
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: Some(40.0),
            max_height: None,
            padding: [8.0, 8.0, 10.0, 10.0],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        },
        children_ids: vec![align_id],
        debug_name: "button".into(),
        width: None,
        height: None,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: None,
    };
    let align = LayoutInputNode {
        id: align_id,
        parent_id: Some(button_id),
        op: LayoutOp::Align,
        children_ids: vec![text_id],
        debug_name: "button-align".into(),
        width: None,
        height: None,
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    };
    let mut text = box_node(23, Some(22), Vec::new(), None, None);
    text.rich_text = Some(vec![TextRun {
        text: "Go".into(),
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
            typography: Default::default(),
        },
    }]);

    let nodes = vec![root, button, align, text];
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(FixedTextMeasurer));
    engine.update(&nodes);
    let snapshot = engine
        .compute_layout(&nodes, root_id, LayoutSize::new(300.0, 80.0), &|_| 0.0)
        .expect("layout");

    let button = snapshot
        .get_node_geometry(button_id)
        .expect("button geometry");
    assert_eq!(button.rect.height(), 40.0);
    assert_eq!(button.rect.y(), 20.0);
}
