use super::{
    Bounds, LiveTestClient, SelectorQuery, SemanticNode, TestCommand, TestPointerKind,
    TestPointerPhase, TestResponse, TestScrollDeltaMode, VisibilityState,
};
use std::io::{Read, Write};
use std::net::TcpListener;

#[test]
fn selector_candidate_suppression_is_opt_in_and_recursive() {
    let default = SelectorQuery::label("Save");
    let default_wire = serde_json::to_value(&default).unwrap();
    assert!(default_wire.get("include_candidates").is_none());

    let quiet = SelectorQuery::label("Save")
        .scoped(SelectorQuery::semantic_identifier("dialog"))
        .without_candidates();
    let quiet_wire = serde_json::to_value(&quiet).unwrap();
    assert_eq!(quiet_wire["include_candidates"], false);
    assert_eq!(quiet_wire["scope"]["include_candidates"], false);

    let diagnostic = quiet.with_candidates();
    let diagnostic_wire = serde_json::to_value(&diagnostic).unwrap();
    assert!(diagnostic_wire.get("include_candidates").is_none());
    assert!(diagnostic_wire["scope"].get("include_candidates").is_none());

    let older_wire = serde_json::to_value(default).unwrap();
    let decoded: SelectorQuery = serde_json::from_value(older_wire).unwrap();
    assert!(decoded.include_candidates);
}

#[test]
fn wait_and_pump_operations_propagate_test_host_errors() {
    let error = super::ensure_success_response(
        "wait_for_idle",
        TestResponse::Error {
            message: "finite motion did not settle".into(),
        },
    )
    .expect_err("a host error must be visible to callers");
    assert!(error
        .to_string()
        .contains("wait_for_idle failed: finite motion did not settle"));

    let unexpected = super::ensure_success_response(
        "pump",
        TestResponse::MotionStatus {
            finite: 1,
            repeating: 0,
            ripples: 0,
        },
    )
    .expect_err("unexpected responses must not look successful");
    assert!(unexpected
        .to_string()
        .contains("unexpected response from pump"));
}

#[test]
fn activate_selector_propagates_error_and_unexpected_action_responses() {
    for (action_response, expected_message) in [
        (
            TestResponse::Error {
                message: "selector action was rejected".into(),
            },
            "activate_selector failed: selector action was rejected",
        ),
        (
            TestResponse::MotionStatus {
                finite: 1,
                repeating: 0,
                ripples: 0,
            },
            "unexpected response from activate_selector",
        ),
    ] {
        let mut commands = Vec::new();
        let error = super::activate_selector_with(
            SelectorQuery::semantic_identifier("sheet.tab.2"),
            |command| match command {
                TestCommand::Pump {} => {
                    commands.push("pump");
                    Ok(TestResponse::Ok {})
                }
                TestCommand::ActivateSelector { .. } => {
                    commands.push("activate");
                    Ok(action_response.clone())
                }
                other => panic!("unexpected activation command: {other:?}"),
            },
        )
        .expect_err("activation failures must reach the caller");

        assert!(error.to_string().contains(expected_message));
        assert_eq!(commands, ["pump", "activate"]);
    }
}

fn populated_semantic_node() -> SemanticNode {
    SemanticNode {
        identifier: Some("test.field".into()),
        widget_id: "widget-1".into(),
        stable_node_id: "node-1".into(),
        parent: Some("node-0".into()),
        children: vec!["node-2".into()],
        role: "ComboBox".into(),
        label: Some("Field".into()),
        value: Some("Choice".into()),
        value_present: true,
        focusable: true,
        sequential_focusable: false,
        text_editable: true,
        disabled: false,
        read_only: false,
        checked: None,
        selected: Some(true),
        expanded: Some(true),
        has_popup: Some("ListBox".into()),
        orientation: Some("Vertical".into()),
        modal: true,
        required: true,
        invalid: true,
        controls: vec!["node-3".into()],
        labelled_by: vec!["node-4".into()],
        described_by: vec!["node-5".into()],
        active_descendant: Some("node-6".into()),
        actions: vec!["Dismiss".into()],
        text_selection: None,
        masked: false,
        scrollable_x: false,
        scrollable_y: true,
        logical_bounds: Bounds {
            x: 1.0,
            y: 2.0,
            width: 100.0,
            height: 40.0,
        },
        visible_bounds: Some(Bounds {
            x: 1.0,
            y: 2.0,
            width: 100.0,
            height: 40.0,
        }),
        visibility: VisibilityState::FullyVisible,
        x: 1.0,
        y: 2.0,
        width: 100.0,
        height: 40.0,
    }
}

#[test]
fn get_tree_streams_large_http_response_without_string_size_limit() {
    let mut node = populated_semantic_node();
    node.label = Some("x".repeat(11 * 1024 * 1024));
    let encoded = serde_json::to_vec(&TestResponse::Tree { nodes: vec![node] }).unwrap();
    assert!(encoded.len() > 10 * 1024 * 1024);

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request_headers = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            stream.read_exact(&mut byte).unwrap();
            request_headers.push(byte[0]);
            if request_headers.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let request_body_length = String::from_utf8(request_headers)
            .unwrap()
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then_some(value.trim())
            })
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let mut request_body = vec![0u8; request_body_length];
        stream.read_exact(&mut request_body).unwrap();

        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            encoded.len()
        )
        .unwrap();
        stream.write_all(&encoded).unwrap();
    });

    let nodes = LiveTestClient::connect(port).get_tree().unwrap();
    server.join().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(
        nodes[0].label.as_deref().map(str::len),
        Some(11 * 1024 * 1024)
    );
}

#[test]
fn extended_semantic_node_fields_round_trip() {
    let encoded = serde_json::to_string(&populated_semantic_node()).unwrap();
    let decoded: SemanticNode = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.selected, Some(true));
    assert_eq!(decoded.expanded, Some(true));
    assert_eq!(decoded.has_popup.as_deref(), Some("ListBox"));
    assert_eq!(decoded.orientation.as_deref(), Some("Vertical"));
    assert!(decoded.modal);
    assert!(decoded.required);
    assert!(decoded.invalid);
    assert_eq!(decoded.controls, vec!["node-3".to_string()]);
    assert_eq!(decoded.labelled_by, vec!["node-4".to_string()]);
    assert_eq!(decoded.described_by, vec!["node-5".to_string()]);
    assert_eq!(decoded.active_descendant.as_deref(), Some("node-6"));
    assert!(!decoded.sequential_focusable);
    assert!(decoded.text_editable);
}

#[test]
fn extended_semantic_node_fields_default_for_older_wire_payloads() {
    let added_fields = [
        "sequential_focusable",
        "text_editable",
        "selected",
        "expanded",
        "has_popup",
        "orientation",
        "modal",
        "required",
        "invalid",
        "controls",
        "labelled_by",
        "described_by",
        "active_descendant",
    ];
    let mut encoded = serde_json::to_value(populated_semantic_node()).unwrap();
    let object = encoded.as_object_mut().unwrap();
    for field in added_fields {
        assert!(object.remove(field).is_some(), "missing test field {field}");
    }

    let decoded: SemanticNode = serde_json::from_value(encoded).unwrap();
    assert!(decoded.sequential_focusable);
    assert!(decoded.is_sequentially_focusable());
    assert!(!decoded.text_editable);
    assert!(!decoded.supports_text_editing());
    assert_eq!(decoded.selected, None);
    assert_eq!(decoded.expanded, None);
    assert_eq!(decoded.has_popup, None);
    assert_eq!(decoded.orientation, None);
    assert!(!decoded.modal);
    assert!(!decoded.required);
    assert!(!decoded.invalid);
    assert!(decoded.controls.is_empty());
    assert!(decoded.labelled_by.is_empty());
    assert!(decoded.described_by.is_empty());
    assert_eq!(decoded.active_descendant, None);

    let mut nonfocusable = populated_semantic_node();
    nonfocusable.focusable = false;
    nonfocusable.text_editable = false;
    nonfocusable.role = "TextInput".into();
    let mut encoded = serde_json::to_value(nonfocusable).unwrap();
    let object = encoded.as_object_mut().unwrap();
    object.remove("sequential_focusable");
    object.remove("text_editable");
    let decoded: SemanticNode = serde_json::from_value(encoded).unwrap();
    assert!(decoded.sequential_focusable);
    assert!(!decoded.is_sequentially_focusable());
    assert!(decoded.supports_text_editing());
}

#[test]
fn deterministic_motion_commands_have_stable_wire_shapes() {
    assert_eq!(
        serde_json::to_value(TestCommand::PauseAnimations {}).expect("serialize pause"),
        serde_json::json!({ "cmd": "PauseAnimations" })
    );
    assert_eq!(
        serde_json::to_value(TestCommand::AdvanceClock { ms: 160 })
            .expect("serialize clock advance"),
        serde_json::json!({ "cmd": "AdvanceClock", "ms": 160 })
    );
    assert_eq!(
        serde_json::to_value(TestCommand::WaitForIdle {
            timeout_ms: 2_000,
            ignore_repeating_motion: true,
        })
        .expect("serialize idle wait"),
        serde_json::json!({
            "cmd": "WaitForIdle",
            "timeout_ms": 2_000,
            "ignore_repeating_motion": true
        })
    );
}

#[test]
fn multi_pointer_commands_have_explicit_wire_metadata() {
    let pointer = TestCommand::PointerDown {
        pointer_id: 7,
        kind: TestPointerKind::Touch,
        x: 12.0,
        y: 24.0,
        modifiers: 0,
    };
    assert_eq!(
        serde_json::to_value(pointer).unwrap(),
        serde_json::json!({
            "cmd": "PointerDown",
            "pointer_id": 7,
            "kind": "touch",
            "x": 12.0,
            "y": 24.0,
            "modifiers": 0
        })
    );

    let scroll = TestCommand::PointerScroll {
        x: 5.0,
        y: 6.0,
        dx: 1.0,
        dy: -2.0,
        delta_mode: TestScrollDeltaMode::Pixel,
        phase: TestPointerPhase::Moved,
        modifiers: 4,
    };
    let value = serde_json::to_value(scroll).unwrap();
    assert_eq!(value["delta_mode"], "pixel");
    assert_eq!(value["phase"], "moved");
    assert_eq!(value["modifiers"], 4);
}
