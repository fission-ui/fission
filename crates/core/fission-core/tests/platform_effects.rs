use fission_core::{
    ActionRegistry, AppState, CapabilityInvocationPayload, DeepLink, DeepLinkConfig,
    DeepLinkReceived, Effect, Effects, NotificationId, NotificationRequest, NotificationResponse,
    NotificationResponseReceived, SHOW_NOTIFICATION,
};
use fission_core::{BarcodeFormat, BarcodeScanRequest, SCAN_BARCODE};
use fission_core::{BiometricAuthenticateRequest, AUTHENTICATE_BIOMETRIC};
use fission_core::{ClipboardWriteTextRequest, WRITE_CLIPBOARD_TEXT};
use fission_core::{NfcRecord, NfcScanRequest, NfcTechnology, SCAN_NFC_TAG};

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

#[test]
fn nfc_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(7, &mut registry);

    effects.nfc().scan_tag(NfcScanRequest {
        technologies: vec![NfcTechnology::Ndef],
        message: Some("Tap a tag".into()),
        timeout_ms: Some(5_000),
        read_multiple_records: true,
    });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 7);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected NFC capability effect");
    };
    assert_eq!(op.capability_name, SCAN_NFC_TAG.name);
    let decoded: NfcScanRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.technologies, vec![NfcTechnology::Ndef]);
}

#[test]
fn nfc_records_are_public_api() {
    let record = NfcRecord::uri("fission://open/1");
    assert_eq!(record.type_name, b"U".to_vec());
}

#[test]
fn biometric_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(11, &mut registry);

    effects
        .biometrics()
        .authenticate(BiometricAuthenticateRequest {
            reason: "Unlock secure data".into(),
            ..Default::default()
        });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 11);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected biometric capability effect");
    };
    assert_eq!(op.capability_name, AUTHENTICATE_BIOMETRIC.name);
    let decoded: BiometricAuthenticateRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.reason, "Unlock secure data");
}

#[test]
fn barcode_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(13, &mut registry);

    effects.barcode_scanner().scan(BarcodeScanRequest {
        formats: vec![BarcodeFormat::QrCode],
        prompt: Some("Scan code".into()),
        ..Default::default()
    });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 13);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected barcode scanner capability effect");
    };
    assert_eq!(op.capability_name, SCAN_BARCODE.name);
    let decoded: BarcodeScanRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.formats, vec![BarcodeFormat::QrCode]);
}

#[test]
fn clipboard_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(17, &mut registry);

    effects.clipboard().write_text(ClipboardWriteTextRequest {
        text: "copied".into(),
    });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 17);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected clipboard capability effect");
    };
    assert_eq!(op.capability_name, WRITE_CLIPBOARD_TEXT.name);
    let decoded: ClipboardWriteTextRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.text, "copied");
}
