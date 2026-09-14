use crate::api::{
    ApiError, StreamBytesRequest, WeatherRequest, WeatherSummary, STREAM_BYTES_JOB, WEATHER_JOB,
};
use crate::data::{work_orders, WorkOrder};
use fission::core::ActionInput;
use fission::prelude::*;
use std::collections::BTreeSet;

mod capability_lines;
mod capability_results;
mod evidence;
mod host_events;
mod inspection;
mod security;
mod sensors;
mod status;

pub use capability_results::*;
pub use evidence::*;
pub use host_events::*;
pub use inspection::*;
pub use security::*;
pub use sensors::*;
pub use status::nfc_uri_for_display;
use status::*;

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
    pub photo_preview: Option<Vec<u8>>,
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
            photo_preview: None,
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

impl GlobalState for FieldInspectorState {}

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
        self.photo_preview = None;
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

#[cfg(test)]
mod tests;
