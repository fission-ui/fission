use anyhow::Result;
use fission_core::ui::{Text, TextContent, Widget};
use fission_core::{Action, ActionEnvelope, ActionId, GlobalState, WidgetId};
use fission_test::TestHarness;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)] // Added Debug
struct MyGlobalState {
    text: String,
}

impl GlobalState for MyGlobalState {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UpdateTextAction {
    new_text: String,
}

impl Action for UpdateTextAction {
    fn static_id() -> ActionId {
        *UPDATE_TEXT_ACTION_ID
    }
}

lazy_static! {
    static ref UPDATE_TEXT_ACTION_ID: ActionId = ActionId::from_name("test::UpdateTextAction");
}

fn update_text_reducer(
    state: &mut MyGlobalState,
    action: &ActionEnvelope,
    _target: WidgetId,
) -> Result<()> {
    let update_action: UpdateTextAction = serde_json::from_slice(&action.payload)?;
    state.text = update_action.new_text;
    Ok(())
}

#[test]
fn test_end_to_end_flow() {
    let mut harness = TestHarness::new(MyGlobalState::default())
        .with_root_widget(Widget::from(Text {
            content: TextContent::Literal("Initial".into()),
            ..Default::default()
        }))
        .register_reducer(*UPDATE_TEXT_ACTION_ID, update_text_reducer);

    // Initial pump
    harness.pump().expect("Pump failed");

    // Check initial state
    // (In a real test we would inspect the display list or semantics tree)

    // Dispatch action
    harness
        .dispatch(UpdateTextAction {
            new_text: "Updated".into(),
        })
        .expect("Dispatch failed");

    // Pump again to process changes
    harness.pump().expect("Pump failed");
}
