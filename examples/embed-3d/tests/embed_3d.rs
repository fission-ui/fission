use embed_3d::{Scene3DEmbedApp, Scene3DEmbedState};
use fission_test::prelude::DisplayOp;
use fission_test::TestHarness;

#[test]
fn scene_3d_example_renders_a_custom_surface_embed() {
    let mut harness = TestHarness::new(Scene3DEmbedState).with_root_widget(Scene3DEmbedApp);

    harness.pump().expect("pump 3d example");

    let display_list = harness.get_last_display_list().expect("display list");
    assert!(display_list.ops.iter().any(|op| matches!(
        op,
        DisplayOp::DrawText { text, .. } if text == "3D embed"
    )));
    assert!(display_list.ops.iter().any(|op| matches!(
        op,
        DisplayOp::DrawSurface { rect, .. }
            if rect.size.width == 480.0 && rect.size.height == 270.0
    )));
}
