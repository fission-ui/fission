use anyhow::Result;
use fission_core::action::AppState as CoreAppState;
use fission_core::env::Env;
use fission_core::lowering::LoweringContext;
use fission_core::ui::{Node, TextInput};
use fission_core::{BuildCtx, Runtime, View};
use fission_layout::{LayoutEngine, LayoutSize, LineMetric, TextMeasurer};
use fission_render::{DisplayList, RenderScene, Renderer};
use fission_widgets::{Checkbox, Portal};
use std::sync::{Arc, Mutex};

use fission_shell_desktop::Pipeline;

#[derive(Debug, Default, Clone)]
struct AppState {
    text: String,
    checked: bool,
    show_portal: bool,
}
impl CoreAppState for AppState {}

#[derive(
    fission_macros::Action, serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq,
)]
struct Toggle;
fn on_toggle(state: &mut AppState, _a: Toggle) {
    state.checked = !state.checked;
}

#[derive(
    fission_macros::Action, serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq,
)]
struct UpdateText(String);
fn on_update(state: &mut AppState, a: UpdateText) {
    state.text = a.0;
}

#[derive(Clone, Default)]
struct MockRenderer(pub Arc<Mutex<Option<DisplayList>>>);
impl Renderer for MockRenderer {
    fn render_scene(&mut self, scene: &RenderScene) -> Result<()> {
        *self.0.lock().unwrap() = Some(scene.flatten());
        Ok(())
    }
}

struct MockMeasurer;
impl TextMeasurer for MockMeasurer {
    fn measure(&self, _text: &str, _font_size: f32, _available_width: Option<f32>) -> (f32, f32) {
        (0.0, 0.0)
    }
    fn hit_test(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        _x: f32,
        _y: f32,
    ) -> usize {
        0
    }
    fn get_line_metrics(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        vec![]
    }
}

struct Root;
impl fission_core::view::Widget<AppState> for Root {
    fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> Node {
        use fission_core::ui::{Column, Row};
        let mut children: Vec<Node> = vec![
            Checkbox {
                checked: view.state.checked,
                on_toggle: Some(ctx.bind(Toggle, on_toggle as fn(&mut AppState, Toggle))),
                label: Some("check".into()),
                ..Default::default()
            }
            .into(),
            TextInput {
                value: view.state.text.clone(),
                placeholder: Some("type".into()),
                on_change: Some(ctx.bind(
                    UpdateText("".into()),
                    on_update as fn(&mut AppState, UpdateText),
                )),
                width: Some(200.0),
                height: Some(40.0),
                ..Default::default()
            }
            .into(),
        ];
        if view.state.show_portal {
            use fission_core::ui::{Overlay, ZStack};
            let overlay = Overlay {
                id: None,
                content: Box::new(Node::Row(Row::default())),
                overlay: Box::new(Node::ZStack(ZStack {
                    id: None,
                    children: vec![
                        // Nested portal to test recursion
                        Portal {
                            child: Node::Row(Row::default()),
                        }
                        .build(ctx, view),
                    ],
                    ..Default::default()
                })),
            };
            children.push(
                Portal {
                    child: Node::Overlay(overlay),
                }
                .build(ctx, view),
            );
        }
        // Add more nodes to ensure structure
        children.push(Node::Row(Row::default()));

        Node::Column(Column {
            children,
            ..Default::default()
        })
    }
}

fn pump(
    runtime: &mut Runtime,
    layout: &mut LayoutEngine,
    pipe: &mut Pipeline,
    env: &Env,
    root: &impl fission_core::view::Widget<AppState>,
) -> (fission_ir::CoreIR, fission_layout::LayoutSnapshot) {
    // Build + wrap portals like desktop
    let node_tree = {
        let state = runtime.get_app_state::<AppState>().unwrap();
        let view = View::new(
            state,
            &runtime.runtime_state,
            env,
            pipe.last_snapshot.as_ref(),
        );
        let mut ctx = BuildCtx::new();
        let mut tree = root.build(&mut ctx, &view);
        runtime.clear_reducers();
        let anim = ctx.take_animation_requests();
        for (t, r) in anim {
            runtime.enqueue_animation(t, r);
        }
        let vids = ctx.take_video_registrations();
        runtime.sync_video_nodes(&vids);
        let portals_with_ids = ctx.take_portals();
        let portals = portals_with_ids
            .into_iter()
            .map(|(id, node)| {
                if let Some(id) = id {
                    fission_core::ui::Container::new(node)
                        .id(id.into())
                        .into_node()
                } else {
                    node
                }
            })
            .collect::<Vec<_>>();
        if !portals.is_empty() {
            use fission_core::ui::{Overlay, Row, ZStack};
            let mut children = Vec::with_capacity(1 + portals.len());
            children.push(tree);
            for p in portals {
                children.push(Node::Overlay(Overlay {
                    id: None,
                    content: Box::new(Node::Row(Row::default())),
                    overlay: Box::new(p),
                }));
            }
            tree = Node::ZStack(ZStack {
                id: None,
                children,
                ..Default::default()
            });
        }
        tree
    };
    // Lower
    let mut cx = LoweringContext::new(
        env,
        &runtime.runtime_state,
        None,
        pipe.last_snapshot.as_ref(),
    );
    let root_id = node_tree.lower(&mut cx);
    cx.ir.root = Some(root_id);
    let ir = cx.ir;
    // Render via pipeline (performing layout inside)
    let viewport = LayoutSize {
        width: 800.0,
        height: 600.0,
    };
    let mut renderer = MockRenderer::default();
    let env = Env::default();
    let _ = pipe
        .render(
            ir.clone(),
            viewport,
            layout,
            &runtime.runtime_state.scroll,
            &mut renderer,
            &runtime.runtime_state.video,
            &runtime.runtime_state.web,
            &env,
        )
        .expect("render ok");
    let snap = pipe.last_snapshot.clone().expect("snapshot");
    (ir, snap)
}

#[test]
fn test_composition_cycles_and_portals() -> Result<()> {
    // This test ensures that portal collection and wrapping doesn't cause infinite rebuild cycles
    // or ID collisions if done repeatedly.
    let env = Env::default();
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(AppState::default()))?;
    let mut layout = LayoutEngine::new().with_measurer(Arc::new(MockMeasurer));
    let mut pipe = Pipeline::new();
    let root = Root;

    // Pump frame 1
    let (ir1, _snap1) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // Toggle portal ON
    {
        let state = runtime.get_app_state_mut::<AppState>().unwrap();
        state.show_portal = true;
    }

    let (ir2, _snap2) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // Toggle portal OFF
    {
        let state = runtime.get_app_state_mut::<AppState>().unwrap();
        state.show_portal = false;
    }

    let (ir3, _snap3) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // Verify structural changes
    assert_ne!(ir1.nodes.len(), ir2.nodes.len());
    assert_eq!(ir1.nodes.len(), ir3.nodes.len()); // Should return to base count (roughly)

    Ok(())
}
