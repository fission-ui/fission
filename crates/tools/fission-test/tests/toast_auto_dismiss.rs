//! A toast with a close action asks to close after its duration, unless it is persistent.

use fission_core::ui::Widget;
use fission_core::{GlobalState, ReducerContext, WidgetId};
use fission_ir::op::Op;
use fission_ir::Role;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::{Toast, ToastDuration, ToastKind, ToastMotion};

#[derive(Debug, Clone)]
struct State {
    shown: bool,
    duration: ToastDuration,
}

impl GlobalState for State {}

#[fission_macros::fission_action]
struct HideToast;

fn hide_toast(state: &mut State, _action: HideToast, _ctx: &mut ReducerContext<State>) {
    state.shown = false;
}

#[derive(Clone)]
struct Page;

impl From<Page> for Widget {
    fn from(_: Page) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        let hide = ctx.bind(
            HideToast,
            hide_toast as fn(&mut State, HideToast, &mut ReducerContext<State>),
        );
        if !view.state().shown {
            return fission_core::ui::Spacer::default().into();
        }
        Toast {
            id: WidgetId::explicit("auto-dismiss.toast"),
            kind: ToastKind::Success,
            message: "Saved".into(),
            on_close: Some(hide),
            duration: view.state().duration,
            motion: Some(ToastMotion::None),
        }
        .into()
    }
}

fn driver(duration: ToastDuration) -> TestDriver<State> {
    let mut driver = TestDriver::new(
        TestHarness::new(State {
            shown: true,
            duration,
        })
        .with_root_widget(Page),
    );
    driver.pump().expect("first frame");
    driver
}

fn shown(driver: &TestDriver<State>) -> bool {
    driver
        .harness
        .runtime
        .get_app_state::<State>()
        .expect("state")
        .shown
}

#[test]
fn a_toast_closes_itself_after_its_duration() {
    let mut driver = driver(ToastDuration::Millis(1_000));
    assert!(shown(&driver));
    driver
        .harness
        .tick(1_200)
        .expect("advance past the duration");
    driver.pump().expect("frame after the duration");
    assert!(!shown(&driver), "the toast dispatched its close action");
}

#[test]
fn a_persistent_toast_stays_until_closed() {
    let mut driver = driver(ToastDuration::Persistent);
    driver.harness.tick(10_000).expect("advance a long time");
    driver.pump().expect("frame after waiting");
    assert!(
        shown(&driver),
        "a persistent toast does not close on its own"
    );
}

#[test]
fn the_close_control_is_a_labelled_button() {
    let driver = driver(ToastDuration::Persistent);
    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let close_buttons = ir
        .nodes
        .values()
        .filter(|node| {
            matches!(&node.op, Op::Semantics(semantics)
                if semantics.role == Role::Button
                    && semantics.label.as_deref() == Some("Dismiss notification"))
        })
        .count();
    assert_eq!(
        close_buttons, 1,
        "the close control is announced as a button"
    );
}
