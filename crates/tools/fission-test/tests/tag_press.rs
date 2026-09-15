//! A pressable tag toggles through its action and reports whether it is on.

use anyhow::Result;
use fission_core::ui::{Container, Widget};
use fission_core::{with_reducer, GlobalState};
use fission_ir::op::Op;
use fission_ir::Role;
use fission_test::{TestDriver, TestHarness};
use fission_widgets::Tag;

#[derive(Clone, Debug, Default)]
struct State {
    on: bool,
}

impl GlobalState for State {}

#[fission_macros::fission_reducer(Toggle)]
fn toggle(state: &mut State) {
    state.on = !state.on;
}

#[derive(Clone)]
struct Root;

impl From<Root> for Widget {
    fn from(_: Root) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        Container::new(Tag {
            label: "Work".into(),
            on_close: None,
            on_press: Some(with_reducer!(ctx, Toggle, toggle)),
            selected: view.state().on,
        })
        .padding_all(20.0)
        .into()
    }
}

fn tag_semantics(driver: &TestDriver<State>) -> (bool, fission_core::LayoutRect) {
    let ir = driver.harness.last_ir.as_ref().expect("ir");
    let snapshot = driver.harness.last_snapshot.as_ref().expect("snapshot");
    ir.nodes
        .iter()
        .find_map(|(id, node)| match &node.op {
            Op::Semantics(semantics)
                if semantics.role == Role::Button && semantics.label.as_deref() == Some("Work") =>
            {
                Some((
                    semantics.selected == Some(true),
                    snapshot.get_node_rect(*id).expect("tag is laid out"),
                ))
            }
            _ => None,
        })
        .expect("the tag is announced as a button named by its label")
}

#[test]
fn pressing_a_tag_toggles_it() -> Result<()> {
    let mut driver = TestDriver::new(TestHarness::new(State::default()).with_root_widget(Root));
    driver.pump()?;
    let (selected, rect) = tag_semantics(&driver);
    assert!(!selected, "the tag starts off");

    let (x, y) = (
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0,
    );
    driver.tap_point(x, y)?;
    driver.pump()?;
    assert!(
        driver
            .harness
            .runtime
            .get_app_state::<State>()
            .expect("state")
            .on
    );
    assert!(tag_semantics(&driver).0, "the tag reports that it is on");
    Ok(())
}
