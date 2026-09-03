use fission_core::ui::{Container, SemanticsRegion, Text, Widget};
use fission_core::{reduce_with, GlobalState, ReducerContext, Role};
use fission_test::TestHarness;
use fission_widgets::{NumberInput, SplitDirection, SplitView};

#[derive(Debug, Default, Clone)]
struct State {
    _counter: f32,
    _text: String,
    modal_open: bool,
}
impl GlobalState for State {}

#[fission_macros::fission_action(no_eq)]
struct DismissAction;

#[fission_macros::fission_action(no_eq)]
struct IncrementAction;

fn ignore_increment(
    _state: &mut State,
    _action: IncrementAction,
    _ctx: &mut ReducerContext<State>,
) {
}

#[test]
fn test_stepper_button_layout() {
    #[derive(Clone)]
    struct StepperTest;
    impl From<StepperTest> for Widget {
        fn from(_component: StepperTest) -> Self {
            let (ctx, _view) = fission_core::build::current::<State>();
            Container::new(NumberInput {
                value: 9.0,
                on_increment: Some(ctx.bind(IncrementAction, reduce_with!(ignore_increment))),
                on_decrement: Some(ctx.bind(IncrementAction, reduce_with!(ignore_increment))),
                ..Default::default()
            })
            .into()
        }
    }
    let mut h = TestHarness::new(State::default());
    h = h.with_root_widget(StepperTest);
    h.pump().unwrap();

    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();

    let mut button_rects = Vec::new();
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == fission_ir::Role::Button {
                if let Some(geom) = snap.get_node_geometry(*id) {
                    button_rects.push(geom.rect);
                }
            }
        }
    }

    assert_eq!(button_rects.len(), 2, "Expected 2 buttons in NumberInput");

    for rect in button_rects {
        println!("Button Rect: {:?}", rect);
        assert_eq!(rect.height(), 32.0, "Button height should be 32.0");
        assert_eq!(rect.width(), 32.0, "Button width should be 32.0");
    }
}

#[test]
fn test_email_list_width() {
    const OUTER_SPLIT_RATIO: f32 = 0.2;
    const INNER_SPLIT_RATIO: f32 = 0.3;
    const HANDLE_SIZE: f32 = 4.0;
    const DEFAULT_VIEWPORT_WIDTH: f32 = 800.0;

    #[derive(Clone)]
    struct InboxLayout;
    impl From<InboxLayout> for Widget {
        fn from(_component: InboxLayout) -> Self {
            let (_ctx, _view) = fission_core::build::current::<State>();
            SplitView {
                id: fission_core::WidgetId::explicit("split"),
                direction: SplitDirection::Horizontal,
                first: Container::new(Text::new("Sidebar")).width(200.0).into(),
                second: SplitView {
                    id: fission_core::WidgetId::explicit("split_inner"),
                    direction: fission_widgets::SplitDirection::Horizontal,
                    first: Container::new(Text::new("List")).into(),
                    second: Container::new(Text::new("Detail")).into(),
                    split_ratio: INNER_SPLIT_RATIO,
                    on_resize: None,
                }
                .into(),
                split_ratio: OUTER_SPLIT_RATIO,
                on_resize: None,
            }
            .into()
        }
    }
    let mut h = TestHarness::new(State::default());
    h = h.with_root_widget(InboxLayout);
    h.pump().unwrap();

    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();

    let mut list_rect = None;
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) = &node.op {
            if text == "List" {
                // Find parent container via geometry logic? Or assume Container wraps Text.
                // We want the SplitView pane size.
                // Text size might be small. Container size fills pane.
                // We'll search for the parent node of Text.
                let parent_id = ir.nodes.get(id).unwrap().parent.unwrap();
                list_rect = Some(snap.get_node_geometry(parent_id).unwrap().rect);
            }
        }
    }

    let rect = list_rect.expect("List text not found");
    println!("List Rect: {:?}", rect);

    // Keep the original 0.3 inner split ratio. Under the test harness' 800px
    // viewport, the ratio-correct first pane is about 190px; the old 250px
    // threshold only passes if the regression scenario is softened to 0.4.
    let outer_second_width = (DEFAULT_VIEWPORT_WIDTH - HANDLE_SIZE) * (1.0 - OUTER_SPLIT_RATIO);
    let expected_width = (outer_second_width - HANDLE_SIZE) * INNER_SPLIT_RATIO;

    assert!(
        (rect.width() - expected_width).abs() <= 0.1,
        "Nested SplitView width should preserve the original ratio: expected {expected_width}, got {}",
        rect.width(),
    );
}

#[test]
fn test_modal_backdrop_dismiss() {
    use fission_core::reduce_with;
    use fission_widgets::Modal;

    #[derive(Clone)]
    struct ModalTest;
    impl From<ModalTest> for Widget {
        fn from(_component: ModalTest) -> Self {
            let (ctx, _view) = fission_core::build::current::<State>();
            Modal {
                id: fission_core::WidgetId::explicit("test_modal"),
                title: "Test".into(),
                content: Text::new("Content").into(),
                is_open: true,
                on_dismiss: Some(ctx.bind(
                    DismissAction,
                    reduce_with!(
                        (|s: &mut State, _, _| {
                            s.modal_open = false;
                        })
                    ),
                )),
                backdrop_semantics_identifier: None,
                close_semantics_identifier: None,
                surface_semantics_identifier: None,
                actions: vec![],
                width: Some(300.0),
                motion: None,
            }
            .into()
        }
    }
    let mut h = TestHarness::new(State {
        modal_open: true,
        ..Default::default()
    });
    h = h.with_root_widget(ModalTest);
    h.pump().unwrap();

    h.send_event(fission_core::InputEvent::Pointer(
        fission_core::PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: fission_core::LayoutPoint::new(10.0, 10.0),
            button: fission_core::PointerButton::Primary,
            modifiers: 0,
        },
    ))
    .unwrap();

    h.send_event(fission_core::InputEvent::Pointer(
        fission_core::PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point: fission_core::LayoutPoint::new(10.0, 10.0),
            button: fission_core::PointerButton::Primary,
            modifiers: 0,
        },
    ))
    .unwrap();

    let state = h.runtime.get_app_state::<State>().unwrap();
    assert!(
        !state.modal_open,
        "Modal should be closed (modal_open = false)"
    );
}

#[test]
fn test_modal_close_button_dismiss() {
    use fission_core::event::{PointerButton, PointerEvent};
    use fission_core::reduce_with;
    use fission_widgets::Modal;

    #[derive(Clone)]
    struct ModalTest;
    impl From<ModalTest> for Widget {
        fn from(_component: ModalTest) -> Self {
            let (ctx, _view) = fission_core::build::current::<State>();
            Modal {
                id: fission_core::WidgetId::explicit("test_modal"),
                title: "Test".into(),
                content: Text::new("Content").into(),
                is_open: true,
                on_dismiss: Some(ctx.bind(
                    DismissAction,
                    reduce_with!(
                        (|s: &mut State, _, _| {
                            s.modal_open = false;
                        })
                    ),
                )),
                backdrop_semantics_identifier: None,
                close_semantics_identifier: None,
                surface_semantics_identifier: None,
                actions: vec![],
                width: Some(300.0),
                motion: None,
            }
            .into()
        }
    }
    let mut h = TestHarness::new(State {
        modal_open: true,
        ..Default::default()
    });
    h = h.with_root_widget(ModalTest);
    h.pump().unwrap();

    // Find the smallest Button semantics node; backdrop is full-screen, close is small.
    let snap = h.last_snapshot.as_ref().unwrap();
    let ir = h.last_ir.as_ref().unwrap();
    let mut buttons = Vec::new();
    for (id, node) in &ir.nodes {
        if let fission_ir::Op::Semantics(s) = &node.op {
            if s.role == fission_ir::Role::Button {
                if let Some(r) = snap.get_node_rect(*id) {
                    buttons.push((*id, r));
                }
            }
        }
    }
    assert!(!buttons.is_empty(), "Expected at least one button in modal");
    buttons.sort_by(|a, b| {
        (a.1.width() * a.1.height())
            .partial_cmp(&(b.1.width() * b.1.height()))
            .unwrap()
    });
    let (_id, r) = buttons[0];

    let center = fission_core::LayoutPoint::new(r.x() + r.width() / 2.0, r.y() + r.height() / 2.0);
    h.send_event(fission_core::InputEvent::Pointer(PointerEvent::Down {
        pointer_id: Default::default(),
        kind: Default::default(),
        point: center,
        button: PointerButton::Primary,
        modifiers: 0,
    }))
    .unwrap();
    h.pump().unwrap();
    h.send_event(fission_core::InputEvent::Pointer(PointerEvent::Up {
        pointer_id: Default::default(),
        kind: Default::default(),
        point: center,
        button: PointerButton::Primary,
        modifiers: 0,
    }))
    .unwrap();
    h.pump().unwrap();

    let state = h.runtime.get_app_state::<State>().unwrap();
    assert!(!state.modal_open, "Modal should be closed via close button");
}

#[test]
fn test_modal_content_keeps_its_intrinsic_height() {
    use fission_widgets::Modal;

    const VIEWPORT_HEIGHT: f32 = 600.0;

    #[derive(Clone)]
    struct IntrinsicModalTest;

    impl From<IntrinsicModalTest> for Widget {
        fn from(_component: IntrinsicModalTest) -> Self {
            Modal {
                id: fission_core::WidgetId::explicit("intrinsic_modal"),
                title: "Required action".into(),
                content: SemanticsRegion::new(Text::new("A short explanation"))
                    .identifier("intrinsic-modal-content")
                    .role(Role::Dialog)
                    .into(),
                is_open: true,
                on_dismiss: None,
                backdrop_semantics_identifier: None,
                close_semantics_identifier: None,
                surface_semantics_identifier: None,
                actions: vec![],
                width: Some(420.0),
                motion: None,
            }
            .into()
        }
    }

    let mut harness = TestHarness::new(State::default()).with_root_widget(IntrinsicModalTest);
    harness.pump().unwrap();

    let ir = harness.last_ir.as_ref().unwrap();
    let snapshot = harness.last_snapshot.as_ref().unwrap();
    let (content_id, surface_id) =
        ir.nodes
            .iter()
            .fold((None, None), |(content, surface), (id, node)| {
                let fission_ir::Op::Semantics(semantics) = &node.op else {
                    return (content, surface);
                };
                match semantics.identifier.as_deref() {
                    Some("intrinsic-modal-content") => (Some(*id), surface),
                    Some("fission-modal-surface") => (content, Some(*id)),
                    _ => (content, surface),
                }
            });
    let content_id = content_id.expect("modal content semantics were not rendered");
    let surface_id = surface_id.expect("modal surface semantics were not rendered");
    let content = snapshot
        .get_node_rect(content_id)
        .expect("modal content was not laid out");
    let surface = snapshot
        .get_node_rect(surface_id)
        .expect("modal surface was not laid out");

    assert!(
        content.height() < VIEWPORT_HEIGHT / 2.0,
        "short modal content should keep an intrinsic height, got {} in a {}px viewport",
        content.height(),
        VIEWPORT_HEIGHT,
    );
    assert!(
        surface.height() < VIEWPORT_HEIGHT / 2.0,
        "short modal surface should keep an intrinsic height, got {} in a {}px viewport",
        surface.height(),
        VIEWPORT_HEIGHT,
    );
}
