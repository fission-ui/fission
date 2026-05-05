//! Side-effect primitives for async operations.
//!
//! Reducers are pure functions -- they must not perform I/O. When a reducer
//! needs to trigger an HTTP request, read a file, or show a system alert, it
//! pushes an [`EffectEnvelope`] through the [`Effects`](crate::Effects) builder.
//! The platform executor fulfils the effect outside the deterministic core and
//! dispatches the `on_ok` / `on_err` callback actions back into the pipeline.

use crate::action::ActionEnvelope;
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
/// Use [`SystemEffect::ReleaseResource`] to free them when no longer needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub u64);

use std::collections::HashMap;

/// Built-in system effects that every platform executor must handle.
///
/// These cover the most common async operations an application needs.
/// For app-specific effects, use [`Effect::App`] with an opaque byte payload.
///
/// # Example
///
/// ```rust,ignore
/// fn fetch_data(state: &mut MyState, _action: FetchTodos, ctx: &mut ReducerContext<MyState>) {
///     ctx.effects.http_get("https://api.example.com/todos")
///         .on_ok(ctx.effects.bind(TodosLoaded, handle_loaded as fn(&mut MyState, TodosLoaded)));
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemEffect {
    /// Show a native alert dialog with a title and message.
    Alert { title: String, message: String },
    /// Perform an HTTP GET request.
    HttpGet {
        url: String,
        headers: HashMap<String, String>,
    },
    /// Read a file from the local filesystem.
    FileRead { path: String },
    /// Cancel a previously issued effect by its request id.
    Cancel { req_id: u64 },
    /// Release a platform-managed resource.
    ReleaseResource { resource_id: u64 },
    /// Open a URL in the system browser or an in-app browser sheet.
    ///
    /// When `in_app` is `true`, the URL opens in a Custom Tab /
    /// SFSafariViewController overlay. When `false`, the URL opens in the
    /// external browser app.
    OpenUrl { url: String, in_app: bool },
    /// Initiate an OAuth / secure authentication session.
    ///
    /// The platform opens the `url` and listens for a redirect matching
    /// `callback_scheme`. The redirect URL is delivered as the effect result.
    Authenticate {
        url: String,
        callback_scheme: String,
    },
}

/// A side-effect emitted by a reducer.
///
/// `System` variants are handled by the platform executor.
/// `App` carries an opaque byte payload for application-defined effects
/// (e.g. database writes, Bluetooth commands).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    /// A built-in system effect (HTTP, file I/O, alerts, etc.).
    System(SystemEffect),
    /// An application-defined effect with an opaque byte payload.
    App(Vec<u8>),
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
/// ctx.effects.http_get("https://example.com/api")
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
}

/// The payload delivered when an effect completes.
///
/// Small results are inlined as bytes; large results reference a
/// platform-managed [`ResourceId`].
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EffectPayload {
    /// The result data, serialised inline.
    InlineBytes(Vec<u8>),
    /// A handle to a platform-managed resource (avoids copying large blobs).
    Resource(u64),
    /// The effect produced no result data.
    Empty,
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
    /// The effect completed successfully.
    EffectOk { req_id: u64, payload: EffectPayload },
    /// The effect failed with an error message.
    EffectErr { req_id: u64, message: String },
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
}

impl ActionInput {
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            ActionInput::EffectOk {
                payload: EffectPayload::InlineBytes(b),
                ..
            } => Some(b),
            ActionInput::InternalDrop { payload, .. } => Some(payload),
            _ => None,
        }
    }

    pub fn as_pointer(&self) -> Option<(f32, f32, f32, f32)> {
        match self {
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
        match self {
            ActionInput::Drop { paths, .. } => Some(paths),
            _ => None,
        }
    }

    pub fn as_internal_drop(&self) -> Option<&[u8]> {
        match self {
            ActionInput::InternalDrop { payload, .. } => Some(payload),
            _ => None,
        }
    }
}
