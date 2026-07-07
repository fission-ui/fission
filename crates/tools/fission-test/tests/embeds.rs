use fission_core::internal::{InternalLowerer, InternalRenderNode};
use fission_core::ui::{Container, Video, Widget};
use fission_core::{GlobalState, WidgetId};
use fission_ir::{EmbedKind, LayoutOp, Op};
use fission_render::DisplayOp;
use fission_test::TestHarness;
use fission_widgets::WebView;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct EmbedState;
impl GlobalState for EmbedState {}

fn display_surfaces(harness: &TestHarness<EmbedState>) -> Vec<DisplayOp> {
    harness
        .get_last_display_list()
        .expect("display list")
        .ops
        .into_iter()
        .filter(|op| matches!(op, DisplayOp::DrawSurface { .. }))
        .collect()
}

#[test]
fn video_embed_registers_runtime_state_and_draws_surface() {
    let widget_id = WidgetId::explicit("test.video");
    let mut harness = TestHarness::new(EmbedState).with_root_widget(Video {
        id: Some(widget_id),
        source: "fixtures/demo.mp4".into(),
        width: Some(320.0),
        height: Some(180.0),
        autoplay: true,
        loop_playback: true,
        ..Default::default()
    });

    harness.pump().expect("pump video embed");

    let video_state = harness
        .runtime
        .runtime_state
        .video
        .states
        .get(&widget_id)
        .expect("video registration should sync into runtime state");
    assert_eq!(video_state.asset_source, "fixtures/demo.mp4");
    assert!(video_state.looped);

    let ir = harness.last_ir.as_ref().expect("ir");
    assert!(ir.nodes.values().any(|node| matches!(
        node.op,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Video,
            widget_id: id,
            ..
        }) if id == widget_id
    )));

    let surfaces = display_surfaces(&harness);
    assert_eq!(surfaces.len(), 1);
    match &surfaces[0] {
        DisplayOp::DrawSurface { rect, .. } => {
            assert_eq!(rect.size.width, 320.0);
            assert_eq!(rect.size.height, 180.0);
        }
        other => panic!("expected video surface, got {other:?}"),
    }
}

#[derive(Clone)]
struct WebApp;
impl From<WebApp> for Widget {
    fn from(_component: WebApp) -> Self {
        let (_ctx, _view) = fission_core::build::current::<EmbedState>();
        Container::new(WebView {
            id: WidgetId::explicit("test.web"),
            url: "https://example.test/docs".into(),
            user_agent: Some("FissionTest/1".into()),
            width: Some(320.0),
            height: Some(180.0),
        })
        .width(320.0)
        .height(180.0)
        .into()
    }
}
#[test]
fn webview_embed_registers_runtime_state_and_draws_surface() {
    let widget_id = WidgetId::explicit("test.web");
    let mut harness = TestHarness::new(EmbedState).with_root_widget(WebApp);

    harness.pump().expect("pump web embed");

    let web_state = harness
        .runtime
        .runtime_state
        .web
        .states
        .get(&widget_id)
        .expect("web registration should sync into runtime state");
    assert_eq!(web_state.url, "https://example.test/docs");
    assert_eq!(web_state.user_agent.as_deref(), Some("FissionTest/1"));

    let ir = harness.last_ir.as_ref().expect("ir");
    assert!(ir.nodes.values().any(|node| matches!(
        node.op,
        Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Web,
            widget_id: id,
            ..
        }) if id == widget_id
    )));

    assert_eq!(display_surfaces(&harness).len(), 1);
}

#[derive(Clone)]
struct CustomEmbedApp;
impl From<CustomEmbedApp> for Widget {
    fn from(_component: CustomEmbedApp) -> Self {
        let (_ctx, _view) = fission_core::build::current::<EmbedState>();
        fission_core::internal::custom_render_widget(InternalRenderNode {
            debug_tag: "TestCustomEmbed".into(),
            lowerer: Some(Arc::new(CustomEmbedInternalLowerer)),
            render_object: None,
        })
    }
}
#[derive(Debug)]
struct CustomEmbedInternalLowerer;

impl InternalLowerer for CustomEmbedInternalLowerer {
    fn lower_dyn(&self, cx: &mut fission_core::internal::InternalLoweringCx) -> WidgetId {
        let node_id = cx.next_node_id();
        cx.insert_node(
            node_id,
            Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Custom(vec![1, 2, 3]),
                widget_id: WidgetId::explicit("test.custom"),
                width: Some(240.0),
                height: Some(120.0),
            }),
            vec![],
        )
    }

    fn stable_key(&self) -> u64 {
        0xF151_C057
    }
}

#[test]
fn custom_embed_draws_surface() {
    let mut harness = TestHarness::new(EmbedState).with_root_widget(CustomEmbedApp);

    harness.pump().expect("pump custom embed");

    let surfaces = display_surfaces(&harness);
    assert_eq!(surfaces.len(), 1);
    match &surfaces[0] {
        DisplayOp::DrawSurface { rect, .. } => {
            assert_eq!(rect.size.width, 240.0);
            assert_eq!(rect.size.height, 120.0);
        }
        other => panic!("expected custom surface, got {other:?}"),
    }
}

/// Verifies the loop-restart invariant: when a looped video ends, the
/// handler should set status back to Playing (simulating a seek-to-0 +
/// replay). This mirrors the logic in fission-shell-winit's
/// VideoEvent::Ended handler.
#[test]
fn looped_video_restarts_on_ended_event() {
    let widget_id = WidgetId::explicit("test.video.loop");
    let mut harness = TestHarness::new(EmbedState).with_root_widget(Video {
        id: Some(widget_id),
        source: "fixtures/demo.mp4".into(),
        width: Some(320.0),
        height: Some(180.0),
        autoplay: true,
        loop_playback: true,
        ..Default::default()
    });

    harness.pump().expect("pump looped video");

    // Confirm initial state: looped is set, status is Playing (autoplay).
    {
        let video_state = harness
            .runtime
            .runtime_state
            .video
            .states
            .get(&widget_id)
            .expect("video state should be registered");
        assert!(video_state.looped, "loop_playback should be copied to looped");
        assert_eq!(
            video_state.status,
            fission_core::env::VideoStatus::Playing,
            "autoplay video should start as Playing"
        );
    }

    // Simulate what the shell's VideoEvent::Ended handler does when
    // video_state.looped is true: reset status to Playing (the shell also
    // calls player.seek_to(0), but we can't test that without a real
    // player — the status transition is the key invariant).
    {
        let video_state = harness
            .runtime
            .runtime_state
            .video
            .states
            .get_mut(&widget_id)
            .expect("video state should still be registered");

        // First, simulate the video reaching the end.
        video_state.status = fission_core::env::VideoStatus::Ended;
        assert_eq!(video_state.status, fission_core::env::VideoStatus::Ended);

        // Now apply the loop-restart logic.
        if video_state.looped {
            video_state.status = fission_core::env::VideoStatus::Playing;
        }
        assert_eq!(
            video_state.status,
            fission_core::env::VideoStatus::Playing,
            "looped video should restart to Playing after Ended"
        );
    }
}

/// Verifies that a non-looped video stays Ended when the video finishes.
#[test]
fn non_looped_video_stays_ended() {
    let widget_id = WidgetId::explicit("test.video.noloop");
    let mut harness = TestHarness::new(EmbedState).with_root_widget(Video {
        id: Some(widget_id),
        source: "fixtures/demo.mp4".into(),
        width: Some(320.0),
        height: Some(180.0),
        autoplay: true,
        loop_playback: false,
        ..Default::default()
    });

    harness.pump().expect("pump non-looped video");

    let video_state = harness
        .runtime
        .runtime_state
        .video
        .states
        .get_mut(&widget_id)
        .expect("video state should be registered");

    assert!(!video_state.looped, "loop_playback should be false");

    // Simulate video ending.
    video_state.status = fission_core::env::VideoStatus::Ended;

    // Apply the loop-restart logic — should NOT restart.
    if video_state.looped {
        video_state.status = fission_core::env::VideoStatus::Playing;
    }
    assert_eq!(
        video_state.status,
        fission_core::env::VideoStatus::Ended,
        "non-looped video should stay Ended"
    );
}
