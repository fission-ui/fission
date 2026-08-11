use super::{
    build_local_paint_list, scroll_offsets_changed, translation_matrix, InvalidationSet, Pipeline,
};
use fission_core::env::{Env, VideoState, VideoStateMap, WebState, WebStateMap};
use fission_core::MotionPropertyId;
use fission_core::ScrollStateMap;
use fission_ir::op::{
    Color, Fill, ImageAlignment, ImageFit, ImageRequest, ImageSource, RichTextAnnotation, TextRun,
    TextStyle,
};
use fission_ir::semantics::ActionTrigger;
use fission_ir::{
    ActionEntry, CompositeScalar, CompositeStyle, CoreIR, EmbedKind, LayoutOp, Op, PaintOp,
    WidgetId,
};
use fission_layout::{LayoutEngine, LayoutRect, LayoutSize};
use fission_render::{DisplayOp, RenderScene, Renderer};
use std::collections::HashMap;
use std::sync::Arc;

struct NullRenderer;

impl Renderer for NullRenderer {
    fn render_scene(&mut self, _scene: &RenderScene) -> anyhow::Result<()> {
        Ok(())
    }
}

fn two_child_layout_ir(second_width: f32) -> CoreIR {
    let root = WidgetId::derived(50, &[0]);
    let first = WidgetId::derived(50, &[1]);
    let second = WidgetId::derived(50, &[2]);
    let mut ir = CoreIR::new();
    ir.add_node(
        first,
        Op::Layout(LayoutOp::Box {
            width: Some(40.0),
            height: Some(20.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        }),
        vec![],
    );
    ir.add_node(
        second,
        Op::Layout(LayoutOp::Box {
            width: Some(second_width),
            height: Some(20.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        }),
        vec![],
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Flex {
            direction: fission_ir::FlexDirection::Column,
            wrap: fission_ir::op::FlexWrap::NoWrap,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            padding: [0.0; 4],
            gap: Some(4.0),
            align_items: fission_ir::op::AlignItems::Start,
            justify_content: fission_ir::op::JustifyContent::Start,
        }),
        vec![first, second],
    );
    ir.set_root(root);
    ir
}

#[test]
fn unchanged_scroll_offsets_do_not_invalidate_cache() {
    let id = WidgetId::derived(1, &[0]);
    let mut prev = HashMap::new();
    prev.insert(id, 12.5f32.to_bits());
    let mut scroll = ScrollStateMap::default();
    scroll.set_offset(id, 12.5);
    assert!(!scroll_offsets_changed(&prev, &scroll));
}

#[test]
fn changed_scroll_offsets_invalidate_cache() {
    let id = WidgetId::derived(2, &[0]);
    let mut prev = HashMap::new();
    prev.insert(id, 0.0f32.to_bits());
    let mut scroll = ScrollStateMap::default();
    scroll.set_offset(id, 4.0);
    assert!(scroll_offsets_changed(&prev, &scroll));
}

#[test]
fn incremental_layout_keeps_rebuild_telemetry_honest() {
    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll = ScrollStateMap::default();

    pipeline.replace_ir(two_child_layout_ir(60.0), &Env::default());
    let first_pass = pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            &mut layout_engine,
            &scroll,
        )
        .expect("initial layout");
    assert_eq!(first_pass, pipeline.layout_input_nodes.len());
    assert_eq!(pipeline.layout_full_rebuild_count, 1);

    pipeline.replace_ir(two_child_layout_ir(90.0), &Env::default());
    let second_pass = pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            &mut layout_engine,
            &scroll,
        )
        .expect("incremental layout");

    assert_eq!(second_pass, 1);
    assert_eq!(pipeline.layout_full_rebuild_count, 1);
    assert!(pipeline.pending_layout_dirty_nodes.is_empty());
}

#[test]
fn rich_text_annotations_flow_into_display_ops() {
    let node_id = WidgetId::derived(9, &[0]);
    let mut ir = CoreIR::new();
    ir.add_node(
        node_id,
        Op::Paint(PaintOp::DrawRichText {
            runs: vec![TextRun {
                text: "docs".into(),
                style: TextStyle {
                    font_size: 14.0,
                    color: Color::BLACK,
                    underline: false,
                    font_family: None,
                    locale: None,
                    font_weight: 400,
                    font_style: fission_ir::op::FontStyle::Normal,
                    line_height: None,
                    letter_spacing: 0.0,
                    background_color: None,
                },
            }],
            wrap: true,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
        vec![],
    );
    ir.custom_render_objects.insert(
        node_id,
        Arc::new(vec![RichTextAnnotation {
            range: 0..4,
            semantics_label: Some("Documentation".into()),
            semantics_identifier: Some("docs-link".into()),
            spell_out: Some(true),
            mouse_cursor: Some(fission_ir::op::MouseCursor::Pointer),
            actions: vec![ActionEntry {
                trigger: ActionTrigger::Default,
                action_id: 7,
                payload_data: Some(vec![1, 2, 3]),
            }],
        }]),
    );

    let node = ir.nodes.get(&node_id).expect("paint node");
    let list = build_local_paint_list(&ir, node_id, node, LayoutRect::new(0.0, 0.0, 160.0, 40.0))
        .expect("display list");
    match list.ops.first() {
        Some(DisplayOp::DrawRichText { annotations, .. }) => {
            assert_eq!(annotations.len(), 1);
            assert_eq!(annotations[0].range, 0..4);
            assert_eq!(
                annotations[0].semantics_identifier.as_deref(),
                Some("docs-link")
            );
        }
        other => panic!("expected rich text op, got {other:?}"),
    }
}

#[test]
fn draw_image_paint_ops_flow_into_display_ops() {
    let node_id = WidgetId::derived(12, &[0]);
    let request = ImageRequest {
        source: ImageSource::Network {
            url: "https://example.com/product.webp".into(),
            headers: Vec::new(),
            cache_policy: Default::default(),
        },
        cache_width: Some(220),
        cache_height: Some(160),
        semantic_label: Some("Product thumbnail".into()),
        ..Default::default()
    };
    let mut ir = CoreIR::new();
    ir.add_node(
        node_id,
        Op::Paint(PaintOp::DrawImage {
            request: request.clone(),
            fit: ImageFit::Cover,
            alignment: ImageAlignment::Center,
        }),
        vec![],
    );

    let node = ir.nodes.get(&node_id).expect("image node");
    let rect = LayoutRect::new(24.0, 32.0, 220.0, 160.0);
    let list = build_local_paint_list(&ir, node_id, node, rect).expect("display list");

    match list.ops.first() {
        Some(DisplayOp::DrawImage {
            rect: image_rect,
            request: image_request,
            fit,
            alignment,
            bounds,
            node_id: Some(image_node_id),
        }) => {
            assert_eq!(*image_rect, rect);
            assert_eq!(image_request, &request);
            assert_eq!(*fit, fission_render::ImageFit::Cover);
            assert_eq!(*alignment, ImageAlignment::Center);
            assert_eq!(*bounds, rect);
            assert_eq!(*image_node_id, node_id);
        }
        other => panic!("expected image display op, got {other:?}"),
    }
}

#[test]
fn retained_pipeline_scene_keeps_draw_image_ops() {
    let image_id = WidgetId::derived(13, &[0]);
    let root_id = WidgetId::derived(13, &[1]);
    let request = ImageRequest {
        source: ImageSource::Network {
            url: "https://example.com/catalog/thumbnail.webp".into(),
            headers: Vec::new(),
            cache_policy: Default::default(),
        },
        semantic_label: Some("Catalog thumbnail".into()),
        ..Default::default()
    };
    let mut ir = CoreIR::new();
    ir.add_node(
        image_id,
        Op::Paint(PaintOp::DrawImage {
            request: request.clone(),
            fit: ImageFit::Cover,
            alignment: ImageAlignment::Center,
        }),
        vec![],
    );
    ir.add_node(
        root_id,
        Op::Layout(LayoutOp::Box {
            width: Some(220.0),
            height: Some(160.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![image_id],
    );
    ir.set_root(root_id);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll = ScrollStateMap::default();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            &mut layout_engine,
            &scroll,
        )
        .unwrap();
    pipeline
        .prepare_current(
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            false,
            &scroll,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    let display_list = pipeline.retained_scene().expect("retained scene").flatten();
    let image_op = display_list.ops.iter().find_map(|op| match op {
        DisplayOp::DrawImage {
            rect,
            request: image_request,
            fit,
            alignment,
            ..
        } => Some((rect, image_request, fit, alignment)),
        _ => None,
    });

    let Some((rect, image_request, fit, alignment)) = image_op else {
        panic!("retained scene dropped DrawImage op");
    };
    assert_eq!(image_request, &request);
    assert_eq!(*fit, fission_render::ImageFit::Cover);
    assert_eq!(*alignment, ImageAlignment::Center);
    assert_eq!(rect.size.width, 220.0);
    assert_eq!(rect.size.height, 160.0);
}

#[test]
fn embed_layout_ops_flow_into_surface_display_ops() {
    let node_id = WidgetId::derived(14, &[0]);
    let widget_id = WidgetId::explicit("embed.surface");
    let mut ir = CoreIR::new();
    ir.add_node(
        node_id,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Web,
            widget_id,
            width: Some(320.0),
            height: Some(180.0),
        }),
        vec![],
    );

    let node = ir.nodes.get(&node_id).expect("embed node");
    let rect = LayoutRect::new(12.0, 24.0, 320.0, 180.0);
    let list = build_local_paint_list(&ir, node_id, node, rect).expect("display list");

    match list.ops.first() {
        Some(DisplayOp::DrawSurface {
            rect: surface_rect,
            bounds,
            node_id: Some(surface_node_id),
            ..
        }) => {
            assert_eq!(*surface_rect, rect);
            assert_eq!(*bounds, rect);
            assert_eq!(*surface_node_id, node_id);
        }
        other => panic!("expected surface display op, got {other:?}"),
    }
}

#[test]
fn custom_embeds_flow_into_native_surfaces() {
    let node_id = WidgetId::derived(15, &[0]);
    let widget_id = WidgetId::explicit("custom.surface");
    let payload = vec![0x4d, 0x41, 0x50, 0x01];
    let mut ir = CoreIR::new();
    ir.add_node(
        node_id,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(payload.clone()),
            widget_id,
            width: Some(320.0),
            height: Some(180.0),
        }),
        vec![],
    );
    ir.set_root(node_id);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll = ScrollStateMap::default();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            &mut layout_engine,
            &scroll,
        )
        .unwrap();
    pipeline
        .prepare_current(
            LayoutSize::new(320.0, 240.0),
            LayoutSize::new(320.0, 240.0),
            false,
            &scroll,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    assert_eq!(
        pipeline.native_surfaces,
        vec![fission_shell::NativeSurfaceFrame {
            widget_id,
            rect: LayoutRect::new(0.0, 0.0, 320.0, 180.0),
            payload,
            visible_rect: LayoutRect::new(0.0, 0.0, 320.0, 180.0),
            transform: None,
            opacity: 1.0,
            paint_order: 0,
        }]
    );
}

#[test]
fn compositor_bound_opacity_animation_is_composite_only() {
    let mut ir = CoreIR::new();
    let child = WidgetId::derived(10, &[1]);
    let root = WidgetId::derived(10, &[0]);
    ir.add_node(child, Op::Layout(LayoutOp::AbsoluteFill), vec![]);
    ir.add_node_with_composite(
        root,
        Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
        CompositeStyle {
            opacity: Some(CompositeScalar::new(0.0).motion(WidgetId::explicit("fade"))),
            ..Default::default()
        },
        vec![child],
    );
    ir.set_root(root);

    let mut pipeline = Pipeline::new();
    pipeline.replace_ir(ir, &Env::default());
    let invalidation = pipeline
        .classify_animation_updates(&[(WidgetId::explicit("fade"), MotionPropertyId::Opacity)]);
    assert_eq!(
        invalidation,
        InvalidationSet {
            build: false,
            layout: false,
            paint: false,
            composite: true,
        }
    );
}

#[test]
fn unbound_custom_animation_requires_build() {
    let pipeline = Pipeline::new();
    let invalidation = pipeline.classify_animation_updates(&[(
        WidgetId::explicit("custom"),
        MotionPropertyId::custom("phase"),
    )]);
    assert!(invalidation.build);
    assert!(invalidation.layout);
}

#[test]
fn compositor_bound_translate_animation_is_composite_only() {
    let mut ir = CoreIR::new();
    let child = WidgetId::derived(11, &[1]);
    let root = WidgetId::derived(11, &[0]);
    ir.add_node(
        child,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node_with_composite(
        root,
        Op::Layout(LayoutOp::Box {
            width: Some(120.0),
            height: Some(64.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        CompositeStyle {
            translate_x: Some(CompositeScalar::new(12.0).motion(WidgetId::explicit("slide"))),
            ..Default::default()
        },
        vec![child],
    );
    ir.set_root(root);

    let mut pipeline = Pipeline::new();
    pipeline.replace_ir(ir, &Env::default());
    let invalidation = pipeline
        .classify_animation_updates(&[(WidgetId::explicit("slide"), MotionPropertyId::TranslateX)]);
    assert_eq!(
        invalidation,
        InvalidationSet {
            build: false,
            layout: false,
            paint: false,
            composite: true,
        }
    );
}

#[test]
fn dynamic_layer_with_static_contents_gets_content_cache_key() {
    let mut ir = CoreIR::new();
    let child = WidgetId::derived(12, &[1]);
    let root = WidgetId::derived(12, &[0]);
    ir.add_node(
        child,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 20,
                g: 40,
                b: 60,
                a: 255,
            })),
            stroke: None,
            corner_radius: 8.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node_with_composite(
        root,
        Op::Layout(LayoutOp::Box {
            width: Some(160.0),
            height: Some(72.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        CompositeStyle {
            opacity: Some(CompositeScalar::new(0.4).motion(WidgetId::explicit("fade-cache"))),
            ..Default::default()
        },
        vec![child],
    );
    ir.set_root(root);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let mut renderer = NullRenderer;
    let scroll = ScrollStateMap::default();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            &mut layout_engine,
            &scroll,
        )
        .unwrap();
    pipeline
        .render_current(
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            false,
            &mut renderer,
            &scroll,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    let scene = pipeline
        .retained_scene
        .as_ref()
        .expect("retained scene missing");
    let presentation_root = match scene.roots.first() {
        Some(fission_render::RenderNode::Layer(layer)) => layer,
        _ => panic!("missing presentation layer"),
    };
    let animated_layer = match presentation_root.children.first() {
        Some(fission_render::RenderNode::Layer(layer)) => layer,
        _ => panic!("missing animated layer"),
    };

    assert!(animated_layer.style.cache_key.is_none());
    assert!(animated_layer.style.content_cache_key.is_some());
}

#[test]
fn nested_dynamic_descendant_becomes_child_texture_plan() {
    let mut ir = CoreIR::new();
    let left_paint = WidgetId::derived(13, &[0]);
    let animated_paint = WidgetId::derived(13, &[1]);
    let animated_wrapper = WidgetId::derived(13, &[2]);
    let outer_static = WidgetId::derived(13, &[3]);
    let outer_group = WidgetId::derived(13, &[4]);
    let root = WidgetId::derived(13, &[5]);

    ir.add_node(
        left_paint,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 10,
                g: 10,
                b: 10,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        animated_paint,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 200,
                g: 40,
                b: 40,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node_with_composite(
        animated_wrapper,
        Op::Layout(LayoutOp::Box {
            width: Some(96.0),
            height: Some(96.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        CompositeStyle {
            opacity: Some(CompositeScalar::new(0.4).motion(WidgetId::explicit("nested-fade"))),
            ..Default::default()
        },
        vec![animated_paint],
    );
    ir.add_node(
        outer_static,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 20,
                g: 100,
                b: 180,
                a: 255,
            })),
            stroke: None,
            corner_radius: 8.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        outer_group,
        Op::Layout(LayoutOp::Box {
            width: Some(160.0),
            height: Some(120.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![outer_static, animated_wrapper],
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Box {
            width: Some(320.0),
            height: Some(240.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![left_paint, outer_group],
    );
    ir.set_root(root);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll = ScrollStateMap::default();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            &mut layout_engine,
            &scroll,
        )
        .unwrap();
    pipeline
        .prepare_current(
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            false,
            &scroll,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    let plans = pipeline.texture_compositor_plans();
    assert!(!plans.is_empty());
    assert!(
        plans.iter().any(|plan| !plan.children.is_empty()),
        "expected at least one retained texture plan to extract nested dynamic descendants"
    );
}

#[test]
fn resize_preview_keeps_texture_compositor_root_transform() {
    let mut ir = CoreIR::new();
    let left = WidgetId::derived(14, &[0]);
    let right = WidgetId::derived(14, &[1]);
    let root = WidgetId::derived(14, &[2]);

    ir.add_node(
        left,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 80,
                g: 80,
                b: 80,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        right,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 180,
                g: 180,
                b: 180,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Box {
            width: Some(300.0),
            height: Some(200.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![left, right],
    );
    ir.set_root(root);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll = ScrollStateMap::default();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 300.0, 200.0),
            &mut layout_engine,
            &scroll,
        )
        .unwrap();
    pipeline
        .prepare_current(
            LayoutSize {
                width: 540.0,
                height: 360.0,
            },
            LayoutSize {
                width: 300.0,
                height: 200.0,
            },
            true,
            &scroll,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    assert!(pipeline.texture_compositor_root_transform().is_none());
    assert!(!pipeline.texture_compositor_plans().is_empty());
}

#[test]
fn scroll_only_layers_patch_retained_transforms_after_offset_changes() {
    let mut ir = CoreIR::new();
    let content = WidgetId::derived(15, &[0]);
    let scroll = WidgetId::derived(15, &[1]);
    let root = WidgetId::derived(15, &[2]);

    ir.add_node(
        content,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 120,
                g: 120,
                b: 220,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        scroll,
        Op::Layout(LayoutOp::Scroll {
            direction: fission_ir::FlexDirection::Column,
            show_scrollbar: true,
            width: Some(320.0),
            height: Some(240.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![content],
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Box {
            width: Some(320.0),
            height: Some(240.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![scroll],
    );
    ir.set_root(root);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll0 = ScrollStateMap::default();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            &mut layout_engine,
            &scroll0,
        )
        .unwrap();
    pipeline
        .prepare_current(
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            false,
            &scroll0,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    let mut scroll1 = ScrollStateMap::default();
    scroll1.set_offset(scroll, 180.0);
    pipeline
        .prepare_current(
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            false,
            &scroll1,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    fn find_layer_by_node(
        node: &fission_render::RenderNode,
        node_id: WidgetId,
    ) -> Option<&fission_render::RenderLayer> {
        match node {
            fission_render::RenderNode::Paint(_) => None,
            fission_render::RenderNode::Layer(layer) => {
                if layer.node_id == Some(node_id) {
                    return Some(layer);
                }
                for child in &layer.children {
                    if let Some(found) = find_layer_by_node(child, node_id) {
                        return Some(found);
                    }
                }
                None
            }
        }
    }

    let scroll_layer = pipeline
        .retained_scene()
        .and_then(|scene| {
            scene
                .roots
                .iter()
                .find_map(|node| find_layer_by_node(node, scroll))
        })
        .expect("expected a retained scroll layer");
    assert!(
        scroll_layer.style.transform.is_none(),
        "scrollbar chrome must not inherit the content scroll transform"
    );
    let transform = scroll_layer
        .children
        .iter()
        .find_map(|child| match child {
            fission_render::RenderNode::Layer(layer) => layer.style.transform,
            fission_render::RenderNode::Paint(_) => None,
        })
        .expect("scroll content layer should carry a compositor transform");
    assert!(
        (transform[13] + 180.0).abs() <= 0.01,
        "expected retained content transform to patch to -180, got {}",
        transform[13]
    );
}

#[test]
fn scrollbar_thumb_patches_after_scroll_offset_changes() {
    let mut ir = CoreIR::new();
    let fill = WidgetId::derived(18, &[0]);
    let content = WidgetId::derived(18, &[1]);
    let scroll = WidgetId::derived(18, &[2]);
    let root = WidgetId::derived(18, &[3]);

    ir.add_node(
        fill,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 120,
                g: 120,
                b: 220,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        content,
        Op::Layout(LayoutOp::Box {
            width: Some(320.0),
            height: Some(640.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![fill],
    );
    ir.add_node(
        scroll,
        Op::Layout(LayoutOp::Scroll {
            direction: fission_ir::FlexDirection::Column,
            show_scrollbar: true,
            width: Some(320.0),
            height: Some(240.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![content],
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Box {
            width: Some(320.0),
            height: Some(240.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![scroll],
    );
    ir.set_root(root);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll0 = ScrollStateMap::default();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            &mut layout_engine,
            &scroll0,
        )
        .unwrap();
    pipeline
        .prepare_current(
            LayoutSize::new(320.0, 240.0),
            LayoutSize::new(320.0, 240.0),
            false,
            &scroll0,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
    let initial_thumb_y = scrollbar_thumb_y(pipeline.retained_scene().unwrap(), scroll)
        .expect("initial scrollbar thumb");

    let mut scroll1 = ScrollStateMap::default();
    scroll1.set_offset(scroll, 200.0);
    pipeline
        .prepare_current(
            LayoutSize::new(320.0, 240.0),
            LayoutSize::new(320.0, 240.0),
            false,
            &scroll1,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
    let moved_thumb_y = scrollbar_thumb_y(pipeline.retained_scene().unwrap(), scroll)
        .expect("moved scrollbar thumb");

    assert!(
        moved_thumb_y > initial_thumb_y,
        "body scroll must patch the retained scrollbar thumb, before={initial_thumb_y}, after={moved_thumb_y}"
    );

    fn scrollbar_thumb_y(scene: &fission_render::RenderScene, scroll: WidgetId) -> Option<f32> {
        fn find(node: &fission_render::RenderNode, scroll: WidgetId) -> Option<f32> {
            match node {
                fission_render::RenderNode::Paint(list) => list.ops.iter().find_map(|op| {
                    if let fission_render::DisplayOp::DrawRect { rect, node_id, .. } = op {
                        if *node_id == Some(scroll)
                            && (rect.width() - 6.0).abs() <= 0.01
                            && rect.height() < 200.0
                        {
                            return Some(rect.origin.y);
                        }
                    }
                    None
                }),
                fission_render::RenderNode::Layer(layer) => {
                    layer.children.iter().find_map(|child| find(child, scroll))
                }
            }
        }
        scene.roots.iter().find_map(|root| find(root, scroll))
    }
}

#[test]
fn overflowing_scroll_nodes_emit_visible_scroll_rails() {
    let mut ir = CoreIR::new();
    let fill = WidgetId::derived(16, &[0]);
    let content = WidgetId::derived(16, &[1]);
    let scroll = WidgetId::derived(16, &[2]);
    let root = WidgetId::derived(16, &[3]);

    ir.add_node(
        fill,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color {
                r: 80,
                g: 120,
                b: 220,
                a: 255,
            })),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
        }),
        vec![],
    );
    ir.add_node(
        content,
        Op::Layout(LayoutOp::Box {
            width: Some(320.0),
            height: Some(640.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![fill],
    );
    ir.add_node(
        scroll,
        Op::Layout(LayoutOp::Scroll {
            direction: fission_ir::FlexDirection::Column,
            show_scrollbar: true,
            width: Some(320.0),
            height: Some(240.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }),
        vec![content],
    );
    ir.add_node(
        root,
        Op::Layout(LayoutOp::Box {
            width: Some(320.0),
            height: Some(240.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0, 0.0, 0.0, 0.0],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
        vec![scroll],
    );
    ir.set_root(root);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll_map = ScrollStateMap::default();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 320.0, 240.0),
            &mut layout_engine,
            &scroll_map,
        )
        .unwrap();
    pipeline
        .prepare_current(
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            LayoutSize {
                width: 320.0,
                height: 240.0,
            },
            false,
            &scroll_map,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    fn count_scroll_rails(node: &fission_render::RenderNode, scroll: WidgetId) -> usize {
        match node {
            fission_render::RenderNode::Paint(list) => list
                .ops
                .iter()
                .filter(|op| match op {
                    fission_render::DisplayOp::DrawRect { rect, node_id, .. } => {
                        *node_id == Some(scroll)
                            && (rect.width() - 6.0).abs() <= 0.01
                            && rect.height() >= 200.0
                    }
                    _ => false,
                })
                .count(),
            fission_render::RenderNode::Layer(layer) => layer
                .children
                .iter()
                .map(|child| count_scroll_rails(child, scroll))
                .sum(),
        }
    }

    let rail_count: usize = pipeline
        .retained_scene()
        .expect("retained scene")
        .roots
        .iter()
        .map(|node| count_scroll_rails(node, scroll))
        .sum();
    assert!(
        rail_count > 0,
        "expected an overflow rail for the scroll node"
    );
}

#[test]
fn custom_embed_fully_outside_scroll_viewport_is_omitted() {
    // Scroll container 200px tall with a 100px custom embed placed at
    // y=250 inside the content. With scroll offset 0 the embed sits
    // below the visible viewport and should be omitted entirely.
    let scroll_id = WidgetId::derived(20, &[0]);
    let embed_id = WidgetId::derived(20, &[1]);
    let widget_id = WidgetId::explicit("custom.offscreen");
    let payload = vec![0xAA, 0xBB];

    let mut ir = CoreIR::new();
    ir.add_node(
        embed_id,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(payload.clone()),
            widget_id,
            width: Some(100.0),
            height: Some(100.0),
        }),
        vec![],
    );
    ir.add_node(
        scroll_id,
        Op::Layout(LayoutOp::Scroll {
            direction: fission_ir::FlexDirection::Column,
            show_scrollbar: false,
            width: Some(200.0),
            height: Some(200.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![embed_id],
    );
    ir.set_root(scroll_id);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll = ScrollStateMap::default(); // offset 0
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 400.0, 400.0),
            &mut layout_engine,
            &scroll,
        )
        .unwrap();
    pipeline
        .prepare_current(
            LayoutSize::new(400.0, 400.0),
            LayoutSize::new(400.0, 400.0),
            false,
            &scroll,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    // The embed fits inside the scroll viewport (content starts at y=0,
    // embed is 100px tall, scroll viewport is 200px tall) so it should
    // be visible. Now scroll the content so the embed is fully off-screen.
    let mut scrolled = ScrollStateMap::default();
    scrolled.set_offset(scroll_id, 300.0); // scrolled 300px down — embed at y=-300, off-screen

    // Re-run pipeline with the scroll offset.
    pipeline.clear_render_caches();
    pipeline
        .prepare_current(
            LayoutSize::new(400.0, 400.0),
            LayoutSize::new(400.0, 400.0),
            false,
            &scrolled,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    assert!(
        pipeline.native_surfaces.is_empty(),
        "expected custom embed to be omitted when scrolled fully outside viewport, got {:?}",
        pipeline.native_surfaces,
    );
}

#[test]
fn custom_embed_partially_clipped_gets_intersected_visible_rect() {
    // Scroll container 200×200, child embed 100×100 at layout y=0.
    // Scroll offset 50 shifts the child up by 50px, so only the bottom
    // 50px of the embed remains inside the scroll viewport. The
    // visible_rect should reflect the intersection.
    let scroll_id = WidgetId::derived(22, &[0]);
    let embed_id = WidgetId::derived(22, &[1]);
    let widget_id = WidgetId::explicit("custom.partial");
    let payload = vec![0xCC, 0xDD];

    let mut ir = CoreIR::new();
    ir.add_node(
        embed_id,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(payload.clone()),
            widget_id,
            width: Some(100.0),
            height: Some(100.0),
        }),
        vec![],
    );
    ir.add_node(
        scroll_id,
        Op::Layout(LayoutOp::Scroll {
            direction: fission_ir::FlexDirection::Column,
            show_scrollbar: false,
            width: Some(200.0),
            height: Some(200.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![embed_id],
    );
    ir.set_root(scroll_id);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    let scroll = ScrollStateMap::default();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 400.0, 400.0),
            &mut layout_engine,
            &scroll,
        )
        .unwrap();

    // Scroll by 50 — the embed starts at y=0, so after offset it's at
    // y = -50 in the scroll viewport coordinate space. The viewport
    // runs from y=0 to y=200, so only the bottom 50px of the embed
    // (from y=0 to y=50 in viewport coords) is visible.
    let mut scrolled = ScrollStateMap::default();
    scrolled.set_offset(scroll_id, 50.0);

    pipeline.clear_render_caches();
    pipeline
        .prepare_current(
            LayoutSize::new(400.0, 400.0),
            LayoutSize::new(400.0, 400.0),
            false,
            &scrolled,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    assert_eq!(pipeline.native_surfaces.len(), 1);
    let surface = &pipeline.native_surfaces[0];
    // The embed's translated rect is shifted up by the scroll offset.
    let visible = surface.visible_rect;
    // The visible portion is the intersection of the embed rect and the
    // scroll viewport. The exact values depend on how the layout engine
    // positions the child, but the visible height must be less than 100.
    assert!(
        visible.height() < 100.0,
        "visible height ({}) should be less than the full embed height (100)",
        visible.height(),
    );
    assert!(
        visible.height() > 0.0,
        "visible height should be positive (embed is partially visible)",
    );
}

#[test]
fn built_in_native_surfaces_follow_scroll_visibility() {
    let scroll_id = WidgetId::derived(23, &[0]);
    let video_id = WidgetId::derived(23, &[1]);
    let web_id = WidgetId::derived(23, &[2]);
    let video_widget = WidgetId::explicit("video.scrolled");
    let web_widget = WidgetId::explicit("web.scrolled");
    let mut ir = CoreIR::new();
    ir.add_node(
        video_id,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Video,
            widget_id: video_widget,
            width: Some(100.0),
            height: Some(100.0),
        }),
        vec![],
    );
    ir.add_node(
        web_id,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Web,
            widget_id: web_widget,
            width: Some(100.0),
            height: Some(100.0),
        }),
        vec![],
    );
    ir.add_node(
        scroll_id,
        Op::Layout(LayoutOp::Scroll {
            direction: fission_ir::FlexDirection::Column,
            show_scrollbar: false,
            width: Some(200.0),
            height: Some(200.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }),
        vec![video_id, web_id],
    );
    ir.set_root(scroll_id);

    let mut videos = VideoStateMap::default();
    videos.states.insert(video_widget, VideoState::default());
    let mut webs = WebStateMap::default();
    webs.states.insert(
        web_widget,
        WebState {
            url: "https://example.invalid".into(),
            ..Default::default()
        },
    );
    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 400.0, 400.0),
            &mut layout_engine,
            &ScrollStateMap::default(),
        )
        .unwrap();

    let mut scrolled = ScrollStateMap::default();
    scrolled.set_offset(scroll_id, 300.0);
    pipeline
        .prepare_current(
            LayoutSize::new(400.0, 400.0),
            LayoutSize::new(400.0, 400.0),
            false,
            &scrolled,
            &Default::default(),
            &videos,
            &webs,
        )
        .unwrap();

    assert!(pipeline.video_surfaces.is_empty());
    assert!(pipeline.web_surfaces.is_empty());
}

#[test]
fn custom_surface_reports_ancestor_transform_and_opacity() {
    let root_id = WidgetId::derived(24, &[0]);
    let embed_id = WidgetId::derived(24, &[1]);
    let widget_id = WidgetId::explicit("custom.transformed");
    let transform = translation_matrix(40.0, 25.0);
    let mut ir = CoreIR::new();
    ir.add_node(
        embed_id,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(vec![1]),
            widget_id,
            width: Some(100.0),
            height: Some(50.0),
        }),
        vec![],
    );
    ir.add_node(
        root_id,
        Op::Layout(LayoutOp::Transform { transform }),
        vec![embed_id],
    );
    ir.nodes.get_mut(&root_id).unwrap().composite.opacity = Some(CompositeScalar::new(0.5));
    ir.set_root(root_id);

    let mut pipeline = Pipeline::new();
    let mut layout_engine = LayoutEngine::new();
    pipeline.replace_ir(ir, &Env::default());
    pipeline
        .ensure_layout(
            LayoutRect::new(0.0, 0.0, 400.0, 400.0),
            &mut layout_engine,
            &ScrollStateMap::default(),
        )
        .unwrap();
    pipeline
        .prepare_current(
            LayoutSize::new(400.0, 400.0),
            LayoutSize::new(400.0, 400.0),
            false,
            &Default::default(),
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

    let surface = &pipeline.native_surfaces[0];
    assert_eq!(surface.transform, Some(transform));
    assert!((surface.rect.x() - 40.0).abs() < 0.01);
    assert!((surface.rect.y() - 25.0).abs() < 0.01);
    assert!((surface.opacity - 0.5).abs() < 0.001);
}
