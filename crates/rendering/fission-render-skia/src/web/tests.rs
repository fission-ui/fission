use fission_ir::op::{ImageAlignment, ImageRequest, ImageSource};
use fission_render::frame::ResourceEpoch;
use fission_render::resource::{
    resolved_resource_content_identity, ResourceEntry, ResourceId, ResourceKind, ResourcePayload,
    ResourceProvenance, ResourceSnapshot, ResourceSource,
};
use fission_render::{
    Color, DisplayList, DisplayOp, Fill, ImageFit, LayoutPoint, LayoutRect, RenderScene,
};
use fission_skia_sys::web::{decode_commands, WebCommand, WebImageAlignment, WebImageFit};

use super::resources::ResourceMap;
use super::{compile_web_scene, compile_web_scene_with_resources};

fn clear_color() -> Color {
    Color {
        r: 1,
        g: 2,
        b: 3,
        a: 255,
    }
}

#[test]
fn retained_shape_scene_uses_canonical_canvaskit_stream() {
    let bounds = LayoutRect::new(2.0, 3.0, 20.0, 10.0);
    let mut list = DisplayList::new(bounds);
    list.push(DisplayOp::Save);
    list.push(DisplayOp::ClipRect(bounds));
    list.push(DisplayOp::Translate(LayoutPoint::new(4.0, 5.0)));
    list.push(DisplayOp::DrawRect {
        rect: bounds,
        fill: Some(Fill::Solid(Color {
            r: 255,
            g: 128,
            b: 0,
            a: 255,
        })),
        stroke: None,
        corner_radius: 3.0,
        shadow: None,
        bounds,
        node_id: None,
    });
    list.push(DisplayOp::Restore);

    let compiled =
        compile_web_scene(&RenderScene::from_display_list(list), 2.0, clear_color()).unwrap();

    assert_eq!(compiled.source_operations, 5);
    assert_eq!(
        decode_commands(&compiled.encoded_commands).unwrap(),
        compiled.commands
    );
    assert!(matches!(compiled.commands[0], WebCommand::Clear(_)));
    assert!(matches!(compiled.commands[1], WebCommand::Save));
    assert!(matches!(compiled.commands[2], WebCommand::ClipRect(_)));
    assert!(matches!(compiled.commands[3], WebCommand::ConcatAffine(_)));
    assert!(matches!(
        compiled.commands[4],
        WebCommand::FillRect { radius: 6.0, .. }
    ));
    assert!(matches!(compiled.commands[5], WebCommand::Restore));
}

#[test]
fn image_uses_the_planned_generational_resource_handle_without_native_decode() {
    let node_id = fission_ir::WidgetId::explicit("web.image");
    let bytes = vec![1, 2, 3, 4];
    let source = ImageSource::Memory {
        bytes: bytes.clone(),
        mime_type: Some("image/test".into()),
    };
    let request = ImageRequest {
        source: source.clone(),
        ..ImageRequest::default()
    };
    let bounds = LayoutRect::new(5.0, 7.0, 40.0, 30.0);
    let mut list = DisplayList::new(bounds);
    list.push(DisplayOp::DrawImage {
        rect: bounds,
        request,
        fit: ImageFit::Cover,
        alignment: ImageAlignment::BottomEnd,
        bounds,
        node_id: Some(node_id),
    });
    let resource_id = ResourceId(17);
    let resources = ResourceSnapshot::try_new(
        ResourceEpoch(1),
        [ResourceEntry::ready(
            resource_id,
            resolved_resource_content_identity(&ResourceKind::Image, &source, &bytes),
            ResourceKind::Image,
            ResourceProvenance {
                source: ResourceSource::Memory,
                locator: Some("memory:image/test".into()),
                requested_by: Some(node_id),
            },
            ResourcePayload::Bytes(bytes),
        )],
    )
    .unwrap();
    let resource_map = ResourceMap::default();
    let plan = resource_map.plan(&resources).unwrap().unwrap();

    let compiled = compile_web_scene_with_resources(
        &RenderScene::from_display_list(list),
        2.0,
        clear_color(),
        &resources,
        None,
        None,
        &|id| plan.handle(id),
    )
    .unwrap();

    assert_eq!(compiled.commands.len(), 2);
    assert_eq!(
        compiled.commands[1],
        WebCommand::DrawImageFit {
            image: plan.handle(resource_id).unwrap(),
            target: fission_skia_sys::Rect::new(10.0, 14.0, 80.0, 60.0),
            fit: WebImageFit::Cover,
            alignment: WebImageAlignment::BottomEnd,
            sampling: fission_skia_sys::ImageSampling::Linear,
        }
    );
}

#[test]
fn svg_uses_authoritative_text_and_lowers_to_neutral_paint_commands() {
    let node_id = fission_ir::WidgetId::explicit("web.svg");
    let content = "<svg viewBox='0 0 4 2'><path d='M0 0L4 0L4 2Z'/></svg>";
    let source = ImageSource::SvgText {
        content: content.into(),
    };
    let bounds = LayoutRect::new(10.0, 20.0, 40.0, 20.0);
    let mut list = DisplayList::new(bounds);
    list.push(DisplayOp::DrawSvg {
        content: content.into(),
        fill: Some(Fill::Solid(Color {
            r: 9,
            g: 8,
            b: 7,
            a: 255,
        })),
        stroke: None,
        bounds,
        node_id: Some(node_id),
    });
    let resources = ResourceSnapshot::try_new(
        ResourceEpoch(1),
        [ResourceEntry::ready(
            ResourceId(3),
            resolved_resource_content_identity(&ResourceKind::Svg, &source, content.as_bytes()),
            ResourceKind::Svg,
            ResourceProvenance {
                source: ResourceSource::Embedded,
                locator: Some("inline-svg".into()),
                requested_by: Some(node_id),
            },
            ResourcePayload::Text(content.into()),
        )],
    )
    .unwrap();

    let compiled = compile_web_scene_with_resources(
        &RenderScene::from_display_list(list),
        1.0,
        clear_color(),
        &resources,
        None,
        None,
        &|_| None,
    )
    .unwrap();

    assert!(compiled
        .commands
        .iter()
        .any(|command| matches!(command, WebCommand::FillPath { .. })));
    assert!(!compiled.commands.iter().any(|command| matches!(
        command,
        WebCommand::DrawSvg { .. } | WebCommand::DrawPicture { .. }
    )));
}

#[test]
fn cached_scene_expands_instead_of_emitting_a_native_picture() {
    let bounds = LayoutRect::new(0.0, 0.0, 20.0, 10.0);
    let mut cached = DisplayList::new(bounds);
    cached.push(DisplayOp::DrawRect {
        rect: bounds,
        fill: Some(Fill::Solid(clear_color())),
        stroke: None,
        corner_radius: 0.0,
        shadow: None,
        bounds,
        node_id: None,
    });
    let mut list = DisplayList::new(bounds);
    list.push(DisplayOp::CachedScene {
        cache_key: 42,
        bounds,
        list: Box::new(cached),
    });
    let resources = ResourceSnapshot::try_new(ResourceEpoch(1), []).unwrap();

    let compiled = compile_web_scene_with_resources(
        &RenderScene::from_display_list(list),
        1.0,
        clear_color(),
        &resources,
        None,
        None,
        &|_| None,
    )
    .unwrap();

    assert_eq!(compiled.reused_layers, 0);
    assert!(matches!(compiled.commands[1], WebCommand::FillRect { .. }));
    assert!(!compiled
        .commands
        .iter()
        .any(|command| matches!(command, WebCommand::DrawPicture { .. })));
}
