use fission_core::{
    hit_test::hit_test_with_scroll, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, Runtime,
};
use fission_ir::{CoreIR, FlexDirection, LayoutOp, NodeId, Op, PaintOp};
use fission_layout::LayoutNodeGeometry;
use std::collections::HashMap;

#[test]
fn test_scroll_hit_test_logic() {
    let scroll_id = NodeId::derived(0, &[1]);
    let column_id = NodeId::derived(0, &[2]);
    let button_id = NodeId::derived(0, &[3]);

    let mut ir = CoreIR::new();
    ir.set_root(scroll_id);

    // Scroll Container
    ir.add_node(
        scroll_id,
        Op::Layout(LayoutOp::Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            width: Some(100.0),
            height: Some(100.0),
            padding: [0.0; 4],
        }),
        vec![column_id],
    );

    // Column (Content)
    ir.add_node(
        column_id,
        Op::Layout(LayoutOp::Box {
            width: Some(100.0),
            height: Some(200.0),
            padding: [0.0; 4],
        }),
        vec![button_id],
    );

    // Button
    ir.add_node(
        button_id,
        Op::Paint(PaintOp::DrawRect {
            fill: None,
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );

    let mut snapshot = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));

    // Scroll: 100x100 viewport
    snapshot.nodes.insert(
        scroll_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            content_size: LayoutSize::new(100.0, 200.0),
        },
    );

    // Column: 100x200 content, at 0,0 relative to Scroll
    snapshot.nodes.insert(
        column_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 0.0, 100.0, 200.0),
            content_size: LayoutSize::new(100.0, 200.0),
        },
    );

    // Button: at 0, 150. Size 100x20.
    snapshot.nodes.insert(
        button_id,
        LayoutNodeGeometry {
            rect: LayoutRect::new(0.0, 150.0, 100.0, 20.0),
            content_size: LayoutSize::new(100.0, 20.0),
        },
    );

    let mut runtime = Runtime::default();

    // Case 1: No Scroll. Click at 60. Should hit Column (if not Button).
    // Button is at 150. Click at 60 misses Button.
    let hit = hit_test_with_scroll(
        &ir,
        &snapshot,
        &runtime.runtime_state.scroll,
        LayoutPoint::new(50.0, 60.0),
    );
    // Should hit column (ID 2) or Scroll (ID 1) if column is transparent?
    // Hit test returns deepest node.
    // Column contains point.
    assert_eq!(hit, Some(column_id));

    // Case 2: Scroll 100. Click at 60.
    // Logical click = 60 + 100 = 160.
    // Button is at 150..170. Should hit Button.
    runtime.runtime_state.scroll.set_offset(scroll_id, 100.0);

    let hit = hit_test_with_scroll(
        &ir,
        &snapshot,
        &runtime.runtime_state.scroll,
        LayoutPoint::new(50.0, 60.0),
    );

    if hit != Some(button_id) {
        println!("Expected Button {:?}, got {:?}", button_id, hit);
    }
    assert_eq!(hit, Some(button_id));
}
