use anyhow::Result;
use fission_core::event::KeyCode;
use fission_core::ui::{Checkbox, Column, Radio, Switch, Widget};
use fission_core::{GlobalState, ReducerContext};
use fission_ir::Role;
use fission_test::{SemanticMatch, TestDriver, TestHarness};

#[derive(Debug, Default, Clone)]
struct State {
    activations: [u32; 6],
}

impl GlobalState for State {}

#[fission_macros::fission_action]
struct Activate(usize);

fn activate(state: &mut State, action: Activate, _ctx: &mut ReducerContext<State>) {
    state.activations[action.0] += 1;
}

#[derive(Clone)]
struct ToggleFixture;

impl From<ToggleFixture> for Widget {
    fn from(_component: ToggleFixture) -> Self {
        let (ctx, _) = fission_core::build::current::<State>();
        let action = |index| {
            ctx.bind(
                Activate(index),
                activate as fn(&mut State, Activate, &mut ReducerContext<State>),
            )
        };

        Column {
            gap: Some(12.0),
            children: vec![
                Checkbox {
                    label: Some("Enabled checkbox".into()),
                    on_toggle: Some(action(0)),
                    ..Default::default()
                }
                .into(),
                Radio {
                    label: Some("Enabled radio".into()),
                    on_select: Some(action(1)),
                    ..Default::default()
                }
                .into(),
                Switch {
                    on_toggle: Some(action(2)),
                    semantics_identifier: Some("enabled-switch".into()),
                    ..Default::default()
                }
                .into(),
                Checkbox {
                    label: Some("Disabled checkbox".into()),
                    on_toggle: Some(action(3)),
                    disabled: true,
                    ..Default::default()
                }
                .into(),
                Radio {
                    label: Some("Disabled radio".into()),
                    on_select: Some(action(4)),
                    disabled: true,
                    ..Default::default()
                }
                .into(),
                Switch {
                    on_toggle: Some(action(5)),
                    semantics_identifier: Some("disabled-switch".into()),
                    disabled: true,
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

fn match_label(driver: &TestDriver<State>, role: Role, label: &str) -> SemanticMatch {
    driver
        .find_role(role)
        .into_iter()
        .find(|candidate| candidate.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("missing {role:?} with label {label:?}"))
}

fn match_identifier(driver: &TestDriver<State>, role: Role, identifier: &str) -> SemanticMatch {
    let ir = driver.harness.last_ir.as_ref().expect("pumped IR");
    driver
        .find_role(role)
        .into_iter()
        .find(|candidate| {
            ir.nodes
                .get(&candidate.node_id)
                .and_then(|node| match &node.op {
                    fission_ir::Op::Semantics(semantics) => semantics.identifier.as_deref(),
                    _ => None,
                })
                == Some(identifier)
        })
        .unwrap_or_else(|| panic!("missing {role:?} with identifier {identifier:?}"))
}

#[test]
fn disabled_toggles_reject_pointer_and_keyboard_activation() -> Result<()> {
    let harness = TestHarness::new(State::default()).with_root_widget(ToggleFixture);
    let mut driver = TestDriver::new(harness);
    driver.set_viewport(480.0, 480.0);
    driver.pump()?;

    let enabled = [
        match_label(&driver, Role::Checkbox, "Enabled checkbox"),
        match_label(&driver, Role::Radio, "Enabled radio"),
        match_identifier(&driver, Role::Switch, "enabled-switch"),
    ];
    let disabled = [
        match_label(&driver, Role::Checkbox, "Disabled checkbox"),
        match_label(&driver, Role::Radio, "Disabled radio"),
        match_identifier(&driver, Role::Switch, "disabled-switch"),
    ];

    for control in enabled.iter().chain(disabled.iter()) {
        driver.tap_point(
            control.bounds.x() + control.bounds.width() / 2.0,
            control.bounds.y() + control.bounds.height() / 2.0,
        )?;
    }
    assert_eq!(
        driver
            .harness
            .runtime
            .get_app_state::<State>()
            .expect("application state")
            .activations,
        [1, 1, 1, 0, 0, 0]
    );

    for (control, key) in enabled
        .iter()
        .zip([KeyCode::Space, KeyCode::Enter, KeyCode::Space])
    {
        driver
            .harness
            .runtime
            .runtime_state
            .interaction
            .set_focused(Some(control.node_id));
        driver.press_key(key, 0)?;
    }
    for (control, key) in disabled
        .iter()
        .zip([KeyCode::Space, KeyCode::Enter, KeyCode::Space])
    {
        // Force focus to prove a disabled control remains inert even if a host
        // presents stale focus from an earlier frame.
        driver
            .harness
            .runtime
            .runtime_state
            .interaction
            .set_focused(Some(control.node_id));
        driver.press_key(key, 0)?;
    }

    assert_eq!(
        driver
            .harness
            .runtime
            .get_app_state::<State>()
            .expect("application state")
            .activations,
        [2, 2, 2, 0, 0, 0]
    );

    Ok(())
}
