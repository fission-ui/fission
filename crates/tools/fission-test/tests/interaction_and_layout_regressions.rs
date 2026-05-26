use fission_core::ui::{Container, Node, Text};
use fission_core::{reduce_with, AppState, BuildCtx, ReducerContext, View, Widget};
use fission_test::TestHarness;
use fission_widgets::{NumberInput, SplitDirection, SplitView};

#[derive(Debug, Default, Clone)]
struct State {
    _counter: f32,
    _text: String,
    modal_open: bool,
}
impl AppState for State {}

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
    struct StepperTest;
    impl Widget<State> for StepperTest {
        fn build(&self, ctx: &mut BuildCtx<State>, _view: &View<State>) -> Node {
            Container::new(
                NumberInput {
                    value: 9.0,
                    on_increment: Some(ctx.bind(IncrementAction, reduce_with!(ignore_increment))),
                    on_decrement: Some(ctx.bind(IncrementAction, reduce_with!(ignore_increment))),
                    ..Default::default()
                }
                .build(ctx, _view),
            )
            .into_node()
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

    struct InboxLayout;
    impl Widget<State> for InboxLayout {
        fn build(&self, _ctx: &mut BuildCtx<State>, _view: &View<State>) -> Node {
            SplitView {
                id: fission_core::WidgetNodeId::explicit("split"),
                direction: SplitDirection::Horizontal,
                first: Box::new(
                    Container::new(Text::new("Sidebar").into_node())
                        .width(200.0)
                        .into_node(),
                ),
                second: Box::new(
                    SplitView {
                        id: fission_core::WidgetNodeId::explicit("split_inner"),
                        direction: fission_widgets::SplitDirection::Horizontal,
                        first: Box::new(Container::new(Text::new("List").into_node()).into_node()),
                        second: Box::new(
                            Container::new(Text::new("Detail").into_node()).into_node(),
                        ),
                        split_ratio: INNER_SPLIT_RATIO,
                        on_resize: None,
                    }
                    .build(_ctx, _view),
                ),
                split_ratio: OUTER_SPLIT_RATIO,
                on_resize: None,
            }
            .build(_ctx, _view)
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

    struct ModalTest;
    impl Widget<State> for ModalTest {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            Modal {
                id: fission_core::WidgetNodeId::explicit("test_modal"),
                title: "Test".into(),
                content: Box::new(Text::new("Content").into_node()),
                is_open: true,
                on_dismiss: Some(ctx.bind(
                    DismissAction,
                    reduce_with!(
                        (|s: &mut State, _, _| {
                            s.modal_open = false;
                        })
                    ),
                )),
                actions: vec![],
                width: Some(300.0),
            }
            .build(ctx, view)
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
            point: fission_core::LayoutPoint::new(10.0, 10.0),
            button: fission_core::PointerButton::Primary,
            modifiers: 0,
        },
    ))
    .unwrap();

    h.send_event(fission_core::InputEvent::Pointer(
        fission_core::PointerEvent::Up {
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

    struct ModalTest;
    impl Widget<State> for ModalTest {
        fn build(&self, ctx: &mut BuildCtx<State>, view: &View<State>) -> Node {
            Modal {
                id: fission_core::WidgetNodeId::explicit("test_modal"),
                title: "Test".into(),
                content: Box::new(Text::new("Content").into_node()),
                is_open: true,
                on_dismiss: Some(ctx.bind(
                    DismissAction,
                    reduce_with!(
                        (|s: &mut State, _, _| {
                            s.modal_open = false;
                        })
                    ),
                )),
                actions: vec![],
                width: Some(300.0),
            }
            .build(ctx, view)
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
        point: center,
        button: PointerButton::Primary,
        modifiers: 0,
    }))
    .unwrap();
    h.pump().unwrap();
    h.send_event(fission_core::InputEvent::Pointer(PointerEvent::Up {
        point: center,
        button: PointerButton::Primary,
        modifiers: 0,
    }))
    .unwrap();
    h.pump().unwrap();

    let state = h.runtime.get_app_state::<State>().unwrap();
    assert!(!state.modal_open, "Modal should be closed via close button");
}
