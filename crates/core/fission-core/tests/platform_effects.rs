use fission_core::{
    ActionRegistry, AppState, CapabilityInvocationPayload, DeepLink, DeepLinkConfig,
    DeepLinkReceived, Effect, Effects, NotificationId, NotificationRequest, NotificationResponse,
    NotificationResponseReceived, SHOW_NOTIFICATION,
};
use fission_core::{BarcodeFormat, BarcodeScanRequest, SCAN_BARCODE};
use fission_core::{BiometricAuthenticateRequest, AUTHENTICATE_BIOMETRIC};
use fission_core::{BluetoothScanRequest, SCAN_BLUETOOTH_DEVICES};
use fission_core::{CameraCaptureRequest, CameraFacing, CAPTURE_PHOTO};
use fission_core::{ClipboardWriteTextRequest, WRITE_CLIPBOARD_TEXT};
use fission_core::{GeolocationPositionRequest, GET_CURRENT_POSITION};
use fission_core::{HapticImpactRequest, HapticImpactStyle, HAPTIC_IMPACT};
use fission_core::{MicrophoneCaptureRequest, CAPTURE_MICROPHONE_AUDIO};
use fission_core::{NfcRecord, NfcScanRequest, NfcTechnology, SCAN_NFC_TAG};
use fission_core::{VolumeSetRequest, VolumeStream, SET_VOLUME_LEVEL};
use fission_core::{WifiScanRequest, SCAN_WIFI_NETWORKS};

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
fn bluetooth_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(12, &mut registry);

    effects.bluetooth().scan_devices(BluetoothScanRequest {
        service_uuids: vec!["180D".into()],
        ..Default::default()
    });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 12);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected Bluetooth capability effect");
    };
    assert_eq!(op.capability_name, SCAN_BLUETOOTH_DEVICES.name);
    let decoded: BluetoothScanRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.service_uuids, vec!["180D"]);
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
fn camera_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(15, &mut registry);

    effects.camera().capture_photo(CameraCaptureRequest {
        facing: CameraFacing::Back,
        ..Default::default()
    });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 15);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected camera capability effect");
    };
    assert_eq!(op.capability_name, CAPTURE_PHOTO.name);
    let decoded: CameraCaptureRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.facing, CameraFacing::Back);
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

#[test]
fn geolocation_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(19, &mut registry);

    effects
        .geolocation()
        .current_position(GeolocationPositionRequest {
            high_accuracy: true,
            ..Default::default()
        });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 19);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected geolocation capability effect");
    };
    assert_eq!(op.capability_name, GET_CURRENT_POSITION.name);
    let decoded: GeolocationPositionRequest = serde_json::from_slice(&op.request).unwrap();
    assert!(decoded.high_accuracy);
}

#[test]
fn haptic_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(23, &mut registry);

    effects.haptics().impact(HapticImpactRequest {
        style: HapticImpactStyle::Heavy,
    });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 23);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected haptic capability effect");
    };
    assert_eq!(op.capability_name, HAPTIC_IMPACT.name);
    let decoded: HapticImpactRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.style, HapticImpactStyle::Heavy);
}

#[test]
fn microphone_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(29, &mut registry);

    effects
        .microphone()
        .capture_audio(MicrophoneCaptureRequest {
            duration_ms: 2_500,
            ..Default::default()
        });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 29);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected microphone capability effect");
    };
    assert_eq!(op.capability_name, CAPTURE_MICROPHONE_AUDIO.name);
    let decoded: MicrophoneCaptureRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.duration_ms, 2_500);
}

#[test]
fn wifi_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(31, &mut registry);

    effects.wifi().scan_networks(WifiScanRequest {
        ssid_prefix: Some("Fis".into()),
        ..Default::default()
    });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 31);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected Wi-Fi capability effect");
    };
    assert_eq!(op.capability_name, SCAN_WIFI_NETWORKS.name);
    let decoded: WifiScanRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.ssid_prefix.as_deref(), Some("Fis"));
}

#[test]
fn volume_convenience_builder_emits_capability_effect() {
    let mut registry = ActionRegistry::<TestState>::new();
    let mut effects = Effects::new(37, &mut registry);

    effects.volume().set_level(VolumeSetRequest {
        stream: VolumeStream::Media,
        level: 75,
        muted: Some(false),
    });

    assert_eq!(effects.out.len(), 1);
    assert_eq!(effects.out[0].req_id, 37);
    let Effect::Capability(CapabilityInvocationPayload::Operation(op)) = &effects.out[0].effect
    else {
        panic!("expected volume capability effect");
    };
    assert_eq!(op.capability_name, SET_VOLUME_LEVEL.name);
    let decoded: VolumeSetRequest = serde_json::from_slice(&op.request).unwrap();
    assert_eq!(decoded.level, 75);
}
