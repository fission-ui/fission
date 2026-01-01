use fission_ir::op::{Color, TextRun, TextStyle};
use fission_ir::{LayoutOp as IrLayoutOp, NodeId};
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutSize, TextMeasurer};
use std::sync::Arc;

struct FixedMeasurer;

impl TextMeasurer for FixedMeasurer {
    fn measure(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let char_width = 10.0;
        let line_height = 20.0;
        let width = text.len() as f32 * char_width;
        if let Some(max_w) = available_width {
            if max_w > 0.0 && width > max_w {
                let lines = (width / max_w).ceil();
                return (max_w, lines * line_height);
            }
        }
        (width, line_height)
    }

    fn measure_rich_text(&self, runs: &[TextRun], available_width: Option<f32>) -> (f32, f32) {
        let text: String = runs.iter().map(|r| r.text.clone()).collect();
        self.measure(&text, 16.0, available_width)
    }
}

fn make_box_node(
    id: u128,
    parent_id: Option<NodeId>,
    width: Option<f32>,
    height: Option<f32>,
    children: Vec<NodeId>,
) -> LayoutInputNode {
    LayoutInputNode {
        id: NodeId::from_u128(id),
        parent_id,
        op: IrLayoutOp::Box {
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
        children_ids: children,
        debug_name: "box".into(),
        width,
        height,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: None,
    }
}

fn make_text_node(id: u128, parent_id: NodeId, text: &str, max_width: Option<f32>) -> LayoutInputNode {
    LayoutInputNode {
        id: NodeId::from_u128(id),
        parent_id: Some(parent_id),
        op: IrLayoutOp::Box {
            width: None,
            height: None,
            min_width: None,
            max_width,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        },
        children_ids: Vec::new(),
        debug_name: "text".into(),
        width: None,
        height: None,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: Some(vec![TextRun {
            text: text.to_string(),
            style: TextStyle {
                font_size: 16.0,
                color: Color::BLACK,
                underline: false,
            },
        }]),
    }
}

fn run_layout(nodes: &[LayoutInputNode], root_id: NodeId) -> fission_layout::LayoutSnapshot {
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(FixedMeasurer));
    engine.rebuild(nodes).unwrap();
    engine
        .compute_layout(nodes, root_id, LayoutSize::new(800.0, 600.0), &|_| 0.0)
        .unwrap()
}

#[test]
fn text_single_line_size_matches_measure() {
    let root_id = NodeId::from_u128(1);
    let text_id = NodeId::from_u128(2);

    let root = make_box_node(1, None, Some(200.0), Some(200.0), vec![text_id]);
    let text = make_text_node(2, root_id, "Hello", None);

    let nodes = vec![root, text];
    let snapshot = run_layout(&nodes, root_id);
    let text_geom = snapshot.get_node_geometry(text_id).unwrap();

    assert_eq!(text_geom.rect.width(), 50.0);
    assert_eq!(text_geom.rect.height(), 20.0);
}

#[test]
fn text_wrap_respects_available_width() {
    let root_id = NodeId::from_u128(10);
    let text_id = NodeId::from_u128(11);

    let root = make_box_node(10, None, Some(50.0), Some(200.0), vec![text_id]);
    let text = make_text_node(11, root_id, "HelloWorld", None);

    let nodes = vec![root, text];
    let snapshot = run_layout(&nodes, root_id);
    let text_geom = snapshot.get_node_geometry(text_id).unwrap();

    assert_eq!(text_geom.rect.width(), 50.0);
    assert_eq!(text_geom.rect.height(), 40.0);
}

#[test]
fn text_max_width_limits_measure() {
    let root_id = NodeId::from_u128(20);
    let text_id = NodeId::from_u128(21);

    let root = make_box_node(20, None, Some(200.0), Some(200.0), vec![text_id]);
    let text = make_text_node(21, root_id, "HelloWorld", Some(40.0));

    let nodes = vec![root, text];
    let snapshot = run_layout(&nodes, root_id);
    let text_geom = snapshot.get_node_geometry(text_id).unwrap();

    assert_eq!(text_geom.rect.width(), 40.0);
    assert_eq!(text_geom.rect.height(), 60.0);
}
