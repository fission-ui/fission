pub mod api;
pub mod components;
pub mod data;
pub mod model;

use api::{fetch_weather, WEATHER_JOB};
use components::app::FieldInspectorApp;
use fission::prelude::*;
use model::{
    on_capability_failed, on_capability_succeeded, on_deep_link_received,
    on_notification_response_received, CapabilityProviderMode, FieldInspectorState,
};

#[cfg(target_os = "android")]
const ANDROID_TEST_CONTROL_PORT: u16 = 48761;

macro_rules! configure_field_inspector_app {
    ($app:expr) => {{
        let demo_hosts = field_inspector_demo_hosts();
        let provider_mode = if demo_hosts {
            CapabilityProviderMode::DemoMemory
        } else {
            CapabilityProviderMode::Native
        };
        let mut app = $app
            .with_title("Fission Field Inspector")
            .with_deep_link_scheme("field-inspector")
            .on_deep_link(reduce_with!(on_deep_link_received))
            .on_notification_response(reduce_with!(on_notification_response_received))
            .with_async(|asyncs| {
                asyncs.register_job(WEATHER_JOB, |request, _| async move {
                    fetch_weather(request).await
                });
            })
            .with_state_init(move |state: &mut FieldInspectorState| {
                state.provider_mode = provider_mode;
            })
            .with_sync_env(|_state: &FieldInspectorState, env: &mut Env| {
                env.theme = Theme::default();
            });
        if demo_hosts {
            app = app
                .with_notification_host(MemoryNotificationHost)
                .with_nfc_host(MemoryNfcHost::new(demo_nfc_tag()))
                .with_biometric_host(MemoryBiometricHost::default())
                .with_passkey_host(MemoryPasskeyHost::default())
                .with_bluetooth_host(MemoryBluetoothHost::default())
                .with_barcode_scanner_host(MemoryBarcodeScannerHost::new(demo_barcode_results()))
                .with_camera_host(MemoryCameraHost::default())
                .with_clipboard_host(MemoryClipboardHost::default())
                .with_geolocation_host(MemoryGeolocationHost::new(demo_position()))
                .with_haptic_host(MemoryHapticHost::default())
                .with_microphone_host(MemoryMicrophoneHost::default())
                .with_wifi_host(MemoryWifiHost::default())
                .with_volume_host(MemoryVolumeHost::default());
        }
        app
    }};
}

fn field_inspector_demo_hosts() -> bool {
    std::env::var("FISSION_FIELD_INSPECTOR_DEMO_HOSTS")
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "demo"
            )
        })
        .unwrap_or(false)
}

fn demo_nfc_tag() -> NfcTag {
    NfcTag {
        id: Some(vec![0xF1, 0x04, 0x8A]),
        technologies: vec![NfcTechnology::Ndef],
        records: vec![NfcRecord::uri("fission://asset/CMP-7A-2219")],
        raw_payload: None,
    }
}

fn demo_barcode_results() -> BarcodeScanResults {
    BarcodeScanResults {
        items: vec![BarcodeScanResult {
            value: "FIELD:CMP-7A-2219".into(),
            format: BarcodeFormat::QrCode,
            raw_bytes: b"FIELD:CMP-7A-2219".to_vec(),
            bounds: Vec::new(),
            symbology_identifier: None,
        }],
    }
}

fn demo_position() -> GeolocationPosition {
    GeolocationPosition {
        latitude: 51.5074,
        longitude: -0.1278,
        altitude_meters: None,
        accuracy_meters: 8.0,
        altitude_accuracy_meters: None,
        heading_degrees: None,
        speed_mps: None,
        timestamp_unix_ms: 1_774_000_000_000,
    }
}

fn callback_reducers() -> fission::core::ActionRegistry<FieldInspectorState> {
    let mut registry = fission::core::ActionRegistry::new();
    registry.register(reduce_with!(on_capability_succeeded));
    registry.register(reduce_with!(on_capability_failed));
    registry
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    let mut app = configure_field_inspector_app!(DesktopApp::new(FieldInspectorApp));
    app.absorb_registry(callback_reducers());
    app.run()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<FieldInspectorState, FieldInspectorApp> {
    let mut app = configure_field_inspector_app!(MobileApp::new(FieldInspectorApp));
    app.absorb_registry(callback_reducers());
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn run_mobile() -> anyhow::Result<()> {
    mobile_app().run()
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app_handle: AndroidApp) {
    let _ = mobile_app().run_with_android_app(app_handle);
}

#[cfg(target_arch = "wasm32")]
fn web_app() -> WebApp<FieldInspectorState, FieldInspectorApp> {
    let mut app =
        configure_field_inspector_app!(WebApp::new(FieldInspectorApp)).mount("#fission-web-mount");
    app.absorb_registry(callback_reducers());
    app
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
