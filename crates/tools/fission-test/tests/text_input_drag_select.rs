use anyhow::Result;
use fission_core::event::{InputEvent, PointerButton, PointerEvent};
use fission_core::op::FlexDirection;
use fission_core::ui::{Column, Scroll, Spacer, TextInput, Widget};
use fission_core::{GlobalState, ReducerContext};
use fission_ir::WidgetId;
use fission_layout::{LayoutPoint, LayoutSize};
use fission_test::{TestDriver, TestHarness};

const FIELD: &str = "drag-select.field";

#[derive(Debug, Clone)]
struct State {
    text: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            text: "select this text by dragging".into(),
        }
    }
}

impl GlobalState for State {}

#[fission_macros::fission_action]
struct UpdateText;

fn update_text(state: &mut State, _action: UpdateText, ctx: &mut ReducerContext<State>) {
    if let Some(change) = ctx.input.text_change() {
        state.text = change.new_text.clone();
    }
}

#[derive(Clone)]
struct ScrollingForm;

impl From<ScrollingForm> for Widget {
    fn from(_: ScrollingForm) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        Scroll {
            child: Some(
                Column {
                    children: vec![
                        TextInput {
                            id: Some(WidgetId::explicit(FIELD)),
                            semantics_identifier: Some(FIELD.into()),
                            value: view.state().text.clone(),
                            on_input: Some(
                                ctx.bind(UpdateText, fission_core::reduce!(update_text)),
                            ),
                            width: Some(360.0),
                            ..Default::default()
                        }
                        .into(),
                        // Taller than the viewport, so the page itself can scroll.
                        Spacer {
                            height: Some(2000.0),
                            ..Default::default()
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
            ),
            direction: FlexDirection::Column,
            ..Default::default()
        }
        .into()
    }
}

fn pointer(point: LayoutPoint, phase: &str) -> InputEvent {
    match phase {
        "down" => InputEvent::Pointer(PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        "up" => InputEvent::Pointer(PointerEvent::Up {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: PointerButton::Primary,
            modifiers: 0,
        }),
        _ => InputEvent::Pointer(PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            modifiers: 0,
        }),
    }
}

#[test]
fn dragging_across_a_text_input_in_a_scrolling_page_selects_text() -> Result<()> {
    let mut driver =
        TestDriver::new(TestHarness::new(State::default()).with_root_widget(ScrollingForm));
    driver.harness.env.viewport_size = LayoutSize::new(800.0, 600.0);
    driver.pump()?;

    let field = driver
        .find_semantics_identifier(FIELD)
        .expect("text input semantics");
    let y = field.bounds.y() + field.bounds.height() / 2.0;
    let start = LayoutPoint::new(field.bounds.x() + 1.0, y);
    let end = LayoutPoint::new(field.bounds.x() + field.bounds.width() - 2.0, y);

    driver.harness.send_event(pointer(start, "down"))?;
    driver.pump()?;
    for step in 1..=12 {
        let t = step as f32 / 12.0;
        let point = LayoutPoint::new(start.x + (end.x - start.x) * t, y);
        driver.harness.send_event(pointer(point, "move"))?;
    }
    driver.harness.send_event(pointer(end, "up"))?;
    driver.pump()?;

    let selection = driver
        .harness
        .runtime
        .runtime_state
        .text_edit
        .get(WidgetId::explicit(FIELD))
        .expect("editing state after the drag")
        .selection_range();
    assert_eq!(
        selection.0, 0,
        "the drag started before the first character"
    );
    assert!(
        selection.1 > selection.0,
        "dragging across the text should select it, got {selection:?}"
    );
    Ok(())
}
