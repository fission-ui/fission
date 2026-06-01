use fission_core::ui::{Container, Node, Text};
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_widgets::router::{Route, Router};
use std::sync::Arc;

#[derive(Default, Clone, Debug)]
struct State;
impl AppState for State {}

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
    let router = Router {
        current_path: "/home".into(),
        routes: vec![Route {
            path: "/home".into(),
            builder: Arc::new(|_, _, _| Text::new("Home").into_node()),
        }],
        not_found: None,
    };

    let node = fission_core::view::lower_widget_to_node(&router, &mut ctx, &view);
    if let Node::Text(t) = node {
        assert_eq!(
            t.content,
            fission_core::ui::TextContent::Literal("Home".into())
        );
    } else {
        panic!("Router should return Home node");
    }
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
    let router = Router {
        current_path: "/user/123".into(),
        routes: vec![Route {
            path: "/user/:id".into(),
            builder: Arc::new(|_, _, params| {
                let id = params.get("id").unwrap();
                Text::new(format!("User {}", id)).into_node()
            }),
        }],
        not_found: None,
    };

    let node = fission_core::view::lower_widget_to_node(&router, &mut ctx, &view);
    if let Node::Text(t) = node {
        assert_eq!(
            t.content,
            fission_core::ui::TextContent::Literal("User 123".into())
        );
    } else {
        panic!("Router should return User 123 node");
    }
}
