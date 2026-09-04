//! Typed handles generated from authoritative game-state fields.

use std::fmt;
use std::marker::PhantomData;

use crate::{StableKey, StableKeyValue, StableSymbol};

/// Typed read handle for an ordinary authoritative state field.
#[derive(Clone)]
pub struct FieldHandle<S, T> {
    symbol: StableSymbol,
    get: fn(&S) -> &T,
}

impl<S, T> FieldHandle<S, T> {
    #[doc(hidden)]
    pub fn generated(symbol: &'static str, get: fn(&S) -> &T) -> Self {
        Self {
            symbol: StableSymbol::generated(symbol),
            get,
        }
    }

    pub fn symbol(&self) -> &StableSymbol {
        &self.symbol
    }

    pub fn get<'a>(&self, state: &'a S) -> &'a T {
        (self.get)(state)
    }
}

impl<S, T> fmt::Debug for FieldHandle<S, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("FieldHandle")
            .field(&self.symbol)
            .finish()
    }
}

/// Typed handle for one game object field.
#[derive(Clone)]
pub struct ObjectHandle<S, T>(FieldHandle<S, T>);

impl<S, T> ObjectHandle<S, T> {
    #[doc(hidden)]
    pub fn generated(symbol: &'static str, get: fn(&S) -> &T) -> Self {
        Self(FieldHandle::generated(symbol, get))
    }

    pub fn symbol(&self) -> &StableSymbol {
        self.0.symbol()
    }

    pub fn get<'a>(&self, state: &'a S) -> &'a T {
        self.0.get(state)
    }
}

impl<S, T> fmt::Debug for ObjectHandle<S, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ObjectHandle")
            .field(self.symbol())
            .finish()
    }
}

/// Typed handle for a bounded game-area field.
#[derive(Clone)]
pub struct AreaHandle<S, T>(FieldHandle<S, T>);

impl<S, T> AreaHandle<S, T> {
    #[doc(hidden)]
    pub fn generated(symbol: &'static str, get: fn(&S) -> &T) -> Self {
        Self(FieldHandle::generated(symbol, get))
    }

    pub fn symbol(&self) -> &StableSymbol {
        self.0.symbol()
    }

    pub fn get<'a>(&self, state: &'a S) -> &'a T {
        self.0.get(state)
    }
}

impl<S, T> fmt::Debug for AreaHandle<S, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AreaHandle")
            .field(self.symbol())
            .finish()
    }
}

/// Typed handle for a repeated object collection with durable item keys.
#[derive(Clone)]
pub struct ObjectGroupHandle<S, T> {
    symbol: StableSymbol,
    get: fn(&S) -> &[T],
    key: fn(&T) -> StableKeyValue,
    _state: PhantomData<fn() -> S>,
}

impl<S, T> ObjectGroupHandle<S, T> {
    #[doc(hidden)]
    pub fn generated(
        symbol: &'static str,
        get: fn(&S) -> &[T],
        key: fn(&T) -> StableKeyValue,
    ) -> Self {
        Self {
            symbol: StableSymbol::generated(symbol),
            get,
            key,
            _state: PhantomData,
        }
    }

    pub fn symbol(&self) -> &StableSymbol {
        &self.symbol
    }

    pub fn get<'a>(&self, state: &'a S) -> &'a [T] {
        (self.get)(state)
    }

    pub fn key(&self, item: &T) -> StableKeyValue {
        (self.key)(item)
    }

    /// Finds one item without making collection position its identity.
    pub fn find<'a>(&self, state: &'a S, key: &impl StableKey) -> Option<&'a T> {
        let key = key.stable_key();
        self.get(state).iter().find(|item| (self.key)(item) == key)
    }
}

impl<S, T> fmt::Debug for ObjectGroupHandle<S, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ObjectGroupHandle")
            .field(&self.symbol)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Item {
        id: u32,
    }

    #[derive(Clone)]
    struct State {
        score: u32,
        items: Vec<Item>,
    }

    #[test]
    fn handles_read_authoritative_fields_and_find_items_by_durable_key() {
        let state = State {
            score: 4,
            items: vec![Item { id: 8 }, Item { id: 13 }],
        };
        let score = FieldHandle::generated("State.score", |state: &State| &state.score);
        let items = ObjectGroupHandle::generated(
            "State.items",
            |state: &State| state.items.as_slice(),
            |item: &Item| item.id.stable_key(),
        );

        assert_eq!(*score.get(&state), 4);
        assert_eq!(items.find(&state, &13_u32).map(|item| item.id), Some(13));
        assert_eq!(items.key(&state.items[0]), StableKeyValue::U64(8));
    }
}
