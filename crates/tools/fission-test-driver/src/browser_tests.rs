use super::*;
use crate::{Bounds, SemanticNode};

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn live_test_client_remains_send_and_sync_with_scoped_browser_deadlines() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<crate::LiveTestClient>();
}

#[test]
fn bridge_poll_scope_uses_bridge_deadline_and_restores_after_error() {
    let started = Instant::now();
    let bridge_deadline = started + Duration::from_secs(30);
    let operation_deadline = CdpOperationDeadline::default();

    let default_deadline = operation_deadline.current();
    assert!(default_deadline > started + Duration::from_secs(14));
    assert!(default_deadline < started + Duration::from_secs(16));

    let poll_result: Result<()> = (|| {
        let _bridge_scope = operation_deadline.scoped(bridge_deadline);
        assert_eq!(operation_deadline.current(), bridge_deadline);

        let reattach_probe_deadline = started + Duration::from_secs(5);
        {
            let _probe_scope = operation_deadline.scoped(reattach_probe_deadline);
            assert_eq!(operation_deadline.current(), reattach_probe_deadline);
        }
        assert_eq!(operation_deadline.current(), bridge_deadline);

        let _later_nested_scope = operation_deadline.scoped(started + Duration::from_secs(60));
        assert_eq!(operation_deadline.current(), bridge_deadline);
        Err(anyhow!("simulated Runtime.evaluate poll failure"))
    })();

    assert!(poll_result.is_err());
    let restored_deadline = operation_deadline.current();
    assert!(restored_deadline > started + Duration::from_secs(14));
    assert!(restored_deadline < started + Duration::from_secs(16));
}

#[test]
fn web_text_input_trace_keeps_bridge_and_fill_stages_disjoint() {
    assert!(is_web_text_input_trace_stage(410));
    assert!(is_web_text_input_trace_stage(435));
    assert!(!is_web_text_input_trace_stage(436));
    assert!(!is_web_text_input_trace_stage(449));
    assert!(is_web_text_input_trace_stage(450));
    assert!(is_web_text_input_trace_stage(466));
    assert!(is_web_text_input_trace_stage(467));
    assert!(is_web_text_input_trace_stage(470));
    assert!(!is_web_text_input_trace_stage(471));
    assert!(is_web_text_input_trace_stage(473));
    assert!(is_web_text_input_trace_stage(476));
}

#[test]
fn webgpu_queue_trace_covers_pending_age_and_submission_markers() {
    assert!(is_webgpu_queue_trace_stage(552));
    assert!(is_webgpu_queue_trace_stage(558));
    assert!(is_webgpu_queue_trace_stage(559));
    assert!(is_webgpu_queue_trace_stage(560));
    assert!(is_webgpu_queue_trace_stage(571));
    assert!(is_webgpu_queue_trace_stage(572));
    assert!(!is_webgpu_queue_trace_stage(551));
    assert!(!is_webgpu_queue_trace_stage(573));
}

#[test]
fn diagnostic_browser_hold_is_opt_in_and_bounded() {
    assert_eq!(diagnostic_hold_after_error_ms(None), 0);
    assert_eq!(diagnostic_hold_after_error_ms(Some("")), 0);
    assert_eq!(diagnostic_hold_after_error_ms(Some("invalid")), 0);
    assert_eq!(diagnostic_hold_after_error_ms(Some("0")), 0);
    assert_eq!(diagnostic_hold_after_error_ms(Some("15000")), 15_000);
    assert_eq!(diagnostic_hold_after_error_ms(Some("60000")), 60_000);
    assert_eq!(diagnostic_hold_after_error_ms(Some("60001")), 60_000);
}

#[test]
fn reduced_motion_is_an_explicit_browser_option() {
    let standard = BrowserTestOptions::new("http://127.0.0.1/");
    assert!(!standard.prefers_reduced_motion);

    let reduced = standard.reduced_motion();
    assert!(reduced.prefers_reduced_motion);
}

#[test]
fn browser_wait_for_idle_pumps_before_reading_motion_status() {
    let mut finite_motion = 1;
    let mut commands = Vec::new();
    let response = wait_for_idle_with(
        1_000,
        true,
        |command| match command {
            TestCommand::Pump {} => {
                commands.push("pump");
                finite_motion = 0;
                Ok(TestResponse::Ok {})
            }
            TestCommand::WaitForIdle {
                timeout_ms,
                ignore_repeating_motion,
            } => {
                commands.push("status");
                assert_eq!(timeout_ms, 0);
                assert!(ignore_repeating_motion);
                Ok(TestResponse::MotionStatus {
                    finite: finite_motion,
                    repeating: 1,
                    ripples: 0,
                })
            }
            _ => panic!("unexpected browser test command: {command:?}"),
        },
        |_| {},
    )
    .expect("wait for idle protocol should succeed");

    assert!(matches!(response, TestResponse::Ok {}));
    assert_eq!(commands, ["pump", "status"]);
}

#[test]
fn browser_wait_for_idle_rejects_unexpected_pump_response() {
    let response = wait_for_idle_with(
        1_000,
        true,
        |_| {
            Ok(TestResponse::MotionStatus {
                finite: 0,
                repeating: 0,
                ripples: 0,
            })
        },
        |_| {},
    )
    .expect("protocol response should decode");

    assert!(matches!(
        response,
        TestResponse::Error { message } if message.contains("unexpected response from browser pump")
    ));
}

#[test]
fn chrome_webgpu_profile_selects_swiftshader_vulkan_only_when_requested() {
    assert!(chrome_webgpu_profile_args(None).unwrap().is_empty());
    assert!(chrome_webgpu_profile_args(Some("default"))
        .unwrap()
        .is_empty());
    assert_eq!(
        chrome_webgpu_profile_args(Some("swiftshader-vulkan")).unwrap(),
        [
            "--use-angle=swiftshader",
            "--enable-unsafe-swiftshader",
            "--use-vulkan=swiftshader",
            "--enable-features=Vulkan",
        ]
    );
    assert!(chrome_webgpu_profile_args(Some("unknown")).is_err());
}

#[test]
fn browser_bridge_decodes_both_value_and_json_string_responses() {
    assert!(matches!(
        decode_bridge_response(json!({"status":"Ok"})).unwrap(),
        TestResponse::Ok {}
    ));

    let encoded_tree = serde_json::to_string(&json!({
        "status": "Tree",
        "nodes": []
    }))
    .unwrap();
    assert!(matches!(
        decode_bridge_response(Value::String(encoded_tree)).unwrap(),
        TestResponse::Tree { nodes } if nodes.is_empty()
    ));
}

#[test]
fn browser_logs_ignore_only_the_root_network_favicon() {
    assert!(ignorable_browser_log(
        "network",
        "http://localhost:8099/favicon.ico",
        "Failed to load resource: status 404"
    ));
    assert!(ignorable_browser_log(
        "network",
        "https://example.com/favicon.ico",
        "Failed to load resource"
    ));
    for url in [
        "http://localhost:8099/assets/favicon.ico",
        "http://localhost:8099/app.js",
        "file:///favicon.ico",
    ] {
        assert!(!ignorable_browser_log(
            "network",
            url,
            "Failed to load resource: status 404"
        ));
    }
    assert!(!ignorable_browser_log(
        "javascript",
        "http://localhost:8099/favicon.ico",
        "application error"
    ));
}

fn resolved_node(visibility: VisibilityState, disabled: bool, value: Option<&str>) -> TestResponse {
    TestResponse::SelectorResolved {
        node: SemanticNode {
            identifier: Some("test.field".into()),
            widget_id: "1".into(),
            stable_node_id: "node-1".into(),
            parent: None,
            children: Vec::new(),
            role: "text_input".into(),
            label: Some("Field".into()),
            value: value.map(str::to_owned),
            value_present: value.is_some(),
            focusable: true,
            sequential_focusable: true,
            text_editable: true,
            disabled,
            read_only: false,
            checked: None,
            selected: None,
            expanded: None,
            has_popup: None,
            orientation: None,
            modal: false,
            required: false,
            invalid: false,
            controls: Vec::new(),
            labelled_by: Vec::new(),
            described_by: Vec::new(),
            active_descendant: None,
            actions: vec!["focus".into()],
            text_selection: None,
            masked: false,
            scrollable_x: false,
            scrollable_y: false,
            logical_bounds: Bounds {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 40.0,
            },
            visible_bounds: Some(Bounds {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 40.0,
            }),
            visibility,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 40.0,
        },
    }
}

#[test]
fn browser_selector_waits_use_native_semantics() {
    let visible = resolved_node(VisibilityState::FullyVisible, false, Some("ready"));
    assert!(selector_wait_matches(
        &SelectorWaitCondition::Present,
        &visible
    ));
    assert!(selector_wait_matches(
        &SelectorWaitCondition::Visible,
        &visible
    ));
    assert!(selector_wait_matches(
        &SelectorWaitCondition::Enabled,
        &visible
    ));
    assert!(selector_wait_matches(
        &SelectorWaitCondition::Value("ready".into()),
        &visible
    ));
    assert!(!selector_wait_matches(
        &SelectorWaitCondition::Value("pending".into()),
        &visible
    ));

    let disabled = resolved_node(VisibilityState::PartiallyVisible, true, None);
    assert!(selector_wait_matches(
        &SelectorWaitCondition::Disabled,
        &disabled
    ));

    let gone = TestResponse::SelectorError {
        failure: SelectorFailure {
            kind: SelectorFailureKind::NoMatch,
            selector: SelectorQuery::label("missing"),
            candidates: Vec::new(),
            message: "not found".into(),
        },
    };
    assert!(selector_wait_matches(&SelectorWaitCondition::Gone, &gone));
}

#[test]
fn browser_status_requires_explicit_live_bridge_signal() {
    let status: RuntimeStatus = serde_json::from_value(serde_json::json!({
        "ready_dom": true,
        "ready_canvas": true,
        "title": "Fission",
        "width": 1280,
        "height": 900,
        "body_text_len": 0,
        "renderer": "webgpu-vello",
        "test_bridge_ready": true
    }))
    .expect("decode browser status");

    assert!(status.test_bridge_ready);
    assert_eq!(status.renderer.as_deref(), Some("webgpu-vello"));
}

#[test]
fn only_transient_missing_execution_context_errors_are_retried() {
    for message in [
        "Runtime.evaluate: {\"code\":-32000,\"message\":\"Cannot find default execution context\"}",
        "Cannot find context with specified id",
        "Execution context was destroyed, most likely because of a navigation.",
        "No default execution context found",
    ] {
        assert!(
            is_transient_execution_context_error(&anyhow!(message)),
            "expected transient context error: {message}"
        );
    }

    for message in [
        "Runtime.evaluate: target closed",
        "CDP command timed out: Runtime.evaluate",
        "browser runtime evaluation failed: TypeError: app crashed",
    ] {
        assert!(
            !is_transient_execution_context_error(&anyhow!(message)),
            "unexpectedly treated permanent error as transient: {message}"
        );
    }
}

#[test]
fn runtime_evaluate_timeout_is_transient_during_bridge_reattachment() {
    assert!(is_transient_bridge_readiness_error(&anyhow!(
        "CDP command timed out: Runtime.evaluate"
    )));
    assert!(is_transient_bridge_readiness_error(&anyhow!(
        "Execution context was destroyed, most likely because of a navigation."
    )));
    assert!(!is_transient_bridge_readiness_error(&anyhow!(
        "Runtime.evaluate: target closed"
    )));
}

#[test]
fn missing_bridge_accessors_are_navigation_transients() {
    for accessor in ["submit", "poll"] {
        let error = anyhow!(format!(
            "browser runtime evaluation failed: TypeError: Cannot read properties of undefined (reading '{accessor}')"
        ));
        assert!(is_transient_bridge_context_error(&error));
    }
    assert!(!is_transient_bridge_context_error(&anyhow!(
        "browser runtime evaluation failed: TypeError: application state is invalid"
    )));
}

#[test]
fn only_read_only_bridge_commands_retry_after_navigation() {
    assert!(bridge_command_is_safe_to_retry(
        &TestCommand::ResolveSelector {
            query: SelectorQuery::label("Increment"),
        }
    ));
    assert!(bridge_command_is_safe_to_retry(&TestCommand::GetText {}));
    assert!(bridge_command_is_safe_to_retry(&TestCommand::GetTree {}));
    assert!(!bridge_command_is_safe_to_retry(
        &TestCommand::TapSelector {
            query: SelectorQuery::label("Increment"),
        }
    ));
    assert!(!bridge_command_is_safe_to_retry(&TestCommand::FillText {
        query: SelectorQuery::label("Input"),
        text: "value".into(),
    }));
}

#[test]
fn a_canvas_and_bridge_without_a_submitted_frame_are_not_ready() {
    let mut status: RuntimeStatus = serde_json::from_value(json!({
        "ready_dom": false, "ready_canvas": true, "title": "Fission", "width": 1280, "height": 900,
        "body_text_len": 0, "renderer": "webgpu-vello", "test_bridge_ready": true
    }))
    .unwrap();
    assert!(!browser_is_ready(
        &status,
        BrowserSmokeMode::FissionCanvas,
        true
    ));
    status.rendered_frames = 1;
    assert!(browser_is_ready(
        &status,
        BrowserSmokeMode::FissionCanvas,
        true
    ));
    status.test_bridge_ready = false;
    assert!(!browser_is_ready(
        &status,
        BrowserSmokeMode::FissionCanvas,
        true
    ));
    assert!(browser_is_ready(
        &status,
        BrowserSmokeMode::FissionCanvas,
        false
    ));
}

#[test]
fn browser_key_events_translate_fission_modifiers_to_cdp() {
    assert_eq!(cdp_modifiers(1), 8); // Shift
    assert_eq!(cdp_modifiers(2), 1); // Alt
    assert_eq!(cdp_modifiers(4), 2); // Control
    assert_eq!(cdp_modifiers(8), 4); // Meta
    assert_eq!(cdp_modifiers(1 | 4 | 8), 8 | 2 | 4);
}

#[test]
fn browser_primary_shortcuts_request_native_editing_commands() {
    assert_eq!(cdp_editing_commands("a", 4), vec!["selectAll"]);
    assert_eq!(cdp_editing_commands("C", 8), vec!["copy"]);
    assert_eq!(cdp_editing_commands("v", 4), vec!["paste"]);
    assert_eq!(cdp_editing_commands("x", 4), vec!["cut"]);
    assert!(cdp_editing_commands("v", 4 | 2).is_empty());
    assert!(cdp_editing_commands("v", 0).is_empty());
}

#[test]
fn browser_key_events_use_dom_key_names() {
    assert_eq!(
        cdp_key_description("Left", 0),
        CdpKeyDescription {
            key: "ArrowLeft".into(),
            code: "ArrowLeft".into(),
            text: String::new(),
            key_code: 37,
            location: 0,
        }
    );
    assert_eq!(
        cdp_key_description("Enter", 0),
        CdpKeyDescription {
            key: "Enter".into(),
            code: "Enter".into(),
            text: "\r".into(),
            key_code: 13,
            location: 0,
        }
    );
    assert_eq!(
        cdp_key_description("a", 0),
        CdpKeyDescription {
            key: "a".into(),
            code: "KeyA".into(),
            text: "a".into(),
            key_code: 65,
            location: 0,
        }
    );
    assert_eq!(cdp_key_description("7", 4).text, "");
}
