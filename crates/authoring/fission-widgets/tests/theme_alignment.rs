use fission_core::action::AppState;
use fission_core::env::{Env, RuntimeState};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::{BuildCtx, View, Widget};
use fission_ir::op::{Color, Fill};
use fission_ir::{CoreIR, LayoutOp, NodeId, Op, PaintOp};
use fission_layout::{LayoutEngine, LayoutSize, TextMeasurer};
use fission_theme::{ComponentTheme, Theme, Tokens};
use fission_widgets::{Badge, Stepper};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default, Debug)]
struct TestState;

impl AppState for TestState {}

struct SimpleMeasurer;

impl TextMeasurer for SimpleMeasurer {
    fn measure(&self, text: &str, _font_size: f32, available_width: Option<f32>) -> (f32, f32) {
        let char_width = 8.0;
        let line_height = 16.0;
        let width = text.len() as f32 * char_width;
        if let Some(max_w) = available_width {
            if max_w > 0.0 && width > max_w {
                let lines = (width / max_w).ceil();
                return (max_w, lines * line_height);
            }
        }
        (width, line_height)
    }

    fn measure_rich_text(&self, runs: &[fission_ir::op::TextRun], available_width: Option<f32>) -> (f32, f32) {
        let text: String = runs.iter().map(|r| r.text.clone()).collect();
        self.measure(&text, 16.0, available_width)
    }
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn parent_map(ir: &CoreIR) -> HashMap<NodeId, NodeId> {
    let mut map = HashMap::new();
    for (id, node) in &ir.nodes {
        for child in &node.children {
            map.insert(*child, *id);
        }
    }
    map
}

fn build_widget_ir(widget: impl Widget<TestState>, env: &Env) -> (CoreIR, NodeId) {
    let runtime_state = RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime_state, env, None);
    let mut ctx = BuildCtx::new();
    let node = widget.build(&mut ctx, &view);

    let measurer: Arc<dyn TextMeasurer> = Arc::new(SimpleMeasurer);
    let measurer_ref = measurer.clone();
    let mut lower = LoweringContext::new(env, &runtime_state, Some(&measurer_ref), None);
    let root_id = node.lower(&mut lower);
    lower.ir.root = Some(root_id);
    (lower.ir, root_id)
}

fn layout_widget(widget: impl Widget<TestState>, env: &Env) -> (CoreIR, NodeId, fission_layout::LayoutSnapshot) {
    let (ir, root_id) = build_widget_ir(widget, env);
    let input_nodes = build_layout_tree(&ir, &env);
    let mut engine = LayoutEngine::new().with_measurer(Arc::new(SimpleMeasurer));
    engine.rebuild(&input_nodes).unwrap();
    let snapshot = engine
        .compute_layout(&input_nodes, root_id, LayoutSize::new(400.0, 300.0), &|_| 0.0)
        .unwrap();
    (ir, root_id, snapshot)
}

fn rect_center(rect: fission_layout::LayoutRect) -> (f32, f32) {
    (rect.x() + rect.width() / 2.0, rect.y() + rect.height() / 2.0)
}

#[test]
fn badge_background_uses_theme_secondary() {
    let mut tokens = Tokens::default();
    tokens.colors.secondary = Color { r: 7, g: 11, b: 13, a: 255 };
    let theme = Theme {
        components: ComponentTheme::from_tokens(&tokens),
        tokens,
    };
    let mut env = Env::default();
    env.theme = theme;

    let (ir, _root_id) = build_widget_ir(Badge { text: "1".into(), ..Default::default() }, &env);

    let mut found = false;
    for node in ir.nodes.values() {
        if let Op::Paint(PaintOp::DrawRect { fill: Some(fission_ir::op::Fill::Solid(color)), .. }) = &node.op {
            if *color == env.theme.tokens.colors.secondary {
                found = true;
                break;
            }
        }
    }

    assert!(found, "expected badge background to use theme secondary color");
}

#[test]
fn stepper_circle_text_centered() {
    let env = Env::default();
    let (ir, _root_id, snapshot) = layout_widget(
        Stepper {
            steps: vec!["Import".into()],
            active_index: 0,
        },
        &env,
    );
    let parents = parent_map(&ir);

    let circle_id = ir.nodes.iter().find_map(|(id, node)| {
        if let Op::Layout(LayoutOp::Box { width: Some(w), height: Some(h), .. }) = &node.op {
            if approx_eq(*w, 24.0) && approx_eq(*h, 24.0) {
                return Some(*id);
            }
        }
        None
    }).expect("stepper circle container");

    let text_paint_id = ir.nodes.iter().find_map(|(id, node)| {
        if let Op::Paint(PaintOp::DrawText { text, .. }) = &node.op {
            if text == "1" {
                return Some(*id);
            }
        }
        None
    }).expect("stepper circle text paint");

    let text_layout_id = parents.get(&text_paint_id).copied().expect("text layout");
    let circle_rect = snapshot.get_node_geometry(circle_id).unwrap().rect;
    let text_rect = snapshot.get_node_geometry(text_layout_id).unwrap().rect;

    let (cx, cy) = rect_center(circle_rect);
    let (tx, ty) = rect_center(text_rect);

    assert!(approx_eq(cx, tx) && approx_eq(cy, ty), "stepper text should be centered in circle");
}

#[test]
fn badge_text_centered() {
    let env = Env::default();
    let (ir, root_id, snapshot) =
        layout_widget(Badge { text: "1".into(), ..Default::default() }, &env);
    let parents = parent_map(&ir);

    let text_paint_id = ir.nodes.iter().find_map(|(id, node)| {
        if let Op::Paint(PaintOp::DrawText { text, .. }) = &node.op {
            if text == "1" {
                return Some(*id);
            }
        }
        None
    }).expect("badge text paint");

    let text_layout_id = parents.get(&text_paint_id).copied().expect("badge text layout");
    let badge_rect = snapshot.get_node_geometry(root_id).unwrap().rect;
    let text_rect = snapshot.get_node_geometry(text_layout_id).unwrap().rect;

    let (bx, by) = rect_center(badge_rect);
    let (tx, ty) = rect_center(text_rect);

    assert!(approx_eq(bx, tx) && approx_eq(by, ty), "badge text should be centered");
}

#[test]
fn stepper_active_text_uses_on_primary() {
    let mut tokens = Tokens::default();
    tokens.colors.on_primary = Color { r: 12, g: 34, b: 56, a: 255 };
    let theme = Theme {
        components: ComponentTheme::from_tokens(&tokens),
        tokens,
    };
    let mut env = Env::default();
    env.theme = theme;

    let (ir, _root_id) = build_widget_ir(
        Stepper {
            steps: vec!["Import".into()],
            active_index: 0,
        },
        &env,
    );

    let mut found = None;
    for node in ir.nodes.values() {
        if let Op::Paint(PaintOp::DrawText { text, color, .. }) = &node.op {
            if text == "1" {
                found = Some(*color);
                break;
            }
        }
    }

    let color = found.expect("stepper active text");
    assert_eq!(
        color, env.theme.tokens.colors.on_primary,
        "stepper active text should use theme on_primary"
    );
}
