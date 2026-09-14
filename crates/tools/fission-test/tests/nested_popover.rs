//! A popover opened from inside another popover is its top layer: it draws above its host and
//! Escape closes it before the host.

use anyhow::Result;
use fission_core::event::{InputEvent, KeyCode, KeyEvent};
use fission_core::ui::{Button, Text, Widget};
use fission_core::{with_reducer, GlobalState, WidgetId};
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{Popover, PopoverMotion};

#[derive(Clone, Debug)]
struct State {
    outer_open: bool,
    inner_open: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            outer_open: true,
            inner_open: true,
        }
    }
}

impl GlobalState for State {}

#[fission_macros::fission_reducer(CloseOuter)]
fn close_outer(state: &mut State) {
    state.outer_open = false;
}

#[fission_macros::fission_reducer(CloseInner)]
fn close_inner(state: &mut State) {
    state.inner_open = false;
}

#[derive(Clone)]
struct Root;

impl From<Root> for Widget {
    fn from(_: Root) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        // Built first, as a date picker inside a filters popover is.
        let inner = Popover {
            id: WidgetId::explicit("nested.inner"),
            is_open: view.state().inner_open,
            on_close: Some(with_reducer!(ctx, CloseInner, close_inner)),
            trigger: Button {
                child: Some(Text::new("Inner trigger").into()),
                width: Some(120.0),
                height: Some(40.0),
                ..Default::default()
            }
            .into(),
            content: Text::new("Inner content").into(),
            motion: Some(PopoverMotion::None),
        };
        Popover {
            id: WidgetId::explicit("nested.outer"),
            is_open: view.state().outer_open,
            on_close: Some(with_reducer!(ctx, CloseOuter, close_outer)),
            trigger: Button {
                child: Some(Text::new("Outer trigger").into()),
                width: Some(120.0),
                height: Some(40.0),
                ..Default::default()
            }
            .into(),
            content: inner.into(),
            motion: Some(PopoverMotion::None),
        }
        .into()
    }
}

#[test]
fn escape_closes_the_nested_popover_before_its_host() -> Result<()> {
    let mut driver = TestDriver::new(TestHarness::new(State::default()).with_root_widget(Root));
    driver.set_viewport(800.0, 600.0);
    driver.pump()?;
    driver.assert_text_visible("Inner content");

    driver
        .harness
        .send_event(InputEvent::Keyboard(KeyEvent::Down {
            key_code: KeyCode::Escape,
            modifiers: 0,
        }))?;
    driver.pump()?;

    let state = driver
        .harness
        .runtime
        .get_app_state::<State>()
        .expect("state");
    assert!(
        !state.inner_open,
        "Escape closes the popover opened from inside the other one"
    );
    assert!(state.outer_open, "the host popover stays open");
    Ok(())
}
