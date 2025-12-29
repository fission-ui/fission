use anyhow::Result;
use fission_core::action::AppState as CoreAppState;
use fission_core::env::Env;
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::ui::{Node, TextInput};
use fission_core::{BuildCtx, InputEvent, LayoutPoint, Runtime, View};
use fission_ir::Role;
use fission_layout::{LayoutEngine, LayoutSize, TextMeasurer, LineMetric};
use fission_render::{DisplayList, Renderer};
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
    fn render(&mut self, dl: &DisplayList) -> Result<()> {
        *self.0.lock().unwrap() = Some(dl.clone());
        Ok(())
    }
}

struct MockMeasurer;
impl TextMeasurer for MockMeasurer {
    fn measure(&self, _text: &str, _font_size: f32, _available_width: Option<f32>) -> (f32, f32) { (0.0, 0.0) }
    fn hit_test(&self, _text: &str, _font_size: f32, _available_width: Option<f32>, _x: f32, _y: f32) -> usize { 0 }
    fn get_line_metrics(&self, _text: &str, _font_size: f32, _available_width: Option<f32>) -> Vec<LineMetric> { vec![] }
}

struct Root;
impl fission_core::view::Widget<AppState> for Root {
    fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> Node {
        use fission_core::ui::{Column, Row, Text, TextContent};
        let mut children: Vec<Node> = vec![
            Checkbox {
                checked: view.state.checked,
                on_toggle: Some(ctx.bind(Toggle, on_toggle)),
                label: Some("check".into()),
                ..Default::default()
            }.into(),
            TextInput {
                value: view.state.text.clone(),
                placeholder: Some("type".into()),
                on_change: Some(ctx.bind(UpdateText("".into()), on_update)),
                width: Some(200.0),
                height: Some(40.0),
                ..Default::default()
            }
            .into(),
        ];
        if view.state.show_portal {
            use fission_core::ui::{Overlay, Row as UIRow, ZStack};
            let overlay = Overlay {
                id: None,
                content: Box::new(Node::Row(UIRow::default())),
                overlay: Box::new(Node::ZStack(ZStack::default())),
            };
            children.push(
                Portal {
                    child: Node::Overlay(overlay),
                }
                .build(ctx, view),
            );
        }
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
        let view = View::new(state, &runtime.runtime_state, env, pipe.last_snapshot.as_ref());
        let mut ctx = BuildCtx::new();
        let mut tree = root.build(&mut ctx, &view);
        runtime.clear_reducers();
        let anim = ctx.take_animation_requests();
        for (t, r) in anim {
            runtime.enqueue_animation(t, r);
        }
        let vids = ctx.take_video_registrations();
        runtime.sync_video_nodes(&vids);
        let portals = ctx.take_portals();
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
            tree = Node::ZStack(ZStack { id: None, children });
        }
        tree
    };
    // Lower
    let mut cx = LoweringContext::new(env, &runtime.runtime_state, None, pipe.last_snapshot.as_ref());
    let root_id = node_tree.lower(&mut cx);
    cx.ir.root = Some(root_id);
    let ir = cx.ir;
    // Render via pipeline (performing layout inside)
    let viewport = LayoutSize {
        width: 800.0,
        height: 600.0,
    };
    let mut renderer = MockRenderer::default();
    let _ = pipe
        .render(
            ir.clone(),
            viewport,
            layout,
            &runtime.runtime_state.scroll,
            &mut renderer,
            &runtime.runtime_state.video,
        )
        .expect("render ok");
    let snap = pipe.last_snapshot.clone().expect("snapshot");
    (ir, snap)
}

#[test]
fn strict_incremental_checkbox_textinput_checkbox_sequence() -> Result<()> {
    // Strict incremental only
    std::env::set_var("FISSION_LAYOUT_STRICT", "1");
    std::env::remove_var("FISSION_ALLOW_FULL_REBUILD");

    let env = Env::default();
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(AppState::default()))?;
    let mut layout = LayoutEngine::new().with_measurer(Arc::new(MockMeasurer));
    let mut pipe = Pipeline::new();
    let root = Root;

    // Initial frame
    let (ir, snap) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // Toggle checkbox (down+up)
    let mut cb = None;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == Role::Checkbox {
                cb = Some(*id);
                break;
            }
        }
    }
    let cb = cb.expect("checkbox semantics");
    let r = snap.get_node_rect(cb).unwrap();
    let p = LayoutPoint::new(r.x() + r.width() / 2.0, r.y() + r.height() / 2.0);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Down {
            point: p,
            button: fission_core::event::PointerButton::Primary,
        }),
        &ir,
        &snap,
    )?;
    let (_ir2, _sn2) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Up {
            point: p,
            button: fission_core::event::PointerButton::Primary,
        }),
        &ir,
        &snap,
    )?;
    let (_ir3, _sn3) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // Focus text input
    let (ir4, sn4) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    let mut ti = None;
    for (id, node) in &ir4.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == Role::TextInput {
                ti = Some(*id);
                break;
            }
        }
    }
    let ti = ti.expect("text input semantics");
    let tr = sn4.get_node_rect(ti).unwrap();
    let tp = LayoutPoint::new(tr.x() + tr.width() / 2.0, tr.y() + tr.height() / 2.0);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Down {
            point: tp,
            button: fission_core::event::PointerButton::Primary,
        }),
        &ir4,
        &sn4,
    )?;
    let (_ir5, _sn5) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // Toggle checkbox again
    let (ir6, sn6) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Down {
            point: p,
            button: fission_core::event::PointerButton::Primary,
        }),
        &ir6,
        &sn6,
    )?;
    let (_ir7, _sn7) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Up {
            point: p,
            button: fission_core::event::PointerButton::Primary,
        }),
        &ir6,
        &sn6,
    )?;
    let (_ir8, _sn8) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // If we reached here without panic, strict incremental survived the sequence.
    Ok(())
}