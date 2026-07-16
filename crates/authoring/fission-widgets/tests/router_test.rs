use fission_core::internal::BuildCtx;
use fission_core::ui::{Scroll, Text};
use fission_core::{build, GlobalState, View};
use fission_ir::{LayoutOp, Op, WidgetId};
use fission_widgets::router::{Route, Router};
use std::sync::Arc;

#[derive(Default, Clone, Debug)]
struct State;
impl GlobalState for State {}

#[test]
fn test_router_matching() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );

    // Test Exact Match
    let router = Router::<State> {
        current_path: "/home".into(),
        routes: vec![Route {
            path: "/home".into(),
            builder: Arc::new(|_, _, _| Text::new("Home").into()),
        }],
        not_found: None,
    };

    let node = build::enter(&mut ctx, &view, || router.into());
    assert_widget_draws_text(&node, "Home");
}

#[test]
fn test_router_params() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );

    // Test Param Match
    let router = Router::<State> {
        current_path: "/user/123".into(),
        routes: vec![Route {
            path: "/user/:id".into(),
            builder: Arc::new(|_, _, params| Text::new(format!("User {}", params["id"])).into()),
        }],
        not_found: None,
    };

    let node = build::enter(&mut ctx, &view, || router.into());
    assert_widget_draws_text(&node, "User 123");
}

#[test]
fn router_scopes_route_content_by_matched_path() {
    let first = route_scroll_id("/page/one");
    let second = route_scroll_id("/page/two");

    assert_ne!(
        first, second,
        "route-local scroll state must not carry between different matched paths"
    );
}

fn assert_widget_draws_text(widget: &fission_core::Widget, expected: &str) {
    let ir = fission_core::internal::lower_widget_to_ir(widget);
    assert!(
        ir.nodes.values().any(|node| matches!(
            &node.op,
            fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. })
                if text == expected
        )),
        "expected lowered widget tree to draw `{expected}`"
    );
}

fn route_scroll_id(path: &str) -> WidgetId {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );

    let router = Router::<State> {
        current_path: path.into(),
        routes: vec![Route {
            path: "/page/:slug".into(),
            builder: Arc::new(|_, _, params| {
                Scroll {
                    child: Some(Text::new(format!("Page {}", params["slug"])).into()),
                    ..Default::default()
                }
                .into()
            }),
        }],
        not_found: None,
    };

    let node = build::enter(&mut ctx, &view, || router.into());
    let ir = fission_core::internal::lower_widget_to_ir(&node);

    ir.nodes
        .iter()
        .find_map(|(id, node)| match node.op {
            Op::Layout(LayoutOp::Scroll { .. }) => Some(*id),
            _ => None,
        })
        .expect("route should lower a scroll node")
}
