use crate::{
    barcode, barcode_decode, biometric, bluetooth, camera, clipboard, geolocation, haptics,
    microphone, nfc, notifications, volume, wifi, BarcodeScannerHost, BiometricHost, BluetoothHost,
    CameraHost, ClipboardHost, GeolocationHost, HapticHost, MicrophoneHost, NfcHost,
    NotificationHost, VolumeHost, WifiHost,
};
use fission_core::{
    AudioSampleFormat, BarcodeImageDecodeRequest, BarcodeScanRequest, BarcodeScanResults,
    BarcodeScannerError, BiometricAuthenticateRequest, BiometricAuthenticateResult,
    BiometricAvailability, BiometricError, BiometricKind, BluetoothAdvertiseReceipt,
    BluetoothAdvertiseRequest, BluetoothAvailability, BluetoothConnectRequest, BluetoothConnection,
    BluetoothDevice, BluetoothDisconnectRequest, BluetoothError, BluetoothMode,
    BluetoothPermission, BluetoothPermissionRequest, BluetoothReadRequest, BluetoothReadResult,
    BluetoothScanRequest, BluetoothScanResult, BluetoothStopAdvertiseRequest,
    BluetoothWriteRequest, CameraAvailability, CameraCapture, CameraCaptureRequest, CameraDevice,
    CameraError, CameraFacing, CameraFlashlightRequest, CameraPermission, CameraPermissionRequest,
    CancelNotificationRequest, ClipboardContent, ClipboardError, ClipboardItem, ClipboardText,
    ClipboardWriteTextRequest, GeolocationError, GeolocationPermission,
    GeolocationPermissionRequest, GeolocationPosition, GeolocationPositionRequest, HapticError,
    HapticImpactRequest, HapticImpactStyle, HapticNotificationKind, HapticNotificationRequest,
    HapticPatternRequest, MicrophoneAvailability, MicrophoneCapture, MicrophoneCaptureRequest,
    MicrophoneDevice, MicrophoneError, MicrophonePermission, MicrophonePermissionRequest,
    NfcAvailability, NfcEmulationRequest, NfcError, NfcScanRequest, NfcSessionReceipt, NfcTag,
    NfcWriteRequest, NotificationError, NotificationId, NotificationPermission,
    NotificationPermissionRequest, NotificationReceipt, NotificationRequest, NotificationSchedule,
    NotificationSettings, PushRegistration, PushRegistrationRequest, SetBadgeCountRequest,
    VolumeAdjustDirection, VolumeAdjustRequest, VolumeError, VolumeLevel, VolumeSetRequest,
    VolumeStream, WifiAvailability, WifiConnectRequest, WifiConnection, WifiDisconnectRequest,
    WifiError, WifiNetwork, WifiPermission, WifiPermissionRequest, WifiScanRequest, WifiScanResult,
    WifiSecurity,
};
use fission_shell::async_host::AsyncRegistry;
use jni::objects::{
    JByteArray, JClass, JDoubleArray, JObject, JObjectArray, JShortArray, JString, JValue,
};
use jni::sys::{jint, jlong, jobject, jshort, JNI_TRUE};
use jni::{errors::Result as JniResult, JNIEnv, JavaVM};
use std::sync::Arc;
use winit::platform::android::activity::AndroidApp;

const ANDROID_PERMISSION_GRANTED: i32 = 0;
const REQUEST_CODE_NOTIFICATIONS: i32 = 0x4601;
const REQUEST_CODE_WIFI: i32 = 0x4602;
const REQUEST_CODE_BLUETOOTH: i32 = 0x4603;
const REQUEST_CODE_GEOLOCATION: i32 = 0x4604;
const REQUEST_CODE_CAMERA: i32 = 0x4605;
const REQUEST_CODE_MICROPHONE: i32 = 0x4606;

const PERMISSION_POST_NOTIFICATIONS: &str = "android.permission.POST_NOTIFICATIONS";
const PERMISSION_ACCESS_FINE_LOCATION: &str = "android.permission.ACCESS_FINE_LOCATION";
const PERMISSION_ACCESS_COARSE_LOCATION: &str = "android.permission.ACCESS_COARSE_LOCATION";
const PERMISSION_NEARBY_WIFI_DEVICES: &str = "android.permission.NEARBY_WIFI_DEVICES";
const PERMISSION_BLUETOOTH_SCAN: &str = "android.permission.BLUETOOTH_SCAN";
const PERMISSION_BLUETOOTH_CONNECT: &str = "android.permission.BLUETOOTH_CONNECT";
const PERMISSION_CAMERA: &str = "android.permission.CAMERA";
const PERMISSION_RECORD_AUDIO: &str = "android.permission.RECORD_AUDIO";

pub(crate) fn register_android_operation_capabilities(
    async_registry: &mut AsyncRegistry,
    app: &AndroidApp,
) {
    let Ok(context) = AndroidHostContext::from_app(app) else {
        return;
    };

    clipboard::register_clipboard_capabilities(
        async_registry,
        Arc::new(AndroidClipboardHost::new(context.clone())),
    );
    haptics::register_haptic_capabilities(
        async_registry,
        Arc::new(AndroidHapticHost::new(context.clone())),
    );
    notifications::register_notification_capabilities(
        async_registry,
        Arc::new(AndroidNotificationHost::new(context.clone())),
    );
    volume::register_volume_capabilities(
        async_registry,
        Arc::new(AndroidVolumeHost::new(context.clone())),
    );
    wifi::register_wifi_capabilities(
        async_registry,
        Arc::new(AndroidWifiHost::new(context.clone())),
    );
    bluetooth::register_bluetooth_capabilities(
        async_registry,
        Arc::new(AndroidBluetoothHost::new(context.clone())),
    );
    geolocation::register_geolocation_capabilities(
        async_registry,
        Arc::new(AndroidGeolocationHost::new(context.clone())),
    );
    microphone::register_microphone_capabilities(
        async_registry,
        Arc::new(AndroidMicrophoneHost::new(context.clone())),
    );
    camera::register_camera_capabilities(
        async_registry,
        Arc::new(AndroidCameraHost::new(context.clone())),
    );
    barcode::register_barcode_scanner_capabilities(
        async_registry,
        Arc::new(AndroidBarcodeScannerHost::new(context.clone())),
    );
    biometric::register_biometric_capabilities(
        async_registry,
        Arc::new(AndroidBiometricHost::new(context.clone())),
    );
    nfc::register_nfc_capabilities(async_registry, Arc::new(AndroidNfcHost::new(context)));
}

#[derive(Clone)]
struct AndroidHostContext {
    vm: Arc<JavaVM>,
    activity: usize,
}

impl AndroidHostContext {
    fn from_app(app: &AndroidApp) -> Result<Self, String> {
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }
            .map_err(|error| format!("failed to access Android JavaVM: {error}"))?;
        Ok(Self {
            vm: Arc::new(vm),
            activity: app.activity_as_ptr() as usize,
        })
    }

    fn with_env<R>(
        &self,
        f: impl for<'env> FnOnce(&mut JNIEnv<'env>, &JObject<'static>) -> JniResult<R>,
    ) -> Result<R, String> {
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|error| format!("failed to attach Android JNI thread: {error}"))?;
        let activity = unsafe { JObject::from_raw(self.activity as jobject) };
        f(&mut env, &activity).map_err(|error| format!("Android JNI call failed: {error}"))
    }

    fn sdk_int(&self) -> Result<i32, String> {
        self.with_env(|env, _activity| {
            env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
                .i()
        })
    }

    fn permission_granted(&self, permission: &str) -> Result<bool, String> {
        self.with_env(|env, activity| {
            let permission = env.new_string(permission)?;
            let permission_obj = JObject::from(permission);
            let value = env
                .call_method(
                    activity,
                    "checkSelfPermission",
                    "(Ljava/lang/String;)I",
                    &[JValue::Object(&permission_obj)],
                )?
                .i()?;
            Ok(value == ANDROID_PERMISSION_GRANTED)
        })
    }

    fn any_permission_granted(&self, permissions: &[&str]) -> Result<bool, String> {
        for permission in permissions {
            if self.permission_granted(permission)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn all_permissions_granted(&self, permissions: &[&str]) -> Result<bool, String> {
        for permission in permissions {
            if !self.permission_granted(permission)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn request_permissions(&self, permissions: &[&str], request_code: i32) -> Result<(), String> {
        if permissions.is_empty() {
            return Ok(());
        }
        self.with_env(|env, activity| {
            let string_class = env.find_class("java/lang/String")?;
            let permission_array =
                env.new_object_array(permissions.len() as jint, string_class, JObject::null())?;
            for (index, permission) in permissions.iter().enumerate() {
                let permission = env.new_string(permission)?;
                let permission_obj = JObject::from(permission);
                env.set_object_array_element(&permission_array, index as jint, permission_obj)?;
            }
            let permission_array_obj = JObject::from(permission_array);
            env.call_method(
                activity,
                "requestPermissions",
                "([Ljava/lang/String;I)V",
                &[
                    JValue::Object(&permission_array_obj),
                    JValue::Int(request_code),
                ],
            )?;
            Ok(())
        })
    }

    fn has_system_feature(&self, feature: &str) -> Result<bool, String> {
        self.with_env(|env, activity| {
            let package_manager = env
                .call_method(
                    activity,
                    "getPackageManager",
                    "()Landroid/content/pm/PackageManager;",
                    &[],
                )?
                .l()?;
            let feature = env.new_string(feature)?;
            let feature_obj = JObject::from(feature);
            env.call_method(
                &package_manager,
                "hasSystemFeature",
                "(Ljava/lang/String;)Z",
                &[JValue::Object(&feature_obj)],
            )?
            .z()
        })
    }
}

struct AndroidClipboardHost {
    context: AndroidHostContext,
}

impl AndroidClipboardHost {
    fn new(context: AndroidHostContext) -> Self {
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

    fn read_content(&self) -> Result<ClipboardContent, ClipboardError> {
        let text = self.read_text()?.text.unwrap_or_default();
        Ok(ClipboardContent {
            items: if text.is_empty() {
                Vec::new()
            } else {
                vec![ClipboardItem {
                    content_type: "text/plain".into(),
                    bytes: text.into_bytes(),
                    suggested_name: None,
                }]
            },
        })
    }

    fn write_content(&self, request: ClipboardContent) -> Result<(), ClipboardError> {
        let Some(item) = request
            .items
            .into_iter()
            .find(|item| item.content_type.starts_with("text/plain"))
        else {
            return Err(ClipboardError::unsupported("write_content_non_text"));
        };
        let text = String::from_utf8(item.bytes)
            .map_err(|error| ClipboardError::new("invalid_text", error.to_string()))?;
        self.write_text(ClipboardWriteTextRequest { text })
    }

    fn clear(&self) -> Result<(), ClipboardError> {
        self.write_text(ClipboardWriteTextRequest {
            text: String::new(),
        })
    }
}

struct AndroidHapticHost {
    context: AndroidHostContext,
}

impl AndroidHapticHost {
    fn new(context: AndroidHostContext) -> Self {
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

struct AndroidVolumeHost {
    context: AndroidHostContext,
}

impl AndroidVolumeHost {
    fn new(context: AndroidHostContext) -> Self {
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

struct AndroidNotificationHost {
    context: AndroidHostContext,
}

impl AndroidNotificationHost {
    fn new(context: AndroidHostContext) -> Self {
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

struct AndroidWifiHost {
    context: AndroidHostContext,
}

impl AndroidWifiHost {
    fn new(context: AndroidHostContext) -> Self {
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

struct AndroidBluetoothHost {
    context: AndroidHostContext,
}

impl AndroidBluetoothHost {
    fn new(context: AndroidHostContext) -> Self {
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

struct AndroidGeolocationHost {
    context: AndroidHostContext,
}

impl AndroidGeolocationHost {
    fn new(context: AndroidHostContext) -> Self {
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

struct AndroidMicrophoneHost {
    context: AndroidHostContext,
}

impl AndroidMicrophoneHost {
    fn new(context: AndroidHostContext) -> Self {
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
    ) -> Result<MicrophoneCapture, MicrophoneError> {
        if self.permission_state()? != MicrophonePermission::Granted {
            return Err(MicrophoneError::new(
                "permission_denied",
                "Android microphone permission is not granted",
            ));
        }
        self.context
            .with_env(|env, _activity| android_capture_audio(env, request))
            .map_err(microphone_host_error)
    }

    fn cancel_capture(&self) -> Result<(), MicrophoneError> {
        Ok(())
    }
}

struct AndroidCameraHost {
    context: AndroidHostContext,
}

impl AndroidCameraHost {
    fn new(context: AndroidHostContext) -> Self {
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

    fn capture_photo(&self, request: CameraCaptureRequest) -> Result<CameraCapture, CameraError> {
        if self.permission_state()? != CameraPermission::Granted {
            return Err(CameraError::new(
                "permission_denied",
                "Android camera permission is not granted",
            ));
        }
        android_capture_photo(&self.context, request).map_err(camera_host_error)
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

struct AndroidBarcodeScannerHost {
    context: AndroidHostContext,
}

impl AndroidBarcodeScannerHost {
    fn new(context: AndroidHostContext) -> Self {
        Self { context }
    }
}

impl BarcodeScannerHost for AndroidBarcodeScannerHost {
    fn scan(&self, request: BarcodeScanRequest) -> Result<BarcodeScanResults, BarcodeScannerError> {
        let capture = android_capture_photo(
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
    ) -> Result<BarcodeScanResults, BarcodeScannerError> {
        barcode_decode::decode_barcode_bytes(&request.bytes, &request.formats)
    }

    fn cancel_scan(&self) -> Result<(), BarcodeScannerError> {
        Ok(())
    }
}

struct AndroidBiometricHost {
    context: AndroidHostContext,
}

impl AndroidBiometricHost {
    fn new(context: AndroidHostContext) -> Self {
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

struct AndroidNfcHost {
    context: AndroidHostContext,
}

impl AndroidNfcHost {
    fn new(context: AndroidHostContext) -> Self {
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

fn system_service<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject<'_>,
    name: &str,
) -> JniResult<JObject<'local>> {
    let name = env.new_string(name)?;
    let name_obj = JObject::from(name);
    env.call_method(
        activity,
        "getSystemService",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        &[JValue::Object(&name_obj)],
    )?
    .l()
}

fn ensure_vibrator(env: &mut JNIEnv<'_>, vibrator: &JObject<'_>) -> JniResult<()> {
    let has_vibrator = env
        .call_method(vibrator, "hasVibrator", "()Z", &[])?
        .z()
        .unwrap_or(false);
    if has_vibrator {
        Ok(())
    } else {
        Err(jni::errors::Error::NullPtr("android vibrator unavailable"))
    }
}

fn char_sequence_to_string(env: &mut JNIEnv<'_>, value: &JObject<'_>) -> JniResult<String> {
    let string = env
        .call_method(value, "toString", "()Ljava/lang/String;", &[])?
        .l()?;
    java_string(env, string)
}

fn java_string(env: &mut JNIEnv<'_>, object: JObject<'_>) -> JniResult<String> {
    if object.as_raw().is_null() {
        return Ok(String::new());
    }
    let string = JString::from(object);
    let value: String = env.get_string(&string)?.into();
    Ok(value)
}

fn java_string_array<'local>(
    env: &mut JNIEnv<'local>,
    values: &[String],
) -> JniResult<JObjectArray<'local>> {
    let string_class = env.find_class("java/lang/String")?;
    let array = env.new_object_array(values.len() as jint, string_class, JObject::null())?;
    for (index, value) in values.iter().enumerate() {
        let value = env.new_string(value)?;
        env.set_object_array_element(&array, index as jint, value)?;
    }
    Ok(array)
}

fn android_bluetooth_device_row(row: &str) -> Option<BluetoothDevice> {
    let mut fields = row.split('\u{1f}');
    let id = fields.next()?.to_string();
    let name = fields
        .next()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let address = fields
        .next()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let rssi = fields
        .next()
        .and_then(|value| value.parse::<i16>().ok())
        .filter(|value| *value != 0);
    let paired = fields
        .next()
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let modes = fields
        .next()
        .map(|value| {
            value
                .split(',')
                .filter_map(|mode| match mode {
                    "classic" => Some(BluetoothMode::Classic),
                    "le" => Some(BluetoothMode::LowEnergy),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(BluetoothDevice {
        id,
        name,
        address,
        rssi,
        paired,
        modes,
    })
}

fn android_biometric_auth_result(
    row: &str,
) -> Result<BiometricAuthenticateResult, (String, String)> {
    let mut fields = row.split('\u{1f}');
    match fields.next().unwrap_or_default() {
        "ok" => {
            let kind = match fields.next().unwrap_or_default() {
                "fingerprint" => Some(BiometricKind::Fingerprint),
                "face" => Some(BiometricKind::Face),
                "device_credential" => Some(BiometricKind::DeviceCredential),
                "biometric" => Some(BiometricKind::Fingerprint),
                _ => None,
            };
            let used_device_credential = matches!(kind, Some(BiometricKind::DeviceCredential));
            Ok(BiometricAuthenticateResult {
                verified: true,
                kind,
                used_device_credential,
            })
        }
        "error" => {
            let code = fields.next().unwrap_or("host_error");
            let message = fields
                .next()
                .unwrap_or("Android biometric authentication failed");
            Err((code.into(), message.into()))
        }
        _ => Err((
            "host_error".into(),
            "Android biometric helper returned an invalid payload".into(),
        )),
    }
}

fn android_stream_id(stream: VolumeStream) -> i32 {
    match stream {
        VolumeStream::Media => 3,
        VolumeStream::Ring => 2,
        VolumeStream::Alarm => 4,
        VolumeStream::Notification => 5,
        VolumeStream::Call => 0,
        VolumeStream::System => 1,
    }
}

fn percent_to_platform_volume(level: u8, max_volume: i32) -> i32 {
    ((i32::from(level.min(100)) * max_volume) + 50) / 100
}

fn platform_volume_to_percent(level: i32, max_volume: i32) -> u8 {
    if max_volume <= 0 {
        return 0;
    }
    ((level.clamp(0, max_volume) * 100) / max_volume) as u8
}

fn android_get_volume_level(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    stream: VolumeStream,
) -> JniResult<VolumeLevel> {
    let audio = system_service(env, activity, "audio")?;
    let stream_id = android_stream_id(stream);
    let level = env
        .call_method(&audio, "getStreamVolume", "(I)I", &[JValue::Int(stream_id)])?
        .i()?;
    let max = env
        .call_method(
            &audio,
            "getStreamMaxVolume",
            "(I)I",
            &[JValue::Int(stream_id)],
        )?
        .i()?
        .max(1);
    let muted = env
        .call_method(&audio, "isStreamMute", "(I)Z", &[JValue::Int(stream_id)])
        .and_then(|value| value.z())
        .unwrap_or(false);
    Ok(VolumeLevel {
        stream,
        level: platform_volume_to_percent(level, max),
        muted,
    })
}

fn android_current_position(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    request: GeolocationPositionRequest,
) -> JniResult<GeolocationPosition> {
    let helper = app_class(
        env,
        activity,
        "rs.fission.runtime.FissionAndroidCapabilities",
    )?;
    let timeout = request
        .timeout_ms
        .unwrap_or(5_000)
        .max(250)
        .min(i32::MAX as u64) as jlong;
    let values = env
        .call_static_method(
            helper,
            "currentLocation",
            "(Landroid/app/Activity;ZJ)[D",
            &[
                JValue::Object(activity),
                JValue::Bool(request.high_accuracy as u8),
                JValue::Long(timeout),
            ],
        )?
        .l()?;
    let values: JDoubleArray<'_> = JDoubleArray::from(values);
    if env.get_array_length(&values)? < 8 {
        return Err(jni::errors::Error::NullPtr(
            "Android location helper returned an invalid payload",
        ));
    }
    let mut payload = [0.0f64; 8];
    env.get_double_array_region(&values, 0, &mut payload)?;
    if !payload[0].is_finite() || !payload[1].is_finite() {
        return Err(jni::errors::Error::NullPtr(
            "Android location is unavailable",
        ));
    }
    let timestamp_unix_ms = payload[7].max(0.0) as u64;
    Ok(GeolocationPosition {
        latitude: payload[0],
        longitude: payload[1],
        altitude_meters: finite_value(payload[2]),
        accuracy_meters: payload[3].max(0.0),
        altitude_accuracy_meters: finite_value(payload[4]),
        heading_degrees: finite_value(payload[5]),
        speed_mps: finite_value(payload[6]),
        timestamp_unix_ms: if timestamp_unix_ms == 0 {
            current_unix_ms()
        } else {
            timestamp_unix_ms
        },
    })
}

fn finite_value(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn android_capture_audio(
    env: &mut JNIEnv<'_>,
    request: MicrophoneCaptureRequest,
) -> JniResult<MicrophoneCapture> {
    let trace = std::env::var_os("FISSION_ANDROID_AUDIO_TRACE").is_some();
    let sample_rate_hz = request
        .sample_rate_hz
        .unwrap_or(48_000)
        .clamp(8_000, 48_000);
    let channels = request.channels.unwrap_or(1).clamp(1, 2);
    let duration_ms = request.duration_ms.clamp(100, 10_000);
    if trace {
        eprintln!(
            "[fission-android-audio] start sample_rate={sample_rate_hz} channels={channels} duration_ms={duration_ms}"
        );
    }
    let channel_config = if channels == 1 { 16 } else { 12 };
    let encoding_pcm_16 = 2;
    let min_buffer = env
        .call_static_method(
            "android/media/AudioRecord",
            "getMinBufferSize",
            "(III)I",
            &[
                JValue::Int(sample_rate_hz as jint),
                JValue::Int(channel_config),
                JValue::Int(encoding_pcm_16),
            ],
        )?
        .i()?;
    if min_buffer <= 0 {
        return Err(jni::errors::Error::NullPtr(
            "Android AudioRecord could not provide a minimum buffer size",
        ));
    }
    let target_samples =
        ((u64::from(sample_rate_hz) * duration_ms * u64::from(channels)) / 1_000) as usize;
    let chunk_samples = ((min_buffer as usize) / std::mem::size_of::<jshort>())
        .max(512)
        .min(target_samples.max(512));
    let buffer_bytes = (chunk_samples * std::mem::size_of::<jshort>()).max(min_buffer as usize);
    let recorder = env.new_object(
        "android/media/AudioRecord",
        "(IIIII)V",
        &[
            JValue::Int(1),
            JValue::Int(sample_rate_hz as jint),
            JValue::Int(channel_config),
            JValue::Int(encoding_pcm_16),
            JValue::Int(buffer_bytes as jint),
        ],
    )?;
    let state = env
        .call_method(&recorder, "getState", "()I", &[])?
        .i()
        .unwrap_or(0);
    if state != 1 {
        env.call_method(&recorder, "release", "()V", &[]).ok();
        return Err(jni::errors::Error::NullPtr(
            "Android AudioRecord failed to initialize",
        ));
    }
    env.call_method(&recorder, "startRecording", "()V", &[])?;
    if trace {
        eprintln!("[fission-android-audio] recording started");
    }
    let buffer: JShortArray<'_> = env.new_short_array(chunk_samples as jint)?;
    let mut captured = Vec::<i16>::with_capacity(target_samples);
    let started_at = std::time::Instant::now();
    let read_deadline = std::time::Duration::from_millis(duration_ms.saturating_add(1_500));
    while captured.len() < target_samples {
        let remaining = (target_samples - captured.len()).min(chunk_samples) as jint;
        let read = env
            .call_method(
                &recorder,
                "read",
                "([SIII)I",
                &[
                    JValue::Object(buffer.as_ref()),
                    JValue::Int(0),
                    JValue::Int(remaining),
                    JValue::Int(1),
                ],
            )?
            .i()?;
        if read < 0 {
            env.call_method(&recorder, "stop", "()V", &[]).ok();
            env.call_method(&recorder, "release", "()V", &[]).ok();
            return Err(jni::errors::Error::NullPtr(
                "Android AudioRecord read returned an error",
            ));
        }
        if read == 0 {
            if started_at.elapsed() >= read_deadline {
                if trace {
                    eprintln!("[fission-android-audio] read deadline reached");
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }
        let mut chunk = vec![0 as jshort; read as usize];
        env.get_short_array_region(&buffer, 0, &mut chunk)?;
        captured.extend(chunk.into_iter().map(|sample| sample as i16));
    }
    env.call_method(&recorder, "stop", "()V", &[]).ok();
    env.call_method(&recorder, "release", "()V", &[]).ok();
    if trace {
        eprintln!(
            "[fission-android-audio] read loop complete samples={}",
            captured.len()
        );
    }
    if captured.is_empty() {
        return Err(jni::errors::Error::NullPtr(
            "Android AudioRecord produced no audio samples",
        ));
    }
    let (bytes, format_label) = encode_audio_samples(&captured, request.sample_format);
    Ok(MicrophoneCapture {
        bytes,
        content_type: format!("audio/pcm; format={format_label}"),
        sample_rate_hz,
        channels,
        duration_ms,
        device_id: Some("android-default-microphone".into()),
    })
}

fn encode_audio_samples(samples: &[i16], format: AudioSampleFormat) -> (Vec<u8>, &'static str) {
    match format {
        AudioSampleFormat::I16 => {
            let mut bytes = Vec::with_capacity(samples.len() * 2);
            for sample in samples {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
            (bytes, "s16le")
        }
        AudioSampleFormat::U8 => {
            let bytes = samples
                .iter()
                .map(|sample| ((*sample as i32 + 32_768) >> 8) as u8)
                .collect();
            (bytes, "u8")
        }
        AudioSampleFormat::F32 => {
            let mut bytes = Vec::with_capacity(samples.len() * 4);
            for sample in samples {
                let value = (*sample as f32) / (i16::MAX as f32);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            (bytes, "f32le")
        }
    }
}

fn android_capture_photo(
    context: &AndroidHostContext,
    request: CameraCaptureRequest,
) -> Result<CameraCapture, String> {
    context.with_env(|env, activity| {
        let camera_id = env.new_string(request.camera_id.as_deref().unwrap_or(""))?;
        let camera_id_obj = JObject::from(camera_id);
        let facing = match request.facing {
            CameraFacing::Front => 0,
            CameraFacing::Back => 1,
            CameraFacing::External => 2,
            CameraFacing::Unspecified => -1,
        };
        let (width, height) = request
            .resolution
            .map(|resolution| (resolution.width as jint, resolution.height as jint))
            .unwrap_or((1280, 720));
        let quality = i32::from(request.quality.unwrap_or(90).clamp(1, 100));
        let flash_mode = match request.flash {
            fission_core::CameraFlashMode::Off => 0,
            fission_core::CameraFlashMode::On => 1,
            fission_core::CameraFlashMode::Auto => 2,
        };
        let helper = app_class(
            env,
            activity,
            "rs.fission.runtime.FissionAndroidCapabilities",
        )?;
        let bytes = env
            .call_static_method(
                helper,
                "captureJpeg",
                "(Landroid/app/Activity;Ljava/lang/String;IIIIIJ)[B",
                &[
                    JValue::Object(activity),
                    JValue::Object(&camera_id_obj),
                    JValue::Int(facing),
                    JValue::Int(width),
                    JValue::Int(height),
                    JValue::Int(quality),
                    JValue::Int(flash_mode),
                    JValue::Long(7_500),
                ],
            )?
            .l()?;
        let bytes: JByteArray<'_> = JByteArray::from(bytes);
        let bytes = env.convert_byte_array(&bytes)?;
        if bytes.is_empty() {
            return Err(jni::errors::Error::NullPtr(
                "Android camera capture returned no bytes",
            ));
        }
        let (actual_width, actual_height) = image::load_from_memory(&bytes)
            .map(|image| (image.width(), image.height()))
            .unwrap_or((width.max(1) as u32, height.max(1) as u32));
        Ok(CameraCapture {
            bytes,
            content_type: "image/jpeg".into(),
            width: actual_width,
            height: actual_height,
            camera_id: request.camera_id,
        })
    })
}

fn barcode_camera_error(error: String) -> BarcodeScannerError {
    BarcodeScannerError::new("camera_error", error)
}

fn app_class<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject<'_>,
    name: &str,
) -> JniResult<JClass<'local>> {
    let class_loader = env
        .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
        .l()?;
    let name = env.new_string(name)?;
    let name_obj = JObject::from(name);
    let class = env
        .call_method(
            &class_loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&name_obj)],
        )?
        .l()?;
    Ok(JClass::from(class))
}

fn android_torch_camera_id(env: &mut JNIEnv<'_>, manager: &JObject<'_>) -> JniResult<String> {
    let ids = env
        .call_method(manager, "getCameraIdList", "()[Ljava/lang/String;", &[])?
        .l()?;
    let ids: JObjectArray<'_> = JObjectArray::from(ids);
    let len = env.get_array_length(&ids)?;
    if len == 0 {
        return Err(jni::errors::Error::NullPtr(
            "no Android cameras are available",
        ));
    }
    let mut fallback = None;
    for index in 0..len {
        let id_obj = env.get_object_array_element(&ids, index)?;
        let id = java_string(env, id_obj)?;
        if fallback.is_none() {
            fallback = Some(id.clone());
        }
        if android_camera_has_flashlight(env, manager, &id).unwrap_or(false) {
            return Ok(id);
        }
    }
    fallback.ok_or(jni::errors::Error::NullPtr(
        "no Android cameras are available",
    ))
}

fn android_camera_facing(
    env: &mut JNIEnv<'_>,
    manager: &JObject<'_>,
    id: &str,
) -> JniResult<CameraFacing> {
    let value = android_camera_characteristic(env, manager, id, "LENS_FACING")?;
    let facing = env.call_method(&value, "intValue", "()I", &[])?.i()?;
    Ok(match facing {
        0 => CameraFacing::Front,
        1 => CameraFacing::Back,
        2 => CameraFacing::External,
        _ => CameraFacing::Unspecified,
    })
}

fn android_camera_has_flashlight(
    env: &mut JNIEnv<'_>,
    manager: &JObject<'_>,
    id: &str,
) -> JniResult<bool> {
    let value = android_camera_characteristic(env, manager, id, "FLASH_INFO_AVAILABLE")?;
    env.call_method(&value, "booleanValue", "()Z", &[])?.z()
}

fn android_camera_characteristic<'local>(
    env: &mut JNIEnv<'local>,
    manager: &JObject<'_>,
    id: &str,
    key: &str,
) -> JniResult<JObject<'local>> {
    let id = env.new_string(id)?;
    let id_obj = JObject::from(id);
    let characteristics = env
        .call_method(
            manager,
            "getCameraCharacteristics",
            "(Ljava/lang/String;)Landroid/hardware/camera2/CameraCharacteristics;",
            &[JValue::Object(&id_obj)],
        )?
        .l()?;
    let key = env
        .get_static_field(
            "android/hardware/camera2/CameraCharacteristics",
            key,
            "Landroid/hardware/camera2/CameraCharacteristics$Key;",
        )?
        .l()?;
    env.call_method(
        &characteristics,
        "get",
        "(Landroid/hardware/camera2/CameraCharacteristics$Key;)Ljava/lang/Object;",
        &[JValue::Object(&key)],
    )?
    .l()
}

fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn android_connected_wifi(
    env: &mut JNIEnv<'_>,
    wifi: &JObject<'_>,
) -> JniResult<Option<WifiNetwork>> {
    let info = env
        .call_method(
            &wifi,
            "getConnectionInfo",
            "()Landroid/net/wifi/WifiInfo;",
            &[],
        )?
        .l()?;
    if info.as_raw().is_null() {
        return Ok(None);
    }
    let ssid_obj = env
        .call_method(&info, "getSSID", "()Ljava/lang/String;", &[])?
        .l()?;
    let ssid = char_sequence_to_string(env, &ssid_obj)?;
    let ssid = normalize_android_ssid(&ssid);
    if ssid.is_empty() || ssid == "<unknown ssid>" {
        return Ok(None);
    }
    let bssid_obj = env
        .call_method(&info, "getBSSID", "()Ljava/lang/String;", &[])?
        .l()?;
    let bssid = char_sequence_to_string(env, &bssid_obj)
        .ok()
        .filter(|value| !value.is_empty());
    let rssi = env
        .call_method(&info, "getRssi", "()I", &[])
        .and_then(|value| value.i())
        .ok()
        .map(|value| value as i16);
    let frequency_mhz = env
        .call_method(&info, "getFrequency", "()I", &[])
        .and_then(|value| value.i())
        .ok()
        .map(|value| value as u32);
    Ok(Some(WifiNetwork {
        ssid,
        bssid,
        rssi,
        frequency_mhz,
        security: WifiSecurity::Unknown,
        connected: true,
    }))
}

fn android_wifi_scan_result(
    env: &mut JNIEnv<'_>,
    result: &JObject<'_>,
) -> JniResult<Option<WifiNetwork>> {
    let ssid_obj = env.get_field(result, "SSID", "Ljava/lang/String;")?.l()?;
    let ssid = java_string(env, ssid_obj)?;
    let ssid = normalize_android_ssid(&ssid);
    let bssid_obj = env.get_field(result, "BSSID", "Ljava/lang/String;")?.l()?;
    let bssid = java_string(env, bssid_obj)
        .ok()
        .filter(|value| !value.is_empty());
    let rssi = env
        .get_field(result, "level", "I")
        .and_then(|value| value.i())
        .ok()
        .map(|value| value as i16);
    let frequency_mhz = env
        .get_field(result, "frequency", "I")
        .and_then(|value| value.i())
        .ok()
        .map(|value| value as u32);
    let capabilities_obj = env
        .get_field(result, "capabilities", "Ljava/lang/String;")?
        .l()?;
    let capabilities = java_string(env, capabilities_obj).unwrap_or_default();
    Ok(Some(WifiNetwork {
        ssid,
        bssid,
        rssi,
        frequency_mhz,
        security: android_wifi_security(&capabilities),
        connected: false,
    }))
}

fn normalize_android_ssid(ssid: &str) -> String {
    ssid.trim_matches('"').to_string()
}

fn android_wifi_security(capabilities: &str) -> WifiSecurity {
    let caps = capabilities.to_ascii_uppercase();
    if caps.contains("SAE") {
        WifiSecurity::Wpa3
    } else if caps.contains("EAP") {
        WifiSecurity::Enterprise
    } else if caps.contains("WPA2") || caps.contains("RSN") || caps.contains("PSK") {
        WifiSecurity::Wpa2
    } else if caps.contains("WPA") {
        WifiSecurity::Wpa
    } else if caps.contains("WEP") {
        WifiSecurity::Wep
    } else if caps.is_empty() || caps.contains("ESS") {
        WifiSecurity::Open
    } else {
        WifiSecurity::Unknown
    }
}

fn notification_id_to_i32(id: &NotificationId) -> i32 {
    let mut hash = 0x811c_9dc5u32;
    for byte in id.0.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash & 0x7fff_ffff) as i32
}

fn clipboard_host_error(error: String) -> ClipboardError {
    ClipboardError::new("host_error", error)
}

fn geolocation_host_error(error: String) -> GeolocationError {
    GeolocationError::new("host_error", error)
}

fn haptic_host_error(error: String) -> HapticError {
    HapticError::new("host_error", error)
}

fn microphone_host_error(error: String) -> MicrophoneError {
    MicrophoneError::new("host_error", error)
}

fn camera_host_error(error: String) -> CameraError {
    CameraError::new("host_error", error)
}

fn biometric_host_error(error: String) -> BiometricError {
    BiometricError::new("host_error", error)
}

fn nfc_host_error(error: String) -> NfcError {
    NfcError::new("host_error", error)
}

fn volume_host_error(error: String) -> VolumeError {
    VolumeError::new("host_error", error)
}

fn notification_host_error(error: String) -> NotificationError {
    NotificationError::new("host_error", error)
}

fn wifi_host_error(error: String) -> WifiError {
    WifiError::new("host_error", error)
}

fn bluetooth_host_error(error: String) -> BluetoothError {
    BluetoothError::new("host_error", error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_volume_percent_mapping_is_bounded() {
        assert_eq!(percent_to_platform_volume(50, 15), 8);
        assert_eq!(platform_volume_to_percent(20, 10), 100);
        assert_eq!(platform_volume_to_percent(0, 0), 0);
    }

    #[test]
    fn android_wifi_security_parses_common_capability_strings() {
        assert_eq!(
            android_wifi_security("[WPA2-PSK-CCMP][ESS]"),
            WifiSecurity::Wpa2
        );
        assert_eq!(android_wifi_security("[SAE][ESS]"), WifiSecurity::Wpa3);
        assert_eq!(android_wifi_security("[WEP][ESS]"), WifiSecurity::Wep);
        assert_eq!(android_wifi_security("[ESS]"), WifiSecurity::Open);
    }

    #[test]
    fn notification_ids_are_stable_positive_values() {
        assert_eq!(
            notification_id_to_i32(&NotificationId::new("sync-complete")),
            630_319_610
        );
        assert!(notification_id_to_i32(&NotificationId::new("x")) >= 0);
    }
}
