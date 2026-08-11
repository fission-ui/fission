use super::*;

pub(super) fn system_service<'local>(
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

pub(super) fn ensure_vibrator(env: &mut JNIEnv<'_>, vibrator: &JObject<'_>) -> JniResult<()> {
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

pub(super) fn char_sequence_to_string(
    env: &mut JNIEnv<'_>,
    value: &JObject<'_>,
) -> JniResult<String> {
    let string = env
        .call_method(value, "toString", "()Ljava/lang/String;", &[])?
        .l()?;
    java_string(env, string)
}

pub(super) fn java_string(env: &mut JNIEnv<'_>, object: JObject<'_>) -> JniResult<String> {
    if object.as_raw().is_null() {
        return Ok(String::new());
    }
    let string = JString::from(object);
    let value: String = env.get_string(&string)?.into();
    Ok(value)
}

pub(super) fn java_string_array<'local>(
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

pub(super) fn android_bluetooth_device_row(row: &str) -> Option<BluetoothDevice> {
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

pub(super) fn android_biometric_auth_result(
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

pub(super) fn android_stream_id(stream: VolumeStream) -> i32 {
    match stream {
        VolumeStream::Media => 3,
        VolumeStream::Ring => 2,
        VolumeStream::Alarm => 4,
        VolumeStream::Notification => 5,
        VolumeStream::Call => 0,
        VolumeStream::System => 1,
    }
}

pub(super) fn percent_to_platform_volume(level: u8, max_volume: i32) -> i32 {
    ((i32::from(level.min(100)) * max_volume) + 50) / 100
}

pub(super) fn platform_volume_to_percent(level: i32, max_volume: i32) -> u8 {
    if max_volume <= 0 {
        return 0;
    }
    ((level.clamp(0, max_volume) * 100) / max_volume) as u8
}

pub(super) fn android_get_volume_level(
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

pub(super) fn android_current_position(
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

pub(super) fn finite_value(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

pub(super) fn android_capture_audio(
    env: &mut JNIEnv<'_>,
    request: MicrophoneCaptureRequest,
    ctx: &CapabilityCtx,
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
    let byte_len = bytes.len() as u64;
    let stream = ctx.register_data_stream(single_chunk_data_stream(bytes));
    Ok(MicrophoneCapture {
        stream,
        byte_len: Some(byte_len),
        content_type: format!("audio/pcm; format={format_label}"),
        sample_rate_hz,
        channels,
        duration_ms,
        device_id: Some("android-default-microphone".into()),
    })
}

pub(super) fn encode_audio_samples(
    samples: &[i16],
    format: AudioSampleFormat,
) -> (Vec<u8>, &'static str) {
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

pub(super) struct AndroidPhotoBytes {
    pub(super) bytes: Vec<u8>,
    width: u32,
    height: u32,
    camera_id: Option<String>,
}

pub(super) fn android_capture_photo_bytes(
    context: &AndroidHostContext,
    request: CameraCaptureRequest,
) -> Result<AndroidPhotoBytes, String> {
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
        Ok(AndroidPhotoBytes {
            bytes,
            width: actual_width,
            height: actual_height,
            camera_id: request.camera_id,
        })
    })
}

pub(super) fn android_capture_photo(
    context: &AndroidHostContext,
    request: CameraCaptureRequest,
    ctx: &CapabilityCtx,
) -> Result<CameraCapture, String> {
    let capture = android_capture_photo_bytes(context, request)?;
    let byte_len = capture.bytes.len() as u64;
    let stream = ctx.register_data_stream(single_chunk_data_stream(capture.bytes));
    Ok(CameraCapture {
        stream,
        byte_len: Some(byte_len),
        content_type: "image/jpeg".into(),
        width: capture.width,
        height: capture.height,
        camera_id: capture.camera_id,
    })
}

pub(super) fn barcode_camera_error(error: String) -> BarcodeScannerError {
    BarcodeScannerError::new("camera_error", error)
}

pub(super) fn app_class<'local>(
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

pub(super) fn android_torch_camera_id(
    env: &mut JNIEnv<'_>,
    manager: &JObject<'_>,
) -> JniResult<String> {
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

pub(super) fn android_camera_facing(
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

pub(super) fn android_camera_has_flashlight(
    env: &mut JNIEnv<'_>,
    manager: &JObject<'_>,
    id: &str,
) -> JniResult<bool> {
    let value = android_camera_characteristic(env, manager, id, "FLASH_INFO_AVAILABLE")?;
    env.call_method(&value, "booleanValue", "()Z", &[])?.z()
}

pub(super) fn android_camera_characteristic<'local>(
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

pub(super) fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

pub(super) fn android_connected_wifi(
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

pub(super) fn android_wifi_scan_result(
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

pub(super) fn normalize_android_ssid(ssid: &str) -> String {
    ssid.trim_matches('"').to_string()
}

pub(super) fn android_wifi_security(capabilities: &str) -> WifiSecurity {
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

pub(super) fn notification_id_to_i32(id: &NotificationId) -> i32 {
    let mut hash = 0x811c_9dc5u32;
    for byte in id.0.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash & 0x7fff_ffff) as i32
}

pub(super) fn clipboard_host_error(error: String) -> ClipboardError {
    ClipboardError::new("host_error", error)
}

pub(super) fn geolocation_host_error(error: String) -> GeolocationError {
    GeolocationError::new("host_error", error)
}

pub(super) fn haptic_host_error(error: String) -> HapticError {
    HapticError::new("host_error", error)
}

pub(super) fn microphone_host_error(error: String) -> MicrophoneError {
    MicrophoneError::new("host_error", error)
}

pub(super) fn camera_host_error(error: String) -> CameraError {
    CameraError::new("host_error", error)
}

pub(super) fn biometric_host_error(error: String) -> BiometricError {
    BiometricError::new("host_error", error)
}

pub(super) fn nfc_host_error(error: String) -> NfcError {
    NfcError::new("host_error", error)
}

pub(super) fn volume_host_error(error: String) -> VolumeError {
    VolumeError::new("host_error", error)
}

pub(super) fn notification_host_error(error: String) -> NotificationError {
    NotificationError::new("host_error", error)
}

pub(super) fn wifi_host_error(error: String) -> WifiError {
    WifiError::new("host_error", error)
}

pub(super) fn bluetooth_host_error(error: String) -> BluetoothError {
    BluetoothError::new("host_error", error)
}
