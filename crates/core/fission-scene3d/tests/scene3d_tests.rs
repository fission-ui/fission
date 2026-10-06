use fission_scene::{NodeId, SceneId, Transform3, Vec2, Vec3};
use fission_scene3d::{
    AlphaMode3D, Material3D, Node3D, NodeContent3D, Primitive3D, RenderCapabilities3D, ResourceId,
    Scene3DIR, Scene3DProcessor, Viewport3D,
};

fn cube(id: u64, z: f32, material: Option<ResourceId>) -> Node3D {
    Node3D {
        id: NodeId(id),
        parent: None,
        transform: Transform3 {
            translation: Vec3::new(0.0, 0.0, z),
            ..Transform3::IDENTITY
        },
        visible: true,
        content: NodeContent3D::Primitive(Primitive3D::Cube { size: Vec3::ONE }),
        material,
        blend_order: 0,
        pickable: true,
    }
}

#[test]
fn scene_ir_round_trips_and_picking_returns_stable_identity() {
    let mut scene = Scene3DIR::new(SceneId(4), Viewport3D::new(640.0, 480.0));
    scene.camera = fission_scene3d::Camera3D::perspective(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        60.0_f32.to_radians(),
    );
    scene.nodes.push(cube(9, 0.0, None));

    let encoded = serde_json::to_string(&scene).unwrap();
    let decoded: Scene3DIR = serde_json::from_str(&encoded).unwrap();
    let prepared = Scene3DProcessor::new().prepare(&decoded, RenderCapabilities3D::default());

    assert!(
        prepared.diagnostics.is_empty(),
        "{:?}",
        prepared.diagnostics
    );
    assert_eq!(
        prepared
            .pick_viewport(Vec2::new(320.0, 240.0))
            .unwrap()
            .node,
        NodeId(9)
    );
}

#[test]
fn transparent_draws_are_deliberately_ordered_after_opaque_draws() {
    let material = ResourceId(1);
    let mut scene = Scene3DIR::new(SceneId(1), Viewport3D::new(640.0, 480.0));
    scene.camera = fission_scene3d::Camera3D::perspective(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        60.0_f32.to_radians(),
    );
    scene.resources.materials.insert(
        material,
        Material3D {
            alpha_mode: AlphaMode3D::Blend,
            ..Material3D::default()
        },
    );
    scene.nodes.push(cube(1, 0.0, material.into()));
    scene.nodes.push(cube(2, -2.0, None));

    let prepared = Scene3DProcessor::new().prepare(&scene, RenderCapabilities3D::default());
    assert_eq!(prepared.draws[0].node, NodeId(2));
    assert_eq!(prepared.draws[1].node, NodeId(1));
}

#[test]
fn unsupported_capability_is_not_a_blank_surface() {
    let material = ResourceId(1);
    let mut scene = Scene3DIR::new(SceneId(1), Viewport3D::new(640.0, 480.0));
    scene.resources.materials.insert(
        material,
        Material3D {
            alpha_mode: AlphaMode3D::Blend,
            ..Material3D::default()
        },
    );
    scene.nodes.push(cube(1, 0.0, Some(material)));

    let prepared = Scene3DProcessor::new().prepare(
        &scene,
        RenderCapabilities3D {
            alpha_blending: false,
            ..RenderCapabilities3D::default()
        },
    );
    assert!(prepared
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "scene3d.unsupported-alpha-blending"));
    assert!(!prepared.is_renderable());
}
