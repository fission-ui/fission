//! Maps host capability results onto the inspector's capability states.

use super::*;

pub(super) fn notification_settings_state(settings: &NotificationSettings) -> CapabilityState {
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

pub(super) fn geolocation_permission_state(permission: GeolocationPermission) -> CapabilityState {
    match permission {
        GeolocationPermission::Granted | GeolocationPermission::Prompt => CapabilityState::Ready,
        GeolocationPermission::Denied | GeolocationPermission::Unsupported => {
            CapabilityState::Unavailable
        }
        GeolocationPermission::Unknown => CapabilityState::Pending,
    }
}

pub(super) fn camera_availability_state(availability: &CameraAvailability) -> CapabilityState {
    if availability.permission == CameraPermission::Granted && !availability.devices.is_empty() {
        CapabilityState::Ready
    } else if availability.permission == CameraPermission::Unknown {
        CapabilityState::Pending
    } else {
        CapabilityState::Unavailable
    }
}

pub(super) fn camera_permission_state(permission: CameraPermission) -> CapabilityState {
    match permission {
        CameraPermission::Granted => CapabilityState::Ready,
        CameraPermission::Unknown => CapabilityState::Pending,
        CameraPermission::Denied | CameraPermission::Restricted => CapabilityState::Unavailable,
    }
}

pub(super) fn microphone_availability_state(
    availability: &MicrophoneAvailability,
) -> CapabilityState {
    if availability.permission == MicrophonePermission::Granted && !availability.devices.is_empty()
    {
        CapabilityState::Ready
    } else if availability.permission == MicrophonePermission::Unknown {
        CapabilityState::Pending
    } else {
        CapabilityState::Unavailable
    }
}

pub(super) fn microphone_permission_state(permission: MicrophonePermission) -> CapabilityState {
    match permission {
        MicrophonePermission::Granted => CapabilityState::Ready,
        MicrophonePermission::Unknown => CapabilityState::Pending,
        MicrophonePermission::Denied | MicrophonePermission::Restricted => {
            CapabilityState::Unavailable
        }
    }
}

pub(super) fn nfc_availability_state(availability: &NfcAvailability) -> CapabilityState {
    if availability.supported && availability.enabled && availability.read {
        CapabilityState::Ready
    } else {
        CapabilityState::Unavailable
    }
}

pub(super) fn bluetooth_availability_state(
    availability: &BluetoothAvailability,
) -> CapabilityState {
    if availability.permission == BluetoothPermission::Granted
        && availability.enabled
        && (availability.supports_classic || availability.supports_low_energy)
    {
        CapabilityState::Ready
    } else {
        CapabilityState::Unavailable
    }
}

pub(super) fn wifi_availability_state(availability: &WifiAvailability) -> CapabilityState {
    if availability.permission == WifiPermission::Granted && availability.enabled {
        CapabilityState::Ready
    } else {
        CapabilityState::Unavailable
    }
}

pub(super) fn secure_availability_state(
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

pub(super) fn kib(byte_len: u64) -> u64 {
    (byte_len.saturating_add(1023)) / 1024
}

pub(super) fn yes_no(value: bool) -> &'static str {
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
