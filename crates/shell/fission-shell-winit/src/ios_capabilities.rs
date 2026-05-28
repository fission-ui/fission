use crate::{
    barcode, barcode_decode, camera, geolocation, microphone, BarcodeScannerHost, CameraHost,
    GeolocationHost, MicrophoneHost,
};
use block::ConcreteBlock;
use dispatch::Queue;
use fission_core::{
    BarcodeImageDecodeRequest, BarcodeScanRequest, BarcodeScanResults, BarcodeScannerError,
    CameraAvailability, CameraCapture, CameraCaptureRequest, CameraDevice, CameraError,
    CameraFacing, CameraFlashlightRequest, CameraImageFormat, CameraPermission,
    CameraPermissionRequest, GeolocationError, GeolocationPermission, GeolocationPermissionRequest,
    GeolocationPosition, GeolocationPositionRequest, MicrophoneAvailability, MicrophoneCapture,
    MicrophoneCaptureRequest, MicrophoneDevice, MicrophoneError, MicrophonePermission,
    MicrophonePermissionRequest,
};
use fission_shell::async_host::AsyncRegistry;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Protocol, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::os::raw::c_void;
use std::ptr;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

#[link(name = "CoreLocation", kind = "framework")]
extern "C" {}

#[link(name = "UIKit", kind = "framework")]
extern "C" {}

pub(crate) fn register_ios_operation_capabilities(async_registry: &mut AsyncRegistry) {
    camera::register_camera_capabilities(async_registry, Arc::new(IosCameraHost));
    barcode::register_barcode_scanner_capabilities(async_registry, Arc::new(IosBarcodeScannerHost));
    geolocation::register_geolocation_capabilities(async_registry, Arc::new(IosGeolocationHost));
    microphone::register_microphone_capabilities(async_registry, Arc::new(IosMicrophoneHost));
}

#[derive(Debug, Default)]
struct IosCameraHost;

impl IosCameraHost {
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

impl CameraHost for IosCameraHost {
    fn availability(&self) -> Result<CameraAvailability, CameraError> {
        Ok(CameraAvailability {
            permission: Self::permission_state(),
            devices: ios_camera_devices(),
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
                "iOS camera permission is not granted",
            ));
        }
        ios_capture_photo(request)
    }

    fn set_flashlight(&self, request: CameraFlashlightRequest) -> Result<(), CameraError> {
        ios_set_flashlight(request)
    }

    fn cancel_capture(&self) -> Result<(), CameraError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct IosBarcodeScannerHost;

impl BarcodeScannerHost for IosBarcodeScannerHost {
    fn scan(&self, request: BarcodeScanRequest) -> Result<BarcodeScanResults, BarcodeScannerError> {
        let capture = IosCameraHost
            .capture_photo(CameraCaptureRequest {
                camera_id: request.camera_id,
                facing: CameraFacing::Back,
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

fn ios_camera_devices() -> Vec<CameraDevice> {
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
                id: ios_device_unique_id(device).unwrap_or_else(|| format!("ios-camera-{index}")),
                label: ios_device_label(device),
                facing: ios_device_facing(device),
                has_flashlight: msg_send![device, hasTorch],
            });
        }
        result
    }
}

fn ios_capture_photo(request: CameraCaptureRequest) -> Result<CameraCapture, CameraError> {
    unsafe {
        let device = select_ios_camera_device(request.camera_id.as_deref(), request.facing)
            .ok_or_else(|| {
                CameraError::new("unavailable", "no matching iOS camera is available")
            })?;
        let input = capture_device_input(device)?;
        let session: *mut Object = msg_send![class!(AVCaptureSession), new];
        let output: *mut Object = msg_send![class!(AVCapturePhotoOutput), new];
        if session.is_null() || output.is_null() {
            return Err(CameraError::new(
                "unavailable",
                "AVFoundation capture session is not available",
            ));
        }

        let can_add_input: bool = msg_send![session, canAddInput: input];
        if !can_add_input {
            return Err(CameraError::new(
                "configuration_failed",
                "iOS camera input cannot be added to the capture session",
            ));
        }
        let _: () = msg_send![session, addInput: input];
        let can_add_output: bool = msg_send![session, canAddOutput: output];
        if !can_add_output {
            return Err(CameraError::new(
                "configuration_failed",
                "iOS photo output cannot be added to the capture session",
            ));
        }
        let _: () = msg_send![session, addOutput: output];

        let state = Arc::new(PhotoCaptureState::new());
        let delegate: *mut Object = msg_send![photo_capture_delegate_class(), new];
        if delegate.is_null() {
            return Err(CameraError::new(
                "configuration_failed",
                "iOS photo delegate could not be created",
            ));
        }
        (*delegate).set_ivar("_state", Arc::as_ptr(&state) as usize);

        let settings: *mut Object = msg_send![class!(AVCapturePhotoSettings), photoSettings];
        let _: () = msg_send![session, startRunning];
        let _: () = msg_send![output, capturePhotoWithSettings: settings delegate: delegate];
        let timeout = Duration::from_millis(7_500);
        let guard = state.result.lock().unwrap();
        let (mut guard, _) = state
            .cvar
            .wait_timeout_while(guard, timeout, |value| value.is_none())
            .unwrap();
        let _: () = msg_send![session, stopRunning];

        let bytes = match guard.take() {
            Some(Ok(bytes)) => bytes,
            Some(Err(message)) => return Err(CameraError::new("capture_failed", message)),
            None => {
                return Err(CameraError::new(
                    "timeout",
                    "iOS did not produce a photo before the request timed out",
                ))
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
            camera_id: ios_device_unique_id(device).or(request.camera_id),
        })
    }
}

fn ios_set_flashlight(request: CameraFlashlightRequest) -> Result<(), CameraError> {
    unsafe {
        let device = select_ios_camera_device(request.camera_id.as_deref(), CameraFacing::Back)
            .ok_or_else(|| {
                CameraError::new("unavailable", "no matching iOS camera is available")
            })?;
        let has_torch: bool = msg_send![device, hasTorch];
        if !has_torch {
            return Err(CameraError::new(
                "unavailable",
                "selected iOS camera does not have a torch",
            ));
        }
        let mut error: *mut Object = ptr::null_mut();
        let locked: bool = msg_send![device, lockForConfiguration: &mut error];
        if !locked || !error.is_null() {
            return Err(CameraError::new(
                "configuration_failed",
                ns_error_message(error).unwrap_or_else(|| "failed to lock iOS camera".into()),
            ));
        }
        if request.enabled {
            let level = request
                .intensity
                .map(|value| (value as f32 / 255.0).clamp(0.01, 1.0))
                .unwrap_or(1.0);
            let mut torch_error: *mut Object = ptr::null_mut();
            let ok: bool =
                msg_send![device, setTorchModeOnWithLevel: level error: &mut torch_error];
            if !ok || !torch_error.is_null() {
                let _: () = msg_send![device, unlockForConfiguration];
                return Err(CameraError::new(
                    "configuration_failed",
                    ns_error_message(torch_error)
                        .unwrap_or_else(|| "failed to enable iOS torch".into()),
                ));
            }
        } else {
            let _: () = msg_send![device, setTorchMode: 0i64];
        }
        let _: () = msg_send![device, unlockForConfiguration];
        Ok(())
    }
}

unsafe fn capture_device_input(device: *mut Object) -> Result<*mut Object, CameraError> {
    let mut error: *mut Object = ptr::null_mut();
    let input: *mut Object =
        msg_send![class!(AVCaptureDeviceInput), deviceInputWithDevice: device error: &mut error];
    if input.is_null() || !error.is_null() {
        return Err(CameraError::new(
            "configuration_failed",
            ns_error_message(error).unwrap_or_else(|| "failed to create iOS camera input".into()),
        ));
    }
    Ok(input)
}

unsafe fn select_ios_camera_device(
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
            if ios_device_unique_id(device).as_deref() == Some(id) {
                return Some(device);
            }
        } else if requested_facing != CameraFacing::Unspecified
            && ios_device_facing(device) == requested_facing
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
        let mut decl = ClassDecl::new("FissionPhotoCaptureDelegate", superclass)
            .expect("register FissionPhotoCaptureDelegate");
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
            Err(ns_error_message(error).unwrap_or_else(|| "iOS photo capture failed".into()))
        } else if photo.is_null() {
            Err("iOS photo capture returned no photo".into())
        } else {
            let data: *mut Object = msg_send![photo, fileDataRepresentation];
            ns_data_to_vec(data).ok_or_else(|| "iOS photo capture returned no bytes".into())
        };
        let state = &*state_ptr;
        if let Ok(mut guard) = state.result.lock() {
            *guard = Some(result);
            state.cvar.notify_all();
        }
    }
}

unsafe fn ios_device_unique_id(device: *mut Object) -> Option<String> {
    let value: *mut Object = msg_send![device, uniqueID];
    ns_string_to_string(value)
}

unsafe fn ios_device_label(device: *mut Object) -> Option<String> {
    let value: *mut Object = msg_send![device, localizedName];
    ns_string_to_string(value)
}

unsafe fn ios_device_facing(device: *mut Object) -> CameraFacing {
    let position: i64 = msg_send![device, position];
    match position {
        1 => CameraFacing::Back,
        2 => CameraFacing::Front,
        _ => CameraFacing::Unspecified,
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
    let ptr: *const std::os::raw::c_char = msg_send![value, UTF8String];
    if ptr.is_null() {
        return None;
    }
    Some(std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned())
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

#[derive(Debug, Default)]
struct IosMicrophoneHost;

const AV_RECORD_PERMISSION_DENIED: usize = u32::from_be_bytes(*b"deny") as usize;
const AV_RECORD_PERMISSION_GRANTED: usize = u32::from_be_bytes(*b"grnt") as usize;

impl IosMicrophoneHost {
    fn permission_state() -> MicrophonePermission {
        let session: *mut objc::runtime::Object =
            unsafe { msg_send![class!(AVAudioSession), sharedInstance] };
        if session.is_null() {
            return MicrophonePermission::Restricted;
        }
        let permission: usize = unsafe { msg_send![session, recordPermission] };
        match_ios_microphone_permission(permission)
    }

    fn request_microphone_permission() -> MicrophonePermission {
        let state = Self::permission_state();
        if state != MicrophonePermission::Unknown {
            return state;
        }
        let session: *mut objc::runtime::Object =
            unsafe { msg_send![class!(AVAudioSession), sharedInstance] };
        if session.is_null() {
            return MicrophonePermission::Restricted;
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
            let _: () = msg_send![session, requestRecordPermission: &*block];
        }
        let (lock, cvar) = &*pair;
        let guard = lock.lock().unwrap();
        let _ = cvar.wait_timeout_while(guard, Duration::from_secs(30), |value| value.is_none());
        Self::permission_state()
    }
}

fn match_ios_microphone_permission(permission: usize) -> MicrophonePermission {
    match permission {
        AV_RECORD_PERMISSION_GRANTED => MicrophonePermission::Granted,
        AV_RECORD_PERMISSION_DENIED => MicrophonePermission::Denied,
        _ => MicrophonePermission::Unknown,
    }
}

impl MicrophoneHost for IosMicrophoneHost {
    fn availability(&self) -> Result<MicrophoneAvailability, MicrophoneError> {
        let session: *mut objc::runtime::Object =
            unsafe { msg_send![class!(AVAudioSession), sharedInstance] };
        let available = if session.is_null() {
            false
        } else {
            unsafe { msg_send![session, isInputAvailable] }
        };
        Ok(MicrophoneAvailability {
            permission: Self::permission_state(),
            devices: if available {
                vec![MicrophoneDevice {
                    id: "ios-default-microphone".into(),
                    label: Some("iOS default microphone".into()),
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
                "iOS microphone permission is not granted",
            ));
        }
        ios_capture_microphone_audio(request)
    }

    fn cancel_capture(&self) -> Result<(), MicrophoneError> {
        Ok(())
    }
}

fn ios_capture_microphone_audio(
    request: MicrophoneCaptureRequest,
) -> Result<MicrophoneCapture, MicrophoneError> {
    unsafe {
        let session: *mut Object = msg_send![class!(AVAudioSession), sharedInstance];
        if session.is_null() {
            return Err(MicrophoneError::new(
                "unavailable",
                "AVAudioSession is not available",
            ));
        }
        let mut session_error: *mut Object = ptr::null_mut();
        let category_ok: bool = msg_send![
            session,
            setCategory: ns_string("AVAudioSessionCategoryRecord")
            error: &mut session_error
        ];
        if !category_ok || !session_error.is_null() {
            return Err(MicrophoneError::new(
                "configuration_failed",
                ns_error_message(session_error)
                    .unwrap_or_else(|| "failed to configure iOS audio session".into()),
            ));
        }
        let mut active_error: *mut Object = ptr::null_mut();
        let active_ok: bool = msg_send![session, setActive: true error: &mut active_error];
        if !active_ok || !active_error.is_null() {
            return Err(MicrophoneError::new(
                "configuration_failed",
                ns_error_message(active_error)
                    .unwrap_or_else(|| "failed to activate iOS audio session".into()),
            ));
        }

        let duration_ms = request.duration_ms.clamp(1, 60_000);
        let sample_rate_hz = request
            .sample_rate_hz
            .unwrap_or(44_100)
            .clamp(8_000, 192_000);
        let channels = request.channels.unwrap_or(1).clamp(1, 2);
        let path = std::env::temp_dir().join(format!(
            "fission-microphone-{}-{}.m4a",
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
                "failed to create iOS audio recording URL",
            ));
        }
        let settings = ios_audio_recorder_settings(sample_rate_hz, channels);
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
                    .unwrap_or_else(|| "failed to create iOS audio recorder".into()),
            ));
        }
        let prepared: bool = msg_send![recorder, prepareToRecord];
        if !prepared {
            return Err(MicrophoneError::new(
                "configuration_failed",
                "iOS audio recorder failed to prepare",
            ));
        }
        let seconds = duration_ms as f64 / 1000.0;
        let recording: bool = msg_send![recorder, recordForDuration: seconds];
        if !recording {
            return Err(MicrophoneError::new(
                "capture_failed",
                "iOS audio recorder failed to start",
            ));
        }
        std::thread::sleep(Duration::from_millis(duration_ms.saturating_add(150)));
        let _: () = msg_send![recorder, stop];
        let _: () = msg_send![session, setActive: false error: ptr::null_mut::<Object>()];
        let bytes = std::fs::read(&path).map_err(|error| {
            MicrophoneError::new(
                "capture_failed",
                format!("failed to read iOS audio recording: {error}"),
            )
        })?;
        let _ = std::fs::remove_file(&path);
        if bytes.is_empty() {
            return Err(MicrophoneError::new(
                "capture_failed",
                "iOS audio recorder produced no bytes",
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
                .or_else(|| Some("ios-default-microphone".into())),
        })
    }
}

unsafe fn ios_audio_recorder_settings(sample_rate_hz: u32, channels: u16) -> *mut Object {
    let settings: *mut Object = msg_send![class!(NSMutableDictionary), dictionary];
    let format_key = ns_string("AVFormatIDKey");
    let sample_rate_key = ns_string("AVSampleRateKey");
    let channels_key = ns_string("AVNumberOfChannelsKey");
    let quality_key = ns_string("AVEncoderAudioQualityKey");
    let _: () = msg_send![
        settings,
        setObject: ns_number_u32(1633772320)
        forKey: format_key
    ];
    let _: () = msg_send![
        settings,
        setObject: ns_number_f64(sample_rate_hz as f64)
        forKey: sample_rate_key
    ];
    let _: () = msg_send![
        settings,
        setObject: ns_number_i32(channels as i32)
        forKey: channels_key
    ];
    let _: () = msg_send![
        settings,
        setObject: ns_number_i32(96)
        forKey: quality_key
    ];
    settings
}

#[derive(Debug, Default)]
struct IosGeolocationHost;

#[repr(C)]
#[derive(Clone, Copy)]
struct CLLocationCoordinate2D {
    latitude: f64,
    longitude: f64,
}

impl IosGeolocationHost {
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

impl GeolocationHost for IosGeolocationHost {
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
            Queue::main().exec_async(ios_request_location_permission_on_main);
        }
        Ok(state)
    }

    fn current_position(
        &self,
        request: GeolocationPositionRequest,
    ) -> Result<GeolocationPosition, GeolocationError> {
        if Self::permission_state() != GeolocationPermission::Granted {
            return Err(GeolocationError::new(
                "permission_denied",
                "iOS location permission is not granted",
            ));
        }
        unsafe {
            let manager: *mut objc::runtime::Object = msg_send![class!(CLLocationManager), new];
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
            let location: *mut objc::runtime::Object = msg_send![manager, location];
            if location.is_null() {
                return Err(GeolocationError::new(
                    "unavailable",
                    "iOS has not provided a current location for this app session",
                ));
            }
            Ok(ios_location_to_position(location))
        }
    }
}

fn ios_request_location_permission_on_main() {
    unsafe {
        let manager: *mut objc::runtime::Object = msg_send![class!(CLLocationManager), new];
        if !manager.is_null() {
            let _: () = msg_send![manager, requestWhenInUseAuthorization];
        }
    }
}

unsafe fn ios_location_to_position(location: *mut objc::runtime::Object) -> GeolocationPosition {
    let coordinate: CLLocationCoordinate2D = msg_send![location, coordinate];
    let altitude: f64 = msg_send![location, altitude];
    let horizontal_accuracy: f64 = msg_send![location, horizontalAccuracy];
    let vertical_accuracy: f64 = msg_send![location, verticalAccuracy];
    let course: f64 = msg_send![location, course];
    let speed: f64 = msg_send![location, speed];
    let timestamp: *mut objc::runtime::Object = msg_send![location, timestamp];
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

fn ns_string(value: &str) -> *mut objc::runtime::Object {
    unsafe {
        let string: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
        msg_send![
            string,
            initWithBytes: value.as_ptr() as *const c_void
            length: value.len()
            encoding: 4usize
        ]
    }
}
