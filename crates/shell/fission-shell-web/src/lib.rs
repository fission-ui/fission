//! Browser/WebAssembly application shell built on Fission's shared winit
//! runtime and canvas renderer.
//!
//! [`WebApp`] owns browser mounting, history integration, input defaults, and
//! browser capability hosts while preserving the same state/reducer/widget
//! model as native graphical targets.

use anyhow::Result;
use fission_core::{Action, ActionRegistry, Env, GlobalState, Widget, WidgetId};
use fission_shell::async_host::AsyncRegistry;
use fission_shell_winit::WinitApp;

pub use fission_shell_winit::{
    BarcodeScannerHost, BiometricHost, BluetoothHost, BrowserDefaults, CameraHost, ClipboardHost,
    FrameDriverContext, FrameDriverResult, GeolocationHost, HapticHost, MemoryBarcodeScannerHost,
    MemoryBiometricHost, MemoryBluetoothHost, MemoryCameraHost, MemoryClipboardHost,
    MemoryGeolocationHost, MemoryHapticHost, MemoryMicrophoneHost, MemoryNfcHost,
    MemoryNotificationHost, MemoryPasskeyHost, MemoryVolumeHost, MemoryWifiHost, MicrophoneHost,
    NfcHost, NotificationHost, PasskeyHost, UnsupportedBarcodeScannerHost,
    UnsupportedBiometricHost, UnsupportedBluetoothHost, UnsupportedCameraHost,
    UnsupportedGeolocationHost, UnsupportedHapticHost, UnsupportedMicrophoneHost,
    UnsupportedNfcHost, UnsupportedNotificationHost, UnsupportedPasskeyHost, UnsupportedVolumeHost,
    UnsupportedWifiHost, VolumeHost, WebNavigationConfig, WebRouteStrategy, WifiHost,
};

/// Stateful Fission application mounted into a browser document.
///
/// `S` is the app's global state and `W` its retained root widget. Constructing
/// this value has no DOM side effects; [`Self::run`] performs the mount.
pub struct WebApp<S: GlobalState, W>
where
    W: Clone + Into<Widget>,
{
    inner: WinitApp<S, W>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct TestState;
    impl GlobalState for TestState {}

    #[test]
    fn common_stateful_builder_contract_is_available() {
        let _app = WebApp::<TestState, _>::new(fission_core::ui::Text::new("web"))
            .with_env(Env::default())
            .with_state_init(|_state| {})
            .with_key_handler(|_state, _key, _modifiers| false)
            .with_frame_driver(|_state, context: FrameDriverContext| {
                let _elapsed = context.elapsed;
                FrameDriverResult::new(false, false)
            })
            .with_sync_env(|_state, _env| {});
    }
}

impl<S, W> WebApp<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    /// Creates a Web app using `S::default()` as its global state.
    pub fn new(root_widget: W) -> Self {
        Self {
            inner: WinitApp::new(root_widget),
        }
    }

    /// Creates a Web app with an explicitly prepared global state.
    pub fn new_with_global_state(root_widget: W, global_state: S) -> Self {
        Self {
            inner: WinitApp::new_with_global_state(root_widget, global_state),
        }
    }

    /// Replaces the registered global state before the app starts.
    pub fn with_global_state(mut self, global_state: S) -> Self {
        self.inner = self.inner.with_global_state(global_state);
        self
    }

    /// Sets the stable identity from which implicit descendant IDs are derived.
    pub fn with_root_id(mut self, root_id: WidgetId) -> Self {
        self.inner = self.inner.with_root_id(root_id);
        self
    }

    /// Registers a host-level keyboard handler before normal widget routing.
    ///
    /// Return `true` when the handler consumes the key. Text entry, focus, and
    /// ordinary shortcuts should normally remain in widgets; this hook is for
    /// application-wide commands such as a command palette shortcut.
    pub fn with_key_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut S, &fission_core::KeyCode, u8) -> bool + Send + Sync + 'static,
    {
        self.inner = self.inner.with_key_handler(handler);
        self
    }

    /// Sets the browser document title used by the running app.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.inner = self.inner.with_title(title);
        self
    }

    /// Selects the existing DOM element that will contain Fission's canvas.
    /// The selector must resolve to exactly the intended app mount.
    pub fn with_mount_selector(mut self, selector: impl Into<String>) -> Self {
        self.inner = self.inner.with_mount_selector(selector);
        self
    }

    /// Selectively permits browser-owned defaults. Fission owns every browser
    /// input category unless it is explicitly included here.
    pub fn with_browser_defaults(mut self, defaults: BrowserDefaults) -> Self {
        self.inner = self.inner.with_browser_defaults(defaults);
        self
    }

    /// Concise alias for [`Self::with_mount_selector`].
    pub fn mount(self, selector: impl Into<String>) -> Self {
        self.with_mount_selector(selector)
    }

    /// Mutates initial global state synchronously before the first frame.
    pub fn with_state_init<F>(mut self, init: F) -> Self
    where
        F: FnOnce(&mut S),
    {
        self.inner = self.inner.with_state_init(init);
        self
    }

    /// Replaces the environment used to build the first and subsequent frames.
    ///
    /// Use this to install app-wide inputs that are not product state, such as
    /// translation bundles, an initial locale, or host presentation values.
    /// Values which change with [`GlobalState`] should be mirrored afterward
    /// with [`Self::with_sync_env`].
    pub fn with_env(mut self, env: Env) -> Self {
        self.inner = self.inner.with_env(env);
        self
    }

    /// Dispatches one typed action after the first widget registry is built.
    pub fn with_startup_action<A: Action>(mut self, action: A) -> Self {
        self.inner = self.inner.with_startup_action(action);
        self
    }

    /// Configures pathname/hash routing and the Web deployment base path.
    pub fn with_navigation(mut self, config: WebNavigationConfig) -> Self {
        self.inner = self.inner.with_web_navigation(config);
        self
    }

    /// Registers the persistent reducer that mirrors browser route changes
    /// into application state.
    pub fn with_route_handler(
        mut self,
        handler: fission_core::registry::Handler<S, fission_core::ShellRouteChanged>,
    ) -> Self {
        self.inner = self.inner.with_route_handler(handler);
        self
    }

    /// Installs a generated design system's theme and packaged fonts.
    pub fn with_design_system<D: fission_theme::DesignSystem>(
        mut self,
        mode: fission_theme::DesignMode,
    ) -> Self {
        self.inner = self.inner.with_design_system::<D>(mode);
        self
    }

    /// Registers packaged application font faces before the first frame.
    pub fn with_fonts(mut self, fonts: &'static [fission_theme::PackagedFont]) -> Self {
        self.inner = self.inner.with_fonts(fonts);
        self
    }

    /// Mirrors global-state presentation values into `Env` before each build.
    pub fn with_sync_env<F>(mut self, sync: F) -> Self
    where
        F: Fn(&S, &mut Env) + Send + Sync + 'static,
    {
        self.inner = self.inner.with_sync_env(sync);
        self
    }

    /// Runs a lightweight host hook between browser frames; return `true` to
    /// request a redraw.
    pub fn with_frame_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&mut S) -> bool + Send + Sync + 'static,
    {
        self.inner = self.inner.with_frame_hook(hook);
        self
    }

    /// Installs a host-side driver that advances external real-time state
    /// before each browser frame is built.
    ///
    /// The driver receives the elapsed frame duration in
    /// [`FrameDriverContext`]. Its [`FrameDriverResult`] independently reports
    /// whether the mutation changed the declarative application state and
    /// whether another frame is needed. This is appropriate for game,
    /// simulation, or media adapters whose high-frequency state should not be
    /// advanced through reducers.
    pub fn with_frame_driver<F>(mut self, driver: F) -> Self
    where
        F: Fn(&mut S, FrameDriverContext) -> FrameDriverResult + Send + Sync + 'static,
    {
        self.inner = self.inner.with_frame_driver(driver);
        self
    }

    /// Registers app jobs, services, and operation capability handlers with
    /// the browser-owned asynchronous host.
    pub fn with_async<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(&mut AsyncRegistry),
    {
        self.inner = self.inner.with_async(configure);
        self
    }

    #[cfg(feature = "store")]
    /// Replaces the key/value store provider used by Store effects.
    pub fn with_store_provider<P>(mut self, provider: P) -> Self
    where
        P: fission_store::StoreProvider + Send + Sync,
    {
        self.inner = self.inner.with_store_provider(provider);
        self
    }

    #[cfg(feature = "store-sql")]
    /// Replaces the SQL-capable store provider used by Store and SQL effects.
    pub fn with_sql_store_provider<P>(mut self, provider: P) -> Self
    where
        P: fission_store::SqlStoreProvider + Send + Sync,
    {
        self.inner = self.inner.with_sql_store_provider(provider);
        self
    }

    /// Installs the implementation used by notification effects.
    pub fn with_notification_host<H>(mut self, host: H) -> Self
    where
        H: NotificationHost,
    {
        self.inner = self.inner.with_notification_host(host);
        self
    }

    /// Installs the implementation used by NFC effects.
    pub fn with_nfc_host<H>(mut self, host: H) -> Self
    where
        H: NfcHost,
    {
        self.inner = self.inner.with_nfc_host(host);
        self
    }

    /// Installs the implementation used by biometric authentication effects.
    pub fn with_biometric_host<H>(mut self, host: H) -> Self
    where
        H: BiometricHost,
    {
        self.inner = self.inner.with_biometric_host(host);
        self
    }

    /// Installs the implementation used by passkey registration and sign-in.
    pub fn with_passkey_host<H>(mut self, host: H) -> Self
    where
        H: PasskeyHost,
    {
        self.inner = self.inner.with_passkey_host(host);
        self
    }

    /// Installs the implementation used by Bluetooth effects.
    pub fn with_bluetooth_host<H>(mut self, host: H) -> Self
    where
        H: BluetoothHost,
    {
        self.inner = self.inner.with_bluetooth_host(host);
        self
    }

    /// Installs the implementation used by barcode scanning effects.
    pub fn with_barcode_scanner_host<H>(mut self, host: H) -> Self
    where
        H: BarcodeScannerHost,
    {
        self.inner = self.inner.with_barcode_scanner_host(host);
        self
    }

    /// Installs the implementation used by camera effects.
    pub fn with_camera_host<H>(mut self, host: H) -> Self
    where
        H: CameraHost,
    {
        self.inner = self.inner.with_camera_host(host);
        self
    }

    /// Installs the implementation used by clipboard effects.
    pub fn with_clipboard_host<H>(mut self, host: H) -> Self
    where
        H: ClipboardHost,
    {
        self.inner = self.inner.with_clipboard_host(host);
        self
    }

    /// Installs the implementation used by geolocation effects.
    pub fn with_geolocation_host<H>(mut self, host: H) -> Self
    where
        H: GeolocationHost,
    {
        self.inner = self.inner.with_geolocation_host(host);
        self
    }

    /// Installs the implementation used by haptic feedback effects.
    pub fn with_haptic_host<H>(mut self, host: H) -> Self
    where
        H: HapticHost,
    {
        self.inner = self.inner.with_haptic_host(host);
        self
    }

    /// Installs the implementation used by microphone capture effects.
    pub fn with_microphone_host<H>(mut self, host: H) -> Self
    where
        H: MicrophoneHost,
    {
        self.inner = self.inner.with_microphone_host(host);
        self
    }

    /// Installs the implementation used by Wi-Fi effects.
    pub fn with_wifi_host<H>(mut self, host: H) -> Self
    where
        H: WifiHost,
    {
        self.inner = self.inner.with_wifi_host(host);
        self
    }

    /// Installs the implementation used by system/media volume effects.
    pub fn with_volume_host<H>(mut self, host: H) -> Self
    where
        H: VolumeHost,
    {
        self.inner = self.inner.with_volume_host(host);
        self
    }

    /// Sets the accepted schemes, domains, and path prefixes for inbound links.
    pub fn with_deep_link_config(mut self, config: fission_core::DeepLinkConfig) -> Self {
        self.inner = self.inner.with_deep_link_config(config);
        self
    }

    /// Adds an accepted custom URI scheme to the deep-link policy.
    pub fn with_deep_link_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.inner = self.inner.with_deep_link_scheme(scheme);
        self
    }

    /// Adds an accepted HTTP(S) host to the deep-link policy.
    pub fn with_deep_link_domain(mut self, domain: impl Into<String>) -> Self {
        self.inner = self.inner.with_deep_link_domain(domain);
        self
    }

    /// Queues an already-resolved inbound link for startup dispatch.
    pub fn with_startup_deep_link(mut self, link: fission_core::DeepLink) -> Self {
        self.inner = self.inner.with_startup_deep_link(link);
        self
    }

    /// Queues a notification interaction received before app startup.
    pub fn with_startup_notification_response(
        mut self,
        response: fission_core::NotificationResponse,
    ) -> Self {
        self.inner = self.inner.with_startup_notification_response(response);
        self
    }

    /// Registers a persistent typed reducer for accepted inbound deep links.
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

    /// Registers a persistent typed reducer for notification interactions.
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

    /// Merges a set of host-owned reducers into the persistent registry.
    pub fn absorb_registry(&mut self, registry: ActionRegistry<S>) {
        self.inner.absorb_registry(registry);
    }

    /// Registers one reducer in the shell's persistent action registry.
    ///
    /// Components normally register reducers declaratively while building.
    /// This lower-level hook is for host-owned actions that must remain
    /// available independently of the current widget tree.
    pub fn register_reducer(
        &mut self,
        action_id: fission_core::ActionId,
        reducer: fission_core::action::Reducer<S>,
    ) -> Result<()> {
        self.inner.register_reducer(action_id, reducer)
    }

    /// Mounts the app, starts the browser event loop, and renders until the
    /// page tears down the WebAssembly instance.
    pub fn run(self) -> Result<()> {
        self.inner.run()
    }
}
