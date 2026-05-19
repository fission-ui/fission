//! Side-effect primitives for async operations.
//!
//! Reducers are pure functions -- they must not perform I/O. When a reducer
//! needs to trigger a host capability, async job, or runtime-control effect, it
//! pushes an [`EffectEnvelope`] through the [`Effects`](crate::Effects) builder.
//! The platform executor fulfils the effect outside the deterministic core and
//! dispatches the `on_ok` / `on_err` callback actions back into the pipeline.

use crate::action::ActionEnvelope;
use crate::async_runtime::{
    JobRef, JobRequestPayload, JobSpec, ResourceExecutionContext, ServiceBindings,
    ServiceCommandPayload, ServiceSpec, ServiceStartPayload, ServiceStopPayload, ServiceType,
};
use crate::capability::CapabilityInvocationPayload;
use crate::capability::{CapabilityType, OperationCapability};
use fission_ir::NodeId;
use serde::{Deserialize, Serialize};

/// An opaque request identifier assigned to each emitted effect.
///
/// The platform executor returns this id when delivering the result so the
/// runtime can correlate responses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReqId(pub u64);

/// An opaque handle to a platform-managed resource (e.g. a large binary blob).
///
/// Resources live outside the action pipeline to avoid copying large payloads.
/// Use [`RuntimeEffect::ReleaseResource`] to free them when no longer needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub u64);

/// Runtime-managed effects that are not host capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeEffect {
    /// Cancel a previously issued effect by its request id.
    Cancel { req_id: u64 },
    /// Release a platform-managed resource.
    ReleaseResource { resource_id: u64 },
}

/// A side-effect emitted by a reducer.
///
/// `Runtime` variants are handled by the runtime itself.
/// All host-facing work is expressed as typed capabilities, jobs, or services.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    /// A runtime-managed effect (cancellation, resource release).
    Runtime(RuntimeEffect),
    /// A typed one-shot host capability invocation.
    Capability(CapabilityInvocationPayload),
    /// A typed one-shot async job.
    Job(JobRequestPayload),
    /// Start a long-lived service for a logical slot.
    StartService(ServiceStartPayload),
    /// Send a command to an already-running service slot.
    ServiceCommand(ServiceCommandPayload),
    /// Stop a running service slot.
    StopService(ServiceStopPayload),
}

/// A queued effect with optional success/failure callbacks.
///
/// The platform executor processes the [`Effect`], then dispatches either
/// `on_ok` or `on_err` back into the runtime. The `req_id` is assigned
/// automatically by the runtime and is globally unique within a session.
///
/// # Example
///
/// ```rust,ignore
/// // Built via the Effects builder -- you rarely construct this manually.
/// ctx.effects.capability(MY_CAPABILITY, request)
///     .on_ok(ok_envelope)
///     .on_err(err_envelope);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EffectEnvelope {
    /// Unique request identifier (assigned by the runtime).
    pub req_id: u64,
    /// The effect to execute.
    pub effect: Effect,
    /// Action dispatched when the effect completes successfully.
    pub on_ok: Option<ActionEnvelope>,
    /// Action dispatched when the effect fails.
    pub on_err: Option<ActionEnvelope>,
    /// Additional bindings used by service lifecycle operations.
    pub service_bindings: Option<ServiceBindings>,
    /// Optional resource ownership metadata used to suppress stale completions.
    pub resource: Option<ResourceExecutionContext>,
}

/// Extra input data passed alongside an action dispatch.
///
/// When the platform delivers an effect result or a drag-and-drop event, it
/// attaches an `ActionInput` so the reducer can access the associated data
/// without encoding it in the action payload.
///
/// # Example
///
/// ```rust,ignore
/// fn on_file_loaded(
///     state: &mut MyState,
///     _action: FileLoaded,
///     ctx: &mut ReducerContext<MyState>,
/// ) {
///     if let Some(bytes) = ctx.input.as_bytes() {
///         state.file_contents = String::from_utf8_lossy(bytes).into_owned();
///     }
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum ActionInput {
    /// No extra input.
    None,
    /// A typed async job completed successfully.
    JobOk {
        job_name: String,
        req_id: u64,
        payload: Vec<u8>,
    },
    /// A typed async job failed.
    JobErr {
        job_name: String,
        req_id: u64,
        payload: Option<Vec<u8>>,
        message: Option<String>,
    },
    /// A service slot started successfully.
    ServiceStarted {
        service_name: String,
        slot_key: String,
        instance_id: u64,
    },
    /// A service slot failed to start.
    ServiceStartFailed {
        service_name: String,
        slot_key: String,
        payload: Option<Vec<u8>>,
        message: Option<String>,
    },
    /// A running service emitted an event.
    ServiceEvent {
        service_name: String,
        slot_key: String,
        instance_id: u64,
        payload: Vec<u8>,
    },
    /// A running service stopped.
    ServiceStopped {
        service_name: String,
        slot_key: String,
        instance_id: u64,
    },
    /// A service command completed successfully.
    ServiceCommandOk {
        service_name: String,
        slot_key: String,
        instance_id: u64,
        req_id: u64,
        payload: Option<Vec<u8>>,
    },
    /// A service command failed.
    ServiceCommandErr {
        service_name: String,
        slot_key: String,
        instance_id: u64,
        req_id: u64,
        payload: Option<Vec<u8>>,
        message: Option<String>,
    },
    /// A typed capability operation succeeded.
    CapabilityOk {
        capability: String,
        req_id: u64,
        payload: Vec<u8>,
    },
    /// A typed capability operation failed.
    CapabilityErr {
        capability: String,
        req_id: u64,
        payload: Option<Vec<u8>>,
        message: Option<String>,
    },
    /// A timer resource fired.
    TimerTick { payload: Vec<u8> },
    /// Pointer coordinates and deltas (used by drag/gesture handlers).
    Pointer {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
    /// External file drop (e.g. from the OS file manager).
    Drop { paths: Vec<String>, x: f32, y: f32 },
    /// Internal drag-and-drop with an opaque byte payload.
    InternalDrop { payload: Vec<u8>, x: f32, y: f32 },
    /// The action was dispatched from a subtree with a raw action scope.
    ScopedRaw {
        scope_id: u128,
        target: NodeId,
        input: Box<ActionInput>,
    },
}

impl ActionInput {
    pub fn scoped_raw(scope_id: u128, target: NodeId, input: ActionInput) -> Self {
        Self::ScopedRaw {
            scope_id,
            target,
            input: Box::new(input),
        }
    }

    pub fn action_scope_id(&self) -> Option<u128> {
        match self {
            ActionInput::ScopedRaw { scope_id, .. } => Some(*scope_id),
            _ => None,
        }
    }

    pub fn scoped_target(&self) -> Option<NodeId> {
        match self {
            ActionInput::ScopedRaw { target, .. } => Some(*target),
            _ => None,
        }
    }

    pub fn unscoped(&self) -> &ActionInput {
        match self {
            ActionInput::ScopedRaw { input, .. } => input.unscoped(),
            _ => self,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self.unscoped() {
            ActionInput::JobOk { payload, .. } => Some(payload),
            ActionInput::CapabilityOk { payload, .. } => Some(payload),
            ActionInput::TimerTick { payload } => Some(payload),
            ActionInput::InternalDrop { payload, .. } => Some(payload),
            _ => None,
        }
    }

    pub fn as_pointer(&self) -> Option<(f32, f32, f32, f32)> {
        match self.unscoped() {
            ActionInput::Pointer {
                x,
                y,
                delta_x,
                delta_y,
            } => Some((*x, *y, *delta_x, *delta_y)),
            ActionInput::Drop { x, y, .. } => Some((*x, *y, 0.0, 0.0)),
            ActionInput::InternalDrop { x, y, .. } => Some((*x, *y, 0.0, 0.0)),
            _ => None,
        }
    }

    pub fn as_drop_paths(&self) -> Option<&[String]> {
        match self.unscoped() {
            ActionInput::Drop { paths, .. } => Some(paths),
            _ => None,
        }
    }

    pub fn as_internal_drop(&self) -> Option<&[u8]> {
        match self.unscoped() {
            ActionInput::InternalDrop { payload, .. } => Some(payload),
            _ => None,
        }
    }

    pub fn job_ok<J: JobSpec>(&self, job: JobRef<J>) -> Option<J::Ok> {
        match self.unscoped() {
            ActionInput::JobOk {
                job_name, payload, ..
            } if job_name == job.name => serde_json::from_slice(payload).ok(),
            _ => None,
        }
    }

    pub fn job_err<J: JobSpec>(&self, job: JobRef<J>) -> Option<J::Err> {
        match self.unscoped() {
            ActionInput::JobErr {
                job_name,
                payload: Some(payload),
                ..
            } if job_name == job.name => serde_json::from_slice(payload).ok(),
            _ => None,
        }
    }

    pub fn job_error_message<J: JobSpec>(&self, job: JobRef<J>) -> Option<&str> {
        match self.unscoped() {
            ActionInput::JobErr {
                job_name,
                message: Some(message),
                ..
            } if job_name == job.name => Some(message.as_str()),
            _ => None,
        }
    }

    pub fn capability_ok<C: OperationCapability>(
        &self,
        capability: CapabilityType<C>,
    ) -> Option<C::Ok> {
        match self.unscoped() {
            ActionInput::CapabilityOk {
                capability: actual,
                payload,
                ..
            } if actual == capability.name => serde_json::from_slice(payload).ok(),
            _ => None,
        }
    }

    pub fn capability_error<C: OperationCapability>(
        &self,
        capability: CapabilityType<C>,
    ) -> Option<C::Err> {
        match self.unscoped() {
            ActionInput::CapabilityErr {
                capability: actual,
                payload: Some(payload),
                ..
            } if actual == capability.name => serde_json::from_slice(payload).ok(),
            _ => None,
        }
    }

    pub fn capability_error_message<C: OperationCapability>(
        &self,
        capability: CapabilityType<C>,
    ) -> Option<&str> {
        match self.unscoped() {
            ActionInput::CapabilityErr {
                capability: actual,
                message: Some(message),
                ..
            } if actual == capability.name => Some(message),
            _ => None,
        }
    }

    pub fn service_event<S: ServiceSpec>(&self, service: ServiceType<S>) -> Option<S::Event> {
        match self.unscoped() {
            ActionInput::ServiceEvent {
                service_name,
                payload,
                ..
            } if service_name == service.name => serde_json::from_slice(payload).ok(),
            _ => None,
        }
    }

    pub fn service_start_err<S: ServiceSpec>(
        &self,
        service: ServiceType<S>,
    ) -> Option<S::StartErr> {
        match self.unscoped() {
            ActionInput::ServiceStartFailed {
                service_name,
                payload: Some(payload),
                ..
            } if service_name == service.name => serde_json::from_slice(payload).ok(),
            _ => None,
        }
    }

    pub fn service_start_error_message<S: ServiceSpec>(
        &self,
        service: ServiceType<S>,
    ) -> Option<&str> {
        match self.unscoped() {
            ActionInput::ServiceStartFailed {
                service_name,
                message: Some(message),
                ..
            } if service_name == service.name => Some(message.as_str()),
            _ => None,
        }
    }

    pub fn service_command_ok<S: ServiceSpec>(
        &self,
        service: ServiceType<S>,
    ) -> Option<S::CommandOk> {
        match self.unscoped() {
            ActionInput::ServiceCommandOk {
                service_name,
                payload: Some(payload),
                ..
            } if service_name == service.name => serde_json::from_slice(payload).ok(),
            _ => None,
        }
    }

    pub fn service_command_err<S: ServiceSpec>(
        &self,
        service: ServiceType<S>,
    ) -> Option<S::CommandErr> {
        match self.unscoped() {
            ActionInput::ServiceCommandErr {
                service_name,
                payload: Some(payload),
                ..
            } if service_name == service.name => serde_json::from_slice(payload).ok(),
            _ => None,
        }
    }

    pub fn timer_tick<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        match self.unscoped() {
            ActionInput::TimerTick { payload } => serde_json::from_slice(payload).ok(),
            _ => None,
        }
    }

    pub fn service_slot_key(&self) -> Option<&str> {
        match self.unscoped() {
            ActionInput::ServiceStarted { slot_key, .. }
            | ActionInput::ServiceStartFailed { slot_key, .. }
            | ActionInput::ServiceEvent { slot_key, .. }
            | ActionInput::ServiceStopped { slot_key, .. }
            | ActionInput::ServiceCommandOk { slot_key, .. }
            | ActionInput::ServiceCommandErr { slot_key, .. } => Some(slot_key.as_str()),
            _ => None,
        }
    }

    pub fn service_instance_id(&self) -> Option<u64> {
        match self.unscoped() {
            ActionInput::ServiceStarted { instance_id, .. }
            | ActionInput::ServiceEvent { instance_id, .. }
            | ActionInput::ServiceStopped { instance_id, .. }
            | ActionInput::ServiceCommandOk { instance_id, .. }
            | ActionInput::ServiceCommandErr { instance_id, .. } => Some(*instance_id),
            _ => None,
        }
    }
}
