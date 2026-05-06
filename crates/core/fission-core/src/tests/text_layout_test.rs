use crate::env::{Env, RuntimeState};
use crate::lowering::{build_layout_tree, LoweringContext};
use crate::ui::widgets::text::TextContent;
use crate::ui::widgets::{Container, Text};
use crate::ui::Node;
use fission_ir::NodeId;
use fission_ir::{LayoutOp, Op};
use fission_layout::{LayoutEngine, LayoutSize};

struct SimpleMeasurer;
impl fission_layout::TextMeasurer for SimpleMeasurer {
    fn measure(&self, text: &str, _size: f32, avail: Option<f32>) -> (f32, f32) {
        let char_width = 10.0;
        let line_height = 20.0;
        let width = text.len() as f32 * char_width;
        if let Some(w) = avail {
            if width > w {
                // Wrap
                let lines = (width / w).ceil();
                return (w, lines * line_height);
            }
        }
        (width, line_height)
    }
    fn hit_test(&self, _: &str, _: f32, _: Option<f32>, _: f32, _: f32) -> usize {
        0
    }
    fn get_line_metrics(&self, _: &str, _: f32, _: Option<f32>) -> Vec<fission_layout::LineMetric> {
        vec![]
    }
    fn get_caret_position(&self, _: &str, _: f32, _: Option<f32>, _: usize) -> (f32, f32) {
        (0.0, 0.0)
    }
    fn measure_rich_text(
        &self,
        runs: &[fission_ir::op::TextRun],
        avail: Option<f32>,
    ) -> (f32, f32) {
        let text: String = runs.iter().map(|r| r.text.clone()).collect();
        self.measure(&text, 16.0, avail)
    }
}

#[test]
fn test_text_wrapping_in_constrained_flex() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);

    let root_base = NodeId::derived(0xABC, &[0]);
    cx.push_scope(root_base);
    let root_id = cx.next_node_id();
    let row_id = cx.next_node_id();
    let text_id = cx.next_node_id();

    // Text "Hello World" (11 chars) -> 110px width.
    let text_builder = crate::lowering::NodeBuilder::new(
        text_id,
        Op::Paint(fission_ir::PaintOp::DrawText {
            text: "Hello World".into(),
            size: 16.0,
            color: fission_ir::op::Color::BLACK,
            underline: false,
            wrap: true,
            caret_index: None,
        }),
    );
    let text_final = text_builder.build(&mut cx);

    // Row: Width 50px.
    let mut row_builder = crate::lowering::NodeBuilder::new(
        row_id,
        Op::Layout(LayoutOp::Flex {
            direction: fission_ir::FlexDirection::Row,
            wrap: fission_ir::FlexWrap::NoWrap,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            padding: [0.0; 4],
            gap: None,
            align_items: fission_ir::op::AlignItems::Stretch,
            justify_content: fission_ir::op::JustifyContent::Start,
        }),
    );
    row_builder.add_child(text_final);
    let row_final = row_builder.build(&mut cx);

    // Root: Box 50px.
    let mut root_builder = crate::lowering::NodeBuilder::new(
        root_id,
        Op::Layout(LayoutOp::Box {
            width: Some(50.0),
            height: Some(100.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
    );
    root_builder.add_child(row_final);
    let root_final = root_builder.build(&mut cx);

    cx.ir.root = Some(root_final);

    let input_nodes = build_layout_tree(&cx.ir, &env);
    let mut engine = LayoutEngine::new().with_measurer(std::sync::Arc::new(SimpleMeasurer));
    engine.rebuild(&input_nodes).unwrap();
    let snapshot = engine
        .compute_layout(
            &input_nodes,
            root_final,
            LayoutSize::new(800.0, 600.0),
            &|_| 0.0,
        )
        .unwrap();

    let text_geom = snapshot.get_node_geometry(text_final).unwrap();

    // Expected: Width 50.0 (Constrained), Height > 20.0 (Wrapped).
    // "Hello World" (110px) in 50px -> 3 lines (50, 50, 10). Height 60.
    assert_eq!(text_geom.rect.width(), 50.0);
    assert!(text_geom.rect.height() > 20.0, "Text should wrap");
}

#[test]
fn text_parent_max_width_drives_wrapping() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);

    let text = Text {
        content: TextContent::Literal("HelloWorld".into()),
        max_width: Some(40.0),
        ..Default::default()
    };
    let root = Container::new(Node::from(text))
        .width(200.0)
        .height(200.0)
        .into_node();

    let root_id = root.lower(&mut cx);
    cx.ir.root = Some(root_id);

    let input_nodes = build_layout_tree(&cx.ir, &env);
    let mut engine = LayoutEngine::new().with_measurer(std::sync::Arc::new(SimpleMeasurer));
    engine.rebuild(&input_nodes).unwrap();
    let snapshot = engine
        .compute_layout(
            &input_nodes,
            root_id,
            LayoutSize::new(800.0, 600.0),
            &|_| 0.0,
        )
        .unwrap();

    let text_paint_id = cx
        .ir
        .nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) if text == "HelloWorld" => {
                Some(*id)
            }
            _ => None,
        })
        .expect("expected DrawText node");

    let text_geom = snapshot.get_node_geometry(text_paint_id).unwrap();
    assert_eq!(text_geom.rect.width(), 40.0);
    assert!(
        text_geom.rect.height() > 20.0,
        "text should wrap when parent max_width is set"
    );
}
