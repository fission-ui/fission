//! Reducer context and effect builder.
//!
//! When a reducer needs to emit side-effects or inspect the [`ActionInput`]
//! that triggered it, it receives a [`ReducerContext`]. The context provides
//! an [`Effects`] builder for issuing system effects (HTTP, file I/O, alerts)
//! and binding callback actions.

use crate::action::{Action, ActionEnvelope, AppState};
use crate::effect::{ActionInput, Effect, EffectEnvelope, SystemEffect};
use crate::registry::{ActionRegistry, IntoHandler};
use std::collections::HashMap;
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
///     // Issue an HTTP GET effect
///     ctx.effects.http_get("https://api.example.com/clicked");
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
///     ctx.effects.http_get("https://api.example.com/save")
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

    pub fn add(&mut self, effect: SystemEffect) -> u64 {
        let req_id = self.next_req_id;
        self.next_req_id += 1;

        self.out.push(EffectEnvelope {
            req_id,
            effect: Effect::System(effect),
            on_ok: None,
            on_err: None,
        });
        req_id
    }

    pub fn system_effect(&mut self, effect: SystemEffect) -> EffectBuilder<'_, 'a, S> {
        let req_id = self.next_req_id;
        self.next_req_id += 1;

        let index = self.out.len();
        self.out.push(EffectEnvelope {
            req_id,
            effect: Effect::System(effect),
            on_ok: None,
            on_err: None,
        });

        EffectBuilder {
            effects: self,
            index,
        }
    }

    pub fn http_get(&mut self, url: impl Into<String>) -> EffectBuilder<'_, 'a, S> {
        self.system_effect(SystemEffect::HttpGet {
            url: url.into(),
            headers: HashMap::new(),
        })
    }

    pub fn file_read(&mut self, path: impl Into<String>) -> EffectBuilder<'_, 'a, S> {
        self.system_effect(SystemEffect::FileRead { path: path.into() })
    }

    pub fn cancel(&mut self, req_id: u64) {
        self.system_effect(SystemEffect::Cancel { req_id });
    }

    pub fn release_resource(&mut self, resource_id: u64) {
        self.system_effect(SystemEffect::ReleaseResource { resource_id });
    }
}

/// Fluent builder returned by [`Effects::system_effect`], [`Effects::http_get`],
/// and [`Effects::file_read`].
///
/// Attach `on_ok` and `on_err` callback envelopes before the builder is dropped.
///
/// # Example
///
/// ```rust,ignore
/// ctx.effects.http_get("https://api.example.com")
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
