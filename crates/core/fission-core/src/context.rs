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
use crate::platform_clipboard::{
    ClipboardContent, ClipboardWriteTextRequest, CLEAR_CLIPBOARD, READ_CLIPBOARD_CONTENT,
    READ_CLIPBOARD_TEXT, WRITE_CLIPBOARD_CONTENT, WRITE_CLIPBOARD_TEXT,
};
use crate::platform_nfc::{
    NfcEmulationRequest, NfcScanRequest, NfcWriteRequest, CANCEL_NFC_SESSION, EMULATE_NFC_TAG,
    GET_NFC_AVAILABILITY, SCAN_NFC_TAG, WRITE_NFC_TAG,
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

    pub fn notifications(&mut self) -> NotificationEffects<'_, 'a, S> {
        NotificationEffects { effects: self }
    }

    pub fn nfc(&mut self) -> NfcEffects<'_, 'a, S> {
        NfcEffects { effects: self }
    }

    pub fn biometrics(&mut self) -> BiometricEffects<'_, 'a, S> {
        BiometricEffects { effects: self }
    }

    pub fn barcode_scanner(&mut self) -> BarcodeScannerEffects<'_, 'a, S> {
        BarcodeScannerEffects { effects: self }
    }

    pub fn clipboard(&mut self) -> ClipboardEffects<'_, 'a, S> {
        ClipboardEffects { effects: self }
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
    pub fn request_permission(
        self,
        request: NotificationPermissionRequest,
    ) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(REQUEST_NOTIFICATION_PERMISSION, request)
    }

    pub fn settings(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_NOTIFICATION_SETTINGS, ())
    }

    pub fn show(self, request: NotificationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SHOW_NOTIFICATION, request)
    }

    pub fn schedule(self, request: NotificationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SCHEDULE_NOTIFICATION, request)
    }

    pub fn cancel(self, request: CancelNotificationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_NOTIFICATION, request)
    }

    pub fn cancel_all(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_ALL_NOTIFICATIONS, ())
    }

    pub fn set_badge_count(self, request: SetBadgeCountRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SET_BADGE_COUNT, request)
    }

    pub fn register_push(self, request: PushRegistrationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects
            .capability(REGISTER_PUSH_NOTIFICATIONS, request)
    }

    pub fn unregister_push(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(UNREGISTER_PUSH_NOTIFICATIONS, ())
    }
}

/// Convenience builder for standard NFC host capabilities.
pub struct NfcEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> NfcEffects<'a, 'b, S> {
    pub fn availability(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_NFC_AVAILABILITY, ())
    }

    pub fn scan_tag(self, request: NfcScanRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SCAN_NFC_TAG, request)
    }

    pub fn write_tag(self, request: NfcWriteRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(WRITE_NFC_TAG, request)
    }

    pub fn emulate_tag(self, request: NfcEmulationRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(EMULATE_NFC_TAG, request)
    }

    pub fn cancel_session(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_NFC_SESSION, ())
    }
}

/// Convenience builder for standard biometric host capabilities.
pub struct BiometricEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> BiometricEffects<'a, 'b, S> {
    pub fn availability(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(GET_BIOMETRIC_AVAILABILITY, ())
    }

    pub fn authenticate(self, request: BiometricAuthenticateRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(AUTHENTICATE_BIOMETRIC, request)
    }

    pub fn cancel_authentication(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_BIOMETRIC_AUTHENTICATION, ())
    }
}

/// Convenience builder for standard barcode scanner host capabilities.
pub struct BarcodeScannerEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> BarcodeScannerEffects<'a, 'b, S> {
    pub fn scan(self, request: BarcodeScanRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(SCAN_BARCODE, request)
    }

    pub fn decode_image(self, request: BarcodeImageDecodeRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(DECODE_BARCODE_IMAGE, request)
    }

    pub fn cancel_scan(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CANCEL_BARCODE_SCAN, ())
    }
}

/// Convenience builder for standard clipboard host capabilities.
pub struct ClipboardEffects<'a, 'b, S: AppState> {
    effects: &'a mut Effects<'b, S>,
}

impl<'a, 'b, S: AppState> ClipboardEffects<'a, 'b, S> {
    pub fn read_text(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(READ_CLIPBOARD_TEXT, ())
    }

    pub fn write_text(self, request: ClipboardWriteTextRequest) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(WRITE_CLIPBOARD_TEXT, request)
    }

    pub fn read_content(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(READ_CLIPBOARD_CONTENT, ())
    }

    pub fn write_content(self, request: ClipboardContent) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(WRITE_CLIPBOARD_CONTENT, request)
    }

    pub fn clear(self) -> EffectBuilder<'a, 'b, S> {
        self.effects.capability(CLEAR_CLIPBOARD, ())
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
