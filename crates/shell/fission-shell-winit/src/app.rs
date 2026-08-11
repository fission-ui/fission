use super::*;

pub type KeyHandler<S> = Arc<dyn Fn(&mut S, &fission_core::KeyCode, u8) -> bool + Send + Sync>;
pub type FrameHook<S> = Arc<dyn Fn(&mut S) -> bool + Send + Sync>;

pub struct WinitApp<S: GlobalState, W>
where
    W: Clone + Into<Widget>,
{
    pub(super) runtime: Runtime,
    pub(super) layout_engine: LayoutEngine,
    pub(super) root_widget: W,
    pub(super) env: Env,
    pub(super) pipeline: Pipeline,
    pub(super) measurer: Arc<VelloTextMeasurer>,
    pub(super) sync_env: Option<Arc<dyn Fn(&S, &mut Env) + Send + Sync>>,
    pub(super) key_handler: Option<KeyHandler<S>>,
    pub(super) frame_hook: Option<FrameHook<S>>,
    pub(super) native_surface_handlers: NativeSurfaceRegistry,
    pub(super) title: String,
    pub(super) initial_maximized: bool,
    pub(super) web_mount_selector: Option<String>,
    pub(super) test_control_port: Option<u16>,
    /// Channel pair for receiving completed background effect results.
    pub(super) effect_result_tx: mpsc::Sender<AsyncMessage>,
    pub(super) effect_result_rx: mpsc::Receiver<AsyncMessage>,
    pub(super) async_registry: AsyncRegistry,
    pub(super) startup_action: Option<ActionEnvelope>,
    #[cfg(feature = "tray")]
    pub(super) tray_config: Option<tray::TrayConfig<S>>,
    pub(super) deep_link_config: DeepLinkConfig,
    pub(super) startup_deep_links: Vec<DeepLink>,
    pub(super) startup_notification_responses: Vec<NotificationResponse>,
    pub(super) _phantom: std::marker::PhantomData<S>,
}

impl<S, W> WinitApp<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    pub fn new(root_widget: W) -> Self {
        Self::new_with_global_state(root_widget, S::default())
    }

    pub fn new_with_global_state(root_widget: W, global_state: S) -> Self {
        let mut runtime = Runtime::default();
        runtime.add_global_state(Box::new(global_state)).unwrap();

        const DEFAULT_FONT_FAMILY: &str = "Fission Default";
        let font_cx = Arc::new(Mutex::new(build_font_context()));
        {
            let mut font_cx = font_cx.lock().unwrap();
            let font_data = fonts::default_font_bytes().to_vec();
            let info_override = FontInfoOverride {
                family_name: Some(DEFAULT_FONT_FAMILY),
                ..Default::default()
            };
            font_cx
                .collection
                .register_fonts(Blob::from(font_data), Some(info_override));
        }
        let measurer = Arc::new(VelloTextMeasurer::new_with_default_family(
            font_cx.clone(),
            DEFAULT_FONT_FAMILY,
        ));
        let env = Env::new(measurer.clone() as Arc<dyn fission_layout::TextMeasurer>);
        let clipboard: Arc<dyn fission_core::env::Clipboard> = Arc::new(DesktopClipboard::new());

        let layout_engine = LayoutEngine::new().with_measurer(measurer.clone());
        let runtime = runtime
            .with_measurer(measurer.clone())
            .with_clipboard(clipboard);

        let (effect_result_tx, effect_result_rx) = mpsc::channel();
        let mut async_registry = AsyncRegistry::new();
        register_builtin_operation_capabilities(&mut async_registry);

        Self {
            runtime,
            layout_engine,
            root_widget,
            env,
            pipeline: Pipeline::new(),
            measurer,
            sync_env: None,
            key_handler: None,
            frame_hook: None,
            native_surface_handlers: NativeSurfaceRegistry::default(),
            title: "Fission".into(),
            initial_maximized: false,
            web_mount_selector: None,
            test_control_port: None,
            effect_result_tx,
            effect_result_rx,
            async_registry,
            startup_action: None,
            #[cfg(feature = "tray")]
            tray_config: None,
            deep_link_config: DeepLinkConfig::default(),
            startup_deep_links: Vec::new(),
            startup_notification_responses: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_global_state(mut self, global_state: S) -> Self {
        *self.runtime.get_global_state_mut::<S>().expect(
            "Fission global state must be registered before WinitApp::with_global_state is called",
        ) = global_state;
        self
    }

    pub fn with_key_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut S, &fission_core::KeyCode, u8) -> bool + Send + Sync + 'static,
    {
        self.key_handler = Some(Arc::new(handler));
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self.env.window.title = fission_core::WindowTitle::plain(self.title.clone());
        self
    }

    /// Requests that the native window start maximized.
    ///
    /// This is opt-in and defaults to `false`. The desktop window manager may
    /// choose whether to honor the request.
    pub fn with_initial_maximized(mut self, maximized: bool) -> Self {
        self.initial_maximized = maximized;
        self
    }

    pub fn with_test_control_port(mut self, port: u16) -> Self {
        self.test_control_port = Some(port);
        self
    }

    pub fn with_mount_selector(mut self, selector: impl Into<String>) -> Self {
        self.web_mount_selector = Some(selector.into());
        self
    }

    /// Mutate the initial application state before the first frame.
    pub fn with_state_init<F>(mut self, init: F) -> Self
    where
        F: FnOnce(&mut S),
    {
        if let Some(state) = self.runtime.get_global_state_mut::<S>() {
            init(state);
        }
        self
    }

    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    pub fn with_design_system<D: fission_theme::DesignSystem>(
        mut self,
        mode: fission_theme::DesignMode,
    ) -> Self {
        register_packaged_fonts(&self.measurer.font_cx(), D::font_faces());
        self.env.theme = D::theme(mode);
        self
    }

    /// Registers packaged application font faces with both text measurement
    /// and rendering before the first frame.
    pub fn with_fonts(self, fonts: &'static [fission_theme::PackagedFont]) -> Self {
        register_packaged_fonts(&self.measurer.font_cx(), fonts);
        self
    }

    pub fn with_sync_env<F>(mut self, f: F) -> Self
    where
        F: Fn(&S, &mut Env) + Send + Sync + 'static,
    {
        self.sync_env = Some(Arc::new(f));
        self
    }

    /// Register a hook that runs on every `AboutToWait` event with mutable
    /// access to the application state.  Return `true` to request a redraw.
    /// Useful for polling background services (e.g. LSP) between key events.
    pub fn with_frame_hook<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut S) -> bool + Send + Sync + 'static,
    {
        self.frame_hook = Some(Arc::new(f));
        self
    }

    /// Registers an extension that presents opaque `EmbedKind::Custom`
    /// surfaces in this native host.
    pub fn with_native_surface_handler<H>(mut self, handler: H) -> Self
    where
        H: NativeSurfaceHandler + 'static,
    {
        self.native_surface_handlers.register(handler);
        self
    }

    pub fn with_async<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(&mut AsyncRegistry),
    {
        configure(&mut self.async_registry);
        self
    }

    /// Registers the host implementation used for notification effects.
    ///
    /// `host` receives requests emitted by `ctx.effects.notifications()`. Use
    /// this to install a real OS/browser notification provider in a shell, or a
    /// deterministic memory provider in tests.
    pub fn with_notification_host<H>(mut self, host: H) -> Self
    where
        H: NotificationHost,
    {
        notifications::register_notification_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Configures the explicit AppUserModelID used by unpackaged Windows apps.
    ///
    /// Packaged Windows apps continue to use their package identity. For an
    /// ordinary desktop installer, this value must also be assigned to the
    /// app's Start Menu shortcut as `System.AppUserModel.ID`. Windows limits
    /// AppUserModelIDs to 128 characters and does not allow spaces.
    ///
    /// This setting has no effect on non-Windows targets.
    pub fn with_windows_app_user_model_id(self, app_user_model_id: impl Into<String>) -> Self {
        #[cfg(target_os = "windows")]
        {
            let mut app = self;
            notifications::register_notification_capabilities(
                &mut app.async_registry,
                Arc::new(
                    notifications::native_notification_host_with_windows_app_user_model_id(
                        app_user_model_id,
                    ),
                ),
            );
            app
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = app_user_model_id;
            self
        }
    }

    /// Registers the host implementation used for NFC effects.
    ///
    /// `host` owns scanning, writing, emulation, and cancellation. Install a
    /// provider only for targets or attached reader hardware that can satisfy the
    /// NFC contract.
    pub fn with_nfc_host<H>(mut self, host: H) -> Self
    where
        H: NfcHost,
    {
        nfc::register_nfc_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for biometric authentication effects.
    ///
    /// `host` should map Fission requests to the platform local-authentication
    /// system and return typed errors for missing enrollment, cancellation, or
    /// unsupported hardware.
    pub fn with_biometric_host<H>(mut self, host: H) -> Self
    where
        H: BiometricHost,
    {
        biometric::register_biometric_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for passkey/WebAuthn effects.
    ///
    /// `host` should map Fission registration and authentication requests to
    /// the platform credential APIs and return WebAuthn data for server-side
    /// verification. It should not treat local biometric unlock as proof of
    /// identity without server verification.
    pub fn with_passkey_host<H>(mut self, host: H) -> Self
    where
        H: PasskeyHost,
    {
        passkey::register_passkey_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for Bluetooth effects.
    ///
    /// `host` owns adapter state, permission, scanning, connecting, reads, writes,
    /// and advertising. Use this boundary to keep platform Bluetooth APIs out of
    /// shared app reducers.
    pub fn with_bluetooth_host<H>(mut self, host: H) -> Self
    where
        H: BluetoothHost,
    {
        bluetooth::register_bluetooth_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for barcode scanner effects.
    ///
    /// `host` may run live camera scanning, decode supplied image streams, or both.
    /// Reducers should rely on this provider instead of depending on a specific
    /// camera or decoder library.
    pub fn with_barcode_scanner_host<H>(mut self, host: H) -> Self
    where
        H: BarcodeScannerHost,
    {
        barcode::register_barcode_scanner_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for camera and flashlight effects.
    ///
    /// `host` owns camera availability, permission, photo capture, torch control,
    /// and cancellation. Use memory hosts for tests and real OS providers for
    /// production shells.
    pub fn with_camera_host<H>(mut self, host: H) -> Self
    where
        H: CameraHost,
    {
        camera::register_camera_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for clipboard effects.
    ///
    /// `host` owns text and typed clipboard access. This is useful for tests,
    /// custom shells, or platforms where clipboard behavior differs from the
    /// default desktop provider.
    pub fn with_clipboard_host<H>(mut self, host: H) -> Self
    where
        H: ClipboardHost,
    {
        clipboard::register_clipboard_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for geolocation effects.
    ///
    /// `host` owns permission checks and current-position requests. It should map
    /// Fission accuracy and cache controls to the platform location service where
    /// available.
    pub fn with_geolocation_host<H>(mut self, host: H) -> Self
    where
        H: GeolocationHost,
    {
        geolocation::register_geolocation_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for haptic feedback effects.
    ///
    /// `host` owns impact, notification, selection, and pattern playback. It
    /// should return unsupported errors on devices without tactile hardware.
    pub fn with_haptic_host<H>(mut self, host: H) -> Self
    where
        H: HapticHost,
    {
        haptics::register_haptic_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for microphone effects.
    ///
    /// `host` owns input-device availability, permission, bounded recording, and
    /// cancellation. Keep recording code behind this provider boundary.
    pub fn with_microphone_host<H>(mut self, host: H) -> Self
    where
        H: MicrophoneHost,
    {
        microphone::register_microphone_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for Wi-Fi effects.
    ///
    /// `host` owns adapter availability, permission, scanning, connection, and
    /// disconnection. Platform Wi-Fi APIs are permission-sensitive, so unsupported
    /// and denied states should be reported explicitly.
    pub fn with_wifi_host<H>(mut self, host: H) -> Self
    where
        H: WifiHost,
    {
        wifi::register_wifi_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for volume-control effects.
    ///
    /// `host` maps Fission volume streams to the platform mixer or media control
    /// model. It should return unsupported errors when the target cannot expose
    /// system volume control to apps.
    pub fn with_volume_host<H>(mut self, host: H) -> Self
    where
        H: VolumeHost,
    {
        volume::register_volume_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    pub fn with_startup_action<A: Action>(mut self, action: A) -> Self {
        self.startup_action = Some(action.into());
        self
    }

    #[cfg(feature = "tray")]
    pub fn with_tray(mut self, config: tray::TrayConfig<S>) -> Self {
        self.tray_config = Some(config);
        self
    }

    /// Installs the deep-link filter used by this shell.
    ///
    /// `config` declares accepted schemes, domains, and path prefixes. The shell
    /// uses it to classify inbound links before dispatching `DeepLinkReceived`
    /// actions into the app.
    pub fn with_deep_link_config(mut self, config: DeepLinkConfig) -> Self {
        self.deep_link_config = config;
        self
    }

    /// Adds one accepted custom deep-link scheme.
    ///
    /// `scheme` is normalized by `DeepLinkConfig`. Use this for app-specific
    /// routes such as `myapp://item/123`.
    pub fn with_deep_link_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.deep_link_config = self.deep_link_config.scheme(scheme);
        self
    }

    /// Adds one accepted HTTP or HTTPS deep-link domain.
    ///
    /// `domain` is normalized by `DeepLinkConfig`. Use this for verified app
    /// links, universal links, or web URLs that should enter the app.
    pub fn with_deep_link_domain(mut self, domain: impl Into<String>) -> Self {
        self.deep_link_config = self.deep_link_config.domain(domain);
        self
    }

    /// Queues a deep link to dispatch after the app starts.
    ///
    /// Use this from host startup code when the platform launched the app because
    /// of an external URL. The link is delivered through the normal action path.
    pub fn with_startup_deep_link(mut self, link: DeepLink) -> Self {
        self.startup_deep_links.push(link);
        self
    }

    /// Queues a notification response to dispatch after the app starts.
    ///
    /// Use this when a notification action or tap launched the app. The response
    /// is delivered as `NotificationResponseReceived` through the normal reducer
    /// path.
    pub fn with_startup_notification_response(mut self, response: NotificationResponse) -> Self {
        self.startup_notification_responses.push(response);
        self
    }

    /// Registers a reducer handler for inbound deep links.
    ///
    /// `handler` receives `DeepLinkReceived` actions from startup links and
    /// runtime host events. Use it to update routing state rather than parsing
    /// deep links inside widgets.
    pub fn on_deep_link<H>(mut self, handler: H) -> Self
    where
        H: fission_core::registry::IntoHandler<S, DeepLinkReceived> + Send + Sync + 'static,
    {
        let mut registry = ActionRegistry::<S>::new();
        registry.register(handler);
        self.runtime.absorb_persistent_registry(registry);
        self
    }

    /// Registers a reducer handler for notification responses.
    ///
    /// `handler` receives `NotificationResponseReceived` actions when the user
    /// taps or acts on a notification. Use it to route the user or process action
    /// ids in normal app state.
    pub fn on_notification_response<H>(mut self, handler: H) -> Self
    where
        H: fission_core::registry::IntoHandler<S, NotificationResponseReceived>
            + Send
            + Sync
            + 'static,
    {
        let mut registry = ActionRegistry::<S>::new();
        registry.register(handler);
        self.runtime.absorb_persistent_registry(registry);
        self
    }

    /// Register a reducer for host/shell route changes.
    ///
    /// The host dispatches [`fission_core::ShellRouteChanged`] when navigation
    /// updates. Applications should store route data in state and render the
    /// corresponding screen.
    pub fn with_route_handler(
        mut self,
        handler: fission_core::registry::Handler<S, fission_core::ShellRouteChanged>,
    ) -> Self {
        let mut registry = ActionRegistry::<S>::new();
        registry.register::<fission_core::ShellRouteChanged, _>(handler);
        self.runtime.absorb_persistent_registry(registry);
        self
    }

    pub fn register_reducer(
        &mut self,
        action_id: ActionId,
        reducer: fission_core::action::Reducer<S>,
    ) -> Result<()> {
        self.runtime.register_reducer::<S>(action_id, reducer)
    }

    pub fn absorb_registry(&mut self, registry: fission_core::ActionRegistry<S>) {
        self.runtime.absorb_persistent_registry(registry);
    }

    pub fn run(self) -> Result<()> {
        self.run_inner(
            #[cfg(target_os = "android")]
            None,
        )
    }

    #[cfg(target_os = "android")]
    pub fn run_with_android_app(self, android_app: AndroidApp) -> Result<()> {
        self.run_inner(Some(android_app))
    }
}
