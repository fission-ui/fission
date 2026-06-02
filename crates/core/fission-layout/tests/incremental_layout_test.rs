use fission_ir::op::{Color, TextRun, TextStyle};
use fission_ir::{FlexDirection, LayoutOp as IrLayoutOp, WidgetId};
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutSize, TextMeasurer};
use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone)]
struct CountingMeasurer {
    calls: Arc<AtomicUsize>,
}

impl CountingMeasurer {
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl TextMeasurer for CountingMeasurer {
    fn measure(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let width = text.len() as f32 * 10.0;
        let height = 20.0;
        if let Some(max_width) = available_width {
            if max_width > 0.0 && width > max_width {
                return (max_width, (width / max_width).ceil() * height);
            }
        }
        (width, height)
    }

    fn measure_rich_text(&self, runs: &[TextRun], available_width: Option<f32>) -> (f32, f32) {
        let text: String = runs.iter().map(|run| run.text.as_str()).collect();
        self.measure(&text, 16.0, available_width)
    }
}

fn flex_root(root_id: WidgetId, children_ids: Vec<WidgetId>) -> LayoutInputNode {
    LayoutInputNode {
        id: root_id,
        parent_id: None,
        op: IrLayoutOp::Flex {
            direction: FlexDirection::Column,
            wrap: fission_ir::op::FlexWrap::NoWrap,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            padding: [0.0; 4],
            gap: Some(4.0),
            align_items: fission_ir::op::AlignItems::Start,
            justify_content: fission_ir::op::JustifyContent::Start,
        },
        children_ids,
        debug_name: "root".into(),
        width: Some(400.0),
        height: Some(300.0),
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: None,
    }
}

fn text_node(id: WidgetId, parent_id: WidgetId, text: &str) -> LayoutInputNode {
    LayoutInputNode {
        id,
        parent_id: Some(parent_id),
        op: IrLayoutOp::Box {
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        },
        children_ids: Vec::new(),
        debug_name: format!("text-{}", id.as_u128()),
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
                font_family: None,
                locale: None,
                font_weight: 400,
                font_style: fission_ir::op::FontStyle::Normal,
                line_height: None,
                letter_spacing: 0.0,
                background_color: None,
            },
        }]),
    }
}

#[test]
fn incremental_layout_reuses_clean_sibling_subtrees() {
    let root_id = WidgetId::from_u128(1);
    let first_id = WidgetId::from_u128(2);
    let second_id = WidgetId::from_u128(3);
    let nodes_v1 = vec![
        flex_root(root_id, vec![first_id, second_id]),
        text_node(first_id, root_id, "alpha"),
        text_node(second_id, root_id, "beta"),
    ];

    let measurer = CountingMeasurer::new();
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(measurer.clone()));
    let first = engine
        .compute_layout(&nodes_v1, root_id, LayoutSize::new(400.0, 300.0), &|_| 0.0)
        .expect("initial layout");
    let initial_calls = measurer.call_count();

    let full_measurer = CountingMeasurer::new();
    let mut full_engine = LayoutEngine::new().with_measurer(Arc::new(full_measurer.clone()));
    let nodes_v2 = vec![
        flex_root(root_id, vec![first_id, second_id]),
        text_node(first_id, root_id, "alpha"),
        text_node(second_id, root_id, "beta beta"),
    ];
    full_engine
        .compute_layout(&nodes_v2, root_id, LayoutSize::new(400.0, 300.0), &|_| 0.0)
        .expect("full recompute layout");
    let full_recompute_calls = full_measurer.call_count();

    let dirty = HashSet::from([second_id]);
    let second = engine
        .compute_layout_incremental(
            &nodes_v2,
            root_id,
            LayoutSize::new(400.0, 300.0),
            &|_| 0.0,
            &first,
            &dirty,
        )
        .expect("incremental layout");

    let incremental_calls = measurer.call_count();
    assert!(incremental_calls > initial_calls);
    assert!(
        incremental_calls - initial_calls < full_recompute_calls,
        "incremental pass should reuse the clean sibling subtree"
    );
    assert_eq!(
        second.get_node_geometry(first_id).unwrap().content_size,
        first.get_node_geometry(first_id).unwrap().content_size,
        "clean sibling subtree should be reused"
    );
    assert!(
        second
            .get_node_geometry(second_id)
            .unwrap()
            .content_size
            .width
            > first
                .get_node_geometry(second_id)
                .unwrap()
                .content_size
                .width
    );
}
