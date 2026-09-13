//! The capability matrix shown on the overview panel.

use super::*;

impl FieldInspectorState {
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
                            p.byte_len.map(kib).unwrap_or_default()
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
                            n.byte_len.map(kib).unwrap_or_default()
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
}
