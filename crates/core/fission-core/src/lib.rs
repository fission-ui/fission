//! # fission-core
//!
//! The runtime, widget system, and action/reducer architecture for the Fission UI
//! framework.
//!
//! `fission-core` provides:
//!
//! - A **declarative widget tree** built from composable primitives ([`Node`], [`Widget`]).
//! - A **unidirectional data-flow** pipeline: [`Action`] -> [`Runtime::dispatch`] -> reducer
//!   -> mutated [`AppState`].
//! - An **effect system** for async side-effects ([`Effect`], [`SystemEffect`]).
//! - Built-in widgets: [`Button`], [`Text`], [`TextInput`], [`Container`], [`Row`],
//!   [`Column`], [`Scroll`], [`ZStack`], [`Grid`], [`LazyColumn`], and more.
//!
//! ## Getting started
//!
//! ```rust,ignore
//! use fission_core::*;
//! use fission_core::ui::*;
//!
//! // Define application state
//! #[derive(Debug, Default)]
//! struct MyState { value: String }
//! impl AppState for MyState {}
//!
//! // Build a widget
//! struct MyWidget;
//! impl Widget<MyState> for MyWidget {
//!     fn build(&self, ctx: &mut BuildCtx<MyState>, view: &View<MyState>) -> Node {
//!         Text::new(&*view.state.value).into_node()
//!     }
//! }
//! ```

use anyhow::Result;
use lazy_static::lazy_static;
use std::any::TypeId;
use std::collections::HashMap;

extern crate self as fission_core;

pub mod action;
pub mod context; // New
pub mod diff;
pub mod effect; // New
pub mod env;
pub mod event;
pub mod hit_test;
pub mod input;
pub mod lowering;
pub mod media;
pub mod registry;
pub mod runtime;
pub mod time;
pub mod ui;

pub mod view;

#[cfg(test)]
mod tests;

pub use action::{Action, ActionEnvelope, ActionId, AppState};
pub use context::{Effects, ReducerContext}; // New
pub use effect::{ActionInput, Effect, EffectEnvelope, EffectPayload, SystemEffect}; // New
pub use env::{Clipboard, Env, ImeHandler, InteractionStateMap, RuntimeState, ScrollStateMap};
pub use runtime::Runtime;

pub use event::{InputEvent, KeyCode, KeyEvent, LifecycleEvent, PointerButton, PointerEvent};
pub use fission_ir::op;
pub use fission_ir::{EmbedKind, NodeId, Op, WidgetNodeId};
pub use fission_layout::{
    BoxConstraints, FlexDirection, LayoutEngine, LayoutOp, LayoutPoint, LayoutRect, LayoutSize,
    LayoutSnapshot, LayoutUnit, TextMeasurer,
};
pub use lowering::{LoweringContext, NodeBuilder};
pub use registry::{
    ActionRegistry, AnimationPropertyId, AnimationRequest, AnimationStartValue, BuildCtx,
    EasingFunction, Handler, PortalLayer, VideoRegistration,
};
pub use time::{Clock, CurrentTime};
pub use ui::{
    Builder, Button, Column, CustomEventResult, CustomHitResult, CustomNode, CustomRenderObject,
    LayoutBuilder, Lower, LowerDyn, Node, Row, Text,
};
pub use view::{Selector, View, Widget};

/// A frame-tick action that advances the runtime clock by a delta.
///
/// The platform shell dispatches `Tick` once per frame so that animations,
/// timers, and other time-dependent logic can progress.
///
/// # Example
///
/// ```rust,ignore
/// // Advance the runtime by 16 ms (~60 fps)
/// runtime.tick(16)?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tick {
    /// Delta time in milliseconds since the last tick.
    pub dt: CurrentTime,
}

impl Action for Tick {
    fn static_id() -> ActionId {
        *TICK_ACTION_ID
    }
}

lazy_static! {
    pub static ref TICK_ACTION_ID: ActionId = ActionId::from_name("fission_core::Tick");
}

/// An action that sets the runtime clock to an absolute timestamp.
///
/// Unlike [`Tick`] which advances by a delta, `AdvanceTo` jumps directly to
/// the given time. Useful for testing and deterministic replay.
///
/// # Example
///
/// ```rust,ignore
/// let envelope: ActionEnvelope = AdvanceTo { time: 5000 }.into();
/// runtime.dispatch(envelope, NodeId::derived(0, &[0]))?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AdvanceTo {
    /// The absolute time (in milliseconds) to set the clock to.
    pub time: CurrentTime,
}

impl Action for AdvanceTo {
    fn static_id() -> ActionId {
        *ADVANCE_TO_ACTION_ID
    }
}

lazy_static! {
    pub static ref ADVANCE_TO_ACTION_ID: ActionId = ActionId::from_name("fission_core::AdvanceTo");
}

/// A type-erased reducer function stored in the [`Runtime`].
///
/// `BoxedReducer` is the internal representation used by the runtime to invoke
/// reducers without knowing the concrete `AppState` or `Action` types. You
/// rarely need to interact with this type directly -- use [`BuildCtx::bind`] or
/// [`ActionRegistry::register`] instead.
pub type BoxedReducer = Box<
    dyn FnMut(
            &mut HashMap<TypeId, Box<dyn AppState>>,
            &ActionEnvelope,
            NodeId,
            &mut Vec<EffectEnvelope>,
            &ActionInput,
        ) -> Result<()>
        + Send
        + Sync,
>;
