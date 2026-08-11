use fission_3d::{Point3D, Primitive3D, Scene3D, Scene3DInternalLowerer};
use fission_core::{
    env::Env,
    internal::{InternalLowerer, InternalLoweringCx},
    op::Color,
    RuntimeState,
};
use fission_ir::op::{EmbedKind, LayoutOp};
use fission_render::embed_surface_id;

#[test]
fn renderer_compatibility_facade_preserves_historical_method_signatures() {
    use fission_3d::render::{Scene3DRenderer, Scene3DViewport};
    use wgpu::{Device, Queue, TextureFormat, TextureView};

    let _: fn(&Device, u32, u32, TextureFormat) -> Scene3DRenderer = Scene3DRenderer::new;
    let _: fn(&mut Scene3DRenderer, &Device, u32, u32) = Scene3DRenderer::resize;
    let _: fn(&mut Scene3DRenderer, &Device, &Queue, &TextureView, &Scene3D) =
        Scene3DRenderer::render;
    let _: fn(
        &mut Scene3DRenderer,
        &Device,
        &Queue,
        &TextureView,
        &Scene3D,
        Scene3DViewport,
    ) = Scene3DRenderer::render_in_rect;
}

#[test]
fn test_scene3d_builder() {
    let scene = Scene3D::new()
        .width(800.0)
        .height(600.0)
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

    assert_eq!(scene.width, Some(800.0));
    assert_eq!(scene.height, Some(600.0));
    assert_eq!(scene.primitives.len(), 2);
}

#[test]
fn test_scene3d_lowering() {
    let scene = Scene3D::new().width(100.0).height(200.0);
    let lowerer = Scene3DInternalLowerer { scene };

    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);

    // Simulate lowering context initialization
    let root_id = cx.next_node_id();
    cx.push_scope(root_id);

    let generated_id = lowerer.lower_dyn(&mut cx);

    let ir = cx.ir;
    let node = ir.nodes.get(&generated_id).expect("Node should exist");

    match &node.op {
        fission_ir::Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(payload),
            width,
            height,
            ..
        }) => {
            assert_eq!(width.as_ref().copied(), Some(100.0));
            assert_eq!(height.as_ref().copied(), Some(200.0));
            assert!(!payload.is_empty());
        }
        _ => panic!("Expected Embed LayoutOp"),
    }
}

#[test]
fn two_scene3d_instances_lower_to_distinct_external_slots() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
    let root_id = cx.next_node_id();
    cx.push_scope(root_id);

    let first_node = Scene3DInternalLowerer {
        scene: Scene3D::new(),
    }
    .lower_dyn(&mut cx);
    let second_node = Scene3DInternalLowerer {
        scene: Scene3D::new(),
    }
    .lower_dyn(&mut cx);

    let embed = |node_id| match &cx.ir.nodes.get(&node_id).expect("lowered 3D node").op {
        fission_ir::Op::Layout(LayoutOp::Embed {
            kind, widget_id, ..
        }) => (kind, *widget_id),
        _ => panic!("expected 3D embed"),
    };
    let (first_kind, first_widget_id) = embed(first_node);
    let (second_kind, second_widget_id) = embed(second_node);

    assert_ne!(first_widget_id, second_widget_id);
    assert_ne!(
        embed_surface_id(first_kind, first_widget_id),
        embed_surface_id(second_kind, second_widget_id)
    );
}
