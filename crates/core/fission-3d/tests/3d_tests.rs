use fission_3d::{Point3D, Primitive3D, Scene3D, Scene3DLowerer};
use fission_core::{env::Env, lowering::LoweringContext, op::Color, ui::traits::LowerDyn, NodeId, WidgetNodeId, RuntimeState};
use fission_ir::op::{EmbedKind, LayoutOp};

#[test]
fn test_scene3d_builder() {
    let scene = Scene3D::new(800.0, 600.0)
        .add_primitive(Primitive3D::Cube {
            center: Point3D::new(0.0, 0.0, 0.0),
            size: 1.0,
            color: Color::RED,
        })
        .add_primitive(Primitive3D::Sphere {
            center: Point3D::new(2.0, 2.0, 2.0),
            radius: 0.5,
            color: Color::BLUE,
        });

    assert_eq!(scene.width, 800.0);
    assert_eq!(scene.height, 600.0);
    assert_eq!(scene.primitives.len(), 2);
}

#[test]
fn test_scene3d_lowering() {
    let scene = Scene3D::new(100.0, 200.0);
    let lowerer = Scene3DLowerer { scene };
    
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);
    
    // Simulate lowering context initialization
    let root_id = cx.next_node_id();
    cx.push_scope(root_id);

    let generated_id = lowerer.lower_dyn(&mut cx);
    
    let ir = cx.ir;
    let node = ir.nodes.get(&generated_id).expect("Node should exist");
    
    match &node.op {
        fission_ir::Op::Layout(LayoutOp::Embed { kind, width, height, .. }) => {
            assert_eq!(*kind, EmbedKind::Custom);
            assert_eq!(*width, Some(100.0));
            assert_eq!(*height, Some(200.0));
        },
        _ => panic!("Expected Embed LayoutOp"),
    }
}
