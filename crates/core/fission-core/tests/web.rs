use fission_core::registry::WebRegistration;
use fission_core::Runtime;
use fission_core::WidgetNodeId;

#[test]
fn runtime_syncs_webview_registrations_and_prunes_removed_nodes() {
    let first = WidgetNodeId::explicit("web.first");
    let second = WidgetNodeId::explicit("web.second");
    let mut runtime = Runtime::default();

    runtime.sync_web_nodes(&[
        WebRegistration {
            node_id: first,
            url: "https://first.example".into(),
            user_agent: Some("FissionTest/1".into()),
        },
        WebRegistration {
            node_id: second,
            url: "https://second.example".into(),
            user_agent: None,
        },
    ]);

    assert_eq!(runtime.runtime_state.web.states.len(), 2);
    let first_state = runtime
        .runtime_state
        .web
        .states
        .get(&first)
        .expect("first web state");
    assert_eq!(first_state.url, "https://first.example");
    assert_eq!(first_state.user_agent.as_deref(), Some("FissionTest/1"));
    assert!(first_state.loading);

    runtime.sync_web_nodes(&[WebRegistration {
        node_id: second,
        url: "https://second.example".into(),
        user_agent: Some("FissionTest/2".into()),
    }]);

    assert!(!runtime.runtime_state.web.states.contains_key(&first));
    let second_state = runtime
        .runtime_state
        .web
        .states
        .get(&second)
        .expect("second web state");
    assert_eq!(second_state.user_agent.as_deref(), Some("FissionTest/2"));
}
