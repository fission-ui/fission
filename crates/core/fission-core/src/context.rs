//! Reducer context and effect builder.
//!
//! When a reducer needs to emit side-effects or inspect the [`ActionInput`]
//! that triggered it, it receives a [`ReducerContext`]. The context provides
//! an [`Effects`] builder for issuing capabilities, jobs, services, and
//! runtime-control effects plus binding callback actions.

use crate::action::{Action, ActionEnvelope, AppState};
use crate::async_runtime::{
    JobRef, JobRequestPayload, JobSpec, ServiceBindings, ServiceCommandPayload, ServiceSlot,
    ServiceSpec, ServiceStartPayload, ServiceStopPayload,
};
use crate::capability::{
    CapabilityInvocationPayload, CapabilityType, OperationCapability, OperationCapabilityInvocation,
};
use crate::effect::{ActionInput, Effect, EffectEnvelope, RuntimeEffect};
use crate::platform::{
    CancelNotificationRequest, NotificationPermissionRequest, NotificationRequest,
    PushRegistrationRequest, SetBadgeCountRequest, CANCEL_ALL_NOTIFICATIONS, CANCEL_NOTIFICATION,
    GET_NOTIFICATION_SETTINGS, REGISTER_PUSH_NOTIFICATIONS, REQUEST_NOTIFICATION_PERMISSION,
    SCHEDULE_NOTIFICATION, SET_BADGE_COUNT, SHOW_NOTIFICATION, UNREGISTER_PUSH_NOTIFICATIONS,
};
use crate::platform_barcode::{
    BarcodeImageDecodeRequest, BarcodeScanRequest, CANCEL_BARCODE_SCAN, DECODE_BARCODE_IMAGE,
    SCAN_BARCODE,
};
use crate::platform_biometric::{
    BiometricAuthenticateRequest, AUTHENTICATE_BIOMETRIC, CANCEL_BIOMETRIC_AUTHENTICATION,
    GET_BIOMETRIC_AVAILABILITY,
};
use crate::platform_bluetooth::{
    BluetoothAdvertiseRequest, BluetoothConnectRequest, BluetoothDisconnectRequest,
    BluetoothPermissionRequest, BluetoothReadRequest, BluetoothScanRequest,
    BluetoothStopAdvertiseRequest, BluetoothWriteRequest, CONNECT_BLUETOOTH_DEVICE,
    DISCONNECT_BLUETOOTH_DEVICE, GET_BLUETOOTH_AVAILABILITY, READ_BLUETOOTH_CHARACTERISTIC,
    REQUEST_BLUETOOTH_PERMISSION, SCAN_BLUETOOTH_DEVICES, START_BLUETOOTH_ADVERTISING,
    STOP_BLUETOOTH_ADVERTISING, WRITE_BLUETOOTH_CHARACTERISTIC,
};
use crate::platform_camera::{
    CameraCaptureRequest, CameraFlashlightRequest, CameraPermissionRequest, CANCEL_CAMERA_CAPTURE,
    CAPTURE_PHOTO, GET_CAMERA_AVAILABILITY, REQUEST_CAMERA_PERMISSION, SET_CAMERA_FLASHLIGHT,
};
use crate::platform_clipboard::{
    ClipboardContent, ClipboardWriteTextRequest, CLEAR_CLIPBOARD, READ_CLIPBOARD_CONTENT,
    READ_CLIPBOARD_TEXT, WRITE_CLIPBOARD_CONTENT, WRITE_CLIPBOARD_TEXT,
};
use crate::platform_geolocation::{
    GeolocationPermissionRequest, GeolocationPositionRequest, GET_CURRENT_POSITION,
    GET_GEOLOCATION_PERMISSION, REQUEST_GEOLOCATION_PERMISSION,
};
use crate::platform_haptics::{
    HapticImpactRequest, HapticNotificationRequest, HapticPatternRequest, HAPTIC_IMPACT,
    HAPTIC_NOTIFICATION, HAPTIC_PATTERN, HAPTIC_SELECTION,
};
use crate::platform_microphone::{
    MicrophoneCaptureRequest, MicrophonePermissionRequest, CANCEL_MICROPHONE_CAPTURE,
    CAPTURE_MICROPHONE_AUDIO, GET_MICROPHONE_AVAILABILITY, REQUEST_MICROPHONE_PERMISSION,
};
use crate::platform_nfc::{
    NfcEmulationRequest, NfcScanRequest, NfcWriteRequest, CANCEL_NFC_SESSION, EMULATE_NFC_TAG,
    GET_NFC_AVAILABILITY, SCAN_NFC_TAG, WRITE_NFC_TAG,
};
use crate::platform_passkey::{
    PasskeyAuthenticationRequest, PasskeyRegistrationRequest, AUTHENTICATE_PASSKEY,
    CANCEL_PASSKEY_OPERATION, GET_PASSKEY_AVAILABILITY, REGISTER_PASSKEY,
};
use crate::platform_volume::{
    VolumeAdjustRequest, VolumeSetRequest, VolumeStream, ADJUST_VOLUME_LEVEL, GET_VOLUME_LEVEL,
    SET_VOLUME_LEVEL,
};
use crate::platform_wifi::{
    WifiConnectRequest, WifiDisconnectRequest, WifiPermissionRequest, WifiScanRequest,
    CONNECT_WIFI_NETWORK, DISCONNECT_WIFI_NETWORK, GET_WIFI_AVAILABILITY, REQUEST_WIFI_PERMISSION,
    SCAN_WIFI_NETWORKS,
};
use crate::registry::{ActionRegistry, IntoHandler};
use std::marker::PhantomData;

/// The context passed to modern 3-argument reducer handlers.
///
/// Provides access to the [`Effects`] builder (for emitting side-effects) and
/// the [`ActionInput`] that accompanied the dispatch (e.g. effect results,
/// pointer coordinates, drop payloads).
///
/// # Example
///
/// ```rust,ignore
/// fn handle_click(
///     state: &mut AppState,
///     action: ClickAction,
///     ctx: &mut ReducerContext<AppState>,
/// ) {
///     // Read pointer position from the input
///     if let Some((x, y, _, _)) = ctx.input.as_pointer() {
///         state.last_click = (x, y);
///     }
///     // Issue a capability effect
///     ctx.effects.capability(MY_CAPABILITY, request);
/// }
/// ```
pub struct ReducerContext<'a, 'b, 'c, S: AppState> {
    /// Mutable reference to the effects builder.
    pub effects: &'a mut Effects<'b, S>,
    /// The input data that accompanied this action dispatch.
    pub input: &'c ActionInput,
}

/// Builder for emitting side-effects from within a reducer.
///
/// `Effects` accumulates [`EffectEnvelope`] values that the runtime collects
/// after the reducer returns. Each effect can carry optional `on_ok` and
/// `on_err` callbacks.
///
/// # Example
///
/// ```rust,ignore
/// fn handle_save(
///     state: &mut MyState,
///     _action: Save,
///     ctx: &mut ReducerContext<MyState>,
/// ) {
///     ctx.effects.capability(MY_CAPABILITY, request)
///         .on_ok(ctx.effects.bind(SaveOk, handle_save_ok as fn(&mut MyState, SaveOk)))
///         .on_err(ctx.effects.bind(SaveErr, handle_save_err as fn(&mut MyState, SaveErr)));
/// }
/// ```
pub struct Effects<'a, S: AppState> {
    /// Accumulated effect envelopes, drained by the runtime after the reducer.
    pub out: Vec<EffectEnvelope>,
    next_req_id: u64,
    pub(crate) registry: Option<&'a mut ActionRegistry<S>>,
    _phantom: PhantomData<S>,
}

impl<'a, S: AppState> Effects<'a, S> {
    pub fn new(next_req_id: u64, registry: &'a mut ActionRegistry<S>) -> Self {
        Self {
            out: Vec::new(),
            next_req_id,
            registry: Some(registry),
            _phantom: PhantomData,
        }
    }

    pub fn new_headless(next_req_id: u64) -> Self {
        Self {
            out: Vec::new(),
            next_req_id,
            registry: None,
            _phantom: PhantomData,
        }
    }

    pub fn bind<A: Action, H>(&mut self, action: A, handler: H) -> ActionEnvelope
    where
        H: IntoHandler<S, A> + Send + Sync + 'static,
    {
        if let Some(registry) = &mut self.registry {
            registry.register(handler);
        }
        ActionEnvelope {
            id: A::static_id(),
            payload: action.encode(),
        }
    }

    pub fn add(&mut self, effect: Effect) -> u64 {
        let req_id = self.next_req_id;
        self.next_req_id += 1;

        self.out.push(EffectEnvelope {
            req_id,
            effect,
            on_ok: None,
            on_err: None,
            service_bindings: None,
            resource: None,
        });
        req_id
    }

    pub fn capability<C: OperationCapability>(
        &mut self,
        capability: CapabilityType<C>,
        request: C::Request,
    ) -> EffectBuilder<'_, 'a, S> {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        let request =
            serde_json::to_vec(&request).expect("capability request serialization must succeed");

        let index = self.out.len();
        self.out.push(EffectEnvelope {
            req_id,
            effect: Effect::Capability(CapabilityInvocationPayload::Operation(
                OperationCapabilityInvocation {
                    capability_name: capability.name.to_string(),
                    request,
                },
            )),
            on_ok: None,
            on_err: None,
            service_bindings: None,
            resource: None,
        });

        EffectBuilder {
            effects: self,
            index,
        }
    }

    /// Starts a typed notification capability request.
    ///
    /// Use this from reducers when the app needs the host to request
    /// notification permission, show or schedule a notification, update a badge,
    /// or register for push delivery. The returned builder records a capability
    /// effect; it does not display anything until the reducer has returned and
    /// the active shell processes queued effects.
    pub fn notifications(&mut self) -> NotificationEffects<'_, 'a, S> {
        NotificationEffects { effects: self }
    }

    /// Starts a typed NFC capability request.
    ///
    /// Use this when the app needs the host to read, write, emulate, or cancel
    /// an NFC session. The helper keeps NFC prompts, tag records, and timeout
    /// choices in typed request values so reducers do not call platform NFC APIs
    /// directly.
    pub fn nfc(&mut self) -> NfcEffects<'_, 'a, S> {
        NfcEffects { effects: self }
    }

    /// Starts a typed biometric authentication capability request.
    ///
    /// Use this for host-owned local user verification such as fingerprint,
    /// face, or device credential fallback. The result reports whether the host
    /// verified the local user; it is not a network identity assertion.
    pub fn biometrics(&mut self) -> BiometricEffects<'_, 'a, S> {
        BiometricEffects { effects: self }
    }

    /// Starts a typed passkey/WebAuthn credential capability request.
    ///
    /// Use this for account sign-in, re-authentication, or credential
    /// registration flows where the server verifies WebAuthn data. This is
    /// intentionally separate from `biometrics()`: the host may use biometrics
    /// to unlock a passkey, but the app receives credential data, not raw face
    /// or fingerprint state.
    pub fn passkeys(&mut self) -> PasskeyEffects<'_, 'a, S> {
        PasskeyEffects { effects: self }
    }

    /// Starts a typed Bluetooth capability request.
    ///
    /// Use this for nearby-device workflows such as adapter availability,
    /// permission requests, scanning, connecting, characteristic reads and
    /// writes, and advertising. Scans and connections are host-owned operations
    /// because permission and hardware behavior differ sharply by platform.
    pub fn bluetooth(&mut self) -> BluetoothEffects<'_, 'a, S> {
        BluetoothEffects { effects: self }
    }

    /// Starts a typed barcode scanner capability request.
    ///
    /// Use this when the host should run a live scanner session or decode image
    /// bytes into barcode results. Live scanning normally depends on camera
    /// permission; image decoding can be tested without camera hardware.
    pub fn barcode_scanner(&mut self) -> BarcodeScannerEffects<'_, 'a, S> {
        BarcodeScannerEffects { effects: self }
    }

    /// Starts a typed camera and flashlight capability request.
    ///
    /// Use this for camera availability, permission, still photo capture, and
    /// torch control. The returned helper emits requests to the shell host so
    /// the app state does not depend on a particular camera API.
    pub fn camera(&mut self) -> CameraEffects<'_, 'a, S> {
        CameraEffects { effects: self }
    }

    /// Starts a typed clipboard capability request.
    ///
    /// Use this for user-visible copy and paste flows. Platforms may restrict
    /// clipboard access to focused windows, secure browser contexts, or direct
    /// user gestures, so reducers should handle errors as normal outcomes.
    pub fn clipboard(&mut self) -> ClipboardEffects<'_, 'a, S> {
        ClipboardEffects { effects: self }
    }

    /// Starts a typed geolocation capability request.
    ///
    /// Use this when the app needs permission state or a current location. The
    /// request controls accuracy, timeout, and cache age so the host can choose
    /// an appropriate platform location call.
    pub fn geolocation(&mut self) -> GeolocationEffects<'_, 'a, S> {
        GeolocationEffects { effects: self }
    }

    /// Starts a typed haptic feedback capability request.
    ///
    /// Use this for tactile feedback tied to meaningful interactions such as
    /// impact, notification, selection, or a bounded pattern. Unsupported hosts
    /// should return a typed error rather than pretending vibration occurred.
    pub fn haptics(&mut self) -> HapticEffects<'_, 'a, S> {
        HapticEffects { effects: self }
    }

    /// Starts a typed microphone capability request.
    ///
    /// Use this for microphone availability, permission, bounded audio capture,
    /// and cancellation. Captures should be explicit and time-bounded because
    /// recording is a sensitive host-owned operation.
    pub fn microphone(&mut self) -> MicrophoneEffects<'_, 'a, S> {
        MicrophoneEffects { effects: self }
    }

    /// Starts a typed Wi-Fi capability request.
    ///
    /// Use this for adapter availability, permission, scanning, connecting, and
    /// disconnecting where the platform allows app-level Wi-Fi management.
    /// Many platforms treat Wi-Fi information as location-sensitive, so reducers
    /// should handle permission and unsupported errors explicitly.
    pub fn wifi(&mut self) -> WifiEffects<'_, 'a, S> {
        WifiEffects { effects: self }
    }

    /// Starts a typed host volume-control capability request.
    ///
    /// Use this for app-approved media, notification, alarm, call, or system
    /// stream adjustments. Some platforms expose only media-element volume or no
    /// system-volume control, so callers should treat unsupported errors as
    /// normal platform outcomes.
    pub fn volume(&mut self) -> VolumeEffects<'_, 'a, S> {
        VolumeEffects { effects: self }
    }

    pub fn app<J: JobSpec>(
        &mut self,
        job: JobRef<J>,
        request: J::Request,
    ) -> EffectBuilder<'_, 'a, S> {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        let payload = serde_json::to_vec(&request).expect("job request serialization must succeed");
        let index = self.out.len();
        self.out.push(EffectEnvelope {
            req_id,
            effect: Effect::Job(JobRequestPayload {
                job_name: job.name.to_string(),
                payload,
            }),
            on_ok: None,
            on_err: None,
            service_bindings: None,
            resource: None,
        });
        EffectBuilder {
            effects: self,
            index,
        }
    }

    pub fn start_service<Svc: ServiceSpec>(
        &mut self,
        slot: ServiceSlot<Svc>,
        config: Svc::Config,
    ) -> ServiceStartBuilder<'_, 'a, S> {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        let index = self.out.len();
        let config =
            serde_json::to_vec(&config).expect("service config serialization must succeed");
        self.out.push(EffectEnvelope {
            req_id,
            effect: Effect::StartService(ServiceStartPayload {
                service_name: slot.ty.name.to_string(),
                slot_key: slot.slot_key().to_string(),
                config,
            }),
            on_ok: None,
            on_err: None,
            service_bindings: Some(ServiceBindings::default()),
            resource: None,
        });
        ServiceStartBuilder {
            effects: self,
            index,
        }
    }

    pub fn command<Svc: ServiceSpec>(
        &mut self,
        slot: ServiceSlot<Svc>,
        command: Svc::Command,
    ) -> EffectBuilder<'_, 'a, S> {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        let index = self.out.len();
        let payload =
            serde_json::to_vec(&command).expect("service command serialization must succeed");
        self.out.push(EffectEnvelope {
            req_id,
            effect: Effect::ServiceCommand(ServiceCommandPayload {
                service_name: slot.ty.name.to_string(),
                slot_key: slot.slot_key().to_string(),
                payload,
            }),
            on_ok: None,
            on_err: None,
            service_bindings: None,
            resource: None,
        });
        EffectBuilder {
            effects: self,
            index,
        }
    }

    pub fn stop_service<Svc: ServiceSpec>(
        &mut self,
        slot: ServiceSlot<Svc>,
    ) -> EffectBuilder<'_, 'a, S> {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        let index = self.out.len();
        self.out.push(EffectEnvelope {
            req_id,
            effect: Effect::StopService(ServiceStopPayload {
                service_name: slot.ty.name.to_string(),
                slot_key: slot.slot_key().to_string(),
            }),
            on_ok: None,
            on_err: None,
            service_bindings: None,
            resource: None,
        });
        EffectBuilder {
            effects: self,
            index,
        }
    }

    pub fn cancel(&mut self, req_id: u64) {
        self.add(Effect::Runtime(RuntimeEffect::Cancel { req_id }));
    }

    pub fn release_resource(&mut self, resource_id: u64) {
        self.add(Effect::Runtime(RuntimeEffect::ReleaseResource {
            resource_id,
        }));
    }
}

/// Convenience builder for the standard notification host capabilities.
pub struct NotificationEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> NotificationEffects<'a, 'b, S> {
    /// Requests notification permission from the active host.
    ///
    /// `request` declares which notification features the app wants, such as
    /// alerts, badges, sounds, or provisional delivery. The returned
    /// `EffectBuilder` should normally bind success and error actions so the
    /// reducer can update state after the user responds to the platform prompt.
    pub fn request_permission(
        self,
        request: NotificationPermissionRequest,
    ) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(REQUEST_NOTIFICATION_PERMISSION, request)
    }

    /// Queries the host's current notification settings without showing a prompt.
    ///
    /// Use this before rendering notification-dependent controls or when a
    /// settings screen needs to explain why delivery is unavailable. The success
    /// action receives `NotificationSettings`.
    pub fn settings(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_NOTIFICATION_SETTINGS, ())
    }

    /// Shows an immediate local notification through the host.
    ///
    /// `request` supplies the stable notification id, visible title/body, badge,
    /// sound policy, optional deep link, and action buttons. Use `schedule`
    /// instead when delivery should happen at a future time.
    pub fn show(self, request: NotificationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SHOW_NOTIFICATION, request)
    }

    /// Schedules a local notification for future delivery.
    ///
    /// The `schedule` field on `request` controls the delivery time. Hosts may
    /// reject schedules they cannot persist, cannot deliver in the background, or
    /// cannot map to the platform notification model.
    pub fn schedule(self, request: NotificationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SCHEDULE_NOTIFICATION, request)
    }

    /// Cancels one pending or displayed notification by id.
    ///
    /// Use the same `NotificationId` that was used for `show` or `schedule`. A
    /// host may treat cancelling an unknown id as success if the desired final
    /// state is already true.
    pub fn cancel(self, request: CancelNotificationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_NOTIFICATION, request)
    }

    /// Cancels all notifications owned by this app where the host supports it.
    ///
    /// Use this for sign-out, workspace switching, or clearing a notification
    /// center state that no longer matches app state. Hosts that cannot bulk
    /// cancel should return `NotificationError`.
    pub fn cancel_all(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_ALL_NOTIFICATIONS, ())
    }

    /// Sets or clears the app badge count.
    ///
    /// `request.count = Some(n)` asks the host to show a badge count.
    /// `request.count = None` clears the badge. Badge support varies by desktop
    /// shell, launcher, browser, and mobile platform.
    pub fn set_badge_count(self, request: SetBadgeCountRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SET_BADGE_COUNT, request)
    }

    /// Registers the app for remote or push notifications.
    ///
    /// `request` carries provider-specific public registration inputs such as a
    /// web push application-server key, Android sender id, or requested topics.
    /// Secrets and store credentials belong in host configuration, not in app
    /// state.
    pub fn register_push(self, request: PushRegistrationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(REGISTER_PUSH_NOTIFICATIONS, request)
    }

    /// Unregisters the app from remote or push notification delivery.
    ///
    /// Use this during sign-out, account removal, or when a user disables remote
    /// notifications. The host should invalidate or delete its platform token
    /// where the provider supports that operation.
    pub fn unregister_push(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(UNREGISTER_PUSH_NOTIFICATIONS, ())
    }
}

/// Convenience builder for standard NFC host capabilities.
pub struct NfcEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> NfcEffects<'a, 'b, S> {
    /// Queries whether NFC is supported, enabled, and which NFC modes are available.
    ///
    /// Use this before showing scan/write controls so the UI can distinguish a
    /// missing NFC chip from a disabled adapter or unsupported operation.
    pub fn availability(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_NFC_AVAILABILITY, ())
    }

    /// Starts a one-shot NFC scan session.
    ///
    /// `request` declares allowed technologies, optional user-facing prompt text,
    /// timeout, and whether multiple records should be collected. The success
    /// action receives an `NfcTag` when the host reads a compatible tag.
    pub fn scan_tag(self, request: NfcScanRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SCAN_NFC_TAG, request)
    }

    /// Starts an NFC tag write session.
    ///
    /// `request.records` contains the portable NDEF-like records to write. Hosts
    /// may require the user to tap a writable tag after the operation starts and
    /// may reject read-only or incompatible tags.
    pub fn write_tag(self, request: NfcWriteRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(WRITE_NFC_TAG, request)
    }

    /// Requests host NFC card emulation for the supplied records.
    ///
    /// Use this only on platforms and devices that support card emulation for
    /// the product scenario. Many hosts support scanning but not emulation.
    pub fn emulate_tag(self, request: NfcEmulationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(EMULATE_NFC_TAG, request)
    }

    /// Cancels the active NFC session, if one is running.
    ///
    /// Use this when the user dismisses the screen that started scanning, writing,
    /// or emulation. Hosts may return success when no session is active.
    pub fn cancel_session(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_NFC_SESSION, ())
    }
}

/// Convenience builder for standard biometric host capabilities.
pub struct BiometricEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> BiometricEffects<'a, 'b, S> {
    /// Queries local biometric support and enrollment state.
    ///
    /// Use this before presenting a biometric-only path. The result tells the app
    /// whether the host supports biometric verification, whether credentials are
    /// enrolled, which modalities may be available, and whether device credential
    /// fallback is possible.
    pub fn availability(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_BIOMETRIC_AVAILABILITY, ())
    }

    /// Asks the host to authenticate the current local user.
    ///
    /// `request.reason` should explain why verification is needed before the
    /// platform prompt appears. The success action receives
    /// `BiometricAuthenticateResult`, which reports the modality and whether a
    /// device credential fallback was used.
    pub fn authenticate(self, request: BiometricAuthenticateRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(AUTHENTICATE_BIOMETRIC, request)
    }

    /// Cancels an active biometric authentication prompt where the host permits it.
    ///
    /// Use this when the screen that requested verification is closed or the app
    /// changes state before the user responds. Some platform prompts cannot be
    /// cancelled programmatically after display.
    pub fn cancel_authentication(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_BIOMETRIC_AUTHENTICATION, ())
    }
}

/// Convenience builder for standard passkey/WebAuthn host capabilities.
pub struct PasskeyEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> PasskeyEffects<'a, 'b, S> {
    /// Queries passkey support for the active host and origin.
    ///
    /// Use this before showing passkey-specific registration or sign-in controls.
    /// The result tells the app whether the host supports passkeys, whether the
    /// current context is secure enough for credential APIs, and whether platform
    /// or conditional UI authenticators may be available.
    pub fn availability(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_PASSKEY_AVAILABILITY, ())
    }

    /// Requests creation of a new passkey credential.
    ///
    /// `request.challenge` must come from the relying-party server and must be
    /// verified by that server when the success action receives
    /// `PasskeyRegistrationResult`. Do not generate production challenges in the
    /// UI reducer or trust registration data until the backend verifies it.
    pub fn register(self, request: PasskeyRegistrationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(REGISTER_PASSKEY, request)
    }

    /// Requests authentication with an existing passkey credential.
    ///
    /// `request.challenge` must come from the server, and the returned
    /// `PasskeyAuthenticationResult` must be verified by the server before the
    /// app treats the user as signed in. The host only gathers credential data.
    pub fn authenticate(self, request: PasskeyAuthenticationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(AUTHENTICATE_PASSKEY, request)
    }

    /// Cancels an active passkey prompt where the host permits cancellation.
    ///
    /// Use this when the sign-in or registration screen disappears before the
    /// host credential picker completes. Some browser or operating-system
    /// prompts cannot be cancelled once shown.
    pub fn cancel(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_PASSKEY_OPERATION, ())
    }
}

/// Convenience builder for standard Bluetooth host capabilities.
pub struct BluetoothEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> BluetoothEffects<'a, 'b, S> {
    /// Queries Bluetooth adapter, permission, and mode availability.
    ///
    /// Use this before showing scan, connect, or advertise controls. The result
    /// lets the UI distinguish missing hardware, disabled Bluetooth, permission
    /// denial, and hosts that support only classic or Low Energy Bluetooth.
    pub fn availability(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_BLUETOOTH_AVAILABILITY, ())
    }

    /// Requests Bluetooth or nearby-device permission from the host.
    ///
    /// `request.reason` should explain the product feature that needs nearby
    /// devices. Hosts map the request to the platform permission model, which may
    /// include Bluetooth, location, or nearby-device prompts depending on target.
    pub fn request_permission(
        self,
        request: BluetoothPermissionRequest,
    ) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(REQUEST_BLUETOOTH_PERMISSION, request)
    }

    /// Scans for Bluetooth devices matching the request filters.
    ///
    /// `request.service_uuids` narrows discovery to product-relevant services.
    /// `timeout_ms` should be set for user-driven scans so the host does not keep
    /// nearby-device discovery running indefinitely.
    pub fn scan_devices(self, request: BluetoothScanRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SCAN_BLUETOOTH_DEVICES, request)
    }

    /// Connects to a discovered or previously known Bluetooth device.
    ///
    /// `request.device_id` must come from a trusted host result or stored pairing
    /// flow. The success action receives a `BluetoothConnection` whose
    /// `connection_id` is used for later read, write, and disconnect requests.
    pub fn connect_device(self, request: BluetoothConnectRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CONNECT_BLUETOOTH_DEVICE, request)
    }

    /// Disconnects a previously opened Bluetooth connection.
    ///
    /// `request.connection_id` should be the id returned by `connect_device`.
    /// Use this when the user leaves the device workflow or when the app no
    /// longer needs the peripheral.
    pub fn disconnect_device(
        self,
        request: BluetoothDisconnectRequest,
    ) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(DISCONNECT_BLUETOOTH_DEVICE, request)
    }

    /// Reads one Bluetooth characteristic from an active connection.
    ///
    /// `request` names the connection, service UUID, and characteristic UUID.
    /// Hosts should return `BluetoothError` when the connection is gone or the
    /// characteristic is unavailable.
    pub fn read_characteristic(self, request: BluetoothReadRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(READ_BLUETOOTH_CHARACTERISTIC, request)
    }

    /// Writes bytes to one Bluetooth characteristic.
    ///
    /// `request.with_response` lets the app choose between acknowledged and
    /// unacknowledged writes where the platform supports both. Reducers should
    /// still handle connection loss and permission errors as normal outcomes.
    pub fn write_characteristic(self, request: BluetoothWriteRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(WRITE_BLUETOOTH_CHARACTERISTIC, request)
    }

    /// Starts Bluetooth advertising for hosts that allow apps to advertise.
    ///
    /// `request` supplies the service UUID, optional service data, display name,
    /// and timeout. Mobile and browser platforms often restrict advertising more
    /// heavily than scanning or connecting.
    pub fn start_advertising(self, request: BluetoothAdvertiseRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(START_BLUETOOTH_ADVERTISING, request)
    }

    /// Stops a Bluetooth advertising session.
    ///
    /// `request.advertisement_id` should be the id returned by
    /// `start_advertising`. Hosts may also stop advertisements automatically when
    /// their timeout expires or the app moves to a background state.
    pub fn stop_advertising(
        self,
        request: BluetoothStopAdvertiseRequest,
    ) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(STOP_BLUETOOTH_ADVERTISING, request)
    }
}

/// Convenience builder for standard barcode scanner host capabilities.
pub struct BarcodeScannerEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> BarcodeScannerEffects<'a, 'b, S> {
    /// Starts a live barcode scanning session.
    ///
    /// `request.formats` should list only formats the product accepts. The host
    /// may open a camera UI, display `prompt`, and return one or more decoded
    /// barcode values depending on `allow_multiple`.
    pub fn scan(self, request: BarcodeScanRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SCAN_BARCODE, request)
    }

    /// Decodes barcode data from image bytes supplied by the app.
    ///
    /// Use this when the image already exists, such as a file import or camera
    /// frame captured elsewhere. The host should not request camera permission
    /// for this operation unless its decoder specifically requires it.
    pub fn decode_image(self, request: BarcodeImageDecodeRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(DECODE_BARCODE_IMAGE, request)
    }

    /// Cancels the active live barcode scanning session.
    ///
    /// Use this when the user leaves the scanning screen or chooses another input
    /// path. Hosts may treat cancellation of a non-running session as success.
    pub fn cancel_scan(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_BARCODE_SCAN, ())
    }
}

/// Convenience builder for standard camera host capabilities.
pub struct CameraEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> CameraEffects<'a, 'b, S> {
    /// Queries camera permission and available camera devices.
    ///
    /// Use this before showing camera-specific controls. The result contains the
    /// current permission state and host-visible devices, including facing
    /// direction and flashlight availability where known.
    pub fn availability(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_CAMERA_AVAILABILITY, ())
    }

    /// Requests camera permission from the host.
    ///
    /// `request.reason` can carry product-facing context for hosts that support a
    /// pre-prompt or custom rationale. The success action receives the resulting
    /// `CameraPermission` state.
    pub fn request_permission(self, request: CameraPermissionRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(REQUEST_CAMERA_PERMISSION, request)
    }

    /// Captures a still photo through the selected host camera.
    ///
    /// `request` chooses camera id or facing direction, optional resolution, image
    /// format, flash behavior, and quality. The success action receives image
    /// bytes plus dimensions and content type.
    pub fn capture_photo(self, request: CameraCaptureRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CAPTURE_PHOTO, request)
    }

    /// Enables, disables, or adjusts the camera flashlight where supported.
    ///
    /// `request.camera_id` selects the device, `enabled` chooses the desired
    /// state, and `intensity` optionally requests a platform-specific brightness
    /// level from 0 to 100. Many desktop cameras have no torch.
    pub fn set_flashlight(self, request: CameraFlashlightRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SET_CAMERA_FLASHLIGHT, request)
    }

    /// Cancels an active camera capture session.
    ///
    /// Use this when the user dismisses the camera flow before a photo is
    /// returned. Hosts may return success when there is no active capture.
    pub fn cancel_capture(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_CAMERA_CAPTURE, ())
    }
}

/// Convenience builder for standard clipboard host capabilities.
pub struct ClipboardEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> ClipboardEffects<'a, 'b, S> {
    /// Reads text from the host clipboard.
    ///
    /// Use this in response to an explicit paste action. The success action
    /// receives `ClipboardText` with `None` when there is no readable text.
    pub fn read_text(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(READ_CLIPBOARD_TEXT, ())
    }

    /// Writes plain text to the host clipboard.
    ///
    /// `request.text` should be the exact text the user asked to copy. Some hosts
    /// may require focus or a user gesture before accepting the write.
    pub fn write_text(self, request: ClipboardWriteTextRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(WRITE_CLIPBOARD_TEXT, request)
    }

    /// Reads typed clipboard content from the host.
    ///
    /// Use this when the product can accept richer content than plain text. The
    /// success action receives zero or more `ClipboardItem` values with content
    /// types and bytes.
    pub fn read_content(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(READ_CLIPBOARD_CONTENT, ())
    }

    /// Writes typed content items to the host clipboard.
    ///
    /// `request.items` should list content types the target host can expose.
    /// Include a `text/plain` item when possible so paste targets have a portable
    /// fallback.
    pub fn write_content(self, request: ClipboardContent) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(WRITE_CLIPBOARD_CONTENT, request)
    }

    /// Clears app-visible clipboard content where the host supports it.
    ///
    /// Use this for explicit privacy actions such as Clear copied password. Some
    /// platforms may not allow apps to clear global clipboard state.
    pub fn clear(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CLEAR_CLIPBOARD, ())
    }
}

/// Convenience builder for standard geolocation host capabilities.
pub struct GeolocationEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> GeolocationEffects<'a, 'b, S> {
    /// Reads the current geolocation permission state without showing a prompt.
    ///
    /// Use this to decide whether a screen should show a request button, an
    /// explanation, or a platform-settings hint. The result is host-reported and
    /// may change outside the app.
    pub fn permission(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_GEOLOCATION_PERMISSION, ())
    }

    /// Requests geolocation permission from the host.
    ///
    /// `request.precise` asks for precise coordinates when the platform exposes a
    /// precise/approximate distinction. `request.background` should only be set
    /// for product flows that genuinely need background location and have matching
    /// platform configuration.
    pub fn request_permission(
        self,
        request: GeolocationPermissionRequest,
    ) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(REQUEST_GEOLOCATION_PERMISSION, request)
    }

    /// Requests the current location from the host.
    ///
    /// `request.high_accuracy`, `timeout_ms`, and `maximum_age_ms` let the app
    /// trade precision, speed, power use, and cached values. The success action
    /// receives latitude, longitude, accuracy, and optional motion metadata.
    pub fn current_position(self, request: GeolocationPositionRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_CURRENT_POSITION, request)
    }
}

/// Convenience builder for standard haptic host capabilities.
pub struct HapticEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> HapticEffects<'a, 'b, S> {
    /// Plays impact-style haptic feedback.
    ///
    /// Use this for physical-feeling interactions such as completing a drag,
    /// snapping to a position, or confirming a strong action. The `style` field
    /// tells the host how heavy the feedback should feel.
    pub fn impact(self, request: HapticImpactRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(HAPTIC_IMPACT, request)
    }

    /// Plays notification-style haptic feedback.
    ///
    /// Use this to reinforce success, warning, or error states when tactile
    /// feedback improves understanding. It should not replace visible or spoken
    /// feedback for accessibility.
    pub fn notification(self, request: HapticNotificationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(HAPTIC_NOTIFICATION, request)
    }

    /// Plays selection-change haptic feedback.
    ///
    /// Use this for picker movement, segmented-control changes, or other repeated
    /// selection adjustments where a light tick helps the user track movement.
    pub fn selection(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(HAPTIC_SELECTION, ())
    }

    /// Plays a bounded custom haptic pattern.
    ///
    /// `request.steps` contains duration and intensity values. Keep patterns
    /// short and meaningful; hosts may reject long, empty, or unsupported
    /// patterns.
    pub fn pattern(self, request: HapticPatternRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(HAPTIC_PATTERN, request)
    }
}

/// Convenience builder for standard microphone host capabilities.
pub struct MicrophoneEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> MicrophoneEffects<'a, 'b, S> {
    /// Queries microphone permission and available input devices.
    ///
    /// Use this before showing recording controls. The result tells the app
    /// whether microphone permission is granted and which host input devices are
    /// visible.
    pub fn availability(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_MICROPHONE_AVAILABILITY, ())
    }

    /// Requests microphone permission from the host.
    ///
    /// `request.reason` can be used by hosts that support a product-specific
    /// rationale before the platform prompt. The success action receives the
    /// resulting `MicrophonePermission` state.
    pub fn request_permission(
        self,
        request: MicrophonePermissionRequest,
    ) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(REQUEST_MICROPHONE_PERMISSION, request)
    }

    /// Captures bounded audio from the selected microphone.
    ///
    /// `request.duration_ms` must define the intended capture length. Optional
    /// sample rate, channel count, and sample format let the host choose the
    /// closest supported recording configuration.
    pub fn capture_audio(self, request: MicrophoneCaptureRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CAPTURE_MICROPHONE_AUDIO, request)
    }

    /// Cancels an active microphone capture session.
    ///
    /// Use this when the user stops recording, closes the screen, or chooses a
    /// different input path before the bounded capture completes.
    pub fn cancel_capture(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_MICROPHONE_CAPTURE, ())
    }
}

/// Convenience builder for standard Wi-Fi host capabilities.
pub struct WifiEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> WifiEffects<'a, 'b, S> {
    /// Queries current Wi-Fi adapter and connection availability.
    ///
    /// Use this before showing scan or connect controls. The result can include
    /// whether the adapter is enabled and which network, if any, is connected.
    pub fn availability(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_WIFI_AVAILABILITY, ())
    }

    /// Requests Wi-Fi or nearby-network permission from the host.
    ///
    /// `request.reason` should describe the feature that needs network discovery
    /// or management. Hosts may map this to Wi-Fi, nearby-device, or location
    /// permission prompts depending on platform policy.
    pub fn request_permission(self, request: WifiPermissionRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(REQUEST_WIFI_PERMISSION, request)
    }

    /// Scans for nearby Wi-Fi networks where the host permits scanning.
    ///
    /// `request.ssid_prefix` narrows results for device-setup flows,
    /// `include_hidden` asks the host to include hidden networks when possible,
    /// and `timeout_ms` bounds the scan.
    pub fn scan_networks(self, request: WifiScanRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SCAN_WIFI_NETWORKS, request)
    }

    /// Requests connection to one Wi-Fi network.
    ///
    /// `request` carries SSID, optional passphrase, security type, and hidden
    /// network flag. Hosts may reject connections that require user confirmation,
    /// saved network profiles, entitlements, or administrator privileges.
    pub fn connect_network(self, request: WifiConnectRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CONNECT_WIFI_NETWORK, request)
    }

    /// Requests disconnection from a Wi-Fi network.
    ///
    /// `request.ssid` can limit the operation to a specific network when the host
    /// supports that distinction. Some platforms do not allow apps to disconnect
    /// global network state.
    pub fn disconnect_network(self, request: WifiDisconnectRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(DISCONNECT_WIFI_NETWORK, request)
    }
}

/// Convenience builder for standard volume-control host capabilities.
pub struct VolumeEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> VolumeEffects<'a, 'b, S> {
    /// Reads the current level for one host volume stream.
    ///
    /// `stream` identifies the logical audio stream the app cares about. Hosts
    /// map that stream to the closest platform mixer or media channel and return
    /// a `VolumeLevel` with level and mute state.
    pub fn get_level(self, stream: VolumeStream) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_VOLUME_LEVEL, stream)
    }

    /// Sets the level and optional mute state for one host volume stream.
    ///
    /// `request.level` is a percentage-like value from 0 to 100. Hosts should
    /// clamp or reject values they cannot represent and return a typed error when
    /// the platform does not expose system volume control.
    pub fn set_level(self, request: VolumeSetRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SET_VOLUME_LEVEL, request)
    }

    /// Adjusts a host volume stream relative to its current level.
    ///
    /// `request.direction` chooses increase, decrease, or toggle mute, and
    /// `request.step` controls the requested amount. Use this for keyboard-like
    /// or remote-control volume actions.
    pub fn adjust_level(self, request: VolumeAdjustRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(ADJUST_VOLUME_LEVEL, request)
    }
}

/// Fluent builder returned by [`Effects::capability`], [`Effects::app`], and
/// related effect constructors.
///
/// Attach `on_ok` and `on_err` callback envelopes before the builder is dropped.
///
/// # Example
///
/// ```rust,ignore
/// ctx.effects.capability(MY_CAPABILITY, request)
///     .on_ok(ok_envelope)
///     .on_err(err_envelope)
///     .dispatch(); // optional -- dropping also finalises
/// ```
pub struct EffectBuilder<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
    index: usize,
}

impl<'a, 'b, S: AppState> EffectBuilder<'a, 'b, S> {
    pub fn on_ok(self, action: ActionEnvelope) -> Self {
        self.effects.out[self.index].on_ok = Some(action);
        self
    }

    pub fn on_err(self, action: ActionEnvelope) -> Self {
        self.effects.out[self.index].on_err = Some(action);
        self
    }

    pub fn dispatch(self) {
        // Drop
    }
}

pub struct ServiceStartBuilder<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
    index: usize,
}

impl<'a, 'b, S: AppState> ServiceStartBuilder<'a, 'b, S> {
    pub fn on_started(self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effects.out[self.index].service_bindings.as_mut() {
            bindings.on_started = Some(action);
        }
        self
    }

    pub fn on_start_failed(self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effects.out[self.index].service_bindings.as_mut() {
            bindings.on_start_failed = Some(action);
        }
        self
    }

    pub fn on_event(self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effects.out[self.index].service_bindings.as_mut() {
            bindings.on_event = Some(action);
        }
        self
    }

    pub fn on_stopped(self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effects.out[self.index].service_bindings.as_mut() {
            bindings.on_stopped = Some(action);
        }
        self
    }

    pub fn on_command_ok(self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effects.out[self.index].service_bindings.as_mut() {
            bindings.on_command_ok = Some(action);
        }
        self
    }

    pub fn on_command_err(self, action: ActionEnvelope) -> Self {
        if let Some(bindings) = self.effects.out[self.index].service_bindings.as_mut() {
            bindings.on_command_err = Some(action);
        }
        self
    }

    pub fn dispatch(self) {}
}
