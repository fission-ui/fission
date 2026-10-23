use crate::core::{self, Action, ActionId, ActionInput, GlobalState, ReducerContext};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fmt::Debug, marker::PhantomData};

/// Event type information is erased only when assigning the final Fission property.
#[derive(Debug)]
pub struct Event<E> {
    envelope: core::ActionEnvelope,
    _event: PhantomData<fn() -> E>,
}
impl<E> Clone for Event<E> {
    fn clone(&self) -> Self {
        Self::new(self.envelope.clone())
    }
}
impl<E> Event<E> {
    pub fn new(envelope: core::ActionEnvelope) -> Self {
        Self {
            envelope,
            _event: PhantomData,
        }
    }
    pub fn envelope(&self) -> &core::ActionEnvelope {
        &self.envelope
    }
    pub fn into_envelope(self) -> core::ActionEnvelope {
        self.envelope
    }
}
impl<E> From<core::ActionEnvelope> for Event<E> {
    fn from(envelope: core::ActionEnvelope) -> Self {
        Self::new(envelope)
    }
}
impl<E> From<Event<E>> for core::ActionEnvelope {
    fn from(event: Event<E>) -> Self {
        event.into_envelope()
    }
}
#[derive(Debug, Clone)]
pub struct Press;
#[derive(Debug, Clone)]
pub struct TextChanged;
#[derive(Debug, Clone)]
pub struct Toggle;

/// Synchronous handler adapter. Captured values are owned dispatch metadata,
/// never build-time references into application state.
pub struct HandlerFn<F>(pub F);
impl<R, A, F> core::registry::IntoHandler<R, A> for HandlerFn<F>
where
    R: GlobalState,
    F: for<'a, 'b, 'c> Fn(&mut R, A, &mut ReducerContext<'a, 'b, 'c, R>),
{
    fn call<'a, 'b, 'c>(&self, state: &mut R, action: A, cx: &mut ReducerContext<'a, 'b, 'c, R>) {
        (self.0)(state, action, cx)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldAction<K> {
    pub key: K,
    pub incarnation: u64,
}
impl<K> Action for FieldAction<K>
where
    K: Debug + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    fn static_id() -> ActionId {
        ActionId::from_name(std::any::type_name::<Self>())
    }
}

/// Editing policies are the control contract, not a rule inferred from the
/// spelling of a property. An adapter selects the policy when it requests a binding.
pub trait EditEvent<T>: Send + Sync + 'static {
    type Edit: Clone + Default + Send + Sync + 'static;
    const RETAIN_EDIT: bool = false;
    fn initial(value: &T) -> Self::Edit;
    fn update(current: &T, edit: &mut Self::Edit, input: &ActionInput) -> Option<T>;
}
impl EditEvent<String> for TextChanged {
    type Edit = ();
    fn initial(_: &String) {}
    fn update(_: &String, _: &mut (), input: &ActionInput) -> Option<String> {
        input.text_change().map(|edit| edit.new_text.clone())
    }
}
impl EditEvent<bool> for Toggle {
    type Edit = ();
    fn initial(_: &bool) {}
    fn update(current: &bool, _: &mut (), input: &ActionInput) -> Option<bool> {
        // Core Switch emits a toggle intent, NOT a precomputed new checked value.
        if input.text_change().is_some() {
            None
        } else {
            Some(!*current)
        }
    }
}
