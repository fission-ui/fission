use embed_video::{VideoEmbedApp, VideoEmbedState};
use fission::prelude::WidgetNodeId;
use fission_test::prelude::DisplayOp;
use fission_test::TestHarness;

#[test]
fn video_example_renders_a_surface_embed_and_syncs_video_state() {
    let widget_id = WidgetNodeId::explicit("embed-video.demo");
    let mut harness = TestHarness::new(VideoEmbedState).with_root_widget(VideoEmbedApp);

    harness.pump().expect("pump video example");

    let expected_source = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/demo.mp4");
    assert!(
        std::path::Path::new(expected_source).exists(),
        "video fixture should exist"
    );

    let video_state = harness
        .runtime
        .runtime_state
        .video
        .states
        .get(&widget_id)
        .expect("video state should be registered");
    assert_eq!(video_state.asset_source, expected_source);
    assert!(video_state.looped);

    let display_list = harness.get_last_display_list().expect("display list");
    assert!(display_list.ops.iter().any(|op| matches!(
        op,
        DisplayOp::DrawText { text, .. } if text == "Video embed"
    )));
    assert!(display_list.ops.iter().any(|op| matches!(
        op,
        DisplayOp::DrawSurface { rect, .. }
            if rect.size.width == 480.0 && rect.size.height == 270.0
    )));
}
