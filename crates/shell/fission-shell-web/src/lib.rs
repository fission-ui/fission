use anyhow::Result;
use fission_core::{Action, ActionRegistry, Env, GlobalState, Widget};
use fission_shell::async_host::AsyncRegistry;
use fission_shell_winit::WinitApp;

pub use fission_shell_winit::{
    BarcodeScannerHost, BiometricHost, BluetoothHost, CameraHost, ClipboardHost, GeolocationHost,
    HapticHost, MemoryBarcodeScannerHost, MemoryBiometricHost, MemoryBluetoothHost,
    MemoryCameraHost, MemoryClipboardHost, MemoryGeolocationHost, MemoryHapticHost,
    MemoryMicrophoneHost, MemoryNfcHost, MemoryNotificationHost, MemoryPasskeyHost,
    MemoryVolumeHost, MemoryWifiHost, MicrophoneHost, NfcHost, NotificationHost, PasskeyHost,
    UnsupportedBarcodeScannerHost, UnsupportedBiometricHost, UnsupportedBluetoothHost,
    UnsupportedCameraHost, UnsupportedGeolocationHost, UnsupportedHapticHost,
    UnsupportedMicrophoneHost, UnsupportedNfcHost, UnsupportedNotificationHost,
    UnsupportedPasskeyHost, UnsupportedVolumeHost, UnsupportedWifiHost, VolumeHost, WifiHost,
};

pub struct WebApp<S: GlobalState, W>
where
    W: Clone + Into<Widget>,
{
    inner: WinitApp<S, W>,
}

impl<S, W> WebApp<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    pub fn new(root_widget: W) -> Self {
        Self {
            inner: WinitApp::new(root_widget),
        }
    }

    pub fn new_with_global_state(root_widget: W, global_state: S) -> Self {
        Self {
            inner: WinitApp::new_with_global_state(root_widget, global_state),
        }
    }

    pub fn with_global_state(mut self, global_state: S) -> Self {
        self.inner = self.inner.with_global_state(global_state);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.inner = self.inner.with_title(title);
        self
    }

    pub fn with_mount_selector(mut self, selector: impl Into<String>) -> Self {
        self.inner = self.inner.with_mount_selector(selector);
        self
    }

    pub fn mount(self, selector: impl Into<String>) -> Self {
        self.with_mount_selector(selector)
    }

    pub fn with_state_init<F>(mut self, init: F) -> Self
    where
        F: FnOnce(&mut S),
    {
        self.inner = self.inner.with_state_init(init);
        self
    }

    pub fn with_startup_action<A: Action>(mut self, action: A) -> Self {
        self.inner = self.inner.with_startup_action(action);
        self
    }

    pub fn with_design_system<D: fission_theme::DesignSystem>(
        mut self,
        mode: fission_theme::DesignMode,
    ) -> Self {
        self.inner = self.inner.with_design_system::<D>(mode);
        self
    }

    pub fn with_sync_env<F>(mut self, sync: F) -> Self
    where
        F: Fn(&S, &mut Env) + Send + Sync + 'static,
    {
        self.inner = self.inner.with_sync_env(sync);
        self
    }

    pub fn with_frame_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&mut S) -> bool + Send + Sync + 'static,
    {
        self.inner = self.inner.with_frame_hook(hook);
        self
    }

    pub fn with_async<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(&mut AsyncRegistry),
    {
        self.inner = self.inner.with_async(configure);
        self
    }

    pub fn with_notification_host<H>(mut self, host: H) -> Self
    where
        H: NotificationHost,
    {
        self.inner = self.inner.with_notification_host(host);
        self
    }

    pub fn with_nfc_host<H>(mut self, host: H) -> Self
    where
        H: NfcHost,
    {
        self.inner = self.inner.with_nfc_host(host);
        self
    }

    pub fn with_biometric_host<H>(mut self, host: H) -> Self
    where
        H: BiometricHost,
    {
        self.inner = self.inner.with_biometric_host(host);
        self
    }

    pub fn with_passkey_host<H>(mut self, host: H) -> Self
    where
        H: PasskeyHost,
    {
        self.inner = self.inner.with_passkey_host(host);
        self
    }

    pub fn with_bluetooth_host<H>(mut self, host: H) -> Self
    where
        H: BluetoothHost,
    {
        self.inner = self.inner.with_bluetooth_host(host);
        self
    }

    pub fn with_barcode_scanner_host<H>(mut self, host: H) -> Self
    where
        H: BarcodeScannerHost,
    {
        self.inner = self.inner.with_barcode_scanner_host(host);
        self
    }

    pub fn with_camera_host<H>(mut self, host: H) -> Self
    where
        H: CameraHost,
    {
        self.inner = self.inner.with_camera_host(host);
        self
    }

    pub fn with_clipboard_host<H>(mut self, host: H) -> Self
    where
        H: ClipboardHost,
    {
        self.inner = self.inner.with_clipboard_host(host);
        self
    }

    pub fn with_geolocation_host<H>(mut self, host: H) -> Self
    where
        H: GeolocationHost,
    {
        self.inner = self.inner.with_geolocation_host(host);
        self
    }

    pub fn with_haptic_host<H>(mut self, host: H) -> Self
    where
        H: HapticHost,
    {
        self.inner = self.inner.with_haptic_host(host);
        self
    }

    pub fn with_microphone_host<H>(mut self, host: H) -> Self
    where
        H: MicrophoneHost,
    {
        self.inner = self.inner.with_microphone_host(host);
        self
    }

    pub fn with_wifi_host<H>(mut self, host: H) -> Self
    where
        H: WifiHost,
    {
        self.inner = self.inner.with_wifi_host(host);
        self
    }

    pub fn with_volume_host<H>(mut self, host: H) -> Self
    where
        H: VolumeHost,
    {
        self.inner = self.inner.with_volume_host(host);
        self
    }

    pub fn with_deep_link_config(mut self, config: fission_core::DeepLinkConfig) -> Self {
        self.inner = self.inner.with_deep_link_config(config);
        self
    }

    pub fn with_deep_link_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.inner = self.inner.with_deep_link_scheme(scheme);
        self
    }

    pub fn with_deep_link_domain(mut self, domain: impl Into<String>) -> Self {
        self.inner = self.inner.with_deep_link_domain(domain);
        self
    }

    pub fn with_startup_deep_link(mut self, link: fission_core::DeepLink) -> Self {
        self.inner = self.inner.with_startup_deep_link(link);
        self
    }

    pub fn with_startup_notification_response(
        mut self,
        response: fission_core::NotificationResponse,
    ) -> Self {
        self.inner = self.inner.with_startup_notification_response(response);
        self
    }

    pub fn on_deep_link<H>(mut self, handler: H) -> Self
    where
        H: fission_core::registry::IntoHandler<S, fission_core::DeepLinkReceived>
            + Send
            + Sync
            + 'static,
    {
        self.inner = self.inner.on_deep_link(handler);
        self
    }

    pub fn on_notification_response<H>(mut self, handler: H) -> Self
    where
        H: fission_core::registry::IntoHandler<S, fission_core::NotificationResponseReceived>
            + Send
            + Sync
            + 'static,
    {
        self.inner = self.inner.on_notification_response(handler);
        self
    }

    pub fn absorb_registry(&mut self, registry: ActionRegistry<S>) {
        self.inner.absorb_registry(registry);
    }

    pub fn run(self) -> Result<()> {
        self.inner.run()
    }
}
