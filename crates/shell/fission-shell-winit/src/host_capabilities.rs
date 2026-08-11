use super::*;

pub(super) type EffectResult = AsyncMessage;

pub(super) type ServiceKey = (String, String);
pub(super) type ServiceBindingKey = (String, String, u64);

pub(super) struct ActiveServiceHandle {
    pub(super) runtime: RunningServiceHandle,
}

#[cfg(any(test, target_os = "windows"))]
pub(super) fn windows_wide(value: &str) -> Result<Vec<u16>, String> {
    if value.encode_utf16().any(|unit| unit == 0) {
        return Err("Windows shell arguments cannot contain NUL characters".into());
    }

    Ok(value.encode_utf16().chain(std::iter::once(0)).collect())
}

#[cfg(any(test, target_os = "windows"))]
pub(super) fn windows_shell_execute_succeeded(status: usize) -> bool {
    status > 32
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
pub(super) fn open_host_url(url: &str, _in_app: bool) -> Result<(), String> {
    use ::windows::{
        core::PCWSTR,
        Win32::{
            Foundation::HWND,
            UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        },
    };

    let operation = windows_wide("open")?;
    let target = windows_wide(url)?;
    // SAFETY: `operation` and `target` remain alive for the duration of the call
    // and are explicitly NUL-terminated. The optional parameters are null.
    let outcome = unsafe {
        ShellExecuteW(
            HWND::default(),
            PCWSTR(operation.as_ptr()),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let status = outcome.0 as usize;
    if windows_shell_execute_succeeded(status) {
        Ok(())
    } else {
        Err(format!(
            "Windows could not open the requested URL (ShellExecuteW status {status})"
        ))
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
pub(super) fn open_host_url(url: &str, _in_app: bool) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn open_host_url(url: &str, in_app: bool) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "browser window is not available".to_string())?;
    if in_app {
        window.location().set_href(url).map_err(js_error_to_string)
    } else {
        window
            .open_with_url_and_target(url, "_blank")
            .map_err(js_error_to_string)?
            .ok_or_else(|| format!("browser blocked opening url `{url}`"))?;
        Ok(())
    }
}

pub(super) fn register_builtin_operation_capabilities(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        OPEN_URL,
        |request: OpenUrlRequest, _| async move {
            open_host_url(&request.url, request.in_app)?;
            Ok(())
        },
    );
    #[cfg(target_arch = "wasm32")]
    {
        web_capabilities::register_web_operation_capabilities(async_registry);
        register_unsupported_file_picker_capability(async_registry);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        file_picker::register_file_picker_capability(async_registry);
        #[cfg(any(target_os = "android", target_os = "ios"))]
        register_unsupported_file_picker_capability(async_registry);

        notifications::register_notification_capabilities(
            async_registry,
            Arc::new(notifications::native_notification_host()),
        );
        nfc::register_nfc_capabilities(async_registry, Arc::new(UnsupportedNfcHost));
        biometric::register_biometric_capabilities(
            async_registry,
            Arc::new(UnsupportedBiometricHost),
        );
        passkey::register_passkey_capabilities(async_registry, Arc::new(UnsupportedPasskeyHost));
        bluetooth::register_bluetooth_capabilities(
            async_registry,
            Arc::new(UnsupportedBluetoothHost),
        );
        barcode::register_barcode_scanner_capabilities(
            async_registry,
            Arc::new(UnsupportedBarcodeScannerHost),
        );
        camera::register_camera_capabilities(async_registry, Arc::new(UnsupportedCameraHost));
        clipboard::register_clipboard_capabilities(
            async_registry,
            Arc::new(DesktopClipboard::new()),
        );
        geolocation::register_geolocation_capabilities(
            async_registry,
            Arc::new(UnsupportedGeolocationHost),
        );
        haptics::register_haptic_capabilities(
            async_registry,
            Arc::new(haptics::native_haptic_host()),
        );
        microphone::register_microphone_capabilities(
            async_registry,
            Arc::new(UnsupportedMicrophoneHost),
        );
        wifi::register_wifi_capabilities(async_registry, Arc::new(UnsupportedWifiHost));
        volume::register_volume_capabilities(
            async_registry,
            Arc::new(volume::native_volume_host()),
        );
        #[cfg(target_os = "macos")]
        macos_capabilities::register_macos_operation_capabilities(async_registry);
        #[cfg(target_os = "ios")]
        ios_capabilities::register_ios_operation_capabilities(async_registry);
    }
}

#[cfg(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))]
pub(super) fn register_unsupported_file_picker_capability(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        fission_core::PICK_OPEN_FILES,
        |_request: fission_core::PickOpenFilesRequest, _| async move {
            Err::<fission_core::PickOpenFilesResult, _>(
                fission_core::PickOpenFilesError::unsupported("pick_open_files"),
            )
        },
    );
}

pub(super) fn collect_startup_deep_links(config: &DeepLinkConfig) -> Vec<DeepLink> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut env_values = Vec::new();
    if let Ok(value) = std::env::var("FISSION_DEEP_LINK_URL") {
        env_values.push(value);
    }
    if let Ok(value) = std::env::var("FISSION_DEEP_LINKS") {
        env_values.extend(
            value
                .split('\n')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
        );
    }

    #[cfg(target_arch = "wasm32")]
    if let Some(window) = web_sys::window() {
        if let Ok(href) = window.location().href() {
            env_values.push(href);
        }
    }

    collect_startup_deep_links_from(config, args, env_values)
}

pub(super) fn collect_startup_deep_links_from(
    config: &DeepLinkConfig,
    args: impl IntoIterator<Item = String>,
    env_values: impl IntoIterator<Item = String>,
) -> Vec<DeepLink> {
    let mut links = Vec::new();
    for url in env_values.into_iter().chain(args) {
        if config.matches(&url) {
            links.push(
                DeepLink::new(url.clone())
                    .cold_start(true)
                    .source(config.source_for(&url)),
            );
        }
    }
    links
}

#[cfg(target_arch = "wasm32")]
pub(super) fn js_error_to_string(error: wasm_bindgen::JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {error:?}"))
}
