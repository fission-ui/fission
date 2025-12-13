use fission_widgets::MyTestAppAction;
use fission_core::{ActionId, Action}; // Import Action trait
use serde_json;

#[test]
fn test_derive_action_id_stability() {
    let action1 = MyTestAppAction { value: 1 };
    let action2 = MyTestAppAction { value: 2 };

    // ActionId should be stable and identical for the same type
    assert_eq!(action1.id(), action2.id());

    // Verify the generated ID is what we expect (based on the full path string)
    // This is dependent on the module path where MyTestAppAction is defined.
    let expected_id = ActionId::from_name("fission_widgets::MyTestAppAction");
    assert_eq!(action1.id(), expected_id);
}

#[test]
fn test_derive_action_serialization() {
    let action = MyTestAppAction { value: 42 };
    let serialized = serde_json::to_string(&action).unwrap();
    let deserialized: MyTestAppAction = serde_json::from_str(&serialized).unwrap();

    assert_eq!(action, deserialized);
}