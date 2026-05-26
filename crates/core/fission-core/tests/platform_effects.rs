use fission_core::{
    ActionRegistry, AppState, CapabilityInvocationPayload, DeepLink, DeepLinkConfig,
    DeepLinkReceived, Effect, Effects, NotificationId, NotificationRequest, NotificationResponse,
    NotificationResponseReceived, SHOW_NOTIFICATION,
};

#[derive(Debug, Default)]
struct TestState;
impl AppState for TestState {}

#[test]
fn notification_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(42, &mut registry);

    effects.notifications().show(NotificationRequest {
        id: NotificationId::new("n1"),
        title: "Title".into(),
        body: "Body".into(),
        ..Default::default()
    });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 42);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected notification capability effect");
    };
    assert_eq!(op.capability_name, SHOW_NOTIFICATION.name);
    let decoded: NotificationRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.id, NotificationId::new("n1"));
}

#[test]
fn deep_link_config_and_inbound_actions_are_public_api() {
    let config = DeepLinkConfig::new()
        .scheme("fission")
        .domain("example.com");
    assert!(config.matches("fission://open/tasks/1"));
    assert!(config.matches("https://example.com/tasks/1"));

    let link_action = DeepLinkReceived {
        link: DeepLink::new("fission://open/tasks/1").cold_start(true),
    };
    let _: fission_core::ActionEnvelope = link_action.into();

    let response_action = NotificationResponseReceived {
        response: NotificationResponse {
            notification_id: NotificationId::new("n1"),
            action_id: Some("open".into()),
            deep_link: Some("fission://open/tasks/1".into()),
            user_text: None,
        },
    };
    let _: fission_core::ActionEnvelope = response_action.into();
}
