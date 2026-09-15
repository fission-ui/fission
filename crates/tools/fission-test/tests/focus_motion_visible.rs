//! A button's animated focus style shows after keyboard focus and not after a click.

use anyhow::Result;
use fission_core::event::{InputEvent, KeyCode, KeyEvent};
use fission_core::ui::{Button, Container, Text, Widget};
use fission_core::{with_reducer, GlobalState, WidgetId};
use fission_ir::op::{Op, PaintOp};
use fission_test::{TestDriver, TestHarness};

#[derive(Clone, Debug, Default)]
struct State {
    presses: u32,
}

impl GlobalState for State {}

#[fission_macros::fission_reducer(Press)]
fn press(state: &mut State) {
    state.presses += 1;
}

#[derive(Clone)]
struct Root;

impl From<Root> for Widget {
    fn from(_: Root) -> Self {
        let (ctx, _) = fission_core::build::current::<State>();
        Container::new(Button {
            id: Some(WidgetId::explicit("focus-motion.button")),
            child: Some(Text::new("Save").into()),
            on_press: Some(with_reducer!(ctx, Press, press)),
            width: Some(120.0),
            height: Some(40.0),
            ..Default::default()
        })
        .padding_all(20.0)
        .into()
    }
}

/// Outer shadows the button paints, which is where its focus ring is drawn.
fn button_shadows(driver: &TestDriver<State>) -> usize {
    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let mut stack = vec![WidgetId::explicit("focus-motion.button")];
    let mut shadows = 0;
    while let Some(id) = stack.pop() {
        let Some(node) = ir.nodes.get(&id) else {
            continue;
        };
        if let Op::Paint(PaintOp::DrawRect {
            shadow: Some(shadow),
            ..
        }) = &node.op
        {
            // A focus ring is a hard-edged spread; elevation shadows are blurred and change with
            // hover and press.
            if !shadow.inset && shadow.blur_radius == 0.0 && shadow.spread_radius > 0.0 {
                shadows += 1;
            }
        }
        stack.extend(node.children.iter().copied());
    }
    shadows
}

#[test]
fn clicking_a_button_does_not_draw_its_keyboard_focus_ring() -> Result<()> {
    let mut driver = TestDriver::new(TestHarness::new(State::default()).with_root_widget(Root));
    driver.pump()?;
    let resting = button_shadows(&driver);

    driver.tap_text("Save")?;
    driver.pump()?;
    driver.tick(400)?;
    assert_eq!(
        driver
            .harness
            .runtime
            .get_app_state::<State>()
            .expect("state")
            .presses,
        1
    );
    assert_eq!(
        button_shadows(&driver),
        resting,
        "a clicked button keeps focus but does not draw the keyboard focus ring"
    );

    driver
        .harness
        .send_event(InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Tab,
            modifiers: 0,
        }))?;
    driver.pump()?;
    driver
        .harness
        .send_event(InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Tab,
            modifiers: 1,
        }))?;
    driver.pump()?;
    driver.tick(400)?;
    assert!(
        driver
            .harness
            .runtime
            .runtime_state
            .interaction
            .focus_visible,
        "keyboard navigation makes focus visible"
    );
    Ok(())
}
