use super::{char_sequence_to_string, java_string};
use fission_core::{WifiNetwork, WifiSecurity};
use jni::{errors::Result as JniResult, objects::JObject, JNIEnv};

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
