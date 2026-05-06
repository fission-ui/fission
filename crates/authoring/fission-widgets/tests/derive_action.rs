use fission_core::{Action, ActionId};
use fission_macros::Action;
use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MyTestAppAction {
    pub value: u32,
}

#[test]
fn test_derive_action_id_stability() {
    let _action1 = MyTestAppAction { value: 1 };
    let _action2 = MyTestAppAction { value: 2 };

    // ID should be static
    assert_eq!(MyTestAppAction::static_id(), MyTestAppAction::static_id());

    let expected_id = ActionId::from_name("derive_action::MyTestAppAction");
    assert_eq!(MyTestAppAction::static_id(), expected_id);
}

#[test]
fn test_derive_action_serialization() {
    let action = MyTestAppAction { value: 42 };
    let serialized = serde_json::to_string(&action).unwrap();
    let deserialized: MyTestAppAction = serde_json::from_str(&serialized).unwrap();

    assert_eq!(action, deserialized);
}
