use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::marker::PhantomData;

/// Persistence scope for one stored value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StoreScope {
    /// Shared by the current application installation, origin, or deployment.
    Application,
    /// Owned by the current shell session.
    Session(String),
    /// Owned by an authenticated application user.
    User(String),
    /// Application-defined scope and owner.
    Named { scope: String, owner: String },
}

impl Default for StoreScope {
    fn default() -> Self {
        Self::Application
    }
}

impl StoreScope {
    pub fn session(owner: impl Into<String>) -> Self {
        Self::Session(owner.into())
    }

    pub fn user(owner: impl Into<String>) -> Self {
        Self::User(owner.into())
    }

    pub fn named(scope: impl Into<String>, owner: impl Into<String>) -> Self {
        Self::Named {
            scope: scope.into(),
            owner: owner.into(),
        }
    }

    pub fn parts(&self) -> (&str, &str) {
        match self {
            Self::Application => ("application", ""),
            Self::Session(owner) => ("session", owner),
            Self::User(owner) => ("user", owner),
            Self::Named { scope, owner } => (scope, owner),
        }
    }
}

/// Erased address used by provider requests.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StoreAddress {
    pub scope: StoreScope,
    pub namespace: String,
    pub key: String,
}

impl StoreAddress {
    pub fn new(namespace: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            scope: StoreScope::Application,
            namespace: namespace.into(),
            key: key.into(),
        }
    }

    pub fn in_scope(mut self, scope: StoreScope) -> Self {
        self.scope = scope;
        self
    }
}

/// Typed key for serializable application data.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct StoreKey<T> {
    address: StoreAddress,
    marker: PhantomData<fn() -> T>,
}

impl<T> Clone for StoreKey<T> {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> StoreKey<T> {
    pub fn new(namespace: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            address: StoreAddress::new(namespace, key),
            marker: PhantomData,
        }
    }

    pub fn in_scope(mut self, scope: StoreScope) -> Self {
        self.address.scope = scope;
        self
    }

    pub fn address(&self) -> &StoreAddress {
        &self.address
    }

    pub fn into_address(self) -> StoreAddress {
        self.address
    }
}

/// Opaque bytes stored in Fission's reserved table.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreValue(pub Vec<u8>);

impl StoreValue {
    pub fn encode<T: Serialize>(value: &T) -> Result<Self, StoreError> {
        serde_json::to_vec(value)
            .map(Self)
            .map_err(|error| StoreError::serialization(error.to_string()))
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, StoreError> {
        serde_json::from_slice(&self.0)
            .map_err(|error| StoreError::serialization(error.to_string()))
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for StoreValue {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreGet {
    pub address: StoreAddress,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreContains {
    pub address: StoreAddress,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreSet {
    pub address: StoreAddress,
    pub value: StoreValue,
}

impl StoreSet {
    pub fn typed<T: Serialize>(key: StoreKey<T>, value: &T) -> Result<Self, StoreError> {
        Ok(Self {
            address: key.into_address(),
            value: StoreValue::encode(value)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreRemove {
    pub address: StoreAddress,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreListPrefix {
    pub scope: StoreScope,
    pub namespace: String,
    pub prefix: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreEntry {
    pub address: StoreAddress,
    pub value: StoreValue,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoreBatchOperation {
    Set(StoreSet),
    Remove(StoreRemove),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreBatch {
    operations: Vec<StoreBatchOperation>,
}

impl StoreBatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, request: StoreSet) -> &mut Self {
        self.operations.push(StoreBatchOperation::Set(request));
        self
    }

    pub fn remove(&mut self, request: StoreRemove) -> &mut Self {
        self.operations.push(StoreBatchOperation::Remove(request));
        self
    }

    pub fn with_set(mut self, request: StoreSet) -> Self {
        self.set(request);
        self
    }

    pub fn with_remove(mut self, request: StoreRemove) -> Self {
        self.remove(request);
        self
    }

    pub fn operations(&self) -> &[StoreBatchOperation] {
        &self.operations
    }

    pub fn into_operations(self) -> Vec<StoreBatchOperation> {
        self.operations
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreBatchResult {
    pub sets: u64,
    pub removals: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoreErrorKind {
    Unavailable,
    Busy,
    QuotaExceeded,
    ReadOnly,
    Serialization,
    InvalidRequest,
    Backend,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreError {
    pub kind: StoreErrorKind,
    pub message: String,
}

impl StoreError {
    pub fn new(kind: StoreErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn serialization(message: impl Into<String>) -> Self {
        Self::new(StoreErrorKind::Serialization, message)
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_values_round_trip_without_exposing_the_codec_to_providers() {
        let key = StoreKey::<Vec<String>>::new("settings", "recent");
        let request = StoreSet::typed(key, &vec!["one".into(), "two".into()]).unwrap();
        assert_eq!(
            request.value.decode::<Vec<String>>().unwrap(),
            vec!["one", "two"]
        );
    }

    #[test]
    fn batches_can_be_extended_in_distant_code() {
        fn add_cleanup(batch: &mut StoreBatch) {
            batch.remove(StoreRemove {
                address: StoreAddress::new("cache", "stale"),
            });
        }

        let mut batch = StoreBatch::new();
        batch.set(StoreSet {
            address: StoreAddress::new("cache", "fresh"),
            value: vec![1, 2, 3].into(),
        });
        add_cleanup(&mut batch);
        assert_eq!(batch.operations().len(), 2);
    }
}
