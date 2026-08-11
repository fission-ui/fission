use super::*;

pub(super) fn render_android_manifest(project: &FissionProject) -> String {
    let capability_entries = render_android_capability_manifest_entries(project);
    let native_application_entries = render_android_native_application_entries(project);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="{app_id}">

    <uses-permission android:name="android.permission.INTERNET" />
{capability_entries}

    <uses-sdk
        android:minSdkVersion="24"
        android:targetSdkVersion="35" />

    <application
        android:extractNativeLibs="true"
        android:hasCode="true"
        android:icon="@drawable/app_icon"
        android:label="{label}">
{native_application_entries}
        <activity
            android:name="rs.fission.runtime.FissionActivity"
            android:configChanges="orientation|keyboardHidden|screenSize|screenLayout|smallestScreenSize|uiMode|density"
            android:exported="true"
            android:launchMode="singleTask"
            android:theme="@style/FissionLaunchTheme">
            <meta-data
                android:name="android.app.lib_name"
                android:value="{lib_name}" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>

</manifest>
"#,
        app_id = project.app.app_id,
        label = ios_bundle_name(project),
        lib_name = android_library_name(project),
        capability_entries = capability_entries,
        native_application_entries = native_application_entries,
    )
}

pub(super) fn render_android_native_application_entries(project: &FissionProject) -> String {
    let mut out = String::new();
    for module in &project.native.modules {
        for entry in &module.android.manifest_application_entries {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            out.push_str("        ");
            out.push_str(entry);
            if !entry.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

pub(super) fn render_android_capability_manifest_entries(project: &FissionProject) -> String {
    let mut out = String::new();
    if project.capabilities.contains(&PlatformCapability::Nfc) {
        out.push_str(&render_android_nfc_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Notifications)
    {
        out.push_str(&render_android_notifications_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Biometric)
    {
        out.push_str(&render_android_biometric_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Bluetooth)
    {
        out.push_str(&render_android_bluetooth_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Camera) {
        out.push_str(&render_android_camera_manifest_entries());
    } else if project
        .capabilities
        .contains(&PlatformCapability::BarcodeScanner)
    {
        out.push_str(&render_android_barcode_camera_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Geolocation)
    {
        out.push_str(&render_android_geolocation_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Haptics) {
        out.push_str(&render_android_haptics_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::Microphone)
    {
        out.push_str(&render_android_microphone_manifest_entries());
    }
    if project
        .capabilities
        .contains(&PlatformCapability::VolumeControl)
    {
        out.push_str(&render_android_volume_manifest_entries());
    }
    if project.capabilities.contains(&PlatformCapability::Wifi) {
        out.push_str(&render_android_wifi_manifest_entries());
    }
    for permission in android_native_module_permissions(project) {
        out.push_str(&format!(
            "    <uses-permission android:name=\"{}\" />\n",
            permission
        ));
    }
    out
}

pub(super) fn android_native_module_permissions(project: &FissionProject) -> BTreeSet<String> {
    project
        .native
        .modules
        .iter()
        .flat_map(|module| module.android.permissions.iter())
        .map(|permission| permission.trim().to_string())
        .filter(|permission| !permission.is_empty())
        .collect()
}

pub(super) fn render_android_nfc_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.NFC\" />\n");
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.nfc\" android:required=\"false\" />\n",
    );
    out
}

pub(super) fn render_android_notifications_manifest_entries() -> String {
    "    <uses-permission android:name=\"android.permission.POST_NOTIFICATIONS\" />\n".to_string()
}

pub(super) fn render_android_biometric_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.USE_BIOMETRIC\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.USE_FINGERPRINT\" android:maxSdkVersion=\"28\" />\n");
    out
}

pub(super) fn render_android_bluetooth_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH\" android:maxSdkVersion=\"30\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_ADMIN\" android:maxSdkVersion=\"30\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_SCAN\" android:usesPermissionFlags=\"neverForLocation\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_CONNECT\" />\n");
    out.push_str(
        "    <uses-permission android:name=\"android.permission.BLUETOOTH_ADVERTISE\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.bluetooth\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.bluetooth_le\" android:required=\"false\" />\n",
    );
    out
}

pub(super) fn render_missing_android_bluetooth_manifest_entries(existing: &str) -> String {
    let mut out = String::new();
    if !existing.contains("android.permission.BLUETOOTH\"") {
        out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH\" android:maxSdkVersion=\"30\" />\n");
    }
    if !existing.contains("android.permission.BLUETOOTH_ADMIN") {
        out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_ADMIN\" android:maxSdkVersion=\"30\" />\n");
    }
    if !existing.contains("android.permission.BLUETOOTH_SCAN") {
        out.push_str("    <uses-permission android:name=\"android.permission.BLUETOOTH_SCAN\" android:usesPermissionFlags=\"neverForLocation\" />\n");
    }
    if !existing.contains("android.permission.BLUETOOTH_CONNECT") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.BLUETOOTH_CONNECT\" />\n",
        );
    }
    if !existing.contains("android.permission.BLUETOOTH_ADVERTISE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.BLUETOOTH_ADVERTISE\" />\n",
        );
    }
    if !existing.contains("android.hardware.bluetooth\"") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.bluetooth\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.bluetooth_le") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.bluetooth_le\" android:required=\"false\" />\n",
        );
    }
    out
}

pub(super) fn render_android_barcode_camera_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.CAMERA\" />\n");
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera.any\" android:required=\"false\" />\n",
    );
    out
}

pub(super) fn render_android_camera_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.CAMERA\" />\n");
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera.any\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera.front\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.camera.flash\" android:required=\"false\" />\n",
    );
    out
}

pub(super) fn render_missing_android_camera_manifest_entries(existing: &str) -> String {
    let mut out = String::new();
    if !existing.contains("android.permission.CAMERA") {
        out.push_str("    <uses-permission android:name=\"android.permission.CAMERA\" />\n");
    }
    if !existing.contains("android.hardware.camera.any") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.camera.any\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.camera\"") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.camera\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.camera.front") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.camera.front\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.camera.flash") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.camera.flash\" android:required=\"false\" />\n",
        );
    }
    out
}

pub(super) fn render_android_geolocation_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str(
        "    <uses-permission android:name=\"android.permission.ACCESS_COARSE_LOCATION\" />\n",
    );
    out.push_str(
        "    <uses-permission android:name=\"android.permission.ACCESS_FINE_LOCATION\" />\n",
    );
    out
}

pub(super) fn render_android_haptics_manifest_entries() -> String {
    "    <uses-permission android:name=\"android.permission.VIBRATE\" />\n".to_string()
}

pub(super) fn render_android_microphone_manifest_entries() -> String {
    "    <uses-permission android:name=\"android.permission.RECORD_AUDIO\" />\n".to_string()
}

pub(super) fn render_android_volume_manifest_entries() -> String {
    "    <uses-permission android:name=\"android.permission.MODIFY_AUDIO_SETTINGS\" />\n"
        .to_string()
}

pub(super) fn render_android_wifi_manifest_entries() -> String {
    let mut out = String::new();
    out.push_str("    <uses-permission android:name=\"android.permission.ACCESS_WIFI_STATE\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.CHANGE_WIFI_STATE\" />\n");
    out.push_str(
        "    <uses-permission android:name=\"android.permission.ACCESS_NETWORK_STATE\" />\n",
    );
    out.push_str(
        "    <uses-permission android:name=\"android.permission.CHANGE_NETWORK_STATE\" />\n",
    );
    out.push_str("    <uses-permission android:name=\"android.permission.NEARBY_WIFI_DEVICES\" android:usesPermissionFlags=\"neverForLocation\" />\n");
    out.push_str("    <uses-permission android:name=\"android.permission.ACCESS_FINE_LOCATION\" android:maxSdkVersion=\"32\" />\n");
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.wifi\" android:required=\"false\" />\n",
    );
    out.push_str(
        "    <uses-feature android:name=\"android.hardware.wifi.direct\" android:required=\"false\" />\n",
    );
    out
}

pub(super) fn render_missing_android_wifi_manifest_entries(existing: &str) -> String {
    let mut out = String::new();
    if !existing.contains("android.permission.ACCESS_WIFI_STATE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.ACCESS_WIFI_STATE\" />\n",
        );
    }
    if !existing.contains("android.permission.CHANGE_WIFI_STATE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.CHANGE_WIFI_STATE\" />\n",
        );
    }
    if !existing.contains("android.permission.ACCESS_NETWORK_STATE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.ACCESS_NETWORK_STATE\" />\n",
        );
    }
    if !existing.contains("android.permission.CHANGE_NETWORK_STATE") {
        out.push_str(
            "    <uses-permission android:name=\"android.permission.CHANGE_NETWORK_STATE\" />\n",
        );
    }
    if !existing.contains("android.permission.NEARBY_WIFI_DEVICES") {
        out.push_str("    <uses-permission android:name=\"android.permission.NEARBY_WIFI_DEVICES\" android:usesPermissionFlags=\"neverForLocation\" />\n");
    }
    if !existing.contains("android.permission.ACCESS_FINE_LOCATION") {
        out.push_str("    <uses-permission android:name=\"android.permission.ACCESS_FINE_LOCATION\" android:maxSdkVersion=\"32\" />\n");
    }
    if !existing.contains("android.hardware.wifi\"") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.wifi\" android:required=\"false\" />\n",
        );
    }
    if !existing.contains("android.hardware.wifi.direct") {
        out.push_str(
            "    <uses-feature android:name=\"android.hardware.wifi.direct\" android:required=\"false\" />\n",
        );
    }
    out
}
