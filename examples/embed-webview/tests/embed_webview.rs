use embed_webview::{WebViewEmbedApp, WebViewEmbedState, WEBVIEW_DEMO_URL};
use fission::prelude::WidgetNodeId;
use fission_test::prelude::DisplayOp;
use fission_test::TestHarness;

#[test]
fn webview_example_renders_a_surface_embed_and_syncs_web_state() {
    let widget_id = WidgetNodeId::explicit("embed-webview.demo");
    let mut harness = TestHarness::new(WebViewEmbedState).with_root_widget(WebViewEmbedApp);

    harness.pump().expect("pump webview example");

    let web_state = harness
        .runtime
        .runtime_state
        .web
        .states
        .get(&widget_id)
        .expect("web state should be registered");
    assert_eq!(web_state.url, WEBVIEW_DEMO_URL);

    let display_list = harness.get_last_display_list().expect("display list");
    assert!(display_list.ops.iter().any(|op| matches!(
        op,
        DisplayOp::DrawText { text, .. } if text == "WebView embed"
    )));
    assert!(display_list.ops.iter().any(|op| matches!(
        op,
        DisplayOp::DrawSurface { rect, .. }
            if rect.size.width == 480.0 && rect.size.height == 270.0
    )));
}
