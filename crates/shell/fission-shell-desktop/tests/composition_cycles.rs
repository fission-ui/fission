use anyhow::Result;
use fission_core::action::AppState as CoreAppState;
use fission_core::env::Env;
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::ui::{Node, TextInput};
use fission_core::{
    action::Action,
    event::{PointerButton, PointerEvent},
    view::Widget,
    BuildCtx, InputEvent, LayoutPoint, Runtime, View,
};
use fission_ir::Role;
use fission_layout::{LayoutEngine, LayoutSize, TextMeasurer, LineMetric};
use fission_render::{DisplayList, Renderer};
use fission_widgets::{checkbox, CheckboxProps, Portal};
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
    fn render(&mut self, display_list: &DisplayList) -> Result<()> {
        *self.0.lock().unwrap() = Some(display_list.clone());
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

impl Widget<AppState> for Root {
    fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> Node {
        use fission_core::ui::{Column, Row, Text, TextContent};

        let mut children: Vec<Node> = vec![
            // Checkbox
            checkbox(CheckboxProps {
                checked: view.state.checked,
                on_toggle: Some(ctx.bind(Toggle, on_toggle)),
                label: Some("check".into()),
            }),
            // Text input
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
            // Register a simple overlay portal to mimic desktop composition wrapping.
            use fission_core::ui::{Overlay, Stack};
            let overlay = Overlay {
                id: None,
                content: Box::new(Node::Row(Row::default())),
                overlay: Box::new(Node::Stack(Stack {
                    id: None,
                    children: vec![Text {
                        content: TextContent::Literal("overlay".into()),
                        ..Default::default()
                    }
                    .into()],
                })),
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

// A heavier tree resembling the counter example: scroll + video + many items
struct CounterLike;

impl Widget<AppState> for CounterLike {
    fn build(&self, ctx: &mut BuildCtx<AppState>, view: &View<AppState>) -> Node {
        use fission_core::ui::{Column, Row, Scroll, Stack, Text, TextContent, Video};
        use fission_core::FlexDirection;

        let mut col_children: Vec<Node> = Vec::new();
        // Video
        col_children.push(
            Video {
                id: None,
                source: "docs/video1.mp4".into(),
                width: Some(240.0),
                height: Some(160.0),
                autoplay: false,
                loop_playback: false,
                ..Default::default()
            }
            .build(ctx, view),
        );

        // Label
        col_children.push(
            Text {
                content: TextContent::Literal("CounterLike".into()),
                ..Default::default()
            }
            .into(),
        );

        // Row with checkbox + text input
        col_children.push(Node::Row(Row {
            children: vec![
                checkbox(CheckboxProps {
                    checked: view.state.checked,
                    on_toggle: Some(ctx.bind(Toggle, on_toggle)),
                    label: Some("Enable feature".into()),
                }),
                TextInput {
                    value: view.state.text.clone(),
                    placeholder: Some("type".into()),
                    on_change: Some(ctx.bind(UpdateText("".into()), on_update)),
                    width: Some(200.0),
                    height: Some(40.0),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }));

        // Filler items
        for i in 0..30u8 {
            col_children.push(
                Text {
                    content: TextContent::Literal(format!("Item {}", i)),
                    ..Default::default()
                }
                .into(),
            );
        }

        let content = Node::Column(Column {
            children: col_children,
            ..Default::default()
        });
        Node::Scroll(Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            child: Some(Box::new(content)),
            ..Default::default()
        })
    }
}

fn pump_once(
    runtime: &mut Runtime,
    layout: &mut LayoutEngine,
    pipeline: &mut Pipeline,
    env: &Env,
    root: &impl Widget<AppState>,
) -> (fission_ir::CoreIR, fission_layout::LayoutSnapshot) {
    // Build
    eprintln!("[test] build begin");
    let node_tree = {
        let state = runtime.get_app_state::<AppState>().unwrap();
        let view = View::new(state, &runtime.runtime_state, env);
        let mut ctx = BuildCtx::new();
        let mut tree = root.build(&mut ctx, &view);
        eprintln!("[test] build done");
        runtime.clear_reducers();
        let animation_requests = ctx.take_animation_requests();
        let video_nodes = ctx.take_video_registrations();
        let portals = ctx.take_portals();
        runtime.absorb_registry(ctx.registry);
        for (target, req) in animation_requests {
            runtime.enqueue_animation(target, req);
        }
        runtime.sync_video_nodes(&video_nodes);
        if !portals.is_empty() {
            use fission_core::ui::{Overlay, Row, Stack};
            let mut children = Vec::with_capacity(1 + portals.len());
            children.push(tree);
            for p in portals {
                children.push(Node::Overlay(Overlay {
                    id: None,
                    content: Box::new(Node::Row(Row::default())),
                    overlay: Box::new(p),
                }));
            }
            tree = Node::Stack(Stack { id: None, children });
        }
        tree
    };

    // Lower
    eprintln!("[test] lower begin");
    let mut lower_cx = LoweringContext::new(env, &runtime.runtime_state, None);
    let root_id = node_tree.lower(&mut lower_cx);
    lower_cx.ir.root = Some(root_id);
    let ir = lower_cx.ir;
    eprintln!("[test] lower done (nodes={})", ir.nodes.len());
    // Debug: dump the TextInput wrapper (200x40) if present
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Layout(fission_ir::LayoutOp::Box {
            width,
            height,
            min_width,
            max_width,
            min_height,
            max_height,
            padding,
        }) = &node.op
        {
            if width == &Some(200.0) && height == &Some(40.0) && padding == &[8.0, 8.0, 4.0, 4.0] {
                eprintln!(
                    "[test] candidate TextInput wrapper id={:?} children={}",
                    id,
                    node.children.len()
                );
                for (idx, cid) in node.children.iter().enumerate() {
                    if let Some(cn) = ir.nodes.get(cid) {
                        eprintln!("  child[{}]={:?} op={:?}", idx, cid, cn.op);
                    } else {
                        eprintln!("  child[{}]={:?} missing in IR?!", idx, cid);
                    }
                }
            }
        }
    }

    // Render via pipeline (which performs layout.update internally)
    let viewport = LayoutSize {
        width: 800.0,
        height: 600.0,
    };
    let mut renderer = MockRenderer::default();
    eprintln!("[test] pipeline render begin");
    let _stats = pipeline
        .render(
            ir.clone(),
            viewport,
            layout,
            &runtime.runtime_state.scroll,
            &mut renderer,
            &runtime.runtime_state.video,
        )
        .expect("pipeline render ok");
    let snapshot = pipeline.last_snapshot.clone().expect("snapshot present");
    eprintln!("[test] pipeline render done");
    (ir, snapshot)
}

#[test]
fn desktop_like_composition_checkbox_toggle_has_no_cycles() -> Result<()> {
    let env = Env::default();
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(AppState::default()))?;
    let mut layout = LayoutEngine::new().with_measurer(Arc::new(MockMeasurer));
    let mut pipeline = Pipeline::new();
    let root = Root;

    // Initial frame
    let (ir, snapshot) = pump_once(&mut runtime, &mut layout, &mut pipeline, &env, &root);

    // Locate checkbox semantics and click (down + up)
    let mut cb_node = None;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == Role::Checkbox {
                cb_node = Some(*id);
                break;
            }
        }
    }
    let id = cb_node.expect("checkbox semantics not found");
    let rect = snapshot.get_node_rect(id).unwrap();
    let center = LayoutPoint::new(
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    );

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            point: center,
            button: PointerButton::Primary,
        }),
        &ir,
        &snapshot,
    )?;
    let (_ir2, _snap2) = pump_once(&mut runtime, &mut layout, &mut pipeline, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            point: center,
            button: PointerButton::Primary,
        }),
        &ir,
        &snapshot,
    )?;
    let (ir3, _snap3) = pump_once(&mut runtime, &mut layout, &mut pipeline, &env, &root);

    // Assert no IR cycles
    if let Some(cycle) = detect_ir_cycle(&ir3) {
        panic!("IR cycle after checkbox toggle: {:?}", cycle);
    }
    Ok(())
}

#[test]
fn desktop_like_composition_text_input_focus_and_type() -> Result<()> {
    let env = Env::default();
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(AppState::default()))?;
    let mut layout = LayoutEngine::new().with_measurer(Arc::new(MockMeasurer));
    let mut pipeline = Pipeline::new();
    let root = Root;

    // Initial frame
    let (ir, snapshot) = pump_once(&mut runtime, &mut layout, &mut pipeline, &env, &root);

    // Find TextInput semantics
    let mut text_node = None;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == Role::TextInput {
                text_node = Some(*id);
                break;
            }
        }
    }
    let id = text_node.expect("text input semantics not found");
    let rect = snapshot.get_node_rect(id).unwrap();
    let center = LayoutPoint::new(
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    );

    // Focus
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            point: center,
            button: PointerButton::Primary,
        }),
        &ir,
        &snapshot,
    )?;
    let (_ir2, _snap2) = pump_once(&mut runtime, &mut layout, &mut pipeline, &env, &root);

    // Type 'a'
    runtime.handle_input(
        InputEvent::Keyboard(fission_core::KeyEvent::Down {
            key_code: fission_core::KeyCode::Char('a'),
            modifiers: 0,
        }),
        pipeline.prev_ir.as_ref().unwrap(),
        pipeline.last_snapshot.as_ref().unwrap(),
    )?;
    let (_ir3, _snap3) = pump_once(&mut runtime, &mut layout, &mut pipeline, &env, &root);

    // Ensure no cycles after typing
    if let Some(cycle) = detect_ir_cycle(pipeline.prev_ir.as_ref().unwrap()) {
        panic!("IR cycle after typing: {:?}", cycle);
    }
    Ok(())
}

#[test]
fn desktop_like_counterish_tree_checkbox_toggle_has_no_cycles() -> Result<()> {
    let env = Env::default();
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(AppState::default()))?;
    let mut layout = LayoutEngine::new().with_measurer(Arc::new(MockMeasurer));
    let mut pipeline = Pipeline::new();
    let root = CounterLike;

    let (ir, snapshot) = pump_once(&mut runtime, &mut layout, &mut pipeline, &env, &root);

    // Locate checkbox semantics and click it (down + up)
    let mut cb_node = None;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == Role::Checkbox {
                cb_node = Some(*id);
                break;
            }
        }
    }
    let id = cb_node.expect("Checkbox semantics not found");
    let rect = snapshot.get_node_rect(id).unwrap();
    let center = LayoutPoint::new(
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    );

    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            point: center,
            button: PointerButton::Primary,
        }),
        &ir,
        &snapshot,
    )?;
    let (_ir2, _snap2) = pump_once(&mut runtime, &mut layout, &mut pipeline, &env, &root);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            point: center,
            button: PointerButton::Primary,
        }),
        &ir,
        &snapshot,
    )?;
    let (ir3, _snap3) = pump_once(&mut runtime, &mut layout, &mut pipeline, &env, &root);

    if let Some(cycle) = detect_ir_cycle(&ir3) {
        panic!("IR cycle after checkbox toggle: {:?}", cycle);
    }
    Ok(())
}

// Local IR cycle detector mirroring desktop pipeline version
fn detect_ir_cycle(ir: &fission_ir::CoreIR) -> Option<Vec<fission_ir::NodeId>> {
    use std::collections::HashSet;
    fn dfs(
        ir: &fission_ir::CoreIR,
        node: fission_ir::NodeId,
        visited: &mut HashSet<fission_ir::NodeId>,
        stack: &mut HashSet<fission_ir::NodeId>,
        path: &mut Vec<fission_ir::NodeId>,
    ) -> Option<Vec<fission_ir::NodeId>> {
        if !visited.insert(node) {
            return None;
        }
        stack.insert(node);
        path.push(node);
        if let Some(n) = ir.nodes.get(&node) {
            for &child in &n.children {
                if stack.contains(&child) {
                    if let Some(pos) = path.iter().position(|&id| id == child) {
                        return Some(path[pos..].to_vec());
                    } else {
                        return Some(vec![child]);
                    }
                }
                if let Some(c) = dfs(ir, child, visited, stack, path) {
                    return Some(c);
                }
            }
        }
        stack.remove(&node);
        path.pop();
        None
    }
    if let Some(root) = ir.root {
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let mut path = Vec::new();
        return dfs(ir, root, &mut visited, &mut stack, &mut path);
    }
    None
}
