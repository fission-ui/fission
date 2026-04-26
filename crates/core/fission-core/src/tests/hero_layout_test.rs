use crate::env::{Env, RuntimeState};
use crate::lowering::{LoweringContext, build_layout_tree, NodeBuilder};
use crate::ui::traits::{Lower, LowerDyn};
use crate::ui::Node;
use fission_ir::{LayoutOp, Op, Semantics};
use fission_layout::{LayoutEngine, LayoutSize};

#[derive(Debug)]
struct MockHero {
    child: Node,
}

impl LowerDyn for MockHero {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> fission_ir::NodeId {
        let child_id = self.child.lower(cx);
        let id = cx.next_node_id();
        
        let semantics = Semantics {
            hero_tag: Some("test".into()),
            ..Default::default()
        };
        
        // Hero lowers to Semantics op
        let mut builder = NodeBuilder::new(id, Op::Semantics(semantics));
        builder.add_child(child_id);
        builder.build(cx)
    }
    fn stable_key(&self) -> u64 { 0 }
}

#[test]
fn test_hero_text_layout_height() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);

    // Construct Hero with Text that needs wrapping
    let text = crate::ui::Text::new("Long Subject That Wraps").into_node();
    let hero = Node::Custom(crate::ui::CustomNode {
        debug_tag: "Hero".into(),
        lowerer: Some(std::sync::Arc::new(MockHero { child: text })),
        render_object: None,
    });

    // VStack
    let vstack = crate::ui::Column::default()
        .children(vec![
            hero,
            crate::ui::Text::new("Preview").into_node()
        ])
        .into_node();

    // Root Container (Constraint 100px width)
    let root = crate::ui::Container::new(vstack)
        .width(100.0)
        .into_node();

    let root_id = root.lower(&mut cx);
    cx.ir.root = Some(root_id);

    // Mock Measurer where "Long Subject That Wraps" > 100px
    struct MockMeasurer;
    impl fission_layout::TextMeasurer for MockMeasurer {
        fn measure(&self, text: &str, _: f32, avail: Option<f32>) -> (f32, f32) {
            // Assume 10px per char
            let width = text.len() as f32 * 10.0;
            if let Some(w) = avail {
                if width > w {
                    // Wrap to 2 lines
                    return (w, 40.0); // 20px * 2
                }
            }
            (width, 20.0)
        }
        fn hit_test(&self, _: &str, _: f32, _: Option<f32>, _: f32, _: f32) -> usize { 0 }
        fn get_line_metrics(&self, _: &str, _: f32, _: Option<f32>) -> Vec<fission_layout::LineMetric> { vec![] }
        fn get_caret_position(&self, _: &str, _: f32, _: Option<f32>, _: usize) -> (f32, f32) { (0.0, 0.0) }
        fn measure_rich_text(&self, runs: &[fission_ir::op::TextRun], avail: Option<f32>) -> (f32, f32) { 
            let text: String = runs.iter().map(|r| r.text.clone()).collect();
            // Simulate that "Preview" fits, but "Long..." wraps
            if text.contains("Long") {
                self.measure(&text, 16.0, avail)
            } else {
                (text.len() as f32 * 10.0, 20.0)
            }
        }
    }

    let input_nodes = build_layout_tree(&cx.ir, &env);
    let mut engine = LayoutEngine::new().with_measurer(std::sync::Arc::new(MockMeasurer));
    engine.rebuild(&input_nodes).unwrap();
    let snapshot = engine.compute_layout(
        &input_nodes, 
        root_id, 
        LayoutSize::new(800.0, 600.0), 
        &|_| 0.0
    ).unwrap();

    // Verify Hero Height
    // root -> container -> column -> hero(semantics) -> text
    let root_node = cx.ir.nodes.get(&root_id).unwrap(); // Container
    let col_id = root_node.children[0]; // Column
    let col_node = cx.ir.nodes.get(&col_id).unwrap();
    let hero_id = col_node.children[0]; // Hero Semantics
    let preview_id = col_node.children[1]; // Preview Text

    let hero_geom = snapshot.get_node_geometry(hero_id).unwrap();
    let preview_geom = snapshot.get_node_geometry(preview_id).unwrap();

    assert_eq!(hero_geom.rect.height(), 40.0, "Hero should wrap to 2 lines");
    assert!(preview_geom.rect.y() >= 40.0, "Preview should be below Hero");
}