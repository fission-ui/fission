use crate::clipboard::ClipboardHostItem;
use crate::{
    barcode, barcode_decode, biometric, bluetooth, camera, clipboard, geolocation, haptics,
    microphone, nfc, notifications, volume, wifi, BarcodeScannerHost, BiometricHost, BluetoothHost,
    CameraHost, ClipboardHost, GeolocationHost, HapticHost, MicrophoneHost, NfcHost,
    NotificationHost, VolumeHost, WifiHost,
};
use fission_core::{
    single_chunk_data_stream, AudioSampleFormat, BarcodeImageDecodeRequest, BarcodeScanRequest,
    BarcodeScanResults, BarcodeScannerError, BiometricAuthenticateRequest,
    BiometricAuthenticateResult, BiometricAvailability, BiometricError, BiometricKind,
    BluetoothAdvertiseReceipt, BluetoothAdvertiseRequest, BluetoothAvailability,
    BluetoothConnectRequest, BluetoothConnection, BluetoothDevice, BluetoothDisconnectRequest,
    BluetoothError, BluetoothMode, BluetoothPermission, BluetoothPermissionRequest,
    BluetoothReadRequest, BluetoothReadResult, BluetoothScanRequest, BluetoothScanResult,
    BluetoothStopAdvertiseRequest, BluetoothWriteRequest, Bytes, CameraAvailability, CameraCapture,
    CameraCaptureRequest, CameraDevice, CameraError, CameraFacing, CameraFlashlightRequest,
    CameraPermission, CameraPermissionRequest, CancelNotificationRequest, CapabilityCtx,
    ClipboardError, ClipboardText, ClipboardWriteTextRequest, GeolocationError,
    GeolocationPermission, GeolocationPermissionRequest, GeolocationPosition,
    GeolocationPositionRequest, HapticError, HapticImpactRequest, HapticImpactStyle,
    HapticNotificationKind, HapticNotificationRequest, HapticPatternRequest,
    MicrophoneAvailability, MicrophoneCapture, MicrophoneCaptureRequest, MicrophoneDevice,
    MicrophoneError, MicrophonePermission, MicrophonePermissionRequest, NfcAvailability,
    NfcEmulationRequest, NfcError, NfcScanRequest, NfcSessionReceipt, NfcTag, NfcWriteRequest,
    NotificationError, NotificationId, NotificationPermission, NotificationPermissionRequest,
    NotificationReceipt, NotificationRequest, NotificationSchedule, NotificationSettings,
    PushRegistration, PushRegistrationRequest, SetBadgeCountRequest, VolumeAdjustDirection,
    VolumeAdjustRequest, VolumeError, VolumeLevel, VolumeSetRequest, VolumeStream,
    WifiAvailability, WifiConnectRequest, WifiConnection, WifiDisconnectRequest, WifiError,
    WifiNetwork, WifiPermission, WifiPermissionRequest, WifiScanRequest, WifiScanResult,
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

mod hosts;
use hosts::*;
mod support;
use support::*;
#[cfg(test)]
mod tests;
