use super::*;

#[test]
fn report_summary_reflects_selected_order() {
    let state = FieldInspectorState::default();
    let summary = state.report_summary();
    assert!(summary.contains("WO-1048"));
    assert!(summary.contains("CMP-7A-2219"));
}

#[test]
fn selecting_order_resets_inspection_state() {
    let mut state = FieldInspectorState::default();
    state.complete_check("identity");
    state.report_submitted = true;
    state.reset_for_order("WO-1052".to_string());
    assert_eq!(state.selected_order().id, "WO-1052");
    assert!(state.completed_checklist.is_empty());
    assert!(!state.report_submitted);
}

#[test]
fn nfc_uri_extracts_portable_uri_record() {
    let tag = NfcTag {
        records: vec![NfcRecord::uri("fission://asset/CMP-7A-2219")],
        ..Default::default()
    };
    assert_eq!(
        nfc_uri_for_display(&tag).as_deref(),
        Some("fission://asset/CMP-7A-2219")
    );
}

#[test]
fn unsupported_availability_is_not_reported_as_ready() {
    let mut state = FieldInspectorState::default();
    state.notification_settings = Some(NotificationSettings {
        permission: NotificationPermission::Unsupported,
        ..Default::default()
    });
    state.camera_availability = Some(CameraAvailability {
        permission: CameraPermission::Denied,
        devices: Vec::new(),
    });
    state.nfc_availability = Some(NfcAvailability::default());
    state.bluetooth_availability = Some(BluetoothAvailability {
        permission: BluetoothPermission::Denied,
        enabled: false,
        supports_classic: false,
        supports_low_energy: false,
    });

    let lines = state.capability_lines();
    for title in ["Notifications", "Camera and flashlight", "NFC", "Bluetooth"] {
        let line = lines
            .iter()
            .find(|line| line.title == title)
            .expect("capability line exists");
        assert_eq!(line.state, CapabilityState::Unavailable, "{title}");
    }
}

#[test]
fn camera_permission_success_updates_existing_availability() {
    let mut runtime = fission::core::Runtime::default();
    runtime
        .add_app_state(Box::new(FieldInspectorState {
            camera_availability: Some(CameraAvailability {
                permission: CameraPermission::Unknown,
                devices: vec![CameraDevice {
                    id: "back".into(),
                    label: Some("Back camera".into()),
                    facing: CameraFacing::Back,
                    has_flashlight: true,
                }],
            }),
            ..Default::default()
        }))
        .unwrap();
    let mut registry = fission::core::registry::ActionRegistry::new();
    registry.register(reduce_with!(on_capability_succeeded));
    runtime.absorb_registry(registry);

    runtime
        .dispatch_with_input(
            CapabilitySucceeded.into(),
            fission::WidgetId::explicit("test_root"),
            &ActionInput::CapabilityOk {
                capability: REQUEST_CAMERA_PERMISSION.name.into(),
                req_id: 1,
                payload: serde_json::to_vec(&CameraPermission::Granted).unwrap(),
            },
        )
        .unwrap();

    let state = runtime.get_app_state::<FieldInspectorState>().unwrap();
    let availability = state.camera_availability.as_ref().unwrap();
    assert_eq!(availability.permission, CameraPermission::Granted);
    assert_eq!(availability.devices.len(), 1);
    assert_eq!(state.logs[0].title, "Camera permission");
    assert_eq!(state.logs[0].state, CapabilityState::Ready);
}

#[test]
fn photo_capture_detail_reports_real_image_payload() {
    let mut runtime = fission::core::Runtime::default();
    runtime
        .add_app_state(Box::new(FieldInspectorState::default()))
        .unwrap();
    let mut registry = fission::core::registry::ActionRegistry::new();
    registry.register(reduce_with!(on_capability_succeeded));
    runtime.absorb_registry(registry);

    runtime
        .dispatch_with_input(
            CapabilitySucceeded.into(),
            fission::WidgetId::explicit("test_root"),
            &ActionInput::CapabilityOk {
                capability: CAPTURE_PHOTO.name.into(),
                req_id: 1,
                payload: serde_json::to_vec(&CameraCapture {
                    stream: DataStreamId(7),
                    byte_len: Some(2049),
                    content_type: "image/jpeg".into(),
                    width: 100,
                    height: 50,
                    camera_id: Some("back".into()),
                })
                .unwrap(),
            },
        )
        .unwrap();

    let state = runtime.get_app_state::<FieldInspectorState>().unwrap();
    let camera_line = state
        .capability_lines()
        .into_iter()
        .find(|line| line.title == "Camera and flashlight")
        .unwrap();
    assert!(camera_line.detail.contains("100x50 image/jpeg"));
    assert!(camera_line.detail.contains("3 KiB"));
    assert_eq!(camera_line.state, CapabilityState::Complete);
    assert!(state.completed_checklist.contains("evidence"));
}

#[test]
fn passkey_registration_result_is_reused_for_authentication() {
    let mut runtime = fission::core::Runtime::default();
    runtime
        .add_app_state(Box::new(FieldInspectorState::default()))
        .unwrap();
    let mut registry = fission::core::registry::ActionRegistry::new();
    registry.register(reduce_with!(on_capability_succeeded));
    registry.register(reduce_with!(on_authenticate_passkey));
    runtime.absorb_registry(registry);

    runtime
        .dispatch_with_input(
            CapabilitySucceeded.into(),
            fission::WidgetId::explicit("test_root"),
            &ActionInput::CapabilityOk {
                capability: REGISTER_PASSKEY.name.into(),
                req_id: 1,
                payload: serde_json::to_vec(&PasskeyRegistrationResult {
                    credential_id: vec![1, 2, 3, 4],
                    raw_id: vec![1, 2, 3, 4],
                    client_data_json: Vec::new(),
                    attestation_object: Vec::new(),
                    authenticator_attachment: Some(PasskeyAuthenticatorAttachment::Platform),
                    transports: vec![PasskeyTransport::Internal],
                })
                .unwrap(),
            },
        )
        .unwrap();

    runtime
        .dispatch(
            AuthenticatePasskey.into(),
            fission::WidgetId::explicit("test_root"),
        )
        .unwrap();

    let state = runtime.get_app_state::<FieldInspectorState>().unwrap();
    assert_eq!(
        state.registered_passkey.as_ref().unwrap().id,
        vec![1, 2, 3, 4]
    );
    assert!(runtime.pending_effects.iter().any(|effect| {
        match &effect.effect {
            fission::core::Effect::Capability(
                fission::core::CapabilityInvocationPayload::Operation(operation),
            ) if operation.capability_name == AUTHENTICATE_PASSKEY.name => {
                let request: PasskeyAuthenticationRequest =
                    serde_json::from_slice(&operation.request).unwrap();
                request.allow_credentials.len() == 1
                    && request.allow_credentials[0].id == vec![1, 2, 3, 4]
            }
            _ => false,
        }
    }));
}

#[test]
fn microphone_error_payload_is_shown_in_activity_log() {
    let mut runtime = fission::core::Runtime::default();
    runtime
        .add_app_state(Box::new(FieldInspectorState::default()))
        .unwrap();
    let mut registry = fission::core::registry::ActionRegistry::new();
    registry.register(reduce_with!(on_capability_failed));
    runtime.absorb_registry(registry);

    runtime
        .dispatch_with_input(
            CapabilityFailed.into(),
            fission::WidgetId::explicit("test_root"),
            &ActionInput::CapabilityErr {
                capability: CAPTURE_MICROPHONE_AUDIO.name.into(),
                req_id: 1,
                payload: Some(
                    serde_json::to_vec(&MicrophoneError::new(
                        "permission_denied",
                        "iOS microphone permission is not granted",
                    ))
                    .unwrap(),
                ),
                message: None,
            },
        )
        .unwrap();

    let state = runtime.get_app_state::<FieldInspectorState>().unwrap();
    assert_eq!(state.logs[0].title, "Microphone");
    assert!(state.logs[0].detail.contains("permission_denied"));
    assert!(state.logs[0].detail.contains("not granted"));
    assert_eq!(state.logs[0].state, CapabilityState::Error);
    assert!(runtime.pending_effects.iter().any(|effect| {
        matches!(
            &effect.effect,
            fission::core::Effect::Capability(
                fission::core::CapabilityInvocationPayload::Operation(operation)
            ) if operation.capability_name == HAPTIC_NOTIFICATION.name
        )
    }));
}

#[test]
fn start_inspection_emits_all_readiness_capabilities() {
    let mut runtime = fission::core::Runtime::default();
    runtime
        .add_app_state(Box::new(FieldInspectorState::default()))
        .unwrap();
    let mut registry = fission::core::registry::ActionRegistry::new();
    registry.register(reduce_with!(on_start_inspection));
    runtime.absorb_registry(registry);

    runtime
        .dispatch(
            StartInspection.into(),
            fission::WidgetId::explicit("test_root"),
        )
        .unwrap();

    let names: BTreeSet<String> = runtime
        .pending_effects
        .iter()
        .filter_map(|effect| match &effect.effect {
            fission::core::Effect::Capability(
                fission::core::CapabilityInvocationPayload::Operation(operation),
            ) => Some(operation.capability_name.clone()),
            _ => None,
        })
        .collect();

    for expected in [
        GET_NOTIFICATION_SETTINGS.name,
        GET_GEOLOCATION_PERMISSION.name,
        REQUEST_GEOLOCATION_PERMISSION.name,
        GET_CURRENT_POSITION.name,
        GET_CAMERA_AVAILABILITY.name,
        GET_MICROPHONE_AVAILABILITY.name,
        GET_NFC_AVAILABILITY.name,
        GET_BIOMETRIC_AVAILABILITY.name,
        GET_PASSKEY_AVAILABILITY.name,
        GET_BLUETOOTH_AVAILABILITY.name,
        SCAN_BLUETOOTH_DEVICES.name,
        GET_WIFI_AVAILABILITY.name,
        SCAN_WIFI_NETWORKS.name,
        GET_VOLUME_LEVEL.name,
    ] {
        assert!(names.contains(expected), "missing {expected}");
    }
}
