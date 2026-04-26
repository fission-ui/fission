use fission_core::ui::{Container, Text, Node, Row, Column};
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_test::TestHarness;
use fission_ir::{Op, Semantics};
use fission_core::lowering::{NodeBuilder, LoweringContext};
use fission_core::LowerDyn;

#[derive(Debug, Default, Clone)]
struct State;
impl AppState for State {}

#[derive(Debug)]
struct MockHero {
    child: Node,
}

impl LowerDyn for MockHero {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> fission_ir::NodeId {
        let child_id = self.child.lower(cx);
        let id = cx.next_node_id();
        let semantics = Semantics { hero_tag: Some("t".into()), ..Default::default() };
        let mut builder = NodeBuilder::new(id, Op::Semantics(semantics));
        builder.add_child(child_id);
        builder.build(cx)
    }
    fn stable_key(&self) -> u64 { 0 }
}

#[test]
fn test_email_list_overlap_regression() {
    // Regression test for issue where wrapped text inside a Hero (Semantics) 
    // container would fail to wrap in layout (due to flex_shrink: 0 default) 
    // causing visual overlap with subsequent items.

    struct EmailRow;
    impl Widget<State> for EmailRow {
        fn build(&self, _ctx: &mut BuildCtx<State>, _view: &View<State>) -> Node {
            Container::new(
                Row::default()
                    .children(vec![
                        // Text Column
                        Container::new(
                            Column::default()
                                .children(vec![
                                    // Hero Subject
                                    Node::Custom(fission_core::ui::CustomNode {
                                        debug_tag: "Hero".into(),
                                        lowerer: Some(std::sync::Arc::new(MockHero {
                                            child: Text::new("Subject 10 Subject 10 Subject 10")
                                                .min_width(0.0) // Ensure it can shrink
                                                .into_node()
                                        })),
                                        render_object: None,
                                    }),
                                    // Preview
                                    Text::new("Short preview...")
                                        .min_width(0.0)
                                        .into_node(),
                                ])
                                .into_node()
                        )
                        .min_width(0.0)
                        .flex_grow(1.0)
                        .into_node(),
                    ])
                    .into_node()
            )
            .width(100.0) // Constrain width
            .into_node()
        }
    }

    let mut h = TestHarness::new(State);
    h = h.with_root_widget(EmailRow);
    h.pump().unwrap();

    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();

    // Find "Short preview..." rect
    let mut preview_rect = None;
    let mut subject_rect = None;
    
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) = &node.op {
            let geom = snap.get_node_geometry(*id).unwrap();
            if text.contains("preview") {
                preview_rect = Some(geom.rect);
            } else if text.contains("Subject") {
                subject_rect = Some(geom.rect);
            }
        }
    }
    
    let subject = subject_rect.expect("Subject text not found");
    let preview = preview_rect.expect("Preview text not found");
    
    println!("Subject: {:?}", subject);
    println!("Preview: {:?}", preview);
    
    let font_size = h.env.theme.tokens.typography.body_medium_size;
    let (_single_line_w, single_line_h) = h
        .measurer
        .measure("Subject 10 Subject 10 Subject 10", font_size, None);
    // Subject height should reflect wrapping. If it's a single line height,
    // it failed to wrap in layout.
    assert!(
        subject.height() > single_line_h,
        "Subject text did not wrap in layout! Height is {}",
        subject.height()
    );
    
    // Preview Y should be below Subject
    assert!(preview.y() >= subject.y() + subject.height(), 
        "Preview overlaps Subject! Preview Y: {}, Subject Bottom: {}", preview.y(), subject.y() + subject.height());
}
