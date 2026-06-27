use fission_core::env::Env;
use fission_core::internal::BuildCtx;
use fission_core::motion::MotionDeclarationKind;
use fission_core::ui::{Button, ButtonMotion, Text, Widget};
use fission_core::{build, GlobalState, View, WidgetId};
use fission_widgets::{MenuButton, MenuItem, Modal, ModalMotion, Popover, Toast, ToastKind};

#[derive(Default, Clone, Debug)]
#[allow(dead_code)]
struct State {
    menu_open: bool,
    toast_visible: bool,
}
impl GlobalState for State {}

fn test_view<'a>(
    runtime: &'a fission_core::Runtime,
    env: &'a Env,
) -> fission_core::View<'a, State> {
    let state = runtime.get_app_state::<State>().unwrap();
    View::new(state, &runtime.runtime_state, env, None)
}

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
        id: WidgetId::explicit("test_menu"),
        label: "Menu".into(),
        items: vec![MenuItem {
            label: "Item 1".into(),
            icon: None,
            on_select: None,
        }],
        is_open: true,
        on_toggle: None,
    };

    let _: Widget = build::enter(&mut ctx, &view, || menu_button.into());

    let portals = ctx.take_portals();
    assert_eq!(
        portals.len(),
        1,
        "MenuButton should register a portal when open"
    );
}

#[test]
fn test_button_motion_is_explicit_opt_in() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State::default())).unwrap();
    let env = Env::default();

    let mut static_ctx = BuildCtx::<State>::new();
    let view = test_view(&runtime, &env);
    let _: Widget = build::enter(&mut static_ctx, &view, || {
        Button {
            id: Some(WidgetId::explicit("static_button")),
            child: Some(Text::new("Static").into()),
            ..Default::default()
        }
        .into()
    });
    assert!(
        static_ctx.take_motion_declarations().is_empty(),
        "button motion defaults to None and emits no motion"
    );

    let mut motion_ctx = BuildCtx::<State>::new();
    let view = test_view(&runtime, &env);
    let _: Widget = build::enter(&mut motion_ctx, &view, || {
        Button {
            id: Some(WidgetId::explicit("motion_button")),
            child: Some(Text::new("Motion").into()),
            motion: Some(ButtonMotion::Default),
            ..Default::default()
        }
        .into()
    });
    assert!(
        motion_ctx
            .take_motion_declarations()
            .iter()
            .any(|declaration| matches!(declaration.kind, MotionDeclarationKind::Tracks { .. })),
        "explicit button motion lowers to native motion tracks"
    );
}

#[test]
fn test_modal_motion_keeps_closed_modal_on_presence_path() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State::default())).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = Env::default();
    let view = test_view(&runtime, &env);

    let _: Widget = build::enter(&mut ctx, &view, || {
        Modal {
            id: WidgetId::explicit("motion_modal"),
            title: "Motion".into(),
            content: Text::new("Body").into(),
            is_open: false,
            on_dismiss: None,
            actions: vec![],
            width: None,
            motion: Some(ModalMotion::Default),
        }
        .into()
    });

    assert_eq!(
        ctx.take_portals().len(),
        1,
        "closed modal with explicit motion still registers the portal so Presence can drive exit"
    );
    assert!(
        ctx.take_motion_declarations()
            .iter()
            .any(|declaration| matches!(declaration.kind, MotionDeclarationKind::Presence { .. })),
        "modal motion lowers to native presence declarations"
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
        id: WidgetId::explicit("test_toast"),
        kind: ToastKind::Success,
        message: "Operation completed".into(),
        on_close: None,
        motion: None,
    };

    let node = build::enter(&mut ctx, &view, || toast.into());

    assert_eq!(fission_core::internal::widget_kind_name(&node), "Container");
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

    let _: Widget = build::enter(&mut ctx, &view, || {
        Popover {
            id: WidgetId::explicit("test_popover_no_close"),
            is_open: true,
            on_toggle: None,
            on_close: None,
            trigger: Text::new("trigger").into(),
            content: Text::new("content").into(),
            motion: None,
        }
        .into()
    });

    let portals = ctx.take_portals();
    assert_eq!(
        portals.len(),
        1,
        "popover should register one flyout portal"
    );
    let ir = fission_core::internal::lower_widget_to_ir(&portals[0].1);
    assert!(
        ir.nodes.len() > 0,
        "popover without on_close should register lowerable flyout content, not a full-screen backdrop"
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

    let _: Widget = build::enter(&mut ctx, &view, || {
        Popover {
            id: WidgetId::explicit("test_popover_with_close"),
            is_open: true,
            on_toggle: None,
            on_close: Some(fission_core::ActionEnvelope {
                id: fission_core::ActionId::from_u128(42),
                payload: vec![],
            }),
            trigger: Text::new("trigger").into(),
            content: Text::new("content").into(),
            motion: None,
        }
        .into()
    });

    let portals = ctx.take_portals();
    assert_eq!(portals.len(), 1, "popover should register one portal");
    assert!(
        fission_core::internal::widget_kind_name(&portals[0].1) == "ZStack",
        "popover with on_close should include the backdrop + flyout stack"
    );
}
