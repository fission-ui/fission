//! Switching a button from keyboard focus to pointer focus must retain its press target.
use fission_core::authoring::LoweringContext;
use fission_core::ui::{Button, Text};
use fission_core::{
    ActionEnvelope, ActionId, Env, GlobalState, InputEvent, LayoutPoint, LayoutRect, LayoutSize,
    LayoutSnapshot, PointerButton, PointerEvent, Runtime, TextEditSource, Widget, WidgetId,
};
use fission_ir::{CoreIR, Op, PaintOp};
use fission_layout::LayoutNodeGeometry;

#[derive(Debug, Default)]
struct State(usize);
impl GlobalState for State {}
const PRESS: ActionId = ActionId::from_u128(101);

fn pressed(state: &mut State, _: &ActionEnvelope, _: WidgetId) -> anyhow::Result<()> {
    state.0 += 1;
    Ok(())
}

fn scene(widget: &Widget, runtime: &Runtime) -> (CoreIR, LayoutSnapshot) {
    let env = Env::default();
    let mut cx = LoweringContext::new(&env, &runtime.runtime_state, None, None);
    let root = fission_core::internal::lower_widget(widget, &mut cx);
    cx.set_root(root);
    let ir = cx.into_ir();
    let mut layout = LayoutSnapshot::new(LayoutSize::new(100.0, 40.0));
    for (id, node) in &ir.nodes {
        let rect = if matches!(node.op, Op::Paint(PaintOp::DrawRichText { .. })) {
            LayoutRect::new(30.0, 10.0, 40.0, 20.0)
        } else {
            LayoutRect::new(0.0, 0.0, 100.0, 40.0)
        };
        layout.nodes.insert(
            *id,
            LayoutNodeGeometry {
                rect,
                content_size: LayoutSize::new(100.0, 40.0),
            },
        );
    }
    (ir, layout)
}

fn click_after_keyboard_focus(point: LayoutPoint) -> anyhow::Result<()> {
    let id = WidgetId::explicit("restore");
    let widget: Widget = Button {
        id: Some(id),
        child: Some(Text::new("Restore").into()),
        on_press: Some(ActionEnvelope {
            id: PRESS,
            payload: vec![],
        }),
        ..Default::default()
    }
    .into();
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(State::default()))?;
    runtime.register_reducer::<State>(PRESS, pressed)?;
    let (ir, _) = scene(&widget, &runtime);
    runtime.set_focused_widget(&ir, Some(id), TextEditSource::Keyboard)?;
    assert!(runtime.runtime_state.interaction.is_focus_visible(id));
    let (ir, layout) = scene(&widget, &runtime);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    assert!(!runtime.runtime_state.interaction.is_focus_visible(id));
    // The real shell rebuilds here, removing the keyboard focus ring.
    let (ir, layout) = scene(&widget, &runtime);
    runtime.handle_input(
        InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        &ir,
        &layout,
    )?;
    assert_eq!(runtime.get_app_state::<State>().unwrap().0, 1);
    Ok(())
}

#[test]
fn clicking_a_keyboard_focused_button_dispatches_after_the_press_rebuild() -> anyhow::Result<()> {
    click_after_keyboard_focus(LayoutPoint::new(50.0, 20.0))
}

#[test]
fn clicking_button_padding_after_keyboard_focus_does_not_capture_the_ring() -> anyhow::Result<()> {
    click_after_keyboard_focus(LayoutPoint::new(4.0, 20.0))
}
