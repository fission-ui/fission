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
        self.add(Effect::Runtime(RuntimeEffect::ReleaseResource { resource_id }));
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
