//! Declarative authoring over Fission's existing widgets, state, dispatch, and effects.
#![forbid(unsafe_code)]
pub extern crate fission_core as core;
extern crate self as fission_compose;
pub use fission_compose_macros::{state_scope, Component};
pub use serde;

pub mod binding;
pub mod events;
pub mod identity;
pub mod scope;

pub use binding::{read, Control, ControlSource, LocalSource, Read, ReadOnly, Source, StateSource};
pub use events::{Event, Press, TextChanged, Toggle};
pub use identity::{BuildSession, Mount, NodeContext, UiKey};
pub use scope::{DefaultScope, Scope};

/// Opt-in contract for editable controls. Composition itself needs no trait:
/// named arguments become ordinary public struct fields, with
/// `..Default::default()` filling the rest. A control implements this only when
/// it can consume a typed two-way state source.
pub trait ComposeBind<S>: Sized {
    fn compose_bind(self, source: S) -> Self;
}

pub mod prelude {
    pub use crate::{
        read, state_scope, Component, ComposeBind, Control, ControlSource, DefaultScope, Event,
        ManySource, Press, Scope, Source, TextChanged, Toggle, UiKey,
    };
}

/// Child-list contract. Ordinary Rust iterators remain usable without a runtime
/// intermediate tree; the collection is the same Vec<Widget> Fission expects.
pub trait ManySource {
    fn children(self) -> Vec<core::Widget>;
}
impl<I, W> ManySource for Read<I>
where
    I: IntoIterator<Item = W>,
    W: Into<core::Widget>,
{
    fn children(self) -> Vec<core::Widget> {
        self.0.into_iter().map(Into::into).collect()
    }
}
#[doc(hidden)]
pub mod view_cx;
