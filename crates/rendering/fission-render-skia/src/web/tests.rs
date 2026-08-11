use fission_render::{Color, DisplayList, DisplayOp, Fill, LayoutPoint, LayoutRect, RenderScene};
use fission_skia_sys::web::{decode_commands, WebCommand};

use super::compile_web_scene;

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

    let compiled = compile_web_scene(
        &RenderScene::from_display_list(list),
        2.0,
        Color {
            r: 1,
            g: 2,
            b: 3,
            a: 255,
        },
    )
    .unwrap();

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
