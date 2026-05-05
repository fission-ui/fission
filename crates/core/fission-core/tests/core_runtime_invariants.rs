use anyhow::Result;
use fission_core::{Action, ActionEnvelope, ActionId, AdvanceTo, AppState, Runtime, Tick};
use fission_ir::NodeId;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

// --- Test AppState --- //
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterState {
    pub count: i32,
}

impl AppState for CounterState {}

// --- Test Action --- //
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Increment {
    pub by: i32,
}

impl Action for Increment {
    fn static_id() -> ActionId {
        *INCREMENT_ACTION_ID
    }
}

lazy_static! {
    static ref INCREMENT_ACTION_ID: ActionId = ActionId::from_name("test_app::Increment");
}

// --- Test Reducer --- //
fn counter_reducer(
    state: &mut CounterState,
    action: &ActionEnvelope,
    _target: NodeId,
) -> Result<()> {
    // Deserialize payload
    let inc_action: Increment = serde_json::from_slice(&action.payload).unwrap();
    state.count += inc_action.by;
    Ok(())
}

#[test]
fn test_runtime_init_and_clock_default() {
    let runtime = Runtime::default();
    assert_eq!(runtime.clock().current_time(), 0);
}

#[test]
fn test_add_get_app_state() -> Result<()> {
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(CounterState::default()))?;

    let counter = runtime
        .get_app_state::<CounterState>()
        .expect("CounterState should be present");
    assert_eq!(counter.count, 0);

    Ok(())
}

#[test]
fn test_dispatch_custom_action_and_state_update() -> Result<()> {
    let mut runtime = Runtime::default();
    runtime.add_app_state(Box::new(CounterState::default()))?;
    runtime.register_reducer::<CounterState>(*INCREMENT_ACTION_ID, counter_reducer)?;

    let target_node_id = NodeId::derived(0, &[1]);
    let action_struct = Increment { by: 5 };
    let envelope: ActionEnvelope = action_struct.into();

    runtime.dispatch(envelope, target_node_id)?;

    let counter = runtime
        .get_app_state::<CounterState>()
        .expect("CounterState should be updated");
    assert_eq!(counter.count, 5);

    Ok(())
}

#[test]
fn test_dispatch_tick_action() -> Result<()> {
    let mut runtime = Runtime::default();
    assert_eq!(runtime.clock().current_time(), 0);

    let action = Tick { dt: 100 };
    runtime.dispatch(action.into(), NodeId::derived(0, &[0]))?;
    assert_eq!(runtime.clock().current_time(), 100);

    Ok(())
}

#[test]
fn test_clock_cannot_go_backward() -> Result<()> {
    let mut runtime = Runtime::default();
    runtime.dispatch(AdvanceTo { time: 500 }.into(), NodeId::derived(0, &[0]))?;
    assert_eq!(runtime.clock().current_time(), 500);

    let res = runtime.dispatch(AdvanceTo { time: 400 }.into(), NodeId::derived(0, &[0]));
    assert!(res.is_err());

    Ok(())
}
