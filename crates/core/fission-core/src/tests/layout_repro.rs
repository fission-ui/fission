use crate::env::{Env, RuntimeState};
use crate::lowering::{build_layout_tree, LoweringContext};
use fission_ir::{FlexDirection, LayoutOp, NodeId, Op};
use fission_layout::{LayoutEngine, LayoutSize};

#[test]
fn test_absolute_fill_inside_grown_container() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);

    let root_base = NodeId::derived(0xDEF, &[0]);
    cx.push_scope(root_base);
    let root_id = cx.next_node_id();
    let row_id = cx.next_node_id();
    let container_id = cx.next_node_id();
    let abs_id = cx.next_node_id();
    let zstack_id = cx.next_node_id();

    // 1. ZStack
    let zstack = crate::lowering::NodeBuilder::new(zstack_id, Op::Layout(LayoutOp::ZStack));
    let zstack_final = zstack.build(&mut cx);

    // 2. AbsFill
    let mut abs = crate::lowering::NodeBuilder::new(abs_id, Op::Layout(LayoutOp::AbsoluteFill));
    abs.add_child(zstack_final);
    let abs_final = abs.build(&mut cx);

    // 3. Container (Box, Auto size, Grow 1)
    let mut container = crate::lowering::NodeBuilder::new(
        container_id,
        Op::Layout(LayoutOp::Box {
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 1.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
    );
    container.add_child(abs_final);
    let container_final = container.build(&mut cx);

    // 4. Row (Flex Row)
    let mut row = crate::lowering::NodeBuilder::new(
        row_id,
        Op::Layout(LayoutOp::Flex {
            direction: FlexDirection::Row,
            wrap: fission_ir::FlexWrap::NoWrap,
            flex_grow: 1.0,
            flex_shrink: 0.0,
            padding: [0.0; 4],
            gap: None,
            align_items: fission_ir::op::AlignItems::Stretch,
            justify_content: fission_ir::op::JustifyContent::Start,
        }),
    );
    row.add_child(container_final);
    let row_final = row.build(&mut cx);

    // 5. Root (Fixed 800x600)
    let mut root = crate::lowering::NodeBuilder::new(
        root_id,
        Op::Layout(LayoutOp::Box {
            width: Some(800.0),
            height: Some(600.0),
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
    root.add_child(row_final);
    let root_final = root.build(&mut cx);

    cx.ir.root = Some(root_final);

    let input_nodes = build_layout_tree(&cx.ir, &env);
    let mut engine = LayoutEngine::new();
    engine.rebuild(&input_nodes).unwrap();
    let snapshot = engine
        .compute_layout(
            &input_nodes,
            root_final,
            LayoutSize::new(800.0, 600.0),
            &|_| 0.0,
        )
        .unwrap();

    let _container_geom = snapshot.get_node_geometry(container_final).unwrap();
    let abs_geom = snapshot.get_node_geometry(abs_final).unwrap();

    assert_eq!(abs_geom.rect.width(), 800.0, "AbsFill width");
}
