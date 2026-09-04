//! Stable game identities used by snapshots, replay, and scene lowering.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// A deterministic symbol generated from Rust type and field identity.
///
/// Everyday game code receives symbols from derives and field handles rather
/// than manually constructing string object IDs. Expert tooling can inspect the
/// stable text representation for diagnostics and snapshots.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StableSymbol(Arc<str>);

impl StableSymbol {
    /// Creates a framework- or macro-generated symbol.
    ///
    /// Application gameplay APIs should prefer generated typed handles so a
    /// typo cannot create a second identity accidentally.
    #[doc(hidden)]
    pub fn generated(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Portable structural encoding returned by [`StableKey`].
///
/// It deliberately stores values rather than Rust hash output, which is not a
/// stable persistence or replay format.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "value")]
pub enum StableKeyValue {
    U64(u64),
    I64(i64),
    Str(Arc<str>),
    Tuple(Vec<Self>),
}

impl StableKeyValue {
    pub(crate) fn canonical(&self) -> String {
        match self {
            Self::U64(value) => format!("u:{value}"),
            Self::I64(value) => format!("i:{value}"),
            Self::Str(value) => format!("s:{}:{value}", value.len()),
            Self::Tuple(values) => {
                let mut encoded = String::from("t:");
                for value in values {
                    let value = value.canonical();
                    encoded.push_str(&format!("{}:{value}", value.len()));
                }
                encoded
            }
        }
    }
}

/// Converts a durable domain key into a deterministic structural value.
pub trait StableKey: Clone + Eq + std::fmt::Debug + 'static {
    fn stable_key(&self) -> StableKeyValue;
}

macro_rules! unsigned_stable_keys {
    ($($ty:ty),+ $(,)?) => {$(
        impl StableKey for $ty {
            fn stable_key(&self) -> StableKeyValue {
                StableKeyValue::U64(u64::from(*self))
            }
        }
    )+};
}

macro_rules! signed_stable_keys {
    ($($ty:ty),+ $(,)?) => {$(
        impl StableKey for $ty {
            fn stable_key(&self) -> StableKeyValue {
                StableKeyValue::I64(i64::from(*self))
            }
        }
    )+};
}

unsigned_stable_keys!(u8, u16, u32, u64);
signed_stable_keys!(i8, i16, i32, i64);

impl StableKey for String {
    fn stable_key(&self) -> StableKeyValue {
        StableKeyValue::Str(Arc::from(self.as_str()))
    }
}

impl StableKey for Arc<str> {
    fn stable_key(&self) -> StableKeyValue {
        StableKeyValue::Str(self.clone())
    }
}

impl<A, B> StableKey for (A, B)
where
    A: StableKey,
    B: StableKey,
{
    fn stable_key(&self) -> StableKeyValue {
        StableKeyValue::Tuple(vec![self.0.stable_key(), self.1.stable_key()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_keys_preserve_values_without_hashing() {
        assert_eq!(42_u32.stable_key(), StableKeyValue::U64(42));
        assert_eq!((-7_i16).stable_key(), StableKeyValue::I64(-7));
        assert_eq!(
            "object".to_owned().stable_key(),
            StableKeyValue::Str(Arc::from("object"))
        );
        assert_eq!(
            (7_u32, 2_u8).stable_key(),
            StableKeyValue::Tuple(vec![StableKeyValue::U64(7), StableKeyValue::U64(2)])
        );
    }
}
