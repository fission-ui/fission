use fission_core::env::Env;
use fission_core::ui::{Node, Text};
use fission_core::{AppState, BuildCtx, View, Widget, WidgetNodeId};
use fission_widgets::{MenuButton, MenuItem, Popover, Toast, ToastKind};

#[derive(Default, Clone, Debug)]
#[allow(dead_code)]
struct State {
    menu_open: bool,
    toast_visible: bool,
}
impl AppState for State {}

#[test]
fn test_menu_button_registers_portal_when_open() {
    let mut runtime = fission_core::Runtime::default();
    runtime
        .add_app_state(Box::new(State {
            menu_open: true,
            toast_visible: false,
        }))
        .unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = Env::default();
    let state = runtime.get_app_state::<State>().unwrap();
    let view = View::new(state, &runtime.runtime_state, &env, None);

    let menu_button = MenuButton {
        id: WidgetNodeId::explicit("test_menu"),
        label: "Menu".into(),
        items: vec![MenuItem {
            label: "Item 1".into(),
            icon: None,
            on_select: None,
        }],
        is_open: true,
        on_toggle: None,
    };

    let _ = menu_button.build(&mut ctx, &view);

    let portals = ctx.take_portals();
    assert_eq!(
        portals.len(),
        1,
        "MenuButton should register a portal when open"
    );
}

#[test]
fn test_toast_renders_content() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State::default())).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = Env::default();
    let state = runtime.get_app_state::<State>().unwrap();
    let view = View::new(state, &runtime.runtime_state, &env, None);

    let toast = Toast {
        id: WidgetNodeId::explicit("test_toast"),
        kind: ToastKind::Success,
        message: "Operation completed".into(),
        on_close: None,
    };

    let node = toast.build(&mut ctx, &view);

    // Toast is a direct widget, it returns a Container node
    if let Node::Container(_) = node {
        // ok
    } else {
        panic!("Toast build should return a Container node");
    }
}

#[test]
fn test_popover_without_on_close_does_not_add_backdrop_layer() {
    let mut runtime = fission_core::Runtime::default();
    runtime
        .add_app_state(Box::new(State::default()))
        .expect("state");

    let mut ctx = BuildCtx::<State>::new();
    let env = Env::default();
    let state = runtime.get_app_state::<State>().unwrap();
    let view = View::new(state, &runtime.runtime_state, &env, None);

    let _ = Popover {
        id: WidgetNodeId::explicit("test_popover_no_close"),
        is_open: true,
        on_toggle: None,
        on_close: None,
        trigger: Box::new(Text::new("trigger").into_node()),
        content: Box::new(Text::new("content").into_node()),
    }
    .build(&mut ctx, &view);

    let portals = ctx.take_portals();
    assert_eq!(
        portals.len(),
        1,
        "popover should register one flyout portal"
    );
    assert!(
        matches!(portals[0].1, Node::Custom(_)),
        "popover without on_close should register only flyout content, not a full-screen backdrop"
    );
}

#[test]
fn test_popover_with_on_close_adds_backdrop_layer() {
    let mut runtime = fission_core::Runtime::default();
    runtime
        .add_app_state(Box::new(State::default()))
        .expect("state");

    let mut ctx = BuildCtx::<State>::new();
    let env = Env::default();
    let state = runtime.get_app_state::<State>().unwrap();
    let view = View::new(state, &runtime.runtime_state, &env, None);

    let _ = Popover {
        id: WidgetNodeId::explicit("test_popover_with_close"),
        is_open: true,
        on_toggle: None,
        on_close: Some(fission_core::ActionEnvelope {
            id: fission_core::ActionId::from_u128(42),
            payload: vec![],
        }),
        trigger: Box::new(Text::new("trigger").into_node()),
        content: Box::new(Text::new("content").into_node()),
    }
    .build(&mut ctx, &view);

    let portals = ctx.take_portals();
    assert_eq!(portals.len(), 1, "popover should register one portal");
    assert!(
        matches!(portals[0].1, Node::ZStack(_)),
        "popover with on_close should include the backdrop + flyout stack"
    );
}
