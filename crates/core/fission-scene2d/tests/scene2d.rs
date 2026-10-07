use fission_core::Widget;
use fission_scene::{
    AssetDescriptor, AssetId, AssetKind, Bounds2, NodeId, PresentationId, Rgba, SceneId,
    Transform2, Vec2,
};
use fission_scene2d::*;

fn rect_node(id: u64, layer: i32, rect: Rect2D) -> Node2D {
    Node2D {
        id: NodeId(id),
        parent: None,
        transform: Transform2::IDENTITY,
        visible: true,
        opacity: 1.0,
        layer,
        blend_mode: BlendMode2D::Normal,
        clip: None,
        content: NodeContent2D::Rectangle {
            rect,
            style: PathStyle2D {
                fill: Some(Fill2D { color: Rgba::WHITE }),
                stroke: None,
            },
            corner_radius: 0.0,
        },
        interaction: None,
    }
}

fn scene() -> Scene2DIR {
    Scene2DIR::new(SceneId(1), Viewport2D::new(200.0, 100.0))
}

fn asset(id: u64, kind: AssetKind) -> AssetDescriptor {
    AssetDescriptor {
        id: AssetId(id),
        kind,
        source: format!("assets/{id}.png"),
        sha256: "a".repeat(64),
    }
}

#[test]
fn scene_ir_is_closed_and_round_trips() {
    let mut scene = scene();
    scene.nodes.push(rect_node(
        1,
        0,
        Rect2D::new(Vec2::ZERO, Vec2::new(20.0, 10.0)),
    ));
    let json = serde_json::to_string(&scene).unwrap();
    assert_eq!(serde_json::from_str::<Scene2DIR>(&json).unwrap(), scene);
}

#[test]
fn hierarchy_camera_and_layer_order_are_deterministic() {
    let mut scene = scene();
    scene.camera.center = Vec2::new(50.0, 0.0);
    let mut parent = Node2D::group(NodeId(1));
    parent.transform.translation = Vec2::new(50.0, 0.0);
    let mut child = rect_node(2, 5, Rect2D::new(Vec2::ZERO, Vec2::new(10.0, 10.0)));
    child.parent = Some(parent.id);
    let front = rect_node(3, 10, Rect2D::new(Vec2::ZERO, Vec2::new(10.0, 10.0)));
    scene.nodes.extend([front, parent, child]);

    let prepared = Scene2DProcessor::prepare(&scene);
    assert!(
        prepared.diagnostics.is_empty(),
        "{:?}",
        prepared.diagnostics
    );
    assert_eq!(prepared.draws.len(), 2);
    assert_eq!(prepared.draws[0].metadata().node, NodeId(2));
    assert_eq!(prepared.draws[1].metadata().node, NodeId(3));
    assert_eq!(
        prepared.bounds(NodeId(2)).unwrap().min,
        Vec2::new(50.0, 0.0)
    );
}

#[test]
fn an_image_batch_survives_processing_as_one_batch() {
    let mut scene = scene();
    scene.assets.assets.push(asset(9, AssetKind::Image));
    scene.nodes.push(Node2D {
        id: NodeId(7),
        parent: None,
        transform: Transform2::IDENTITY,
        visible: true,
        opacity: 1.0,
        layer: 0,
        blend_mode: BlendMode2D::Normal,
        clip: None,
        interaction: None,
        content: NodeContent2D::ImageBatch(ImageBatch2D {
            image: ImageHandle2D::new(AssetId(9)),
            sampling: ImageSampling2D::Nearest,
            instances: vec![
                ImageInstance2D {
                    transform: Transform2 {
                        translation: Vec2::new(0.0, 0.0),
                        ..Transform2::IDENTITY
                    },
                    destination: Rect2D::new(Vec2::ZERO, Vec2::new(8.0, 8.0)),
                    source: None,
                    tint: Rgba::WHITE,
                    opacity: 1.0,
                },
                ImageInstance2D {
                    transform: Transform2 {
                        translation: Vec2::new(1000.0, 0.0),
                        ..Transform2::IDENTITY
                    },
                    destination: Rect2D::new(Vec2::ZERO, Vec2::new(8.0, 8.0)),
                    source: None,
                    tint: Rgba::WHITE,
                    opacity: 1.0,
                },
            ],
        }),
    });

    let prepared = Scene2DProcessor::prepare(&scene);
    assert_eq!(prepared.draws.len(), 1);
    let DrawCommand2D::ImageBatch { instances, .. } = &prepared.draws[0] else {
        panic!("batch was expanded during scene processing");
    };
    assert_eq!(instances.len(), 1, "offscreen instances should be culled");
    assert_eq!(prepared.stats.batches, 1);
    assert_eq!(prepared.stats.drawn, 1);
    assert_eq!(prepared.stats.retained_resources, 1);
}

#[test]
fn lowering_keeps_one_scene_batch_as_one_paint_operation() {
    let mut scene = scene();
    scene.assets.assets.push(asset(9, AssetKind::Image));
    scene.nodes.push(Node2D {
        id: NodeId(7),
        parent: None,
        transform: Transform2::IDENTITY,
        visible: true,
        opacity: 1.0,
        layer: 0,
        blend_mode: BlendMode2D::Normal,
        clip: None,
        interaction: Some(Interaction2D {
            tap: Some(ActionBinding2D::new(ActionToken(77), b"batch".to_vec())),
            semantic_label: Some("Batch item".into()),
            semantic_role: Some(SemanticRole2D::Button),
            ..Default::default()
        }),
        content: NodeContent2D::ImageBatch(ImageBatch2D {
            image: ImageHandle2D::new(AssetId(9)),
            sampling: ImageSampling2D::Nearest,
            instances: vec![
                ImageInstance2D {
                    transform: Transform2::IDENTITY,
                    destination: Rect2D::new(Vec2::ZERO, Vec2::new(8.0, 8.0)),
                    source: None,
                    tint: Rgba::WHITE,
                    opacity: 1.0,
                },
                ImageInstance2D {
                    transform: Transform2 {
                        translation: Vec2::new(12.0, 0.0),
                        ..Transform2::IDENTITY
                    },
                    destination: Rect2D::new(Vec2::ZERO, Vec2::new(8.0, 8.0)),
                    source: None,
                    tint: Rgba::WHITE,
                    opacity: 1.0,
                },
            ],
        }),
    });

    let ir = fission_core::internal::lower_widget_to_ir(&Widget::from(Scene2D::new(scene)));
    let batches = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            fission_ir::Op::Paint(fission_ir::PaintOp::DrawImageBatch {
                request,
                instances,
                ..
            }) => Some((request, instances)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].1.len(), 2);
    assert!(matches!(
        &batches[0].0.source,
        fission_ir::op::ImageSource::Asset { path } if path == "assets/9.png"
    ));
    assert!(!ir.nodes.values().any(|node| matches!(
        node.op,
        fission_ir::Op::Layout(fission_ir::LayoutOp::Embed { .. })
    )));
    assert!(
        ir.nodes.iter().all(|(id, node)| node.parent != Some(*id)),
        "retained nodes must not parent themselves"
    );
    let viewport_clips = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            fission_ir::Op::Layout(fission_ir::LayoutOp::Clip { path }) => path.as_deref(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(viewport_clips, vec!["M0 0 L200 0 L200 100 L0 100 Z"]);
    let identifiers = ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            fission_ir::Op::Semantics(semantics) => semantics.identifier.as_deref(),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        identifiers,
        std::collections::BTreeSet::from([
            "scene2d:1:node:7:instance:0",
            "scene2d:1:node:7:instance:1",
        ])
    );
}

#[test]
fn image_handles_must_resolve_to_image_descriptors() {
    let mut scene = scene();
    scene.assets.assets.push(asset(41, AssetKind::Texture));
    for (node_id, asset_id) in [(1, 40), (2, 41)] {
        scene.nodes.push(Node2D {
            id: NodeId(node_id),
            parent: None,
            transform: Transform2::IDENTITY,
            visible: true,
            opacity: 1.0,
            layer: 0,
            blend_mode: BlendMode2D::Normal,
            clip: None,
            interaction: None,
            content: NodeContent2D::Image {
                image: ImageHandle2D::new(AssetId(asset_id)),
                destination: Rect2D::new(Vec2::ZERO, Vec2::new(16.0, 16.0)),
                source: None,
                sampling: ImageSampling2D::Linear,
                tint: Rgba::WHITE,
            },
        });
    }

    let prepared = Scene2DProcessor::prepare(&scene);
    assert!(prepared.draws.is_empty());
    assert_eq!(prepared.stats.rejected, 2);
    let missing = prepared
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "scene2d.missing-image-asset")
        .unwrap();
    assert_eq!(missing.asset, Some(AssetId(40)));
    assert_eq!(missing.node, Some(NodeId(1)));
    let wrong_kind = prepared
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "scene2d.wrong-asset-kind")
        .unwrap();
    assert_eq!(wrong_kind.asset, Some(AssetId(41)));
    assert_eq!(wrong_kind.node, Some(NodeId(2)));
}

#[test]
fn decorative_content_is_not_an_interaction_target() {
    let mut scene = scene();
    let rect = Rect2D::new(Vec2::new(-10.0, -10.0), Vec2::new(20.0, 20.0));
    scene.nodes.push(rect_node(1, 0, rect));
    let mut interactive = rect_node(2, 1, rect);
    interactive.interaction = Some(Interaction2D {
        tap: Some(ActionBinding2D::new(ActionToken(42), b"collect".to_vec())),
        long_press: Some(ActionBinding2D::new(ActionToken(43), b"inspect".to_vec())),
        drag: Some(DragActions2D {
            cancel: Some(ActionBinding2D::new(ActionToken(44), b"cancel".to_vec())),
            ..DragActions2D::default()
        }),
        semantic_label: Some("Collect".into()),
        semantic_role: Some(SemanticRole2D::Button),
        ..Interaction2D::default()
    });
    scene.nodes.push(interactive);

    let prepared = Scene2DProcessor::prepare(&scene);
    let center = Vec2::new(100.0, 50.0);
    assert_eq!(prepared.pick(center).unwrap().node, NodeId(2));
    assert_eq!(prepared.pick_interactive(center).unwrap().node, NodeId(2));
    assert_eq!(
        prepared.draws[1]
            .metadata()
            .interaction
            .as_ref()
            .unwrap()
            .action_for(InteractionEvent2D::Activate),
        Some(&ActionBinding2D::new(ActionToken(42), b"collect".to_vec()))
    );

    scene.nodes.pop();
    assert!(Scene2DProcessor::prepare(&scene)
        .pick_interactive(center)
        .is_none());
}

#[test]
fn lowering_makes_decorations_inert_and_actions_semantic() {
    let mut scene = scene();
    let rect = Rect2D::new(Vec2::new(-10.0, -10.0), Vec2::new(20.0, 20.0));
    scene.nodes.push(rect_node(1, 0, rect));
    let mut interactive = rect_node(2, 1, rect);
    interactive.interaction = Some(Interaction2D {
        tap: Some(ActionBinding2D::new(ActionToken(42), b"collect".to_vec())),
        long_press: Some(ActionBinding2D::new(ActionToken(43), b"inspect".to_vec())),
        drag: Some(DragActions2D {
            cancel: Some(ActionBinding2D::new(ActionToken(44), b"cancel".to_vec())),
            ..DragActions2D::default()
        }),
        semantic_label: Some("Collect".into()),
        semantic_role: Some(SemanticRole2D::Button),
        ..Default::default()
    });
    scene.nodes.push(interactive);

    let ir = fission_core::internal::lower_widget_to_ir(&Widget::from(
        Scene2D::new(scene).presentation_id(PresentationId::new(9)),
    ));
    let inert_visuals = ir
        .nodes
        .values()
        .filter(|node| {
            matches!(
                node.op,
                fission_ir::Op::Structural(fission_ir::StructuralOp::PointerTransparent { .. })
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(inert_visuals.len(), 2);
    assert!(inert_visuals.iter().all(|visual| {
        visual.parent.is_some_and(|parent| {
            matches!(
                ir.nodes.get(&parent).map(|node| &node.op),
                Some(fission_ir::Op::Layout(
                    fission_ir::LayoutOp::Positioned { .. }
                ))
            )
        })
    }));
    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            fission_ir::Op::Semantics(value) => Some(value),
            _ => None,
        })
        .expect("interactive scene object semantics");
    assert_eq!(semantics.identifier.as_deref(), Some("scene2d:9:node:2"));
    let target = semantics.scene_target.as_ref().expect("scene target");
    assert_eq!(target.scene_id, 1);
    assert_eq!(target.presentation_id, 9);
    let action = |trigger| {
        semantics
            .actions
            .entries
            .iter()
            .find(|entry| entry.trigger == trigger)
            .expect("declared scene action")
    };
    assert_eq!(
        action(fission_ir::semantics::ActionTrigger::Default).action_id,
        42
    );
    assert_eq!(
        action(fission_ir::semantics::ActionTrigger::Default)
            .payload_data
            .as_deref(),
        Some(b"collect".as_slice())
    );
    assert_eq!(
        action(fission_ir::semantics::ActionTrigger::LongPress).action_id,
        43
    );
    assert_eq!(
        action(fission_ir::semantics::ActionTrigger::LongPress)
            .payload_data
            .as_deref(),
        Some(b"inspect".as_slice())
    );
    assert_eq!(
        action(fission_ir::semantics::ActionTrigger::DragCancel).action_id,
        44
    );
    assert_eq!(
        action(fission_ir::semantics::ActionTrigger::DragCancel)
            .payload_data
            .as_deref(),
        Some(b"cancel".as_slice())
    );
}

#[test]
fn cycles_missing_resources_and_offscreen_content_are_accounted_for() {
    let mut scene = scene();
    let mut first = Node2D::group(NodeId(1));
    first.parent = Some(NodeId(2));
    let mut second = Node2D::group(NodeId(2));
    second.parent = Some(NodeId(1));
    scene.nodes.extend([first, second]);
    scene.nodes.push(Node2D {
        id: NodeId(3),
        parent: None,
        transform: Transform2::IDENTITY,
        visible: true,
        opacity: 1.0,
        layer: 0,
        blend_mode: BlendMode2D::Normal,
        clip: None,
        content: NodeContent2D::Path {
            path: ResourceId(99),
            style: PathStyle2D {
                fill: Some(Fill2D { color: Rgba::WHITE }),
                stroke: None,
            },
        },
        interaction: None,
    });
    scene.nodes.push(rect_node(
        4,
        0,
        Rect2D::new(Vec2::new(1000.0, 1000.0), Vec2::new(5.0, 5.0)),
    ));

    let prepared = Scene2DProcessor::prepare(&scene);
    assert!(prepared
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "scene2d.hierarchy-cycle"));
    assert!(prepared.stats.rejected >= 3);
    assert_eq!(prepared.stats.culled, 1);
}

#[test]
fn clips_and_parent_opacity_are_retained_on_draws() {
    let mut scene = scene();
    let mut parent = Node2D::group(NodeId(1));
    parent.opacity = 0.5;
    parent.clip = Some(Clip2D::Rectangle {
        rect: Rect2D::new(Vec2::new(-5.0, -5.0), Vec2::new(10.0, 10.0)),
    });
    let mut child = rect_node(
        2,
        0,
        Rect2D::new(Vec2::new(-10.0, -10.0), Vec2::new(20.0, 20.0)),
    );
    child.parent = Some(parent.id);
    child.opacity = 0.5;
    scene.nodes.extend([parent, child]);

    let prepared = Scene2DProcessor::prepare(&scene);
    let metadata = prepared.draws[0].metadata();
    assert_eq!(metadata.opacity, 0.25);
    assert_eq!(metadata.clips.len(), 1);
    assert!(metadata.clips[0]
        .view_bounds
        .contains(Vec2::new(100.0, 50.0)));
}

#[test]
fn resolved_text_and_paths_are_retained_resources() {
    let mut scene = scene();
    scene.resources.paths.insert(
        ResourceId(1),
        PathResource2D {
            commands: vec![
                PathCommand2D::MoveTo { point: Vec2::ZERO },
                PathCommand2D::LineTo {
                    point: Vec2::new(10.0, 10.0),
                },
            ],
            bounds: Bounds2::new(Vec2::ZERO, Vec2::new(10.0, 10.0)),
        },
    );
    scene.resources.text.insert(
        ResourceId(2),
        TextResource2D {
            text: "Hi".into(),
            font_key: "resolved/sans/regular".into(),
            font_size: 16.0,
            glyphs: vec![Glyph2D {
                id: 72,
                position: Vec2::ZERO,
                advance: 8.0,
            }],
            bounds: Bounds2::new(Vec2::ZERO, Vec2::new(16.0, 18.0)),
            color: Rgba::WHITE,
        },
    );
    scene.nodes.push(Node2D {
        content: NodeContent2D::Path {
            path: ResourceId(1),
            style: PathStyle2D {
                fill: None,
                stroke: Some(Stroke2D {
                    color: Rgba::WHITE,
                    width: 1.0,
                }),
            },
        },
        ..Node2D::group(NodeId(1))
    });
    scene.nodes.push(Node2D {
        content: NodeContent2D::Text {
            text: ResourceId(2),
        },
        ..Node2D::group(NodeId(2))
    });

    let prepared = Scene2DProcessor::prepare(&scene);
    assert!(
        prepared.diagnostics.is_empty(),
        "{:?}",
        prepared.diagnostics
    );
    assert_eq!(prepared.stats.retained_resources, 2);
}
