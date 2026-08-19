//! Side-effect primitives for async operations.
//!
//! Reducers are pure functions -- they must not perform I/O. When a reducer
//! needs to trigger a host capability, async job, or runtime-control effect, it
//! pushes an [`EffectEnvelope`] through the [`Effects`](crate::Effects) builder.
//! The platform executor fulfils the effect outside the deterministic core and
//! dispatches the `on_ok` / `on_err` callback actions back into the pipeline.

use crate::action::{ActionEnvelope, UpdateTextInput};
use crate::async_runtime::{
    JobRef, JobRequestPayload, JobSpec, ResourceExecutionContext, ServiceBindings,
    ServiceCommandPayload, ServiceSpec, ServiceStartPayload, ServiceStopPayload, ServiceType,
};
use crate::capability::CapabilityInvocationPayload;
use crate::capability::{CapabilityType, OperationCapability};
use crate::env::RouteLocation;
use fission_ir::WidgetId;
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

/// Axis selection for runtime scroll positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollAxis {
    /// Adjust vertical scroll offsets.
    Vertical,
    /// Adjust horizontal scroll offsets.
    Horizontal,
    /// Adjust any matching scroll axis.
    Both,
}

/// Desired placement of a target inside a scroll viewport.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ScrollAlignment {
    /// Align the target's leading edge with the viewport's leading edge.
    Start,
    /// Center the target in the viewport.
    Center,
    /// Align the target's trailing edge with the viewport's trailing edge.
    End,
    /// Use the smallest scroll delta that makes the target visible.
    Nearest,
    /// Place the target at a fractional position in the viewport.
    ///
    /// `0.0` behaves like [`ScrollAlignment::Start`], `0.5` centers the target,
    /// and `1.0` behaves like [`ScrollAlignment::End`].
    Fraction(f32),
}

/// Runtime behavior for a scroll request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollBehavior {
    /// Apply the computed offset immediately.
    Instant,
    /// Reserve a smooth-scroll request. Current shells resolve this immediately.
    Smooth,
}

/// Request a post-layout scroll adjustment that reveals a target widget.
///
/// Reducers can emit this as a runtime effect when application state changes.
/// The runtime resolves it after the next layout pass, when target and container
/// rectangles are available, then mutates the scroll state and schedules another
/// frame so paint, hit testing, and semantics see the new offset.
///
/// # Example
///
/// ```rust,ignore
/// ctx.effects.scroll_into_view(ScrollIntoViewRequest {
///     container: Some(WidgetId::explicit("document.canvas.scroll")),
///     target: WidgetId::explicit("document.page.3"),
///     axis: ScrollAxis::Vertical,
///     alignment: ScrollAlignment::Start,
///     padding: [24.0, 24.0, 24.0, 24.0],
///     behavior: ScrollBehavior::Instant,
///     if_needed: false,
/// });
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrollIntoViewRequest {
    /// Explicit scroll container, or `None` to use the nearest matching scroll ancestor.
    pub container: Option<WidgetId>,
    /// Descendant widget that should become visible.
    pub target: WidgetId,
    /// Axis to scroll.
    pub axis: ScrollAxis,
    /// Alignment to use when computing the new offset.
    pub alignment: ScrollAlignment,
    /// Reveal margin as `[left, right, top, bottom]`.
    pub padding: [f32; 4],
    /// Whether to jump immediately or request smooth behavior.
    pub behavior: ScrollBehavior,
    /// If `true`, leave the offset unchanged when the target is already fully visible.
    pub if_needed: bool,
}

/// Runtime-managed effects that are not host capabilities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuntimeEffect {
    /// Cancel a previously issued effect by its request id.
    Cancel { req_id: u64 },
    /// Release a platform-managed resource.
    ReleaseResource { resource_id: u64 },
    /// Reveal a widget inside a scroll container after the next layout pass.
    ScrollIntoView(ScrollIntoViewRequest),
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ActionInput {
    /// No extra input.
    None,
    /// The host shell delivered a route/navigation change.
    RouteChanged { location: RouteLocation },
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
    /// Runtime details accompanying a text-input action.
    ///
    /// The action envelope retains the application-defined payload, while this
    /// input carries the edited value and selection independently.
    TextChanged(UpdateTextInput),
    /// External file drop (e.g. from the OS file manager).
    Drop {
        paths: Vec<String>,
        x: f32,
        y: f32,
        /// Modifier bitmask active during the drop (Shift=1, Alt=2,
        /// Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// Internal drag-and-drop with an opaque byte payload.
    InternalDrop {
        payload: Vec<u8>,
        x: f32,
        y: f32,
        /// Modifier bitmask active during the drop (Shift=1, Alt=2,
        /// Ctrl=4, Super=8).
        modifiers: u8,
    },
    /// The action was dispatched from a subtree with a raw action scope.
    ScopedRaw {
        scope_id: u128,
        target: WidgetId,
        input: Box<ActionInput>,
    },
}

impl ActionInput {
    /// Encodes this runtime input into an opaque representation suitable for
    /// storage or transport.
    pub fn encode_opaque(&self) -> Result<Vec<u8>, ActionInputCodecError> {
        serde_json::to_vec(self).map_err(ActionInputCodecError)
    }

    /// Decodes an input previously produced by [`Self::encode_opaque`].
    pub fn decode_opaque(bytes: &[u8]) -> Result<Self, ActionInputCodecError> {
        serde_json::from_slice(bytes).map_err(ActionInputCodecError)
    }

    pub fn scoped_raw(scope_id: u128, target: WidgetId, input: ActionInput) -> Self {
        Self::ScopedRaw {
            scope_id,
            target: target.into(),
            input: Box::new(input),
        }
    }

    pub fn action_scope_id(&self) -> Option<u128> {
        match self {
            ActionInput::ScopedRaw { scope_id, .. } => Some(*scope_id),
            _ => None,
        }
    }

    pub fn scoped_target(&self) -> Option<WidgetId> {
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

    /// Returns the edit accompanying a text-input action.
    ///
    /// Scoped actions are unwrapped automatically, matching the other typed
    /// input accessors.
    pub fn text_change(&self) -> Option<&UpdateTextInput> {
        match self.unscoped() {
            ActionInput::TextChanged(change) => Some(change),
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

    /// Modifier bitmask active during a drop action.
    ///
    /// This lets app reducers choose copy/move/link semantics without binding
    /// that product rule into the drag runtime itself.
    pub fn as_drop_modifiers(&self) -> Option<u8> {
        match self.unscoped() {
            ActionInput::Drop { modifiers, .. } => Some(*modifiers),
            ActionInput::InternalDrop { modifiers, .. } => Some(*modifiers),
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

/// Failure to encode or decode an opaque [`ActionInput`] representation.
#[derive(Debug)]
pub struct ActionInputCodecError(serde_json::Error);

impl std::fmt::Display for ActionInputCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("action input codec failed")
    }
}

impl std::error::Error for ActionInputCodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[cfg(test)]
mod action_input_codec_tests {
    use super::*;

    #[test]
    fn opaque_codec_round_trips_full_width_scope_ids() {
        let input = ActionInput::scoped_raw(
            u128::MAX - 1,
            WidgetId::from_u128(u128::MAX - 2),
            ActionInput::TextChanged(UpdateTextInput {
                node_id: WidgetId::from_u128(7),
                new_text: "hello".into(),
                new_caret: 4,
                new_anchor: 1,
            }),
        );
        let bytes = input.encode_opaque().expect("input should encode");
        let decoded = ActionInput::decode_opaque(&bytes).expect("input should decode");
        assert_eq!(decoded, input);
    }
}
