use fission_core::env::{Env, RuntimeState};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::ui::widgets::text::{InlineWidgetSpan, RichTextChild, RichTextSpan};
use fission_core::ui::{Checkbox, Node, Radio, RichText, Spacer};
use fission_ir::{CoreIR, LayoutOp, NodeId, Op};
use fission_layout::{
    LayoutEngine, LayoutSize, RichTextInlineBox, RichTextLayoutInfo, TextMeasurer,
};
use std::collections::HashMap;
use std::sync::Arc;

struct SimpleMeasurer;

impl TextMeasurer for SimpleMeasurer {
    fn measure(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let char_width = 8.0;
        let line_height = 16.0;
        let width = text.len() as f32 * char_width;
        if let Some(max_w) = available_width {
            if max_w > 0.0 && width > max_w {
                let lines = (width / max_w).ceil();
                return (max_w, lines * line_height);
            }
        }
        (width, line_height)
    }

    fn measure_rich_text(
        &self,
        runs: &[fission_ir::op::TextRun],
        available_width: Option<f32>,
    ) -> (f32, f32) {
        let text: String = runs.iter().map(|r| r.text.clone()).collect();
        self.measure(&text, 16.0, available_width)
    }
}

struct InlineWidgetMeasurer;

impl TextMeasurer for InlineWidgetMeasurer {
    fn measure(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        SimpleMeasurer.measure(text, 16.0, available_width)
    }

    fn measure_rich_text(
        &self,
        runs: &[fission_ir::op::TextRun],
        available_width: Option<f32>,
    ) -> (f32, f32) {
        layout_info_size(self.layout_rich_text(runs, available_width))
    }

    fn layout_rich_text(
        &self,
        _runs: &[fission_ir::op::TextRun],
        _available_width: Option<f32>,
    ) -> RichTextLayoutInfo {
        RichTextLayoutInfo {
            width: 72.0,
            height: 20.0,
            inline_boxes: vec![RichTextInlineBox {
                id: 0,
                x: 14.0,
                y: 6.0,
                width: 18.0,
                height: 10.0,
            }],
        }
    }
}

fn layout_info_size(value: RichTextLayoutInfo) -> (f32, f32) {
    (value.width, value.height)
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn parent_map(ir: &CoreIR) -> HashMap<NodeId, NodeId> {
    let mut map = HashMap::new();
    for (id, node) in &ir.nodes {
        for child in &node.children {
            map.insert(*child, *id);
        }
    }
    map
}

fn find_boxes_by_size(ir: &CoreIR, width: f32, height: f32) -> Vec<NodeId> {
    let mut out = Vec::new();
    for (id, node) in &ir.nodes {
        if let Op::Layout(LayoutOp::Box {
            width: Some(w),
            height: Some(h),
            ..
        }) = &node.op
        {
            if approx_eq(*w, width) && approx_eq(*h, height) {
                out.push(*id);
            }
        }
    }
    out
}

fn layout_from_node(node: Node) -> (CoreIR, fission_layout::LayoutSnapshot) {
    layout_from_node_with_measurer(node, Arc::new(SimpleMeasurer))
}

fn layout_from_node_with_measurer(
    node: Node,
    measurer: Arc<dyn TextMeasurer>,
) -> (CoreIR, fission_layout::LayoutSnapshot) {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let measurer_ref = measurer.clone();

    let mut cx = LoweringContext::new(&env, &runtime_state, Some(&measurer_ref), None);
    let root_id = node.lower(&mut cx);
    cx.ir.root = Some(root_id);
    let input_nodes = build_layout_tree(&cx.ir, &env);

    let mut engine = LayoutEngine::new().with_measurer(measurer);
    engine.rebuild(&input_nodes).unwrap();
    let snapshot = engine
        .compute_layout(
            &input_nodes,
            root_id,
            LayoutSize::new(200.0, 200.0),
            &|_| 0.0,
        )
        .unwrap();
    (cx.ir, snapshot)
}

fn rect_center(rect: fission_layout::LayoutRect) -> (f32, f32) {
    (
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    )
}

#[test]
fn checkbox_checkmark_centered() {
    let checkbox = Checkbox {
        checked: true,
        ..Default::default()
    };
    let (ir, snapshot) = layout_from_node(checkbox.into_node());
    let parents = parent_map(&ir);

    let check_id = find_boxes_by_size(&ir, 10.0, 10.0)
        .into_iter()
        .next()
        .expect("checkbox check box");
    let mut current = Some(check_id);
    let mut square_id = None;
    while let Some(id) = current {
        if let Op::Layout(LayoutOp::Box {
            width: Some(w),
            height: Some(h),
            ..
        }) = &ir.nodes.get(&id).unwrap().op
        {
            if approx_eq(*w, 18.0) && approx_eq(*h, 18.0) {
                square_id = Some(id);
                break;
            }
        }
        current = parents.get(&id).copied();
    }
    let square_id = square_id.expect("checkbox square box");

    let square_rect = snapshot.get_node_geometry(square_id).unwrap().rect;
    let check_rect = snapshot.get_node_geometry(check_id).unwrap().rect;

    let (sx, sy) = rect_center(square_rect);
    let (cx, cy) = rect_center(check_rect);

    assert!(
        approx_eq(sx, cx) && approx_eq(sy, cy),
        "checkbox checkmark should be centered"
    );
}

#[test]
fn radio_dot_centered() {
    let radio = Radio {
        checked: true,
        ..Default::default()
    };
    let (ir, snapshot) = layout_from_node(radio.into_node());
    let parents = parent_map(&ir);

    let dot_id = find_boxes_by_size(&ir, 9.0, 9.0)
        .into_iter()
        .next()
        .expect("radio dot box");

    let mut current = Some(dot_id);
    let mut container_id = None;
    while let Some(id) = current {
        if let Op::Layout(LayoutOp::Box {
            width: Some(w),
            height: Some(h),
            ..
        }) = &ir.nodes.get(&id).unwrap().op
        {
            if approx_eq(*w, 18.0) && approx_eq(*h, 18.0) {
                container_id = Some(id);
                break;
            }
        }
        current = parents.get(&id).copied();
    }

    let container_id = container_id.expect("radio dot container");
    let container_rect = snapshot.get_node_geometry(container_id).unwrap().rect;
    let dot_rect = snapshot.get_node_geometry(dot_id).unwrap().rect;

    let (sx, sy) = rect_center(container_rect);
    let (cx, cy) = rect_center(dot_rect);

    assert!(
        approx_eq(sx, cx) && approx_eq(sy, cy),
        "radio dot should be centered"
    );
}

#[test]
fn rich_text_inline_widget_uses_layout_inline_box_positions() {
    let rich_text = RichText::from_spans(vec![
        RichTextChild::from(RichTextSpan::new("Before ")),
        RichTextChild::from(InlineWidgetSpan::new(
            Spacer {
                width: Some(18.0),
                height: Some(10.0),
                ..Default::default()
            }
            .into_node(),
            18.0,
            10.0,
        )),
        RichTextChild::from(RichTextSpan::new(" after")),
    ]);

    let (ir, snapshot) =
        layout_from_node_with_measurer(rich_text.into_node(), Arc::new(InlineWidgetMeasurer));

    let paint_node = ir
        .nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Paint(fission_ir::PaintOp::DrawRichText { .. }) => Some((*id, node)),
            _ => None,
        })
        .expect("rich text paint node");

    assert_eq!(paint_node.1.children.len(), 1);
    let inline_widget_id = paint_node.1.children[0];
    let inline_rect = snapshot
        .get_node_geometry(inline_widget_id)
        .expect("inline widget geometry")
        .rect;

    assert!(approx_eq(inline_rect.x(), 14.0));
    assert!(approx_eq(inline_rect.y(), 6.0));
    assert!(approx_eq(inline_rect.width(), 18.0));
    assert!(approx_eq(inline_rect.height(), 10.0));
}
