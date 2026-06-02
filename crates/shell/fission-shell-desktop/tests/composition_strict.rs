use anyhow::Result;
use fission_core::action::GlobalState as CoreGlobalState;
use fission_core::env::Env;
use fission_core::internal::BuildCtx;
use fission_core::internal::InternalLoweringCx;
use fission_core::ui::{TextInput, Widget};
use fission_core::{
    reduce_with, InputEvent, LayoutPoint, ReducerContext, Runtime, View, WidgetIdExt,
};
use fission_ir::Role;
use fission_layout::{LayoutEngine, LayoutSize, LineMetric, TextMeasurer};
use fission_render::{DisplayList, RenderScene, Renderer};
use fission_widgets::{Checkbox, Portal};
use std::sync::{Arc, Mutex};

use fission_shell_desktop::Pipeline;

#[derive(Debug, Default, Clone)]
struct GlobalState {
    text: String,
    checked: bool,
    show_portal: bool,
}
impl CoreGlobalState for GlobalState {}

#[fission_macros::fission_action]
struct Toggle;
fn on_toggle(state: &mut GlobalState, _a: Toggle, _ctx: &mut ReducerContext<GlobalState>) {
    state.checked = !state.checked;
}

#[fission_macros::fission_action]
struct UpdateText(String);
fn on_update(state: &mut GlobalState, a: UpdateText, _ctx: &mut ReducerContext<GlobalState>) {
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

#[derive(Clone)]
struct Root;
impl From<Root> for Widget {
    fn from(_component: Root) -> Self {
        let (ctx, view) = fission_core::build::current::<GlobalState>();
        use fission_core::ui::Column;
        let mut children: Vec<Widget> = vec![
            Checkbox {
                checked: view.state().checked,
                on_toggle: Some(ctx.bind(Toggle, reduce_with!(on_toggle))),
                label: Some("check".into()),
                ..Default::default()
            }
            .into(),
            TextInput {
                value: view.state().text.clone(),
                placeholder: Some("type".into()),
                on_change: Some(ctx.bind(UpdateText("".into()), reduce_with!(on_update))),
                width: Some(200.0),
                height: Some(40.0),
                ..Default::default()
            }
            .into(),
        ];
        if view.state().show_portal {
            use fission_core::ui::{Overlay, Row as UIRow, ZStack};
            let overlay = Overlay {
                id: None,
                content: UIRow::default().into(),
                overlay: ZStack::default().into(),
            };
            children.push(
                Portal {
                    child: overlay.into(),
                }
                .into(),
            );
        }
        Column {
            children,
            ..Default::default()
        }
        .into()
    }
}
fn pump<W>(
    runtime: &mut Runtime,
    layout: &mut LayoutEngine,
    pipe: &mut Pipeline,
    env: &Env,
    root: &W,
) -> (fission_ir::CoreIR, fission_layout::LayoutSnapshot)
where
    W: Clone + Into<Widget>,
{
    // Build + wrap portals like desktop
    let node_tree = {
        let state = runtime.get_global_state::<GlobalState>().unwrap();
        let view = View::new(
            state,
            &runtime.runtime_state,
            env,
            pipe.last_snapshot.as_ref(),
        );
        let mut ctx = BuildCtx::new();
        let mut tree = fission_core::build::enter(&mut ctx, &view, || (*root).clone().into());
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
                    fission_core::ui::Container::new(node).id(id).into()
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
                children.push(
                    Overlay {
                        id: None,
                        content: Row::default().into(),
                        overlay: p,
                    }
                    .into(),
                );
            }
            tree = ZStack { id: None, children }.into();
        }
        tree
    };
    // InternalLower
    let mut cx = InternalLoweringCx::new(
        env,
        &runtime.runtime_state,
        None,
        pipe.last_snapshot.as_ref(),
    );
    let root_id = fission_core::internal::lower_widget(&node_tree, &mut cx);
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
fn strict_incremental_checkbox_textinput_checkbox_sequence() -> Result<()> {
    // Strict incremental only
    std::env::set_var("FISSION_LAYOUT_STRICT", "1");
    std::env::remove_var("FISSION_ALLOW_FULL_REBUILD");

    let env = Env::default();
    let mut runtime = Runtime::default();
    runtime.add_global_state(Box::new(GlobalState::default()))?;
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
            modifiers: 0,
        }),
        &ir,
        &snap,
    )?;
    let (_ir2, _sn2) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Up {
            point: p,
            button: fission_core::event::PointerButton::Primary,
            modifiers: 0,
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
            modifiers: 0,
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
            modifiers: 0,
        }),
        &ir6,
        &sn6,
    )?;
    let (_ir7, _sn7) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(fission_core::event::PointerEvent::Up {
            point: p,
            button: fission_core::event::PointerButton::Primary,
            modifiers: 0,
        }),
        &ir6,
        &sn6,
    )?;
    let (_ir8, _sn8) = pump(&mut runtime, &mut layout, &mut pipe, &env, &root);

    // If we reached here without panic, strict incremental survived the sequence.
    Ok(())
}
