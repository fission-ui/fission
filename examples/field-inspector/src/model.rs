use crate::api::{ApiError, WeatherRequest, WeatherSummary, WEATHER_JOB};
use crate::data::{work_orders, WorkOrder};
use fission::core::ActionInput;
use fission::prelude::*;
use std::collections::BTreeSet;

const DEFAULT_LATITUDE: f64 = 51.5074;
const DEFAULT_LONGITUDE: f64 = -0.1278;
const PASSKEY_RELYING_PARTY_ID: &str = "";
const READ_SERVICE_UUID: &str = "0000181a-0000-1000-8000-00805f9b34fb";
const READ_CHARACTERISTIC_UUID: &str = "00002a6e-0000-1000-8000-00805f9b34fb";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InspectorPanel {
    Overview,
    Verify,
    Evidence,
    Sensors,
    Security,
    Review,
}

impl InspectorPanel {
    pub fn label(self) -> &'static str {
        match self {
            InspectorPanel::Overview => "Overview",
            InspectorPanel::Verify => "Verify",
            InspectorPanel::Evidence => "Evidence",
            InspectorPanel::Sensors => "Sensors",
            InspectorPanel::Security => "Security",
            InspectorPanel::Review => "Review",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityState {
    Idle,
    Pending,
    Ready,
    Complete,
    Unavailable,
    Warning,
    Error,
}

impl CapabilityState {
    pub fn label(self) -> &'static str {
        match self {
            CapabilityState::Idle => "Idle",
            CapabilityState::Pending => "Pending",
            CapabilityState::Ready => "Ready",
            CapabilityState::Complete => "Complete",
            CapabilityState::Unavailable => "Unavailable",
            CapabilityState::Warning => "Warning",
            CapabilityState::Error => "Error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityProviderMode {
    Native,
    DemoMemory,
}

impl CapabilityProviderMode {
    pub fn label(self) -> &'static str {
        match self {
            CapabilityProviderMode::Native => "Native host mode",
            CapabilityProviderMode::DemoMemory => "Demo memory mode",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            CapabilityProviderMode::Native => {
                "Capability calls go to the active shell. Unsupported or unavailable host APIs are shown as unavailable instead of being faked."
            }
            CapabilityProviderMode::DemoMemory => {
                "Capability calls use deterministic in-memory providers so the workflow can be exercised without device hardware."
            }
        }
    }

    pub fn state(self) -> CapabilityState {
        match self {
            CapabilityProviderMode::Native => CapabilityState::Ready,
            CapabilityProviderMode::DemoMemory => CapabilityState::Warning,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityLine {
    pub title: &'static str,
    pub detail: String,
    pub state: CapabilityState,
}

#[derive(Debug, Clone)]
pub struct CapabilityLog {
    pub title: String,
    pub detail: String,
    pub state: CapabilityState,
}

#[derive(Debug, Clone)]
pub struct FieldInspectorState {
    pub orders: Vec<WorkOrder>,
    pub selected_order_id: String,
    pub panel: InspectorPanel,
    pub started: bool,
    pub provider_mode: CapabilityProviderMode,
    pub completed_checklist: BTreeSet<String>,
    pub weather: AsyncSnapshot<WeatherSummary, ApiError>,
    pub weather_generation: u64,
    pub position: Option<GeolocationPosition>,
    pub geolocation_permission: Option<GeolocationPermission>,
    pub notification_settings: Option<NotificationSettings>,
    pub notification_receipt: Option<NotificationReceipt>,
    pub camera_availability: Option<CameraAvailability>,
    pub microphone_availability: Option<MicrophoneAvailability>,
    pub nfc_availability: Option<NfcAvailability>,
    pub biometric_availability: Option<BiometricAvailability>,
    pub passkey_availability: Option<PasskeyAvailability>,
    pub bluetooth_availability: Option<BluetoothAvailability>,
    pub wifi_availability: Option<WifiAvailability>,
    pub scanned_barcode: Option<BarcodeScanResults>,
    pub scanned_nfc: Option<NfcTag>,
    pub photo_capture: Option<CameraCapture>,
    pub voice_note: Option<MicrophoneCapture>,
    pub bluetooth_devices: Vec<BluetoothDevice>,
    pub bluetooth_connection: Option<BluetoothConnection>,
    pub sensor_reading: Option<String>,
    pub wifi_networks: Vec<WifiNetwork>,
    pub volume_level: Option<VolumeLevel>,
    pub torch_on: bool,
    pub sensitive_unlocked: bool,
    pub passkey_verified: bool,
    pub registered_passkey: Option<PasskeyCredentialDescriptor>,
    pub copied_summary: Option<String>,
    pub last_deep_link: Option<DeepLink>,
    pub report_submitted: bool,
    pub logs: Vec<CapabilityLog>,
}

impl Default for FieldInspectorState {
    fn default() -> Self {
        let orders = work_orders();
        let selected_order_id = orders
            .first()
            .map(|order| order.id.to_string())
            .unwrap_or_default();
        Self {
            orders,
            selected_order_id,
            panel: InspectorPanel::Overview,
            started: false,
            provider_mode: CapabilityProviderMode::Native,
            completed_checklist: BTreeSet::new(),
            weather: AsyncSnapshot::waiting(),
            weather_generation: 0,
            position: None,
            geolocation_permission: None,
            notification_settings: None,
            notification_receipt: None,
            camera_availability: None,
            microphone_availability: None,
            nfc_availability: None,
            biometric_availability: None,
            passkey_availability: None,
            bluetooth_availability: None,
            wifi_availability: None,
            scanned_barcode: None,
            scanned_nfc: None,
            photo_capture: None,
            voice_note: None,
            bluetooth_devices: Vec::new(),
            bluetooth_connection: None,
            sensor_reading: None,
            wifi_networks: Vec::new(),
            volume_level: None,
            torch_on: false,
            sensitive_unlocked: false,
            passkey_verified: false,
            registered_passkey: None,
            copied_summary: None,
            last_deep_link: None,
            report_submitted: false,
            logs: Vec::new(),
        }
    }
}

impl AppState for FieldInspectorState {}

fn notification_settings_state(settings: &NotificationSettings) -> CapabilityState {
    if matches!(
        settings.permission,
        NotificationPermission::Granted | NotificationPermission::Provisional
    ) && (settings.alerts || settings.badge || settings.sound || settings.scheduling)
    {
        CapabilityState::Ready
    } else {
        CapabilityState::Unavailable
    }
}

fn geolocation_permission_state(permission: GeolocationPermission) -> CapabilityState {
    match permission {
        GeolocationPermission::Granted | GeolocationPermission::Prompt => CapabilityState::Ready,
        GeolocationPermission::Denied | GeolocationPermission::Unsupported => {
            CapabilityState::Unavailable
        }
        GeolocationPermission::Unknown => CapabilityState::Pending,
    }
}

fn camera_availability_state(availability: &CameraAvailability) -> CapabilityState {
    if availability.permission == CameraPermission::Granted && !availability.devices.is_empty() {
        CapabilityState::Ready
    } else if availability.permission == CameraPermission::Unknown {
        CapabilityState::Pending
    } else {
        CapabilityState::Unavailable
    }
}

fn camera_permission_state(permission: CameraPermission) -> CapabilityState {
    match permission {
        CameraPermission::Granted => CapabilityState::Ready,
        CameraPermission::Unknown => CapabilityState::Pending,
        CameraPermission::Denied | CameraPermission::Restricted => CapabilityState::Unavailable,
    }
}

fn microphone_availability_state(availability: &MicrophoneAvailability) -> CapabilityState {
    if availability.permission == MicrophonePermission::Granted && !availability.devices.is_empty()
    {
        CapabilityState::Ready
    } else if availability.permission == MicrophonePermission::Unknown {
        CapabilityState::Pending
    } else {
        CapabilityState::Unavailable
    }
}

fn microphone_permission_state(permission: MicrophonePermission) -> CapabilityState {
    match permission {
        MicrophonePermission::Granted => CapabilityState::Ready,
        MicrophonePermission::Unknown => CapabilityState::Pending,
        MicrophonePermission::Denied | MicrophonePermission::Restricted => {
            CapabilityState::Unavailable
        }
    }
}

fn nfc_availability_state(availability: &NfcAvailability) -> CapabilityState {
    if availability.supported && availability.enabled && availability.read {
        CapabilityState::Ready
    } else {
        CapabilityState::Unavailable
    }
}

fn bluetooth_availability_state(availability: &BluetoothAvailability) -> CapabilityState {
    if availability.permission == BluetoothPermission::Granted
        && availability.enabled
        && (availability.supports_classic || availability.supports_low_energy)
    {
        CapabilityState::Ready
    } else {
        CapabilityState::Unavailable
    }
}

fn wifi_availability_state(availability: &WifiAvailability) -> CapabilityState {
    if availability.permission == WifiPermission::Granted && availability.enabled {
        CapabilityState::Ready
    } else {
        CapabilityState::Unavailable
    }
}

fn secure_availability_state(
    biometric: Option<&BiometricAvailability>,
    passkey: Option<&PasskeyAvailability>,
) -> CapabilityState {
    let biometric_ready =
        biometric.is_some_and(|b| b.supported && (b.enrolled || b.device_credential));
    let passkey_ready = passkey.is_some_and(|p| p.supported && p.secure_context);
    if biometric_ready || passkey_ready {
        CapabilityState::Ready
    } else if biometric.is_some() || passkey.is_some() {
        CapabilityState::Unavailable
    } else {
        CapabilityState::Idle
    }
}

impl FieldInspectorState {
    pub fn selected_order(&self) -> &WorkOrder {
        self.orders
            .iter()
            .find(|order| order.id == self.selected_order_id)
            .or_else(|| self.orders.first())
            .expect("field inspector has seed work orders")
    }

    pub fn weather_request(&self) -> WeatherRequest {
        let position = self.position.as_ref();
        WeatherRequest {
            latitude: position.map(|p| p.latitude).unwrap_or(DEFAULT_LATITUDE),
            longitude: position.map(|p| p.longitude).unwrap_or(DEFAULT_LONGITUDE),
            generation: self.weather_generation,
        }
    }

    pub fn checklist_progress(&self) -> (usize, usize) {
        let total = self.selected_order().checklist.len();
        let complete = self
            .selected_order()
            .checklist
            .iter()
            .filter(|item| self.completed_checklist.contains(item.id))
            .count();
        (complete, total)
    }

    pub fn report_summary(&self) -> String {
        let order = self.selected_order();
        let (complete, total) = self.checklist_progress();
        let location = self
            .position
            .as_ref()
            .map(|p| format!("{:.5}, {:.5}", p.latitude, p.longitude))
            .unwrap_or_else(|| "location pending".into());
        format!(
            "{} / {}: {} at {}. Checklist {}/{}. Location {}. Barcode {}. NFC {}. Photo {}. Voice note {}. Sensor {}.",
            order.id,
            order.asset.id,
            order.title,
            order.site,
            complete,
            total,
            location,
            yes_no(self.asset_barcode_matches()),
            yes_no(self.asset_nfc_matches()),
            yes_no(self.photo_capture.is_some()),
            yes_no(self.voice_note.is_some()),
            self.sensor_reading.as_deref().unwrap_or("pending")
        )
    }

    pub fn capability_lines(&self) -> Vec<CapabilityLine> {
        vec![
            CapabilityLine {
                title: "Notifications",
                detail: self
                    .notification_settings
                    .as_ref()
                    .map(|s| {
                        format!(
                            "{:?}, alerts {}, schedules {}",
                            s.permission, s.alerts, s.scheduling
                        )
                    })
                    .or_else(|| {
                        self.notification_receipt
                            .as_ref()
                            .map(|r| format!("receipt {}", r.id.0))
                    })
                    .unwrap_or_else(|| "Reminder and submit alerts".into()),
                state: if self.notification_receipt.is_some() {
                    CapabilityState::Complete
                } else if let Some(settings) = &self.notification_settings {
                    notification_settings_state(settings)
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Deep links",
                detail: self
                    .last_deep_link
                    .as_ref()
                    .map(|link| link.url.clone())
                    .unwrap_or_else(|| "Open directly into a work order".into()),
                state: if self.last_deep_link.is_some() {
                    CapabilityState::Complete
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Geolocation",
                detail: self
                    .position
                    .as_ref()
                    .map(|p| {
                        format!(
                            "{:.5}, {:.5} within {:.0} m",
                            p.latitude, p.longitude, p.accuracy_meters
                        )
                    })
                    .or_else(|| {
                        self.geolocation_permission
                            .map(|permission| format!("permission {:?}", permission))
                    })
                    .unwrap_or_else(|| "Attach GPS to report".into()),
                state: if self.position.is_some() {
                    CapabilityState::Ready
                } else if let Some(permission) = self.geolocation_permission {
                    geolocation_permission_state(permission)
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Camera and flashlight",
                detail: self
                    .photo_capture
                    .as_ref()
                    .map(|p| {
                        format!(
                            "{}x{} {}, {} KiB",
                            p.width,
                            p.height,
                            p.content_type,
                            (p.bytes.len().saturating_add(1023)) / 1024
                        )
                    })
                    .or_else(|| {
                        self.camera_availability.as_ref().map(|a| {
                            format!(
                                "{} camera(s), torch {}",
                                a.devices.len(),
                                yes_no(a.devices.iter().any(|d| d.has_flashlight))
                            )
                        })
                    })
                    .unwrap_or_else(|| "Capture evidence and control torch".into()),
                state: if self.photo_capture.is_some() {
                    CapabilityState::Complete
                } else if let Some(availability) = &self.camera_availability {
                    camera_availability_state(availability)
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Barcode scanner",
                detail: self
                    .scanned_barcode
                    .as_ref()
                    .and_then(|r| r.items.first())
                    .map(|item| item.value.clone())
                    .unwrap_or_else(|| "Scan asset label".into()),
                state: if self.asset_barcode_matches() {
                    CapabilityState::Complete
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "NFC",
                detail: self
                    .scanned_nfc
                    .as_ref()
                    .and_then(nfc_uri_for_display)
                    .or_else(|| {
                        self.nfc_availability
                            .as_ref()
                            .map(|n| format!("read {}, write {}", n.read, n.write))
                    })
                    .unwrap_or_else(|| "Tap asset service tag".into()),
                state: if self.asset_nfc_matches() {
                    CapabilityState::Complete
                } else if let Some(availability) = &self.nfc_availability {
                    nfc_availability_state(availability)
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Microphone",
                detail: self
                    .voice_note
                    .as_ref()
                    .map(|n| {
                        format!(
                            "{} ms, {} Hz, {} KiB",
                            n.duration_ms,
                            n.sample_rate_hz,
                            (n.bytes.len().saturating_add(1023)) / 1024
                        )
                    })
                    .or_else(|| {
                        self.microphone_availability
                            .as_ref()
                            .map(|m| format!("{} input device(s)", m.devices.len()))
                    })
                    .unwrap_or_else(|| "Record a short voice note".into()),
                state: if self.voice_note.is_some() {
                    CapabilityState::Complete
                } else if let Some(availability) = &self.microphone_availability {
                    microphone_availability_state(availability)
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Bluetooth",
                detail: self
                    .bluetooth_connection
                    .as_ref()
                    .map(|c| {
                        format!(
                            "connected to {}",
                            c.device.name.clone().unwrap_or_else(|| c.device.id.clone())
                        )
                    })
                    .or_else(|| {
                        (!self.bluetooth_devices.is_empty())
                            .then(|| format!("{} nearby device(s)", self.bluetooth_devices.len()))
                    })
                    .or_else(|| {
                        self.bluetooth_availability.as_ref().map(|availability| {
                            format!(
                                "enabled {}, classic {}, LE {}",
                                availability.enabled,
                                availability.supports_classic,
                                availability.supports_low_energy
                            )
                        })
                    })
                    .unwrap_or_else(|| "Scan and connect to sensor bridge".into()),
                state: if self.bluetooth_connection.is_some() {
                    CapabilityState::Complete
                } else if !self.bluetooth_devices.is_empty() {
                    CapabilityState::Ready
                } else if let Some(availability) = &self.bluetooth_availability {
                    bluetooth_availability_state(availability)
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Wi-Fi",
                detail: self
                    .wifi_availability
                    .as_ref()
                    .and_then(|w| w.connected_network.as_ref())
                    .map(|n| format!("connected to {}", n.ssid))
                    .or_else(|| {
                        (!self.wifi_networks.is_empty())
                            .then(|| format!("{} network(s) visible", self.wifi_networks.len()))
                    })
                    .or_else(|| {
                        self.wifi_availability
                            .as_ref()
                            .map(|availability| format!("enabled {}", availability.enabled))
                    })
                    .unwrap_or_else(|| "Check site network context".into()),
                state: if !self.wifi_networks.is_empty() {
                    CapabilityState::Ready
                } else if let Some(availability) = &self.wifi_availability {
                    wifi_availability_state(availability)
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Biometrics and passkeys",
                detail: if self.sensitive_unlocked && self.passkey_verified {
                    "Sensitive panel unlocked and account verified".into()
                } else if self.sensitive_unlocked {
                    "Biometric unlock complete".into()
                } else if self.biometric_availability.is_some()
                    || self.passkey_availability.is_some()
                {
                    match secure_availability_state(
                        self.biometric_availability.as_ref(),
                        self.passkey_availability.as_ref(),
                    ) {
                        CapabilityState::Ready => "Secure verification available".into(),
                        CapabilityState::Unavailable => "Secure verification unavailable".into(),
                        _ => "Gate sensitive site data".into(),
                    }
                } else {
                    "Gate sensitive site data".into()
                },
                state: if self.sensitive_unlocked && self.passkey_verified {
                    CapabilityState::Complete
                } else {
                    secure_availability_state(
                        self.biometric_availability.as_ref(),
                        self.passkey_availability.as_ref(),
                    )
                },
            },
            CapabilityLine {
                title: "Clipboard",
                detail: self
                    .copied_summary
                    .as_ref()
                    .map(|s| format!("{} chars copied", s.len()))
                    .unwrap_or_else(|| "Copy report summary".into()),
                state: if self.copied_summary.is_some() {
                    CapabilityState::Complete
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Haptics",
                detail: "Selection, scan, success, and error feedback".into(),
                state: if self.logs.iter().any(|log| log.title.contains("Haptic")) {
                    CapabilityState::Complete
                } else {
                    CapabilityState::Idle
                },
            },
            CapabilityLine {
                title: "Volume control",
                detail: self
                    .volume_level
                    .as_ref()
                    .map(|v| format!("media {}%, muted {}", v.level, v.muted))
                    .unwrap_or_else(|| "Read and adjust alert volume".into()),
                state: if self.volume_level.is_some() {
                    CapabilityState::Ready
                } else {
                    CapabilityState::Idle
                },
            },
        ]
    }

    pub fn asset_barcode_matches(&self) -> bool {
        let expected = self.selected_order().asset.expected_barcode;
        self.scanned_barcode
            .as_ref()
            .and_then(|results| results.items.first())
            .is_some_and(|item| item.value == expected)
    }

    pub fn asset_nfc_matches(&self) -> bool {
        let expected = self.selected_order().asset.expected_nfc_uri;
        self.scanned_nfc
            .as_ref()
            .and_then(nfc_uri_for_display)
            .is_some_and(|uri| uri == expected)
    }

    fn reset_for_order(&mut self, order_id: String) {
        self.selected_order_id = order_id;
        self.panel = InspectorPanel::Overview;
        self.started = false;
        self.completed_checklist.clear();
        self.weather = AsyncSnapshot::waiting();
        self.weather_generation = self.weather_generation.saturating_add(1);
        self.position = None;
        self.geolocation_permission = None;
        self.notification_settings = None;
        self.notification_receipt = None;
        self.camera_availability = None;
        self.microphone_availability = None;
        self.nfc_availability = None;
        self.biometric_availability = None;
        self.passkey_availability = None;
        self.bluetooth_availability = None;
        self.wifi_availability = None;
        self.scanned_barcode = None;
        self.scanned_nfc = None;
        self.photo_capture = None;
        self.voice_note = None;
        self.bluetooth_devices.clear();
        self.bluetooth_connection = None;
        self.sensor_reading = None;
        self.wifi_networks.clear();
        self.volume_level = None;
        self.torch_on = false;
        self.sensitive_unlocked = false;
        self.passkey_verified = false;
        self.registered_passkey = None;
        self.copied_summary = None;
        self.report_submitted = false;
        self.logs.clear();
    }

    fn complete_check(&mut self, id: &str) {
        self.completed_checklist.insert(id.to_string());
    }

    fn log(&mut self, title: impl Into<String>, detail: impl Into<String>, state: CapabilityState) {
        self.logs.insert(
            0,
            CapabilityLog {
                title: title.into(),
                detail: detail.into(),
                state,
            },
        );
        self.logs.truncate(12);
    }
}

#[fission_reducer(SelectOrder)]
pub fn on_select_order(state: &mut FieldInspectorState, order_id: String) {
    if state.orders.iter().any(|order| order.id == order_id) {
        state.reset_for_order(order_id);
    }
}

#[fission_reducer(SelectPanel)]
pub fn on_select_panel(state: &mut FieldInspectorState, panel: InspectorPanel) {
    state.panel = panel;
}

#[fission_reducer(StartInspection)]
pub fn on_start_inspection(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.started = true;
    state.weather = AsyncSnapshot::waiting();
    state.weather_generation = state.weather_generation.saturating_add(1);
    state.log(
        "Inspection started",
        "Capability readiness checks are running",
        CapabilityState::Pending,
    );

    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));

    ctx.effects
        .notifications()
        .settings()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .geolocation()
        .permission()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .geolocation()
        .request_permission(GeolocationPermissionRequest {
            precise: true,
            background: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .geolocation()
        .current_position(GeolocationPositionRequest {
            high_accuracy: true,
            timeout_ms: Some(5_000),
            maximum_age_ms: Some(60_000),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .camera()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .camera()
        .request_permission(CameraPermissionRequest {
            reason: Some("Capture field evidence and scan asset labels".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .microphone()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .microphone()
        .request_permission(MicrophonePermissionRequest {
            reason: Some("Attach a short voice note to the field report".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .nfc()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .biometrics()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .passkeys()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .bluetooth()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .bluetooth()
        .scan_devices(BluetoothScanRequest {
            service_uuids: vec![state.selected_order().asset.sensor_service_uuid.to_string()],
            timeout_ms: Some(2_000),
            include_paired: true,
            allow_duplicates: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .wifi()
        .availability()
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .wifi()
        .scan_networks(WifiScanRequest {
            ssid_prefix: None,
            include_hidden: false,
            timeout_ms: Some(2_000),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .volume()
        .get_level(VolumeStream::Media)
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(VerifyWithBarcode)]
pub fn on_verify_with_barcode(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Verify;
    state.log(
        "Barcode scan",
        "Opening scanner for the selected asset",
        CapabilityState::Pending,
    );
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .camera()
        .request_permission(CameraPermissionRequest {
            reason: Some("Scan the selected asset label".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .barcode_scanner()
        .scan(BarcodeScanRequest {
            formats: vec![BarcodeFormat::QrCode, BarcodeFormat::Code128],
            prompt: Some(format!("Scan {}", state.selected_order().asset.id)),
            camera_id: None,
            timeout_ms: Some(10_000),
            allow_multiple: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects.haptics().selection().on_ok(ok).on_err(err);
}

#[fission_reducer(VerifyWithNfc)]
pub fn on_verify_with_nfc(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Verify;
    state.log(
        "NFC scan",
        "Waiting for the asset service tag",
        CapabilityState::Pending,
    );
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .nfc()
        .scan_tag(NfcScanRequest {
            technologies: vec![NfcTechnology::Ndef],
            message: Some(format!("Tap {}", state.selected_order().asset.id)),
            timeout_ms: Some(10_000),
            read_multiple_records: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .haptics()
        .impact(HapticImpactRequest {
            style: HapticImpactStyle::Medium,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(CaptureEvidencePhoto)]
pub fn on_capture_evidence_photo(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Evidence;
    state.log(
        "Photo capture",
        "Capturing a still image for the report",
        CapabilityState::Pending,
    );
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .camera()
        .request_permission(CameraPermissionRequest {
            reason: Some("Capture a still image for the field report".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .camera()
        .capture_photo(CameraCaptureRequest {
            camera_id: None,
            facing: CameraFacing::Back,
            resolution: Some(CameraResolution {
                width: 1600,
                height: 1200,
            }),
            format: CameraImageFormat::Jpeg,
            flash: if state.torch_on {
                CameraFlashMode::On
            } else {
                CameraFlashMode::Auto
            },
            quality: Some(86),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ToggleTorch)]
pub fn on_toggle_torch(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Evidence;
    state.torch_on = !state.torch_on;
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .camera()
        .set_flashlight(CameraFlashlightRequest {
            camera_id: None,
            enabled: state.torch_on,
            intensity: Some(if state.torch_on { 80 } else { 0 }),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(RecordVoiceNote)]
pub fn on_record_voice_note(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Evidence;
    state.log(
        "Voice note",
        "Recording a bounded one-second note",
        CapabilityState::Pending,
    );
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .microphone()
        .capture_audio(MicrophoneCaptureRequest {
            device_id: None,
            duration_ms: 1_000,
            sample_rate_hz: Some(48_000),
            channels: Some(1),
            sample_format: AudioSampleFormat::F32,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ScanSensors)]
pub fn on_scan_sensors(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Sensors;
    state.log(
        "Sensor scan",
        "Scanning Bluetooth and Wi-Fi context",
        CapabilityState::Pending,
    );
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .bluetooth()
        .request_permission(BluetoothPermissionRequest {
            reason: Some("Find the asset sensor bridge".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .bluetooth()
        .scan_devices(BluetoothScanRequest {
            service_uuids: vec![state.selected_order().asset.sensor_service_uuid.to_string()],
            timeout_ms: Some(3_000),
            include_paired: true,
            allow_duplicates: false,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .wifi()
        .request_permission(WifiPermissionRequest {
            reason: Some("Confirm the technician is on a site network".into()),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .wifi()
        .scan_networks(WifiScanRequest {
            ssid_prefix: None,
            include_hidden: false,
            timeout_ms: Some(3_000),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ConnectSensor)]
pub fn on_connect_sensor(
    state: &mut FieldInspectorState,
    device_id: String,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Sensors;
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .bluetooth()
        .connect_device(BluetoothConnectRequest {
            device_id,
            service_uuids: vec![state.selected_order().asset.sensor_service_uuid.to_string()],
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ReadSensor)]
pub fn on_read_sensor(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Sensors;
    let Some(connection) = &state.bluetooth_connection else {
        state.log(
            "Sensor read",
            "Connect to the sensor bridge first",
            CapabilityState::Warning,
        );
        return;
    };
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .bluetooth()
        .read_characteristic(BluetoothReadRequest {
            connection_id: connection.connection_id.clone(),
            service_uuid: READ_SERVICE_UUID.into(),
            characteristic_uuid: READ_CHARACTERISTIC_UUID.into(),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(SecureUnlock)]
pub fn on_secure_unlock(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Security;
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .biometrics()
        .authenticate(BiometricAuthenticateRequest {
            reason: format!("Unlock protected notes for {}", state.selected_order().site),
            title: Some("Unlock site data".into()),
            subtitle: Some(state.selected_order().asset.id.into()),
            fallback_title: Some("Use device credential".into()),
            cancel_title: Some("Cancel".into()),
            allow_device_credential: true,
            required_strength: BiometricStrength::Any,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(RegisterPasskey)]
pub fn on_register_passkey(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Security;
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .passkeys()
        .register(PasskeyRegistrationRequest {
            relying_party: PasskeyRelyingParty::new(PASSKEY_RELYING_PARTY_ID, "Field Inspector"),
            user: PasskeyUser::new(
                vec![7, 42],
                "technician@example.com",
                state.selected_order().assigned_to,
            ),
            challenge: b"demo-registration-challenge".to_vec(),
            pub_key_algorithms: vec![PasskeyAlgorithm::ES256, PasskeyAlgorithm::RS256],
            timeout_ms: Some(60_000),
            attestation: PasskeyAttestationConveyance::None,
            authenticator_selection: Some(PasskeyAuthenticatorSelection {
                attachment: Some(PasskeyAuthenticatorAttachment::Platform),
                resident_key: PasskeyResidentKeyRequirement::Required,
                user_verification: PasskeyUserVerification::Preferred,
            }),
            exclude_credentials: Vec::new(),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(AuthenticatePasskey)]
pub fn on_authenticate_passkey(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Security;
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .passkeys()
        .authenticate(PasskeyAuthenticationRequest {
            relying_party_id: PASSKEY_RELYING_PARTY_ID.into(),
            challenge: b"demo-authentication-challenge".to_vec(),
            allow_credentials: state.registered_passkey.iter().cloned().collect(),
            user_verification: PasskeyUserVerification::Preferred,
            mediation: PasskeyMediation::Required,
            timeout_ms: Some(60_000),
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(CopyReportSummary)]
pub fn on_copy_report_summary(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Review;
    let summary = state.report_summary();
    state.copied_summary = Some(summary.clone());
    state.complete_check("report");
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .clipboard()
        .write_text(ClipboardWriteTextRequest { text: summary })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ScheduleReminder)]
pub fn on_schedule_reminder(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Review;
    let order = state.selected_order();
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .notifications()
        .schedule(NotificationRequest {
            id: NotificationId::new(format!("{}-reminder", order.id)),
            title: format!("Inspection reminder: {}", order.id),
            body: format!("Finish {} before {}", order.asset.name, order.due),
            subtitle: Some(order.site.into()),
            badge: Some(1),
            sound: NotificationSound::Default,
            deep_link: Some(format!("field-inspector://work-orders/{}", order.id)),
            actions: vec![NotificationActionButton {
                id: "open".into(),
                title: "Open job".into(),
                foreground: true,
                ..Default::default()
            }],
            schedule: NotificationSchedule::AfterMillis(30_000),
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .notifications()
        .set_badge_count(SetBadgeCountRequest { count: Some(1) })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(AdjustAlertVolume)]
pub fn on_adjust_alert_volume(
    state: &mut FieldInspectorState,
    direction: VolumeAdjustDirection,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Review;
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .volume()
        .adjust_level(VolumeAdjustRequest {
            stream: VolumeStream::Media,
            direction,
            step: 8,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(SubmitReport)]
pub fn on_submit_report(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    state.panel = InspectorPanel::Review;
    state.report_submitted = true;
    state.complete_check("report");
    let order = state.selected_order();
    let ok = ctx
        .effects
        .bind(CapabilitySucceeded, reduce_with!(on_capability_succeeded));
    let err = ctx
        .effects
        .bind(CapabilityFailed, reduce_with!(on_capability_failed));
    ctx.effects
        .notifications()
        .show(NotificationRequest {
            id: NotificationId::new(format!("{}-submitted", order.id)),
            title: "Inspection submitted".into(),
            body: format!("{} is ready for review", order.asset.id),
            subtitle: Some(order.site.into()),
            badge: Some(0),
            sound: NotificationSound::Default,
            deep_link: Some(format!("field-inspector://work-orders/{}/report", order.id)),
            actions: Vec::new(),
            schedule: NotificationSchedule::Immediate,
        })
        .on_ok(ok.clone())
        .on_err(err.clone());
    ctx.effects
        .haptics()
        .notification(HapticNotificationRequest {
            kind: HapticNotificationKind::Success,
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(CompleteChecklist)]
pub fn on_complete_checklist(state: &mut FieldInspectorState, id: String) {
    state.complete_check(&id);
}

pub fn on_deep_link_received(
    state: &mut FieldInspectorState,
    action: DeepLinkReceived,
    _ctx: &mut ReducerContext<FieldInspectorState>,
) {
    apply_deep_link(state, action.link);
}

fn apply_deep_link(state: &mut FieldInspectorState, link: DeepLink) {
    state.last_deep_link = Some(link.clone());
    if let Some(order_id) = link.url.rsplit('/').next() {
        if state.orders.iter().any(|order| order.id == order_id) {
            state.selected_order_id = order_id.to_string();
            state.panel = InspectorPanel::Overview;
        }
    }
    state.log("Deep link", link.url, CapabilityState::Complete);
}

pub fn on_notification_response_received(
    state: &mut FieldInspectorState,
    action: NotificationResponseReceived,
    _ctx: &mut ReducerContext<FieldInspectorState>,
) {
    if let Some(link) = action.response.deep_link {
        apply_deep_link(
            state,
            DeepLink::new(link).source(DeepLinkSource::Notification),
        );
    } else {
        state.log(
            "Notification response",
            action.response.notification_id.0,
            CapabilityState::Complete,
        );
    }
}

#[fission_reducer(WeatherLoaded)]
pub fn on_weather_loaded(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    if let Some(weather) = ctx.input.job_ok(WEATHER_JOB) {
        state.weather = AsyncSnapshot::with_data(AsyncConnectionState::Done, weather.clone());
        state.log(
            "Weather",
            format!("{} C, {}", weather.temperature_c.round(), weather.label),
            CapabilityState::Ready,
        );
    }
}

#[fission_reducer(WeatherFailed)]
pub fn on_weather_failed(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    let error = ctx.input.job_err(WEATHER_JOB).unwrap_or_else(|| ApiError {
        message: ctx
            .input
            .job_error_message(WEATHER_JOB)
            .unwrap_or("Weather unavailable")
            .to_string(),
    });
    state.weather = AsyncSnapshot::with_error(AsyncConnectionState::Done, error.clone());
    state.log("Weather", error.message, CapabilityState::Warning);
}

#[fission_reducer(CapabilitySucceeded)]
pub fn on_capability_succeeded(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    if let Some(settings) = ctx.input.capability_ok(GET_NOTIFICATION_SETTINGS) {
        let status = notification_settings_state(&settings);
        state.notification_settings = Some(settings);
        state.log("Notifications", "Host settings loaded", status);
    }
    if let Some(permission) = ctx.input.capability_ok(REQUEST_NOTIFICATION_PERMISSION) {
        let status = notification_settings_state(&permission);
        state.notification_settings = Some(permission);
        state.log("Notifications", "Permission request completed", status);
    }
    if let Some(receipt) = ctx.input.capability_ok(SHOW_NOTIFICATION) {
        state.notification_receipt = Some(receipt);
        state.log(
            "Notification",
            "Immediate notification accepted",
            CapabilityState::Complete,
        );
    }
    if let Some(receipt) = ctx.input.capability_ok(SCHEDULE_NOTIFICATION) {
        state.notification_receipt = Some(receipt);
        state.log(
            "Notification",
            "Reminder scheduled",
            CapabilityState::Complete,
        );
    }
    if ctx.input.capability_ok(SET_BADGE_COUNT).is_some() {
        state.log(
            "Notification badge",
            "Badge count updated",
            CapabilityState::Complete,
        );
    }
    if let Some(permission) = ctx.input.capability_ok(GET_GEOLOCATION_PERMISSION) {
        state.geolocation_permission = Some(permission);
        state.log(
            "Location permission",
            format!("{:?}", permission),
            geolocation_permission_state(permission),
        );
    }
    if let Some(permission) = ctx.input.capability_ok(REQUEST_GEOLOCATION_PERMISSION) {
        state.geolocation_permission = Some(permission);
        state.log(
            "Location permission",
            format!("{:?}", permission),
            geolocation_permission_state(permission),
        );
    }
    if let Some(position) = ctx.input.capability_ok(GET_CURRENT_POSITION) {
        state.position = Some(position);
        state.weather = AsyncSnapshot::waiting();
        state.weather_generation = state.weather_generation.saturating_add(1);
        state.log(
            "Location",
            "Current position attached",
            CapabilityState::Ready,
        );
    }
    if let Some(availability) = ctx.input.capability_ok(GET_CAMERA_AVAILABILITY) {
        let status = camera_availability_state(&availability);
        state.camera_availability = Some(availability);
        state.log("Camera", "Availability loaded", status);
    }
    if let Some(permission) = ctx.input.capability_ok(REQUEST_CAMERA_PERMISSION) {
        if let Some(availability) = &mut state.camera_availability {
            availability.permission = permission;
        } else {
            state.camera_availability = Some(CameraAvailability {
                permission,
                devices: Vec::new(),
            });
        }
        state.log(
            "Camera permission",
            format!("{:?}", permission),
            camera_permission_state(permission),
        );
    }
    if ctx.input.capability_ok(SET_CAMERA_FLASHLIGHT).is_some() {
        state.log(
            "Flashlight",
            if state.torch_on {
                "Torch enabled"
            } else {
                "Torch disabled"
            },
            CapabilityState::Complete,
        );
    }
    if let Some(capture) = ctx.input.capability_ok(CAPTURE_PHOTO) {
        let detail = format!(
            "{}x{} {}, {} KiB",
            capture.width,
            capture.height,
            capture.content_type,
            (capture.bytes.len().saturating_add(1023)) / 1024
        );
        state.photo_capture = Some(capture);
        state.complete_check("evidence");
        state.log("Photo", detail, CapabilityState::Complete);
    }
    if let Some(results) = ctx.input.capability_ok(SCAN_BARCODE) {
        let detail = results
            .items
            .first()
            .map(|item| format!("{:?} {}", item.format, item.value))
            .unwrap_or_else(|| "No barcode found in captured image".into());
        state.scanned_barcode = Some(results);
        if state.asset_barcode_matches() {
            state.complete_check("identity");
            state.log("Barcode", detail, CapabilityState::Complete);
        } else {
            state.log("Barcode", detail, CapabilityState::Warning);
        }
    }
    if let Some(availability) = ctx.input.capability_ok(GET_NFC_AVAILABILITY) {
        let status = nfc_availability_state(&availability);
        state.nfc_availability = Some(availability);
        state.log("NFC", "Availability loaded", status);
    }
    if let Some(tag) = ctx.input.capability_ok(SCAN_NFC_TAG) {
        state.scanned_nfc = Some(tag);
        if state.asset_nfc_matches() {
            state.complete_check("identity");
            state.log(
                "NFC",
                "Asset service tag matched",
                CapabilityState::Complete,
            );
        } else {
            state.log(
                "NFC",
                "Tag did not match selected asset",
                CapabilityState::Warning,
            );
        }
    }
    if let Some(availability) = ctx.input.capability_ok(GET_MICROPHONE_AVAILABILITY) {
        let status = microphone_availability_state(&availability);
        state.microphone_availability = Some(availability);
        state.log("Microphone", "Availability loaded", status);
    }
    if let Some(permission) = ctx.input.capability_ok(REQUEST_MICROPHONE_PERMISSION) {
        if let Some(availability) = &mut state.microphone_availability {
            availability.permission = permission;
        } else {
            state.microphone_availability = Some(MicrophoneAvailability {
                permission,
                devices: Vec::new(),
            });
        }
        state.log(
            "Microphone permission",
            format!("{:?}", permission),
            microphone_permission_state(permission),
        );
    }
    if let Some(capture) = ctx.input.capability_ok(CAPTURE_MICROPHONE_AUDIO) {
        state.voice_note = Some(capture);
        state.complete_check("voice");
        state.log(
            "Voice note",
            "Audio evidence attached",
            CapabilityState::Complete,
        );
    }
    if let Some(availability) = ctx.input.capability_ok(GET_BLUETOOTH_AVAILABILITY) {
        let status = bluetooth_availability_state(&availability);
        state.bluetooth_availability = Some(availability);
        state.log("Bluetooth", "Availability loaded", status);
    }
    if let Some(permission) = ctx.input.capability_ok(REQUEST_BLUETOOTH_PERMISSION) {
        state.log(
            "Bluetooth permission",
            format!("{:?}", permission),
            CapabilityState::Ready,
        );
    }
    if let Some(scan) = ctx.input.capability_ok(SCAN_BLUETOOTH_DEVICES) {
        state.bluetooth_devices = scan.devices;
        state.log(
            "Bluetooth",
            format!("{} device(s) discovered", state.bluetooth_devices.len()),
            CapabilityState::Ready,
        );
    }
    if let Some(connection) = ctx.input.capability_ok(CONNECT_BLUETOOTH_DEVICE) {
        state.bluetooth_connection = Some(connection);
        state.log(
            "Bluetooth",
            "Sensor bridge connected",
            CapabilityState::Complete,
        );
    }
    if let Some(read) = ctx.input.capability_ok(READ_BLUETOOTH_CHARACTERISTIC) {
        let reading =
            String::from_utf8(read.value).unwrap_or_else(|_| "binary sensor payload".into());
        state.sensor_reading = Some(format!("{} telemetry", reading));
        state.complete_check("sensors");
        state.log(
            "Sensor",
            "Bluetooth characteristic read",
            CapabilityState::Complete,
        );
    }
    if let Some(availability) = ctx.input.capability_ok(GET_WIFI_AVAILABILITY) {
        let status = wifi_availability_state(&availability);
        state.wifi_availability = Some(availability);
        state.log("Wi-Fi", "Availability loaded", status);
    }
    if let Some(permission) = ctx.input.capability_ok(REQUEST_WIFI_PERMISSION) {
        state.log(
            "Wi-Fi permission",
            format!("{:?}", permission),
            CapabilityState::Ready,
        );
    }
    if let Some(scan) = ctx.input.capability_ok(SCAN_WIFI_NETWORKS) {
        state.wifi_networks = scan.networks;
        state.log(
            "Wi-Fi",
            format!("{} network(s) visible", state.wifi_networks.len()),
            CapabilityState::Ready,
        );
    }
    if let Some(availability) = ctx.input.capability_ok(GET_BIOMETRIC_AVAILABILITY) {
        let status =
            secure_availability_state(Some(&availability), state.passkey_availability.as_ref());
        state.biometric_availability = Some(availability);
        state.log("Biometrics", "Availability loaded", status);
    }
    if let Some(result) = ctx.input.capability_ok(AUTHENTICATE_BIOMETRIC) {
        state.sensitive_unlocked = result.verified;
        state.log(
            "Biometric unlock",
            format!("verified {}", result.verified),
            CapabilityState::Complete,
        );
    }
    if let Some(availability) = ctx.input.capability_ok(GET_PASSKEY_AVAILABILITY) {
        let status =
            secure_availability_state(state.biometric_availability.as_ref(), Some(&availability));
        state.passkey_availability = Some(availability);
        state.log("Passkeys", "Availability loaded", status);
    }
    if let Some(result) = ctx.input.capability_ok(REGISTER_PASSKEY) {
        state.registered_passkey = Some(PasskeyCredentialDescriptor::new(
            result.credential_id.clone(),
            result.transports.clone(),
        ));
        state.passkey_verified = true;
        state.log(
            "Passkey",
            format!("registered credential {} bytes", result.credential_id.len()),
            CapabilityState::Complete,
        );
    }
    if let Some(result) = ctx.input.capability_ok(AUTHENTICATE_PASSKEY) {
        state.passkey_verified = true;
        state.log(
            "Passkey",
            format!("assertion {} bytes", result.signature.len()),
            CapabilityState::Complete,
        );
    }
    if ctx.input.capability_ok(WRITE_CLIPBOARD_TEXT).is_some() {
        state.log(
            "Clipboard",
            "Report summary copied",
            CapabilityState::Complete,
        );
    }
    if let Some(level) = ctx.input.capability_ok(GET_VOLUME_LEVEL) {
        state.volume_level = Some(level);
        state.log(
            "Volume",
            "Current media level loaded",
            CapabilityState::Ready,
        );
    }
    if let Some(level) = ctx.input.capability_ok(ADJUST_VOLUME_LEVEL) {
        state.volume_level = Some(level);
        state.log("Volume", "Alert volume adjusted", CapabilityState::Ready);
    }
    if ctx.input.capability_ok(HAPTIC_SELECTION).is_some()
        || ctx.input.capability_ok(HAPTIC_IMPACT).is_some()
        || ctx.input.capability_ok(HAPTIC_NOTIFICATION).is_some()
    {
        state.log(
            "Haptic feedback",
            "Feedback request accepted",
            CapabilityState::Complete,
        );
    }
}

#[fission_reducer(CapabilityFailed)]
pub fn on_capability_failed(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    let (capability, message) =
        if let Some(error) = ctx.input.capability_error(CAPTURE_MICROPHONE_AUDIO) {
            (
                "Microphone".into(),
                format!("{}: {}", error.code, error.message),
            )
        } else {
            match ctx.input.unscoped() {
                ActionInput::CapabilityErr {
                    capability,
                    message,
                    ..
                } => (
                    capability.clone(),
                    message
                        .clone()
                        .unwrap_or_else(|| "Capability request failed".into()),
                ),
                _ => ("capability".into(), "Capability request failed".into()),
            }
        };
    state.log(capability, message, CapabilityState::Error);
    ctx.effects
        .haptics()
        .notification(HapticNotificationRequest {
            kind: HapticNotificationKind::Error,
        });
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

pub fn nfc_uri_for_display(tag: &NfcTag) -> Option<String> {
    tag.records
        .iter()
        .find(|record| record.type_name == b"U")
        .and_then(|record| record.payload.get(1..))
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
}

#[cfg(test)]
mod tests {
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
                fission::core::NodeId::from_u128(0),
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
                fission::core::NodeId::from_u128(0),
                &ActionInput::CapabilityOk {
                    capability: CAPTURE_PHOTO.name.into(),
                    req_id: 1,
                    payload: serde_json::to_vec(&CameraCapture {
                        bytes: vec![42; 2049],
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
                fission::core::NodeId::from_u128(0),
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
                fission::core::NodeId::from_u128(0),
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
                fission::core::NodeId::from_u128(0),
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
            .dispatch(StartInspection.into(), fission::core::NodeId::from_u128(0))
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
}
