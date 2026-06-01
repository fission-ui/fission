use anyhow::Result;
use fission_core::event::{PointerButton, PointerEvent};
use fission_core::op::Color;
use fission_core::ui::{Button, Column, Container, Row, Text};
use fission_core::{AppState, BuildCtx, InputEvent, IntoWidget, LayoutPoint, View, Widget};
use fission_ir::Role;
use fission_test::TestHarness;

#[derive(Debug, Default, Clone)]
struct CounterState {
    count: i32,
}

impl AppState for CounterState {}

#[fission_macros::fission_reducer(Increment)]
fn increment(state: &mut CounterState) {
    state.count += 1;
}

struct CounterRoot;

impl Widget<CounterState> for CounterRoot {
    fn build(
        &self,
        ctx: &mut BuildCtx<CounterState>,
        view: &View<CounterState>,
    ) -> impl IntoWidget<CounterState> {
        let increment = fission_core::with_reducer!(ctx, Increment, increment);

        Container::new(
            Column {
                gap: Some(12.0),
                children: vec![
                    Text::new("Counter").size(24.0).into_node(),
                    Text::new(format!("Count: {}", view.state.count))
                        .color(Color {
                            r: 20,
                            g: 40,
                            b: 60,
                            a: 255,
                        })
                        .into_node(),
                    Row {
                        gap: Some(8.0),
                        children: vec![Button {
                            on_press: Some(increment),
                            child: Some(Box::new(Text::new("Increment").into_node())),
                            ..Default::default()
                        }
                        .into_node()],
                        ..Default::default()
                    }
                    .into_node(),
                ],
                ..Default::default()
            }
            .into_node(),
        )
        .padding_all(16.0)
    }
}

#[test]
fn live_harness_dispatches_from_widget_returning_into_widget() -> Result<()> {
    let mut harness = TestHarness::new(CounterState::default()).with_root_widget(CounterRoot);
    harness.pump()?;

    let button_id = harness
        .last_ir
        .as_ref()
        .expect("ir")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            if let fission_ir::Op::Semantics(semantics) = &node.op {
                (semantics.role == Role::Button).then_some(*id)
            } else {
                None
            }
        })
        .expect("button semantics");

    let rect = harness
        .last_snapshot
        .as_ref()
        .expect("layout")
        .get_node_rect(button_id)
        .expect("button rect");
    let center = LayoutPoint::new(
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    );

    harness.send_event(InputEvent::Pointer(PointerEvent::Down {
        point: center,
        button: PointerButton::Primary,
        modifiers: 0,
    }))?;
    harness.send_event(InputEvent::Pointer(PointerEvent::Up {
        point: center,
        button: PointerButton::Primary,
        modifiers: 0,
    }))?;
    harness.pump()?;

    let state = harness
        .runtime
        .get_app_state::<CounterState>()
        .expect("counter state");
    assert_eq!(state.count, 1);

    let has_updated_text = harness
        .last_ir
        .as_ref()
        .expect("ir")
        .nodes
        .values()
        .any(|node| {
            matches!(
                &node.op,
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. })
                    if text == "Count: 1"
            )
        });
    assert!(has_updated_text, "updated count text should be rendered");

    Ok(())
}
