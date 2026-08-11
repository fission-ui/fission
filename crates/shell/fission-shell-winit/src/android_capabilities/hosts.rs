use super::support::*;
use super::*;

pub(super) struct AndroidClipboardHost {
    context: AndroidHostContext,
}

impl AndroidClipboardHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }
}

impl ClipboardHost for AndroidClipboardHost {
    fn read_text(&self) -> Result<ClipboardText, ClipboardError> {
        let text = self
            .context
            .with_env(|env, activity| {
                let clipboard = system_service(env, activity, "clipboard")?;
                let has_clip = env
                    .call_method(&clipboard, "hasPrimaryClip", "()Z", &[])?
                    .z()?;
                if !has_clip {
                    return Ok(None);
                }
                let clip = env
                    .call_method(
                        &clipboard,
                        "getPrimaryClip",
                        "()Landroid/content/ClipData;",
                        &[],
                    )?
                    .l()?;
                if clip.as_raw().is_null() {
                    return Ok(None);
                }
                let item = env
                    .call_method(
                        &clip,
                        "getItemAt",
                        "(I)Landroid/content/ClipData$Item;",
                        &[JValue::Int(0)],
                    )?
                    .l()?;
                if item.as_raw().is_null() {
                    return Ok(None);
                }
                let text = env
                    .call_method(
                        &item,
                        "coerceToText",
                        "(Landroid/content/Context;)Ljava/lang/CharSequence;",
                        &[JValue::Object(activity)],
                    )?
                    .l()?;
                if text.as_raw().is_null() {
                    return Ok(None);
                }
                Ok(Some(char_sequence_to_string(env, &text)?))
            })
            .map_err(clipboard_host_error)?;
        Ok(ClipboardText { text })
    }

    fn write_text(&self, request: ClipboardWriteTextRequest) -> Result<(), ClipboardError> {
        self.context
            .with_env(|env, activity| {
                let clipboard = system_service(env, activity, "clipboard")?;
                let label = env.new_string("Fission")?;
                let label_obj = JObject::from(label);
                let text = env.new_string(&request.text)?;
                let text_obj = JObject::from(text);
                let clip = env
                    .call_static_method(
                        "android/content/ClipData",
                        "newPlainText",
                        "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;",
                        &[JValue::Object(&label_obj), JValue::Object(&text_obj)],
                    )?
                    .l()?;
                env.call_method(
                    &clipboard,
                    "setPrimaryClip",
                    "(Landroid/content/ClipData;)V",
                    &[JValue::Object(&clip)],
                )?;
                Ok(())
            })
            .map_err(clipboard_host_error)
    }

    fn read_content(&self) -> Result<Vec<ClipboardHostItem>, ClipboardError> {
        let text = self.read_text()?.text.unwrap_or_default();
        Ok(if text.is_empty() {
            Vec::new()
        } else {
            vec![ClipboardHostItem {
                content_type: "text/plain".into(),
                bytes: Bytes::from(text),
                suggested_name: None,
            }]
        })
    }

    fn write_content(&self, request: Vec<ClipboardHostItem>) -> Result<(), ClipboardError> {
        let Some(item) = request
            .into_iter()
            .find(|item| item.content_type.starts_with("text/plain"))
        else {
            return Err(ClipboardError::unsupported("write_content_non_text"));
        };
        let text = String::from_utf8(item.bytes.to_vec())
            .map_err(|error| ClipboardError::new("invalid_text", error.to_string()))?;
        self.write_text(ClipboardWriteTextRequest { text })
    }

    fn clear(&self) -> Result<(), ClipboardError> {
        self.write_text(ClipboardWriteTextRequest {
            text: String::new(),
        })
    }
}

pub(super) struct AndroidHapticHost {
    context: AndroidHostContext,
}

impl AndroidHapticHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }

    fn vibrate_one_shot(&self, duration_ms: i64, amplitude: i32) -> Result<(), HapticError> {
        self.context
            .with_env(|env, activity| {
                let vibrator = system_service(env, activity, "vibrator")?;
                ensure_vibrator(env, &vibrator)?;
                let sdk = env
                    .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
                    .i()?;
                if sdk >= 26 {
                    let effect = env
                        .call_static_method(
                            "android/os/VibrationEffect",
                            "createOneShot",
                            "(JI)Landroid/os/VibrationEffect;",
                            &[JValue::Long(duration_ms as jlong), JValue::Int(amplitude)],
                        )?
                        .l()?;
                    env.call_method(
                        &vibrator,
                        "vibrate",
                        "(Landroid/os/VibrationEffect;)V",
                        &[JValue::Object(&effect)],
                    )?;
                } else {
                    env.call_method(&vibrator, "vibrate", "(J)V", &[JValue::Long(duration_ms)])?;
                }
                Ok(())
            })
            .map_err(haptic_host_error)
    }
}

impl HapticHost for AndroidHapticHost {
    fn impact(&self, request: HapticImpactRequest) -> Result<(), HapticError> {
        let (duration_ms, amplitude) = match request.style {
            HapticImpactStyle::Light | HapticImpactStyle::Soft => (20, 80),
            HapticImpactStyle::Medium => (35, 160),
            HapticImpactStyle::Heavy | HapticImpactStyle::Rigid => (55, 255),
        };
        self.vibrate_one_shot(duration_ms, amplitude)
    }

    fn notification(&self, request: HapticNotificationRequest) -> Result<(), HapticError> {
        let (duration_ms, amplitude) = match request.kind {
            HapticNotificationKind::Success => (35, 150),
            HapticNotificationKind::Warning => (50, 200),
            HapticNotificationKind::Error => (75, 255),
        };
        self.vibrate_one_shot(duration_ms, amplitude)
    }

    fn selection(&self) -> Result<(), HapticError> {
        self.vibrate_one_shot(12, 80)
    }

    fn pattern(&self, request: HapticPatternRequest) -> Result<(), HapticError> {
        if request.steps.is_empty() {
            return Ok(());
        }
        self.context
            .with_env(|env, activity| {
                let vibrator = system_service(env, activity, "vibrator")?;
                ensure_vibrator(env, &vibrator)?;
                let sdk = env
                    .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
                    .i()?;
                let timings = env.new_long_array(request.steps.len() as jint)?;
                let timing_values = request
                    .steps
                    .iter()
                    .map(|step| step.duration_ms.min(i64::MAX as u64) as jlong)
                    .collect::<Vec<_>>();
                env.set_long_array_region(&timings, 0, &timing_values)?;
                if sdk >= 26 {
                    let amplitudes = env.new_int_array(request.steps.len() as jint)?;
                    let amplitude_values = request
                        .steps
                        .iter()
                        .map(|step| step.intensity.clamp(1, 255) as jint)
                        .collect::<Vec<_>>();
                    env.set_int_array_region(&amplitudes, 0, &amplitude_values)?;
                    let timings_obj = JObject::from(timings);
                    let amplitudes_obj = JObject::from(amplitudes);
                    let effect = env
                        .call_static_method(
                            "android/os/VibrationEffect",
                            "createWaveform",
                            "([J[II)Landroid/os/VibrationEffect;",
                            &[
                                JValue::Object(&timings_obj),
                                JValue::Object(&amplitudes_obj),
                                JValue::Int(-1),
                            ],
                        )?
                        .l()?;
                    env.call_method(
                        &vibrator,
                        "vibrate",
                        "(Landroid/os/VibrationEffect;)V",
                        &[JValue::Object(&effect)],
                    )?;
                } else {
                    let timings_obj = JObject::from(timings);
                    env.call_method(
                        &vibrator,
                        "vibrate",
                        "([JI)V",
                        &[JValue::Object(&timings_obj), JValue::Int(-1)],
                    )?;
                }
                Ok(())
            })
            .map_err(haptic_host_error)
    }
}

pub(super) struct AndroidVolumeHost {
    context: AndroidHostContext,
}

impl AndroidVolumeHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }
}

impl VolumeHost for AndroidVolumeHost {
    fn get_level(&self, stream: VolumeStream) -> Result<VolumeLevel, VolumeError> {
        self.context
            .with_env(|env, activity| android_get_volume_level(env, activity, stream))
            .map_err(volume_host_error)
    }

    fn set_level(&self, request: VolumeSetRequest) -> Result<VolumeLevel, VolumeError> {
        self.context
            .with_env(|env, activity| {
                let audio = system_service(env, activity, "audio")?;
                let stream = android_stream_id(request.stream);
                let max = env
                    .call_method(&audio, "getStreamMaxVolume", "(I)I", &[JValue::Int(stream)])?
                    .i()?
                    .max(1);
                let platform_level = percent_to_platform_volume(request.level.min(100), max);
                env.call_method(
                    &audio,
                    "setStreamVolume",
                    "(III)V",
                    &[
                        JValue::Int(stream),
                        JValue::Int(platform_level),
                        JValue::Int(0),
                    ],
                )?;
                if let Some(muted) = request.muted {
                    let direction = if muted { -100 } else { 100 };
                    env.call_method(
                        &audio,
                        "adjustStreamVolume",
                        "(III)V",
                        &[JValue::Int(stream), JValue::Int(direction), JValue::Int(0)],
                    )?;
                }
                android_get_volume_level(env, activity, request.stream)
            })
            .map_err(volume_host_error)
    }

    fn adjust_level(&self, request: VolumeAdjustRequest) -> Result<VolumeLevel, VolumeError> {
        let current = self.get_level(request.stream)?;
        let next = match request.direction {
            VolumeAdjustDirection::Up => current.level.saturating_add(request.step).min(100),
            VolumeAdjustDirection::Down => current.level.saturating_sub(request.step),
        };
        self.set_level(VolumeSetRequest {
            stream: request.stream,
            level: next,
            muted: None,
        })
    }
}

pub(super) struct AndroidNotificationHost {
    context: AndroidHostContext,
}

impl AndroidNotificationHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }

    fn notification_settings(&self) -> Result<NotificationSettings, NotificationError> {
        let sdk = self.context.sdk_int().map_err(notification_host_error)?;
        let permission_granted = if sdk >= 33 {
            self.context
                .permission_granted(PERMISSION_POST_NOTIFICATIONS)
                .map_err(notification_host_error)?
        } else {
            true
        };
        let enabled = if permission_granted {
            self.context
                .with_env(|env, activity| {
                    let manager = system_service(env, activity, "notification")?;
                    if sdk >= 24 {
                        env.call_method(&manager, "areNotificationsEnabled", "()Z", &[])?
                            .z()
                    } else {
                        Ok(true)
                    }
                })
                .unwrap_or(true)
        } else {
            false
        };
        Ok(NotificationSettings {
            permission: if enabled {
                NotificationPermission::Granted
            } else {
                NotificationPermission::Denied
            },
            alerts: enabled,
            badge: false,
            sound: enabled,
            scheduling: true,
            push: false,
        })
    }
}

impl NotificationHost for AndroidNotificationHost {
    fn request_permission(
        &self,
        _request: NotificationPermissionRequest,
    ) -> Result<NotificationSettings, NotificationError> {
        let sdk = self.context.sdk_int().map_err(notification_host_error)?;
        if sdk >= 33
            && !self
                .context
                .permission_granted(PERMISSION_POST_NOTIFICATIONS)
                .map_err(notification_host_error)?
        {
            self.context
                .request_permissions(&[PERMISSION_POST_NOTIFICATIONS], REQUEST_CODE_NOTIFICATIONS)
                .map_err(notification_host_error)?;
        }
        self.notification_settings()
    }

    fn settings(&self) -> Result<NotificationSettings, NotificationError> {
        self.notification_settings()
    }

    fn show(&self, request: NotificationRequest) -> Result<NotificationReceipt, NotificationError> {
        match request.schedule {
            NotificationSchedule::Immediate => {}
            _ => return Err(NotificationError::unsupported("schedule")),
        }
        if self.notification_settings()?.permission != NotificationPermission::Granted {
            return Err(NotificationError::new(
                "permission_denied",
                "Android notification permission is not granted",
            ));
        }
        self.context
            .with_env(|env, activity| {
                let sdk = env
                    .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
                    .i()?;
                let manager = system_service(env, activity, "notification")?;
                let channel_id = env.new_string("fission-default")?;
                let channel_id_obj = JObject::from(channel_id);
                if sdk >= 26 {
                    let channel_name = env.new_string("Fission")?;
                    let channel_name_obj = JObject::from(channel_name);
                    let importance = env
                        .get_static_field(
                            "android/app/NotificationManager",
                            "IMPORTANCE_DEFAULT",
                            "I",
                        )?
                        .i()?;
                    let channel = env.new_object(
                        "android/app/NotificationChannel",
                        "(Ljava/lang/String;Ljava/lang/CharSequence;I)V",
                        &[
                            JValue::Object(&channel_id_obj),
                            JValue::Object(&channel_name_obj),
                            JValue::Int(importance),
                        ],
                    )?;
                    env.call_method(
                        &manager,
                        "createNotificationChannel",
                        "(Landroid/app/NotificationChannel;)V",
                        &[JValue::Object(&channel)],
                    )?;
                }

                let builder = if sdk >= 26 {
                    env.new_object(
                        "android/app/Notification$Builder",
                        "(Landroid/content/Context;Ljava/lang/String;)V",
                        &[JValue::Object(activity), JValue::Object(&channel_id_obj)],
                    )?
                } else {
                    env.new_object(
                        "android/app/Notification$Builder",
                        "(Landroid/content/Context;)V",
                        &[JValue::Object(activity)],
                    )?
                };
                let icon = env
                    .get_static_field("android/R$drawable", "ic_dialog_info", "I")?
                    .i()?;
                env.call_method(
                    &builder,
                    "setSmallIcon",
                    "(I)Landroid/app/Notification$Builder;",
                    &[JValue::Int(icon)],
                )?;
                let title = env.new_string(&request.title)?;
                let title_obj = JObject::from(title);
                env.call_method(
                    &builder,
                    "setContentTitle",
                    "(Ljava/lang/CharSequence;)Landroid/app/Notification$Builder;",
                    &[JValue::Object(&title_obj)],
                )?;
                let body = env.new_string(&request.body)?;
                let body_obj = JObject::from(body);
                env.call_method(
                    &builder,
                    "setContentText",
                    "(Ljava/lang/CharSequence;)Landroid/app/Notification$Builder;",
                    &[JValue::Object(&body_obj)],
                )?;
                env.call_method(
                    &builder,
                    "setAutoCancel",
                    "(Z)Landroid/app/Notification$Builder;",
                    &[JValue::Bool(JNI_TRUE)],
                )?;
                let notification = env
                    .call_method(&builder, "build", "()Landroid/app/Notification;", &[])?
                    .l()?;
                env.call_method(
                    &manager,
                    "notify",
                    "(ILandroid/app/Notification;)V",
                    &[
                        JValue::Int(notification_id_to_i32(&request.id)),
                        JValue::Object(&notification),
                    ],
                )?;
                Ok(())
            })
            .map_err(notification_host_error)?;
        Ok(NotificationReceipt {
            id: request.id,
            scheduled: false,
            delivered: true,
        })
    }

    fn schedule(
        &self,
        request: NotificationRequest,
    ) -> Result<NotificationReceipt, NotificationError> {
        match request.schedule {
            NotificationSchedule::Immediate => self.show(request),
            NotificationSchedule::AfterMillis(ms) => {
                let mut deliver = request.clone();
                deliver.schedule = NotificationSchedule::Immediate;
                let host = AndroidNotificationHost::new(self.context.clone());
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    let _ = host.show(deliver);
                });
                Ok(NotificationReceipt {
                    id: request.id,
                    scheduled: true,
                    delivered: false,
                })
            }
            NotificationSchedule::AtUnixMillis(ms) => {
                let now_ms = current_unix_ms();
                let delay = ms.saturating_sub(now_ms);
                let mut deliver = request.clone();
                deliver.schedule = NotificationSchedule::Immediate;
                let host = AndroidNotificationHost::new(self.context.clone());
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    let _ = host.show(deliver);
                });
                Ok(NotificationReceipt {
                    id: request.id,
                    scheduled: true,
                    delivered: false,
                })
            }
        }
    }

    fn cancel(&self, request: CancelNotificationRequest) -> Result<(), NotificationError> {
        self.context
            .with_env(|env, activity| {
                let manager = system_service(env, activity, "notification")?;
                env.call_method(
                    &manager,
                    "cancel",
                    "(I)V",
                    &[JValue::Int(notification_id_to_i32(&request.id))],
                )?;
                Ok(())
            })
            .map_err(notification_host_error)
    }

    fn cancel_all(&self) -> Result<(), NotificationError> {
        self.context
            .with_env(|env, activity| {
                let manager = system_service(env, activity, "notification")?;
                env.call_method(&manager, "cancelAll", "()V", &[])?;
                Ok(())
            })
            .map_err(notification_host_error)
    }

    fn set_badge_count(&self, _request: SetBadgeCountRequest) -> Result<(), NotificationError> {
        Err(NotificationError::unsupported("set_badge_count"))
    }

    fn register_push(
        &self,
        _request: PushRegistrationRequest,
    ) -> Result<PushRegistration, NotificationError> {
        Err(NotificationError::unsupported("register_push"))
    }

    fn unregister_push(&self) -> Result<(), NotificationError> {
        Err(NotificationError::unsupported("unregister_push"))
    }
}

pub(super) struct AndroidWifiHost {
    context: AndroidHostContext,
}

impl AndroidWifiHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }

    fn permission(&self) -> Result<WifiPermission, WifiError> {
        let sdk = self.context.sdk_int().map_err(wifi_host_error)?;
        let granted = if sdk >= 33 {
            self.context
                .any_permission_granted(&[
                    PERMISSION_NEARBY_WIFI_DEVICES,
                    PERMISSION_ACCESS_FINE_LOCATION,
                    PERMISSION_ACCESS_COARSE_LOCATION,
                ])
                .map_err(wifi_host_error)?
        } else {
            self.context
                .any_permission_granted(&[
                    PERMISSION_ACCESS_FINE_LOCATION,
                    PERMISSION_ACCESS_COARSE_LOCATION,
                ])
                .map_err(wifi_host_error)?
        };
        Ok(if granted {
            WifiPermission::Granted
        } else {
            WifiPermission::Denied
        })
    }
}

impl WifiHost for AndroidWifiHost {
    fn availability(&self) -> Result<WifiAvailability, WifiError> {
        let permission = self.permission()?;
        self.context
            .with_env(|env, activity| {
                let wifi = system_service(env, activity, "wifi")?;
                let enabled = env.call_method(&wifi, "isWifiEnabled", "()Z", &[])?.z()?;
                let connected_network = if permission == WifiPermission::Granted {
                    android_connected_wifi(env, &wifi).ok().flatten()
                } else {
                    None
                };
                Ok(WifiAvailability {
                    permission,
                    enabled,
                    connected_network,
                })
            })
            .map_err(wifi_host_error)
    }

    fn request_permission(
        &self,
        _request: WifiPermissionRequest,
    ) -> Result<WifiPermission, WifiError> {
        let sdk = self.context.sdk_int().map_err(wifi_host_error)?;
        let permissions = if sdk >= 33 {
            &[PERMISSION_NEARBY_WIFI_DEVICES][..]
        } else {
            &[PERMISSION_ACCESS_FINE_LOCATION][..]
        };
        self.context
            .request_permissions(permissions, REQUEST_CODE_WIFI)
            .map_err(wifi_host_error)?;
        self.permission()
    }

    fn scan_networks(&self, request: WifiScanRequest) -> Result<WifiScanResult, WifiError> {
        if self.permission()? != WifiPermission::Granted {
            return Err(WifiError::new(
                "permission_denied",
                "Android Wi-Fi scan requires location or nearby-device permission",
            ));
        }
        self.context
            .with_env(|env, activity| {
                let wifi = system_service(env, activity, "wifi")?;
                let _ = env.call_method(&wifi, "startScan", "()Z", &[]);
                let results = env
                    .call_method(&wifi, "getScanResults", "()Ljava/util/List;", &[])?
                    .l()?;
                if results.as_raw().is_null() {
                    return Ok(WifiScanResult { networks: vec![] });
                }
                let size = env.call_method(&results, "size", "()I", &[])?.i()?.max(0);
                let mut networks = Vec::new();
                for index in 0..size {
                    let result = env
                        .call_method(
                            &results,
                            "get",
                            "(I)Ljava/lang/Object;",
                            &[JValue::Int(index)],
                        )?
                        .l()?;
                    if result.as_raw().is_null() {
                        continue;
                    }
                    if let Some(network) = android_wifi_scan_result(env, &result)? {
                        if !request.include_hidden && network.ssid.is_empty() {
                            continue;
                        }
                        if let Some(prefix) = request.ssid_prefix.as_deref() {
                            if !network.ssid.starts_with(prefix) {
                                continue;
                            }
                        }
                        networks.push(network);
                    }
                }
                Ok(WifiScanResult { networks })
            })
            .map_err(wifi_host_error)
    }

    fn connect_network(&self, _request: WifiConnectRequest) -> Result<WifiConnection, WifiError> {
        Err(WifiError::unsupported("connect_network"))
    }

    fn disconnect_network(&self, _request: WifiDisconnectRequest) -> Result<(), WifiError> {
        Err(WifiError::unsupported("disconnect_network"))
    }
}

pub(super) struct AndroidBluetoothHost {
    context: AndroidHostContext,
}

impl AndroidBluetoothHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }

    fn permission(&self) -> Result<BluetoothPermission, BluetoothError> {
        let sdk = self.context.sdk_int().map_err(bluetooth_host_error)?;
        let granted = if sdk >= 31 {
            self.context
                .all_permissions_granted(&[PERMISSION_BLUETOOTH_CONNECT, PERMISSION_BLUETOOTH_SCAN])
                .map_err(bluetooth_host_error)?
        } else {
            self.context
                .any_permission_granted(&[
                    PERMISSION_ACCESS_FINE_LOCATION,
                    PERMISSION_ACCESS_COARSE_LOCATION,
                ])
                .map_err(bluetooth_host_error)?
        };
        Ok(if granted {
            BluetoothPermission::Granted
        } else {
            BluetoothPermission::Denied
        })
    }
}

impl BluetoothHost for AndroidBluetoothHost {
    fn availability(&self) -> Result<BluetoothAvailability, BluetoothError> {
        let permission = self.permission()?;
        let supports_classic = self
            .context
            .has_system_feature("android.hardware.bluetooth")
            .unwrap_or(false);
        let supports_low_energy = self
            .context
            .has_system_feature("android.hardware.bluetooth_le")
            .unwrap_or(false);
        let enabled = self
            .context
            .with_env(|env, _activity| {
                let adapter = env
                    .call_static_method(
                        "android/bluetooth/BluetoothAdapter",
                        "getDefaultAdapter",
                        "()Landroid/bluetooth/BluetoothAdapter;",
                        &[],
                    )?
                    .l()?;
                if adapter.as_raw().is_null() {
                    return Ok(false);
                }
                env.call_method(&adapter, "isEnabled", "()Z", &[])?.z()
            })
            .map_err(bluetooth_host_error)?;
        Ok(BluetoothAvailability {
            permission,
            enabled,
            supports_classic,
            supports_low_energy,
        })
    }

    fn request_permission(
        &self,
        _request: BluetoothPermissionRequest,
    ) -> Result<BluetoothPermission, BluetoothError> {
        let sdk = self.context.sdk_int().map_err(bluetooth_host_error)?;
        let permissions = if sdk >= 31 {
            &[PERMISSION_BLUETOOTH_SCAN, PERMISSION_BLUETOOTH_CONNECT][..]
        } else {
            &[PERMISSION_ACCESS_FINE_LOCATION][..]
        };
        self.context
            .request_permissions(permissions, REQUEST_CODE_BLUETOOTH)
            .map_err(bluetooth_host_error)?;
        self.permission()
    }

    fn scan_devices(
        &self,
        request: BluetoothScanRequest,
    ) -> Result<BluetoothScanResult, BluetoothError> {
        if self.permission()? != BluetoothPermission::Granted {
            return Err(BluetoothError::new(
                "permission_denied",
                "Android Bluetooth scan/connect permission is not granted",
            ));
        }
        self.context
            .with_env(|env, activity| {
                let helper = app_class(
                    env,
                    activity,
                    "rs.fission.runtime.FissionAndroidCapabilities",
                )?;
                let service_uuids = java_string_array(env, &request.service_uuids)?;
                let service_uuids_obj = JObject::from(service_uuids);
                let timeout = request.timeout_ms.unwrap_or(3_000).min(i32::MAX as u64) as jlong;
                let rows = env
                    .call_static_method(
                        helper,
                        "scanBluetoothDevices",
                        "(Landroid/app/Activity;[Ljava/lang/String;ZZJ)[Ljava/lang/String;",
                        &[
                            JValue::Object(activity),
                            JValue::Object(&service_uuids_obj),
                            JValue::Bool(request.include_paired as u8),
                            JValue::Bool(request.allow_duplicates as u8),
                            JValue::Long(timeout),
                        ],
                    )?
                    .l()?;
                let rows: JObjectArray<'_> = JObjectArray::from(rows);
                let len = env.get_array_length(&rows)?;
                let mut devices = Vec::new();
                for index in 0..len {
                    let row = env.get_object_array_element(&rows, index)?;
                    let row = java_string(env, row)?;
                    if let Some(device) = android_bluetooth_device_row(&row) {
                        devices.push(device);
                    }
                }
                Ok(BluetoothScanResult { devices })
            })
            .map_err(bluetooth_host_error)
    }

    fn connect_device(
        &self,
        _request: BluetoothConnectRequest,
    ) -> Result<BluetoothConnection, BluetoothError> {
        Err(BluetoothError::unsupported("connect_device"))
    }

    fn disconnect_device(
        &self,
        _request: BluetoothDisconnectRequest,
    ) -> Result<(), BluetoothError> {
        Err(BluetoothError::unsupported("disconnect_device"))
    }

    fn read_characteristic(
        &self,
        _request: BluetoothReadRequest,
    ) -> Result<BluetoothReadResult, BluetoothError> {
        Err(BluetoothError::unsupported("read_characteristic"))
    }

    fn write_characteristic(&self, _request: BluetoothWriteRequest) -> Result<(), BluetoothError> {
        Err(BluetoothError::unsupported("write_characteristic"))
    }

    fn start_advertising(
        &self,
        _request: BluetoothAdvertiseRequest,
    ) -> Result<BluetoothAdvertiseReceipt, BluetoothError> {
        Err(BluetoothError::unsupported("start_advertising"))
    }

    fn stop_advertising(
        &self,
        _request: BluetoothStopAdvertiseRequest,
    ) -> Result<(), BluetoothError> {
        Err(BluetoothError::unsupported("stop_advertising"))
    }
}

pub(super) struct AndroidGeolocationHost {
    context: AndroidHostContext,
}

impl AndroidGeolocationHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }

    fn permission_state(&self) -> Result<GeolocationPermission, GeolocationError> {
        let granted = self
            .context
            .any_permission_granted(&[
                PERMISSION_ACCESS_FINE_LOCATION,
                PERMISSION_ACCESS_COARSE_LOCATION,
            ])
            .map_err(geolocation_host_error)?;
        Ok(if granted {
            GeolocationPermission::Granted
        } else {
            GeolocationPermission::Denied
        })
    }
}

impl GeolocationHost for AndroidGeolocationHost {
    fn permission(&self) -> Result<GeolocationPermission, GeolocationError> {
        self.permission_state()
    }

    fn request_permission(
        &self,
        request: GeolocationPermissionRequest,
    ) -> Result<GeolocationPermission, GeolocationError> {
        let permissions = if request.precise {
            &[
                PERMISSION_ACCESS_FINE_LOCATION,
                PERMISSION_ACCESS_COARSE_LOCATION,
            ][..]
        } else {
            &[PERMISSION_ACCESS_COARSE_LOCATION][..]
        };
        self.context
            .request_permissions(permissions, REQUEST_CODE_GEOLOCATION)
            .map_err(geolocation_host_error)?;
        self.permission_state()
    }

    fn current_position(
        &self,
        request: GeolocationPositionRequest,
    ) -> Result<GeolocationPosition, GeolocationError> {
        if self.permission_state()? != GeolocationPermission::Granted {
            return Err(GeolocationError::new(
                "permission_denied",
                "Android location permission is not granted",
            ));
        }
        self.context
            .with_env(|env, activity| android_current_position(env, activity, request))
            .map_err(geolocation_host_error)
    }
}

pub(super) struct AndroidMicrophoneHost {
    context: AndroidHostContext,
}

impl AndroidMicrophoneHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }

    fn permission_state(&self) -> Result<MicrophonePermission, MicrophoneError> {
        let granted = self
            .context
            .permission_granted(PERMISSION_RECORD_AUDIO)
            .map_err(microphone_host_error)?;
        Ok(if granted {
            MicrophonePermission::Granted
        } else {
            MicrophonePermission::Denied
        })
    }
}

impl MicrophoneHost for AndroidMicrophoneHost {
    fn availability(&self) -> Result<MicrophoneAvailability, MicrophoneError> {
        let permission = self.permission_state()?;
        let has_microphone = self
            .context
            .has_system_feature("android.hardware.microphone")
            .unwrap_or(true);
        Ok(MicrophoneAvailability {
            permission,
            devices: if has_microphone {
                vec![MicrophoneDevice {
                    id: "android-default-microphone".into(),
                    label: Some("Android default microphone".into()),
                    is_default: true,
                }]
            } else {
                Vec::new()
            },
        })
    }

    fn request_permission(
        &self,
        _request: MicrophonePermissionRequest,
    ) -> Result<MicrophonePermission, MicrophoneError> {
        self.context
            .request_permissions(&[PERMISSION_RECORD_AUDIO], REQUEST_CODE_MICROPHONE)
            .map_err(microphone_host_error)?;
        self.permission_state()
    }

    fn capture_audio(
        &self,
        request: MicrophoneCaptureRequest,
        ctx: &CapabilityCtx,
    ) -> Result<MicrophoneCapture, MicrophoneError> {
        if self.permission_state()? != MicrophonePermission::Granted {
            return Err(MicrophoneError::new(
                "permission_denied",
                "Android microphone permission is not granted",
            ));
        }
        self.context
            .with_env(|env, _activity| android_capture_audio(env, request, ctx))
            .map_err(microphone_host_error)
    }

    fn cancel_capture(&self) -> Result<(), MicrophoneError> {
        Ok(())
    }
}

pub(super) struct AndroidCameraHost {
    context: AndroidHostContext,
}

impl AndroidCameraHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }

    fn permission_state(&self) -> Result<CameraPermission, CameraError> {
        let granted = self
            .context
            .permission_granted(PERMISSION_CAMERA)
            .map_err(camera_host_error)?;
        Ok(if granted {
            CameraPermission::Granted
        } else {
            CameraPermission::Denied
        })
    }
}

impl CameraHost for AndroidCameraHost {
    fn availability(&self) -> Result<CameraAvailability, CameraError> {
        let permission = self.permission_state()?;
        self.context
            .with_env(|env, activity| {
                let manager = system_service(env, activity, "camera")?;
                let ids = env
                    .call_method(&manager, "getCameraIdList", "()[Ljava/lang/String;", &[])?
                    .l()?;
                let ids: JObjectArray<'_> = JObjectArray::from(ids);
                let len = env.get_array_length(&ids)?;
                let mut devices = Vec::new();
                for index in 0..len {
                    let id_obj = env.get_object_array_element(&ids, index)?;
                    let id = java_string(env, id_obj)?;
                    let facing = android_camera_facing(env, &manager, &id).unwrap_or_default();
                    let has_flashlight =
                        android_camera_has_flashlight(env, &manager, &id).unwrap_or(false);
                    devices.push(CameraDevice {
                        id: id.clone(),
                        label: Some(format!("Android camera {id}")),
                        facing,
                        has_flashlight,
                    });
                }
                Ok(CameraAvailability {
                    permission,
                    devices,
                })
            })
            .map_err(camera_host_error)
    }

    fn request_permission(
        &self,
        _request: CameraPermissionRequest,
    ) -> Result<CameraPermission, CameraError> {
        self.context
            .request_permissions(&[PERMISSION_CAMERA], REQUEST_CODE_CAMERA)
            .map_err(camera_host_error)?;
        self.permission_state()
    }

    fn capture_photo(
        &self,
        request: CameraCaptureRequest,
        ctx: &CapabilityCtx,
    ) -> Result<CameraCapture, CameraError> {
        if self.permission_state()? != CameraPermission::Granted {
            return Err(CameraError::new(
                "permission_denied",
                "Android camera permission is not granted",
            ));
        }
        android_capture_photo(&self.context, request, ctx).map_err(camera_host_error)
    }

    fn set_flashlight(&self, request: CameraFlashlightRequest) -> Result<(), CameraError> {
        if self.permission_state()? != CameraPermission::Granted {
            return Err(CameraError::new(
                "permission_denied",
                "Android camera permission is not granted",
            ));
        }
        self.context
            .with_env(|env, activity| {
                let manager = system_service(env, activity, "camera")?;
                let camera_id = match request.camera_id.as_deref() {
                    Some(id) => id.to_string(),
                    None => android_torch_camera_id(env, &manager)?,
                };
                let id = env.new_string(camera_id)?;
                let id_obj = JObject::from(id);
                env.call_method(
                    &manager,
                    "setTorchMode",
                    "(Ljava/lang/String;Z)V",
                    &[JValue::Object(&id_obj), JValue::Bool(request.enabled as u8)],
                )?;
                Ok(())
            })
            .map_err(camera_host_error)
    }

    fn cancel_capture(&self) -> Result<(), CameraError> {
        Ok(())
    }
}

pub(super) struct AndroidBarcodeScannerHost {
    context: AndroidHostContext,
}

impl AndroidBarcodeScannerHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }
}

impl BarcodeScannerHost for AndroidBarcodeScannerHost {
    fn scan(&self, request: BarcodeScanRequest) -> Result<BarcodeScanResults, BarcodeScannerError> {
        let capture = android_capture_photo_bytes(
            &self.context,
            CameraCaptureRequest {
                camera_id: request.camera_id,
                facing: CameraFacing::Back,
                resolution: None,
                format: fission_core::CameraImageFormat::Jpeg,
                flash: fission_core::CameraFlashMode::Auto,
                quality: Some(90),
            },
        )
        .map_err(barcode_camera_error)?;
        let mut results = barcode_decode::decode_barcode_bytes(&capture.bytes, &request.formats)?;
        if !request.allow_multiple {
            results.items.truncate(1);
        }
        Ok(results)
    }

    fn decode_image(
        &self,
        request: BarcodeImageDecodeRequest,
        image: Bytes,
    ) -> Result<BarcodeScanResults, BarcodeScannerError> {
        barcode_decode::decode_barcode_bytes(&image, &request.formats)
    }

    fn cancel_scan(&self) -> Result<(), BarcodeScannerError> {
        Ok(())
    }
}

pub(super) struct AndroidBiometricHost {
    context: AndroidHostContext,
}

impl AndroidBiometricHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }
}

impl BiometricHost for AndroidBiometricHost {
    fn availability(&self) -> Result<BiometricAvailability, BiometricError> {
        let sdk = self.context.sdk_int().map_err(biometric_host_error)?;
        if sdk < 29 {
            return Ok(BiometricAvailability {
                reason: Some("Android BiometricManager requires API 29 or newer".into()),
                ..Default::default()
            });
        }
        let can_authenticate = self
            .context
            .with_env(|env, activity| {
                let manager = system_service(env, activity, "biometric")?;
                env.call_method(&manager, "canAuthenticate", "()I", &[])?
                    .i()
            })
            .map_err(biometric_host_error)?;
        let available = can_authenticate == 0;
        Ok(BiometricAvailability {
            supported: available,
            enrolled: available,
            strong: available,
            weak: available,
            device_credential: true,
            kinds: vec![
                BiometricKind::Fingerprint,
                BiometricKind::Face,
                BiometricKind::DeviceCredential,
            ],
            reason: if available {
                None
            } else {
                Some(format!(
                    "Android BiometricManager canAuthenticate returned {can_authenticate}"
                ))
            },
        })
    }

    fn authenticate(
        &self,
        request: BiometricAuthenticateRequest,
    ) -> Result<BiometricAuthenticateResult, BiometricError> {
        if !self.availability()?.supported {
            return Err(BiometricError::new(
                "unavailable",
                "Android biometric authentication is not available",
            ));
        }
        let row = self
            .context
            .with_env(|env, activity| {
                let helper = app_class(
                    env,
                    activity,
                    "rs.fission.runtime.FissionAndroidCapabilities",
                )?;
                let title = env.new_string(request.title.as_deref().unwrap_or("Authenticate"))?;
                let title_obj = JObject::from(title);
                let subtitle = env.new_string(request.subtitle.as_deref().unwrap_or(""))?;
                let subtitle_obj = JObject::from(subtitle);
                let reason = env.new_string(&request.reason)?;
                let reason_obj = JObject::from(reason);
                let timeout = 30_000_i64;
                let row = env
                    .call_static_method(
                        helper,
                        "authenticateBiometric",
                        "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZJ)Ljava/lang/String;",
                        &[
                            JValue::Object(activity),
                            JValue::Object(&title_obj),
                            JValue::Object(&subtitle_obj),
                            JValue::Object(&reason_obj),
                            JValue::Bool(request.allow_device_credential as u8),
                            JValue::Long(timeout),
                        ],
                    )?
                    .l()?;
                java_string(env, row)
            })
            .map_err(biometric_host_error)?;
        android_biometric_auth_result(&row)
            .map_err(|(code, message)| BiometricError::new(code, message))
    }

    fn cancel_authentication(&self) -> Result<(), BiometricError> {
        Ok(())
    }
}

pub(super) struct AndroidNfcHost {
    context: AndroidHostContext,
}

impl AndroidNfcHost {
    pub(super) fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }
}

impl NfcHost for AndroidNfcHost {
    fn availability(&self) -> Result<NfcAvailability, NfcError> {
        self.context
            .with_env(|env, activity| {
                let adapter = env
                    .call_static_method(
                        "android/nfc/NfcAdapter",
                        "getDefaultAdapter",
                        "(Landroid/content/Context;)Landroid/nfc/NfcAdapter;",
                        &[JValue::Object(activity)],
                    )?
                    .l()?;
                if adapter.as_raw().is_null() {
                    return Ok(NfcAvailability::default());
                }
                let enabled = env.call_method(&adapter, "isEnabled", "()Z", &[])?.z()?;
                Ok(NfcAvailability {
                    supported: true,
                    enabled,
                    read: enabled,
                    write: enabled,
                    card_emulation: false,
                })
            })
            .map_err(nfc_host_error)
    }

    fn scan_tag(&self, _request: NfcScanRequest) -> Result<NfcTag, NfcError> {
        Err(NfcError::unsupported("scan_tag"))
    }

    fn write_tag(&self, _request: NfcWriteRequest) -> Result<NfcSessionReceipt, NfcError> {
        Err(NfcError::unsupported("write_tag"))
    }

    fn emulate_tag(&self, _request: NfcEmulationRequest) -> Result<NfcSessionReceipt, NfcError> {
        Err(NfcError::unsupported("emulate_tag"))
    }

    fn cancel_session(&self) -> Result<(), NfcError> {
        Ok(())
    }
}
