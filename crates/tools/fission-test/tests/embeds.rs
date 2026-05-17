use fission_core::ui::{Container, CustomNode, LowerDyn, Node, Video};
use fission_core::{AppState, BuildCtx, View, Widget, WidgetNodeId};
use fission_ir::{EmbedKind, LayoutOp, NodeId, Op};
use fission_render::DisplayOp;
use fission_test::TestHarness;
use fission_widgets::WebView;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct EmbedState;
impl AppState for EmbedState {}

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
    let widget_id = WidgetNodeId::explicit("test.video");
    let mut harness = TestHarness::new(EmbedState).with_root_widget(Video {
        id: Some(widget_id),
        source: "fixtures/demo.mp4".into(),
        width: Some(320.0),
        height: Some(180.0),
        autoplay: true,
        loop_playback: true,
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

struct WebApp;
impl Widget<EmbedState> for WebApp {
    fn build(&self, ctx: &mut BuildCtx<EmbedState>, view: &View<EmbedState>) -> Node {
        Container::new(
            WebView {
                id: WidgetNodeId::explicit("test.web"),
                url: "https://example.test/docs".into(),
                user_agent: Some("FissionTest/1".into()),
                width: Some(320.0),
                height: Some(180.0),
            }
            .build(ctx, view),
        )
        .width(320.0)
        .height(180.0)
        .into_node()
    }
}

#[test]
fn webview_embed_registers_runtime_state_and_draws_surface() {
    let widget_id = WidgetNodeId::explicit("test.web");
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

struct CustomEmbedApp;
impl Widget<EmbedState> for CustomEmbedApp {
    fn build(&self, _ctx: &mut BuildCtx<EmbedState>, _view: &View<EmbedState>) -> Node {
        Node::Custom(CustomNode {
            debug_tag: "TestCustomEmbed".into(),
            lowerer: Some(Arc::new(CustomEmbedLowerer)),
            render_object: None,
        })
    }
}

#[derive(Debug)]
struct CustomEmbedLowerer;

impl LowerDyn for CustomEmbedLowerer {
    fn lower_dyn(&self, cx: &mut fission_core::lowering::LoweringContext) -> NodeId {
        let node_id = cx.next_node_id();
        cx.insert_node(
            node_id,
            Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Custom(vec![1, 2, 3]),
                widget_id: WidgetNodeId::explicit("test.custom"),
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
