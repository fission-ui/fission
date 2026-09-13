//! Reducers that record what each host capability request returned.

use super::*;

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
            capture.byte_len.map(kib).unwrap_or_default()
        );
        let stream = capture.stream;
        state.photo_capture = Some(capture);
        state.photo_preview = None;
        state.complete_check("evidence");
        state.log("Photo", detail, CapabilityState::Complete);
        let preview_ok = ctx
            .effects
            .bind(PhotoPreviewLoaded, reduce_with!(on_photo_preview_loaded));
        let preview_err = ctx
            .effects
            .bind(PhotoPreviewFailed, reduce_with!(on_photo_preview_failed));
        ctx.effects
            .app(STREAM_BYTES_JOB, StreamBytesRequest { stream })
            .on_ok(preview_ok)
            .on_err(preview_err);
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
