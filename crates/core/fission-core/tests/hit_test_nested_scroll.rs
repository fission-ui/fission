use fission_core::{
    hit_test::hit_test_with_scroll, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, Runtime,
};
use fission_ir::{CoreIR, FlexDirection, LayoutOp, NodeId, Op, PaintOp};
use fission_layout::LayoutNodeGeometry;

#[test]
fn test_nested_scroll_hit_test() {
    let scroll_a = NodeId::derived(0, &[1]);
    let scroll_b = NodeId::derived(0, &[2]);
    let button_c = NodeId::derived(0, &[3]);

    let mut ir = CoreIR::new();
    ir.set_root(scroll_a);

    // Scroll A: 100x100 viewport at 0,0.
    ir.add_node(
        scroll_a,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            width: Some(100.0),
            height: Some(100.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![scroll_b],
    );

    // Scroll B: 100x100 viewport at 0, 150 (relative to A's content).
    ir.add_node(
        scroll_b,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            width: Some(100.0),
            height: Some(100.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![button_c],
    );

    // Button C: at 0, 50 relative to B's content (so 0, 150+50 = 200 relative to A's content).
    let mut semantics = fission_ir::Semantics::default();
    semantics.focusable = true;
    ir.add_node(
        button_c,
        Op::Semantics(semantics),
        vec![],
    );

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));

    // Scroll A: Absolute (0, 0, 100, 100). Content 100x1000.
    snapshot.nodes.insert(
        scroll_a,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            content_size: LayoutSize::new(100.0, 1000.0),
        },
    );

    // Scroll B: Absolute (0, 150, 100, 100). (Since A is at 0,0 and B is at 150 in A).
    snapshot.nodes.insert(
        scroll_b,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 150.0, 100.0, 100.0),
            content_size: LayoutSize::new(100.0, 1000.0),
        },
    );

    // Button C: Absolute (0, 200, 100, 20). (Since B is at 150 and C is at 50 in B).
    snapshot.nodes.insert(
        button_c,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 200.0, 100.0, 20.0),
            content_size: LayoutSize::new(100.0, 20.0),
        },
    );

    let mut runtime = Runtime::default();

    // Scroll A by 120. B (at 150) is now visible at screen y = 150 - 120 = 30.
    runtime.runtime_state.scroll.set_offset(scroll_a, 120.0);
    
    // Scroll B by 40. C (at 200 absolute, so at 50 relative to B) is now visible at screen y = 30 + (50 - 40) = 40.
    runtime.runtime_state.scroll.set_offset(scroll_b, 40.0);

    // Click at screen y = 40.
    let hit = hit_test_with_scroll(
        &ir,
        &snapshot,
        &runtime.runtime_state.scroll,
        LayoutPoint::new(50.0, 40.0),
    );

    // Expected: logical y relative to A = 40 + 120 = 160. (Hits B at 150..250).
    // logical y relative to B = 160 + 40 = 200. (Hits C at 200..220).
    
    assert_eq!(hit, Some(button_c));
}
