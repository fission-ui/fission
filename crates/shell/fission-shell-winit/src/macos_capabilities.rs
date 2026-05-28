use crate::{
    barcode, barcode_decode, biometric, bluetooth, camera, geolocation, microphone, wifi,
    BarcodeScannerHost, BiometricHost, BluetoothHost, CameraHost, GeolocationHost, MicrophoneHost,
    WifiHost,
};
use block::ConcreteBlock;
use fission_core::{
    BarcodeImageDecodeRequest, BarcodeScanRequest, BarcodeScanResults, BarcodeScannerError,
    BiometricAuthenticateRequest, BiometricAuthenticateResult, BiometricAvailability,
    BiometricError, BiometricKind, BluetoothAvailability, BluetoothConnectRequest,
    BluetoothConnection, BluetoothDevice, BluetoothDisconnectRequest, BluetoothError,
    BluetoothPermission, BluetoothPermissionRequest, BluetoothReadRequest, BluetoothReadResult,
    BluetoothScanRequest, BluetoothScanResult, BluetoothWriteRequest, CameraAvailability,
    CameraCapture, CameraCaptureRequest, CameraDevice, CameraError, CameraFacing,
    CameraFlashlightRequest, CameraImageFormat, CameraPermission, CameraPermissionRequest,
    GeolocationError, GeolocationPermission, GeolocationPermissionRequest, GeolocationPosition,
    GeolocationPositionRequest, MicrophoneAvailability, MicrophoneCapture,
    MicrophoneCaptureRequest, MicrophoneDevice, MicrophoneError, MicrophonePermission,
    MicrophonePermissionRequest, WifiAvailability, WifiConnectRequest, WifiConnection,
    WifiDisconnectRequest, WifiError, WifiNetwork, WifiPermission, WifiPermissionRequest,
    WifiScanRequest, WifiScanResult, WifiSecurity,
};
use fission_shell::async_host::AsyncRegistry;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Protocol, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::process::Command;
use std::ptr;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

#[link(name = "CoreLocation", kind = "framework")]
extern "C" {}

#[link(name = "LocalAuthentication", kind = "framework")]
extern "C" {}

pub(crate) fn register_macos_operation_capabilities(async_registry: &mut AsyncRegistry) {
    camera::register_camera_capabilities(async_registry, Arc::new(MacosCameraHost));
    barcode::register_barcode_scanner_capabilities(
        async_registry,
        Arc::new(MacosBarcodeScannerHost),
    );
    microphone::register_microphone_capabilities(async_registry, Arc::new(MacosMicrophoneHost));
    geolocation::register_geolocation_capabilities(async_registry, Arc::new(MacosGeolocationHost));
    biometric::register_biometric_capabilities(async_registry, Arc::new(MacosBiometricHost));
    bluetooth::register_bluetooth_capabilities(async_registry, Arc::new(MacosBluetoothHost));
    wifi::register_wifi_capabilities(async_registry, Arc::new(MacosWifiHost));
}

#[derive(Debug, Default)]
struct MacosCameraHost;

impl MacosCameraHost {
    fn permission_state() -> CameraPermission {
        let status: i64 = unsafe {
            msg_send![
                class!(AVCaptureDevice),
                authorizationStatusForMediaType: ns_string("vide")
            ]
        };
        match status {
            3 => CameraPermission::Granted,
            2 => CameraPermission::Denied,
            1 => CameraPermission::Restricted,
            _ => CameraPermission::Unknown,
        }
    }

    fn request_camera_permission() -> CameraPermission {
        let state = Self::permission_state();
        if state != CameraPermission::Unknown {
            return state;
        }
        let pair = Arc::new((Mutex::new(None), Condvar::new()));
        let pair_for_block = pair.clone();
        let block = ConcreteBlock::new(move |granted: bool| {
            let (lock, cvar) = &*pair_for_block;
            if let Ok(mut result) = lock.lock() {
                *result = Some(granted);
                cvar.notify_all();
            }
        })
        .copy();
        unsafe {
            let _: () = msg_send![
                class!(AVCaptureDevice),
                requestAccessForMediaType: ns_string("vide")
                completionHandler: &*block
            ];
        }
        let (lock, cvar) = &*pair;
        let guard = lock.lock().unwrap();
        let _ = cvar.wait_timeout_while(guard, Duration::from_secs(30), |value| value.is_none());
        Self::permission_state()
    }
}

impl CameraHost for MacosCameraHost {
    fn availability(&self) -> Result<CameraAvailability, CameraError> {
        Ok(CameraAvailability {
            permission: Self::permission_state(),
            devices: macos_camera_devices(),
        })
    }

    fn request_permission(
        &self,
        _request: CameraPermissionRequest,
    ) -> Result<CameraPermission, CameraError> {
        Ok(Self::request_camera_permission())
    }

    fn capture_photo(&self, request: CameraCaptureRequest) -> Result<CameraCapture, CameraError> {
        let permission = if Self::permission_state() == CameraPermission::Unknown {
            Self::request_camera_permission()
        } else {
            Self::permission_state()
        };
        if permission != CameraPermission::Granted {
            return Err(CameraError::new(
                "permission_denied",
                "macOS camera permission is not granted",
            ));
        }
        macos_capture_photo(request)
    }

    fn set_flashlight(&self, _request: CameraFlashlightRequest) -> Result<(), CameraError> {
        Err(CameraError::unsupported("set_flashlight"))
    }

    fn cancel_capture(&self) -> Result<(), CameraError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct MacosBarcodeScannerHost;

impl BarcodeScannerHost for MacosBarcodeScannerHost {
    fn scan(&self, request: BarcodeScanRequest) -> Result<BarcodeScanResults, BarcodeScannerError> {
        let capture = MacosCameraHost
            .capture_photo(CameraCaptureRequest {
                camera_id: request.camera_id,
                facing: CameraFacing::Unspecified,
                resolution: None,
                format: CameraImageFormat::Jpeg,
                flash: fission_core::CameraFlashMode::Auto,
                quality: Some(90),
            })
            .map_err(|error| BarcodeScannerError::new("camera_error", error.message))?;
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

struct PhotoCaptureState {
    result: Mutex<Option<Result<Vec<u8>, String>>>,
    cvar: Condvar,
}

impl PhotoCaptureState {
    fn new() -> Self {
        Self {
            result: Mutex::new(None),
            cvar: Condvar::new(),
        }
    }
}

fn macos_camera_devices() -> Vec<CameraDevice> {
    unsafe {
        let devices: *mut Object =
            msg_send![class!(AVCaptureDevice), devicesWithMediaType: ns_string("vide")];
        if devices.is_null() {
            return Vec::new();
        }
        let count: usize = msg_send![devices, count];
        let mut result = Vec::new();
        for index in 0..count {
            let device: *mut Object = msg_send![devices, objectAtIndex: index];
            if device.is_null() {
                continue;
            }
            result.push(CameraDevice {
                id: av_device_unique_id(device).unwrap_or_else(|| format!("macos-camera-{index}")),
                label: av_device_label(device),
                facing: av_device_facing(device),
                has_flashlight: false,
            });
        }
        result
    }
}

fn macos_capture_photo(request: CameraCaptureRequest) -> Result<CameraCapture, CameraError> {
    unsafe {
        let device = select_av_camera_device(request.camera_id.as_deref(), request.facing)
            .ok_or_else(|| CameraError::new("unavailable", "no macOS camera is available"))?;
        let input = capture_device_input(device)?;
        let session: *mut Object = msg_send![class!(AVCaptureSession), new];
        let output: *mut Object = msg_send![class!(AVCapturePhotoOutput), new];
        if session.is_null() || output.is_null() {
            return Err(CameraError::new(
                "unavailable",
                "AVFoundation photo capture is not available",
            ));
        }

        let can_add_input: bool = msg_send![session, canAddInput: input];
        if !can_add_input {
            return Err(CameraError::new(
                "configuration_failed",
                "macOS camera input cannot be added to the capture session",
            ));
        }
        let _: () = msg_send![session, addInput: input];
        let can_add_output: bool = msg_send![session, canAddOutput: output];
        if !can_add_output {
            return Err(CameraError::new(
                "configuration_failed",
                "macOS photo output cannot be added to the capture session",
            ));
        }
        let _: () = msg_send![session, addOutput: output];

        let state = Arc::new(PhotoCaptureState::new());
        let delegate: *mut Object = msg_send![photo_capture_delegate_class(), new];
        if delegate.is_null() {
            return Err(CameraError::new(
                "configuration_failed",
                "macOS photo delegate could not be created",
            ));
        }
        (*delegate).set_ivar("_state", Arc::as_ptr(&state) as usize);

        let settings: *mut Object = msg_send![class!(AVCapturePhotoSettings), photoSettings];
        let _: () = msg_send![session, startRunning];
        let _: () = msg_send![output, capturePhotoWithSettings: settings delegate: delegate];
        let guard = state.result.lock().unwrap();
        let (mut guard, _) = state
            .cvar
            .wait_timeout_while(guard, Duration::from_millis(7_500), |value| value.is_none())
            .unwrap();
        let _: () = msg_send![session, stopRunning];

        let bytes = match guard.take() {
            Some(Ok(bytes)) => bytes,
            Some(Err(message)) => return Err(CameraError::new("capture_failed", message)),
            None => {
                return Err(CameraError::new(
                    "timeout",
                    "macOS did not produce a photo before the request timed out",
                ));
            }
        };
        let (width, height) = image_dimensions(&bytes).unwrap_or_else(|| {
            request
                .resolution
                .map(|resolution| (resolution.width, resolution.height))
                .unwrap_or((0, 0))
        });
        Ok(CameraCapture {
            bytes,
            content_type: "image/jpeg".into(),
            width,
            height,
            camera_id: av_device_unique_id(device).or(request.camera_id),
        })
    }
}

unsafe fn capture_device_input(device: *mut Object) -> Result<*mut Object, CameraError> {
    let mut error: *mut Object = ptr::null_mut();
    let input: *mut Object =
        msg_send![class!(AVCaptureDeviceInput), deviceInputWithDevice: device error: &mut error];
    if input.is_null() || !error.is_null() {
        return Err(CameraError::new(
            "configuration_failed",
            ns_error_message(error).unwrap_or_else(|| "failed to create macOS camera input".into()),
        ));
    }
    Ok(input)
}

unsafe fn select_av_camera_device(
    requested_id: Option<&str>,
    requested_facing: CameraFacing,
) -> Option<*mut Object> {
    let devices: *mut Object =
        msg_send![class!(AVCaptureDevice), devicesWithMediaType: ns_string("vide")];
    if devices.is_null() {
        return None;
    }
    let count: usize = msg_send![devices, count];
    let mut fallback: *mut Object = ptr::null_mut();
    for index in 0..count {
        let device: *mut Object = msg_send![devices, objectAtIndex: index];
        if device.is_null() {
            continue;
        }
        if fallback.is_null() {
            fallback = device;
        }
        if let Some(id) = requested_id {
            if av_device_unique_id(device).as_deref() == Some(id) {
                return Some(device);
            }
        } else if requested_facing != CameraFacing::Unspecified
            && av_device_facing(device) == requested_facing
        {
            return Some(device);
        }
    }
    (!fallback.is_null()).then_some(fallback)
}

fn photo_capture_delegate_class() -> &'static Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    let ptr = *CLASS.get_or_init(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("FissionMacosPhotoCaptureDelegate", superclass)
            .expect("register FissionMacosPhotoCaptureDelegate");
        decl.add_ivar::<usize>("_state");
        if let Some(protocol) = Protocol::get("AVCapturePhotoCaptureDelegate") {
            decl.add_protocol(protocol);
        }
        unsafe {
            decl.add_method(
                sel!(captureOutput:didFinishProcessingPhoto:error:),
                photo_capture_did_finish
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, *mut Object),
            );
        }
        decl.register() as *const Class as usize
    });
    unsafe { &*(ptr as *const Class) }
}

extern "C" fn photo_capture_did_finish(
    this: &mut Object,
    _cmd: Sel,
    _output: *mut Object,
    photo: *mut Object,
    error: *mut Object,
) {
    unsafe {
        let state_ptr = *this.get_ivar::<usize>("_state") as *const PhotoCaptureState;
        if state_ptr.is_null() {
            return;
        }
        let result = if !error.is_null() {
            Err(ns_error_message(error).unwrap_or_else(|| "macOS photo capture failed".into()))
        } else if photo.is_null() {
            Err("macOS photo capture returned no photo".into())
        } else {
            let data: *mut Object = msg_send![photo, fileDataRepresentation];
            ns_data_to_vec(data).ok_or_else(|| "macOS photo capture returned no bytes".into())
        };
        let state = &*state_ptr;
        if let Ok(mut guard) = state.result.lock() {
            *guard = Some(result);
            state.cvar.notify_all();
        }
    }
}

#[derive(Debug, Default)]
struct MacosMicrophoneHost;

impl MacosMicrophoneHost {
    fn permission_state() -> MicrophonePermission {
        let status: i64 = unsafe {
            msg_send![
                class!(AVCaptureDevice),
                authorizationStatusForMediaType: ns_string("soun")
            ]
        };
        match status {
            3 => MicrophonePermission::Granted,
            2 => MicrophonePermission::Denied,
            1 => MicrophonePermission::Restricted,
            _ => MicrophonePermission::Unknown,
        }
    }

    fn request_microphone_permission() -> MicrophonePermission {
        let state = Self::permission_state();
        if state != MicrophonePermission::Unknown {
            return state;
        }
        let pair = Arc::new((Mutex::new(None), Condvar::new()));
        let pair_for_block = pair.clone();
        let block = ConcreteBlock::new(move |granted: bool| {
            let (lock, cvar) = &*pair_for_block;
            if let Ok(mut result) = lock.lock() {
                *result = Some(granted);
                cvar.notify_all();
            }
        })
        .copy();
        unsafe {
            let _: () = msg_send![
                class!(AVCaptureDevice),
                requestAccessForMediaType: ns_string("soun")
                completionHandler: &*block
            ];
        }
        let (lock, cvar) = &*pair;
        let guard = lock.lock().unwrap();
        let _ = cvar.wait_timeout_while(guard, Duration::from_secs(30), |value| value.is_none());
        Self::permission_state()
    }
}

impl MicrophoneHost for MacosMicrophoneHost {
    fn availability(&self) -> Result<MicrophoneAvailability, MicrophoneError> {
        Ok(MicrophoneAvailability {
            permission: Self::permission_state(),
            devices: macos_microphone_devices(),
        })
    }

    fn request_permission(
        &self,
        _request: MicrophonePermissionRequest,
    ) -> Result<MicrophonePermission, MicrophoneError> {
        Ok(Self::request_microphone_permission())
    }

    fn capture_audio(
        &self,
        request: MicrophoneCaptureRequest,
    ) -> Result<MicrophoneCapture, MicrophoneError> {
        let permission = if Self::permission_state() == MicrophonePermission::Unknown {
            Self::request_microphone_permission()
        } else {
            Self::permission_state()
        };
        if permission != MicrophonePermission::Granted {
            return Err(MicrophoneError::new(
                "permission_denied",
                "macOS microphone permission is not granted",
            ));
        }
        macos_capture_microphone_audio(request)
    }

    fn cancel_capture(&self) -> Result<(), MicrophoneError> {
        Ok(())
    }
}

fn macos_microphone_devices() -> Vec<MicrophoneDevice> {
    unsafe {
        let devices: *mut Object =
            msg_send![class!(AVCaptureDevice), devicesWithMediaType: ns_string("soun")];
        if devices.is_null() {
            return Vec::new();
        }
        let count: usize = msg_send![devices, count];
        let mut result = Vec::new();
        for index in 0..count {
            let device: *mut Object = msg_send![devices, objectAtIndex: index];
            if device.is_null() {
                continue;
            }
            result.push(MicrophoneDevice {
                id: av_device_unique_id(device)
                    .unwrap_or_else(|| format!("macos-microphone-{index}")),
                label: av_device_label(device),
                is_default: index == 0,
            });
        }
        result
    }
}

fn macos_capture_microphone_audio(
    request: MicrophoneCaptureRequest,
) -> Result<MicrophoneCapture, MicrophoneError> {
    unsafe {
        let duration_ms = request.duration_ms.clamp(1, 60_000);
        let sample_rate_hz = request
            .sample_rate_hz
            .unwrap_or(44_100)
            .clamp(8_000, 192_000);
        let channels = request.channels.unwrap_or(1).clamp(1, 2);
        let path = std::env::temp_dir().join(format!(
            "fission-macos-microphone-{}-{}.m4a",
            std::process::id(),
            monotonic_millis()
        ));
        let url: *mut Object = msg_send![
            class!(NSURL),
            fileURLWithPath: ns_string(&path.to_string_lossy())
        ];
        if url.is_null() {
            return Err(MicrophoneError::new(
                "configuration_failed",
                "failed to create macOS audio recording URL",
            ));
        }
        let settings = audio_recorder_settings(sample_rate_hz, channels);
        let mut recorder_error: *mut Object = ptr::null_mut();
        let recorder: *mut Object = msg_send![class!(AVAudioRecorder), alloc];
        let recorder: *mut Object = msg_send![
            recorder,
            initWithURL: url
            settings: settings
            error: &mut recorder_error
        ];
        if recorder.is_null() || !recorder_error.is_null() {
            return Err(MicrophoneError::new(
                "configuration_failed",
                ns_error_message(recorder_error)
                    .unwrap_or_else(|| "failed to create macOS audio recorder".into()),
            ));
        }
        let prepared: bool = msg_send![recorder, prepareToRecord];
        if !prepared {
            return Err(MicrophoneError::new(
                "configuration_failed",
                "macOS audio recorder failed to prepare",
            ));
        }
        let seconds = duration_ms as f64 / 1000.0;
        let recording: bool = msg_send![recorder, recordForDuration: seconds];
        if !recording {
            return Err(MicrophoneError::new(
                "capture_failed",
                "macOS audio recorder failed to start",
            ));
        }
        std::thread::sleep(Duration::from_millis(duration_ms.saturating_add(150)));
        let _: () = msg_send![recorder, stop];
        let bytes = std::fs::read(&path).map_err(|error| {
            MicrophoneError::new(
                "capture_failed",
                format!("failed to read macOS audio recording: {error}"),
            )
        })?;
        let _ = std::fs::remove_file(&path);
        if bytes.is_empty() {
            return Err(MicrophoneError::new(
                "capture_failed",
                "macOS audio recorder produced no bytes",
            ));
        }
        Ok(MicrophoneCapture {
            bytes,
            content_type: "audio/mp4".into(),
            sample_rate_hz,
            channels,
            duration_ms,
            device_id: request
                .device_id
                .or_else(|| Some("macos-default-microphone".into())),
        })
    }
}

unsafe fn audio_recorder_settings(sample_rate_hz: u32, channels: u16) -> *mut Object {
    let settings: *mut Object = msg_send![class!(NSMutableDictionary), dictionary];
    let _: () = msg_send![
        settings,
        setObject: ns_number_u32(1633772320)
        forKey: ns_string("AVFormatIDKey")
    ];
    let _: () = msg_send![
        settings,
        setObject: ns_number_f64(sample_rate_hz as f64)
        forKey: ns_string("AVSampleRateKey")
    ];
    let _: () = msg_send![
        settings,
        setObject: ns_number_i32(channels as i32)
        forKey: ns_string("AVNumberOfChannelsKey")
    ];
    let _: () = msg_send![
        settings,
        setObject: ns_number_i32(96)
        forKey: ns_string("AVEncoderAudioQualityKey")
    ];
    settings
}

#[derive(Debug, Default)]
struct MacosGeolocationHost;

#[repr(C)]
#[derive(Clone, Copy)]
struct CLLocationCoordinate2D {
    latitude: f64,
    longitude: f64,
}

impl MacosGeolocationHost {
    fn permission_state() -> GeolocationPermission {
        let enabled: bool =
            unsafe { msg_send![class!(CLLocationManager), locationServicesEnabled] };
        if !enabled {
            return GeolocationPermission::Denied;
        }
        let status: i64 = unsafe { msg_send![class!(CLLocationManager), authorizationStatus] };
        match status {
            3 | 4 => GeolocationPermission::Granted,
            2 => GeolocationPermission::Denied,
            1 => GeolocationPermission::Denied,
            0 => GeolocationPermission::Prompt,
            _ => GeolocationPermission::Unknown,
        }
    }
}

impl GeolocationHost for MacosGeolocationHost {
    fn permission(&self) -> Result<GeolocationPermission, GeolocationError> {
        Ok(Self::permission_state())
    }

    fn request_permission(
        &self,
        _request: GeolocationPermissionRequest,
    ) -> Result<GeolocationPermission, GeolocationError> {
        let state = Self::permission_state();
        if matches!(
            state,
            GeolocationPermission::Prompt | GeolocationPermission::Unknown
        ) {
            unsafe {
                let manager = location_manager();
                if !manager.is_null() {
                    let _: () = msg_send![manager, requestWhenInUseAuthorization];
                }
            }
        }
        Ok(Self::permission_state())
    }

    fn current_position(
        &self,
        request: GeolocationPositionRequest,
    ) -> Result<GeolocationPosition, GeolocationError> {
        if Self::permission_state() != GeolocationPermission::Granted {
            return Err(GeolocationError::new(
                "permission_denied",
                "macOS location permission is not granted",
            ));
        }
        unsafe {
            let manager = location_manager();
            if manager.is_null() {
                return Err(GeolocationError::new(
                    "unavailable",
                    "CLLocationManager is not available",
                ));
            }
            let desired_accuracy = if request.high_accuracy {
                -1.0f64
            } else {
                3000.0f64
            };
            let _: () = msg_send![manager, setDesiredAccuracy: desired_accuracy];
            let _: () = msg_send![manager, startUpdatingLocation];
            std::thread::sleep(Duration::from_millis(500));
            let location: *mut Object = msg_send![manager, location];
            let _: () = msg_send![manager, stopUpdatingLocation];
            if location.is_null() {
                return Err(GeolocationError::new(
                    "unavailable",
                    "macOS has not provided a current location for this app session",
                ));
            }
            Ok(location_to_position(location))
        }
    }
}

unsafe fn location_manager() -> *mut Object {
    static MANAGER: OnceLock<usize> = OnceLock::new();
    *MANAGER.get_or_init(|| {
        let manager: *mut Object = msg_send![class!(CLLocationManager), new];
        manager as usize
    }) as *mut Object
}

#[derive(Debug, Default)]
struct MacosBiometricHost;

impl MacosBiometricHost {
    fn availability_for_policy(policy: i64) -> (bool, Option<String>, Option<BiometricKind>) {
        unsafe {
            let context: *mut Object = msg_send![class!(LAContext), new];
            if context.is_null() {
                return (false, Some("LAContext is not available".into()), None);
            }
            let mut error: *mut Object = ptr::null_mut();
            let ok: bool = msg_send![context, canEvaluatePolicy: policy error: &mut error];
            let kind = biometric_kind_for_context(context);
            (ok, ns_error_message(error), kind)
        }
    }
}

impl BiometricHost for MacosBiometricHost {
    fn availability(&self) -> Result<BiometricAvailability, BiometricError> {
        let (biometric_ok, reason, kind) = Self::availability_for_policy(1);
        let (credential_ok, _, _) = Self::availability_for_policy(2);
        Ok(BiometricAvailability {
            supported: biometric_ok || credential_ok,
            enrolled: biometric_ok,
            strong: biometric_ok,
            weak: biometric_ok,
            device_credential: credential_ok,
            kinds: kind.into_iter().collect(),
            reason: if biometric_ok || credential_ok {
                None
            } else {
                reason
            },
        })
    }

    fn authenticate(
        &self,
        request: BiometricAuthenticateRequest,
    ) -> Result<BiometricAuthenticateResult, BiometricError> {
        let policy = if request.allow_device_credential {
            2i64
        } else {
            1i64
        };
        let (available, reason, kind) = Self::availability_for_policy(policy);
        if !available {
            return Err(BiometricError::new(
                "unavailable",
                reason.unwrap_or_else(|| "macOS biometric authentication is unavailable".into()),
            ));
        }
        let pair = Arc::new((Mutex::new(None), Condvar::new()));
        let pair_for_block = pair.clone();
        let block = ConcreteBlock::new(move |success: bool, error: *mut Object| {
            let message = unsafe { ns_error_message(error) };
            let (lock, cvar) = &*pair_for_block;
            if let Ok(mut result) = lock.lock() {
                *result = Some((success, message));
                cvar.notify_all();
            }
        })
        .copy();
        unsafe {
            let context: *mut Object = msg_send![class!(LAContext), new];
            if context.is_null() {
                return Err(BiometricError::new(
                    "unavailable",
                    "LAContext is not available",
                ));
            }
            if let Some(cancel_title) = request.cancel_title.as_deref() {
                let _: () = msg_send![context, setLocalizedCancelTitle: ns_string(cancel_title)];
            }
            if let Some(fallback_title) = request.fallback_title.as_deref() {
                let _: () =
                    msg_send![context, setLocalizedFallbackTitle: ns_string(fallback_title)];
            }
            let _: () = msg_send![
                context,
                evaluatePolicy: policy
                localizedReason: ns_string(&request.reason)
                reply: &*block
            ];
        }
        let (lock, cvar) = &*pair;
        let guard = lock.lock().unwrap();
        let (mut guard, _) = cvar
            .wait_timeout_while(guard, Duration::from_secs(90), |value| value.is_none())
            .unwrap();
        match guard.take() {
            Some((true, _)) => Ok(BiometricAuthenticateResult {
                verified: true,
                kind: kind.clone(),
                used_device_credential: policy == 2 && kind.is_none(),
            }),
            Some((false, Some(message))) => Err(BiometricError::new("not_verified", message)),
            Some((false, None)) => Err(BiometricError::new(
                "not_verified",
                "macOS biometric authentication was not verified",
            )),
            None => Err(BiometricError::new(
                "timeout",
                "macOS biometric authentication timed out",
            )),
        }
    }

    fn cancel_authentication(&self) -> Result<(), BiometricError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct MacosBluetoothHost;

impl BluetoothHost for MacosBluetoothHost {
    fn availability(&self) -> Result<BluetoothAvailability, BluetoothError> {
        Ok(BluetoothAvailability {
            permission: BluetoothPermission::Granted,
            enabled: macos_bluetooth_powered(),
            supports_classic: true,
            supports_low_energy: true,
        })
    }

    fn request_permission(
        &self,
        _request: BluetoothPermissionRequest,
    ) -> Result<BluetoothPermission, BluetoothError> {
        Ok(BluetoothPermission::Granted)
    }

    fn scan_devices(
        &self,
        _request: BluetoothScanRequest,
    ) -> Result<BluetoothScanResult, BluetoothError> {
        Ok(BluetoothScanResult {
            devices: macos_paired_bluetooth_devices(),
        })
    }

    fn connect_device(
        &self,
        request: BluetoothConnectRequest,
    ) -> Result<BluetoothConnection, BluetoothError> {
        let device = macos_paired_bluetooth_devices()
            .into_iter()
            .find(|device| device.id == request.device_id)
            .ok_or_else(|| BluetoothError::new("not_found", "Bluetooth device was not found"))?;
        Ok(BluetoothConnection {
            connection_id: format!("macos-bluetooth-{}", device.id),
            device,
        })
    }

    fn disconnect_device(
        &self,
        _request: BluetoothDisconnectRequest,
    ) -> Result<(), BluetoothError> {
        Ok(())
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
        _request: fission_core::BluetoothAdvertiseRequest,
    ) -> Result<fission_core::BluetoothAdvertiseReceipt, BluetoothError> {
        Err(BluetoothError::unsupported("start_advertising"))
    }

    fn stop_advertising(
        &self,
        _request: fission_core::BluetoothStopAdvertiseRequest,
    ) -> Result<(), BluetoothError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct MacosWifiHost;

impl WifiHost for MacosWifiHost {
    fn availability(&self) -> Result<WifiAvailability, WifiError> {
        let connected = macos_current_wifi_network();
        Ok(WifiAvailability {
            permission: WifiPermission::Granted,
            enabled: connected.is_some() || macos_wifi_powered(),
            connected_network: connected,
        })
    }

    fn request_permission(
        &self,
        _request: WifiPermissionRequest,
    ) -> Result<WifiPermission, WifiError> {
        Ok(WifiPermission::Granted)
    }

    fn scan_networks(&self, request: WifiScanRequest) -> Result<WifiScanResult, WifiError> {
        let mut networks = macos_scan_wifi_networks();
        if let Some(prefix) = request.ssid_prefix.as_deref() {
            networks.retain(|network| network.ssid.starts_with(prefix));
        }
        Ok(WifiScanResult { networks })
    }

    fn connect_network(&self, _request: WifiConnectRequest) -> Result<WifiConnection, WifiError> {
        Err(WifiError::unsupported("connect_network"))
    }

    fn disconnect_network(&self, _request: WifiDisconnectRequest) -> Result<(), WifiError> {
        Err(WifiError::unsupported("disconnect_network"))
    }
}

fn macos_bluetooth_powered() -> bool {
    Command::new("system_profiler")
        .args(["SPBluetoothDataType"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|text| text.contains("State: On") || text.contains("Bluetooth Power: On"))
}

fn macos_paired_bluetooth_devices() -> Vec<BluetoothDevice> {
    let output = Command::new("system_profiler")
        .args(["SPBluetoothDataType"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default();
    let mut devices = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with(':')
            && !trimmed.contains("Bluetooth")
            && !trimmed.contains("Devices")
            && !trimmed.contains("Services")
        {
            let name = trimmed.trim_end_matches(':').trim();
            if !name.is_empty() && name.len() < 80 {
                devices.push(BluetoothDevice {
                    id: sanitize_id(name),
                    name: Some(name.to_string()),
                    address: None,
                    rssi: None,
                    paired: true,
                    modes: vec![fission_core::BluetoothMode::LowEnergy],
                });
            }
        }
    }
    devices.truncate(12);
    devices
}

fn macos_wifi_powered() -> bool {
    Command::new("networksetup")
        .args(["-getairportpower", "en0"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|text| text.to_ascii_lowercase().contains("on"))
}

fn macos_current_wifi_network() -> Option<WifiNetwork> {
    let output = Command::new("networksetup")
        .args(["-getairportnetwork", "en0"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())?;
    let ssid = output.split(':').nth(1)?.trim();
    if ssid.is_empty()
        || ssid.eq_ignore_ascii_case("you are not associated with an airport network")
    {
        return None;
    }
    Some(WifiNetwork {
        ssid: ssid.to_string(),
        bssid: None,
        rssi: None,
        frequency_mhz: None,
        security: WifiSecurity::Unknown,
        connected: true,
    })
}

fn macos_scan_wifi_networks() -> Vec<WifiNetwork> {
    let airport =
        "/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport";
    let output = Command::new(airport)
        .arg("-s")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default();
    let mut networks = Vec::new();
    for line in output.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((ssid, rest)) = split_airport_scan_line(trimmed) else {
            continue;
        };
        let columns = rest.split_whitespace().collect::<Vec<_>>();
        let rssi = columns.get(1).and_then(|value| value.parse::<i16>().ok());
        let channel = columns.get(2).and_then(|value| value.split(',').next());
        let frequency_mhz = channel
            .and_then(|value| value.parse::<u32>().ok())
            .map(|channel| {
                if channel <= 14 {
                    2407 + channel * 5
                } else {
                    5000 + channel * 5
                }
            });
        networks.push(WifiNetwork {
            ssid,
            bssid: columns.first().map(|value| (*value).to_string()),
            rssi,
            frequency_mhz,
            security: WifiSecurity::Unknown,
            connected: false,
        });
    }
    if networks.is_empty() {
        if let Some(current) = macos_current_wifi_network() {
            networks.push(current);
        }
    }
    networks.truncate(40);
    networks
}

fn split_airport_scan_line(line: &str) -> Option<(String, &str)> {
    let bssid_start = find_bssid_start(line)?;
    let ssid = line[..bssid_start].trim().to_string();
    let rest = line[bssid_start..].trim();
    (!ssid.is_empty()).then_some((ssid, rest))
}

fn find_bssid_start(line: &str) -> Option<usize> {
    line.char_indices()
        .filter(|(index, _)| *index == 0 || line[..*index].ends_with(char::is_whitespace))
        .map(|(index, _)| index)
        .find(|index| {
            let candidate = line.get(*index..index.saturating_add(17)).unwrap_or("");
            candidate.len() == 17
                && candidate.chars().enumerate().all(|(pos, ch)| {
                    if pos % 3 == 2 {
                        ch == ':'
                    } else {
                        ch.is_ascii_hexdigit()
                    }
                })
        })
}

unsafe fn av_device_unique_id(device: *mut Object) -> Option<String> {
    let value: *mut Object = msg_send![device, uniqueID];
    ns_string_to_string(value)
}

unsafe fn av_device_label(device: *mut Object) -> Option<String> {
    let value: *mut Object = msg_send![device, localizedName];
    ns_string_to_string(value)
}

unsafe fn av_device_facing(device: *mut Object) -> CameraFacing {
    let position: i64 = msg_send![device, position];
    match position {
        1 => CameraFacing::Back,
        2 => CameraFacing::Front,
        _ => CameraFacing::Unspecified,
    }
}

unsafe fn biometric_kind_for_context(context: *mut Object) -> Option<BiometricKind> {
    let value: i64 = msg_send![context, biometryType];
    match value {
        1 => Some(BiometricKind::Fingerprint),
        2 => Some(BiometricKind::Face),
        _ => None,
    }
}

unsafe fn location_to_position(location: *mut Object) -> GeolocationPosition {
    let coordinate: CLLocationCoordinate2D = msg_send![location, coordinate];
    let altitude: f64 = msg_send![location, altitude];
    let horizontal_accuracy: f64 = msg_send![location, horizontalAccuracy];
    let vertical_accuracy: f64 = msg_send![location, verticalAccuracy];
    let course: f64 = msg_send![location, course];
    let speed: f64 = msg_send![location, speed];
    let timestamp: *mut Object = msg_send![location, timestamp];
    let timestamp_seconds: f64 = if timestamp.is_null() {
        0.0
    } else {
        msg_send![timestamp, timeIntervalSince1970]
    };
    GeolocationPosition {
        latitude: coordinate.latitude,
        longitude: coordinate.longitude,
        altitude_meters: (vertical_accuracy >= 0.0).then_some(altitude),
        accuracy_meters: horizontal_accuracy.max(0.0),
        altitude_accuracy_meters: (vertical_accuracy >= 0.0).then_some(vertical_accuracy),
        heading_degrees: (course >= 0.0).then_some(course),
        speed_mps: (speed >= 0.0).then_some(speed),
        timestamp_unix_ms: (timestamp_seconds.max(0.0) * 1000.0) as u64,
    }
}

unsafe fn ns_data_to_vec(data: *mut Object) -> Option<Vec<u8>> {
    if data.is_null() {
        return None;
    }
    let len: usize = msg_send![data, length];
    if len == 0 {
        return None;
    }
    let ptr: *const u8 = msg_send![data, bytes];
    if ptr.is_null() {
        return None;
    }
    Some(std::slice::from_raw_parts(ptr, len).to_vec())
}

unsafe fn ns_number_i32(value: i32) -> *mut Object {
    msg_send![class!(NSNumber), numberWithInt: value]
}

unsafe fn ns_number_u32(value: u32) -> *mut Object {
    msg_send![class!(NSNumber), numberWithUnsignedInt: value]
}

unsafe fn ns_number_f64(value: f64) -> *mut Object {
    msg_send![class!(NSNumber), numberWithDouble: value]
}

unsafe fn ns_error_message(error: *mut Object) -> Option<String> {
    if error.is_null() {
        return None;
    }
    let description: *mut Object = msg_send![error, localizedDescription];
    ns_string_to_string(description)
}

unsafe fn ns_string_to_string(value: *mut Object) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let ptr: *const c_char = msg_send![value, UTF8String];
    if ptr.is_null() {
        return None;
    }
    Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
}

fn ns_string(value: &str) -> *mut Object {
    unsafe {
        let string: *mut Object = msg_send![class!(NSString), alloc];
        msg_send![
            string,
            initWithBytes: value.as_ptr() as *const c_void
            length: value.len()
            encoding: 4usize
        ]
    }
}

fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    image::load_from_memory(bytes)
        .ok()
        .map(|image| (image.width(), image.height()))
}

fn monotonic_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn airport_scan_line_splits_ssid_from_bssid_columns() {
        let (ssid, rest) =
            split_airport_scan_line("Fission Lab       00:11:22:33:44:55 -42 11 Y -- WPA2")
                .unwrap();
        assert_eq!(ssid, "Fission Lab");
        assert!(rest.starts_with("00:11:22:33:44:55"));
    }
}
