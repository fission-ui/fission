use std::collections::btree_map::{Entry, Iter};
use std::collections::BTreeMap;
use std::fmt;

use fission_ir::WidgetId;
use serde::de;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::frame::ResourceEpoch;

/// Stable logical identity assigned to a resource by Fission's resource layer.
///
/// The identity survives backend replacement. It must never be a backend-native
/// object or allocation address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub u64);

/// Stable identity for one immutable version of resource content.
///
/// Fission's resource layer chooses the identity scheme (for example a digest
/// or versioned asset identity). Rendering backends treat it as opaque and may
/// use it to determine whether decoded data can be reused.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ResourceContentIdentity(String);

impl ResourceContentIdentity {
    pub fn try_new(value: impl Into<String>) -> Result<Self, InvalidResourceContentIdentity> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(InvalidResourceContentIdentity);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ResourceContentIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidResourceContentIdentity;

impl fmt::Display for InvalidResourceContentIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("resource content identity must not be empty")
    }
}

impl std::error::Error for InvalidResourceContentIdentity {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceKind {
    Image,
    Svg,
    Font,
    Text,
    Binary,
    Custom(String),
}

/// Where upstream resource code obtained the source material.
///
/// This is diagnostic provenance only. The rendering contract never opens the
/// locator or performs I/O.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceSource {
    Embedded,
    Asset,
    File,
    Network,
    Memory,
    Generated,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceProvenance {
    pub source: ResourceSource,
    /// Human-readable asset key, path, URL, or generator label. Backends must
    /// not interpret this as a fetch instruction.
    pub locator: Option<String>,
    pub requested_by: Option<WidgetId>,
}

impl ResourceProvenance {
    pub fn new(source: ResourceSource) -> Self {
        Self {
            source,
            locator: None,
            requested_by: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourcePayload {
    Bytes(Vec<u8>),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceStatus {
    Loading,
    Ready,
    Failed,
    Invalidated,
}

/// Structured upstream failure retained for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceFailure {
    code: String,
    message: String,
    retryable: bool,
}

impl ResourceFailure {
    pub fn try_new(
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Result<Self, InvalidResourceFailure> {
        let failure = Self {
            code: code.into(),
            message: message.into(),
            retryable,
        };
        failure.validate()?;
        Ok(failure)
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }

    fn validate(&self) -> Result<(), InvalidResourceFailure> {
        if self.code.trim().is_empty() {
            return Err(InvalidResourceFailure::EmptyCode);
        }
        if self.message.trim().is_empty() {
            return Err(InvalidResourceFailure::EmptyMessage);
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for ResourceFailure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedFailure {
            code: String,
            message: String,
            retryable: bool,
        }

        let failure = SerializedFailure::deserialize(deserializer)?;
        Self::try_new(failure.code, failure.message, failure.retryable).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidResourceFailure {
    EmptyCode,
    EmptyMessage,
}

impl fmt::Display for InvalidResourceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCode => formatter.write_str("resource failure code must not be empty"),
            Self::EmptyMessage => formatter.write_str("resource failure message must not be empty"),
        }
    }
}

impl std::error::Error for InvalidResourceFailure {}

/// Immutable state for one logical resource in a frame snapshot.
///
/// State, payload, and failure fields are private so a ready entry always owns
/// source content and a failed entry always owns diagnostic information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceEntry {
    id: ResourceId,
    content_identity: ResourceContentIdentity,
    kind: ResourceKind,
    provenance: ResourceProvenance,
    status: ResourceStatus,
    payload: Option<ResourcePayload>,
    failure: Option<ResourceFailure>,
}

impl ResourceEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        id: ResourceId,
        content_identity: ResourceContentIdentity,
        kind: ResourceKind,
        provenance: ResourceProvenance,
        status: ResourceStatus,
        payload: Option<ResourcePayload>,
        failure: Option<ResourceFailure>,
    ) -> Result<Self, ResourceEntryError> {
        let entry = Self {
            id,
            content_identity,
            kind,
            provenance,
            status,
            payload,
            failure,
        };
        entry.validate()?;
        Ok(entry)
    }

    pub fn loading(
        id: ResourceId,
        content_identity: ResourceContentIdentity,
        kind: ResourceKind,
        provenance: ResourceProvenance,
    ) -> Self {
        Self::try_new(
            id,
            content_identity,
            kind,
            provenance,
            ResourceStatus::Loading,
            None,
            None,
        )
        .expect("loading resource constructor creates a valid state")
    }

    pub fn ready(
        id: ResourceId,
        content_identity: ResourceContentIdentity,
        kind: ResourceKind,
        provenance: ResourceProvenance,
        payload: ResourcePayload,
    ) -> Self {
        Self::try_new(
            id,
            content_identity,
            kind,
            provenance,
            ResourceStatus::Ready,
            Some(payload),
            None,
        )
        .expect("ready resource constructor creates a valid state")
    }

    pub fn failed(
        id: ResourceId,
        content_identity: ResourceContentIdentity,
        kind: ResourceKind,
        provenance: ResourceProvenance,
        failure: ResourceFailure,
    ) -> Self {
        Self::try_new(
            id,
            content_identity,
            kind,
            provenance,
            ResourceStatus::Failed,
            None,
            Some(failure),
        )
        .expect("failed resource constructor creates a valid state")
    }

    pub fn invalidated(
        id: ResourceId,
        content_identity: ResourceContentIdentity,
        kind: ResourceKind,
        provenance: ResourceProvenance,
    ) -> Self {
        Self::try_new(
            id,
            content_identity,
            kind,
            provenance,
            ResourceStatus::Invalidated,
            None,
            None,
        )
        .expect("invalidated resource constructor creates a valid state")
    }

    pub fn id(&self) -> ResourceId {
        self.id
    }

    pub fn content_identity(&self) -> &ResourceContentIdentity {
        &self.content_identity
    }

    pub fn kind(&self) -> &ResourceKind {
        &self.kind
    }

    pub fn provenance(&self) -> &ResourceProvenance {
        &self.provenance
    }

    pub fn status(&self) -> ResourceStatus {
        self.status
    }

    pub fn payload(&self) -> Option<&ResourcePayload> {
        self.payload.as_ref()
    }

    pub fn failure(&self) -> Option<&ResourceFailure> {
        self.failure.as_ref()
    }

    fn validate(&self) -> Result<(), ResourceEntryError> {
        let has_payload = self.payload.is_some();
        let has_failure = self.failure.is_some();
        let valid = match self.status {
            ResourceStatus::Loading | ResourceStatus::Invalidated => !has_payload && !has_failure,
            ResourceStatus::Ready => has_payload && !has_failure,
            ResourceStatus::Failed => !has_payload && has_failure,
        };

        if !valid {
            return Err(ResourceEntryError::InvalidState {
                status: self.status,
                has_payload,
                has_failure,
            });
        }
        if let Some(failure) = &self.failure {
            failure
                .validate()
                .map_err(ResourceEntryError::InvalidFailure)?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for ResourceEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedEntry {
            id: ResourceId,
            content_identity: ResourceContentIdentity,
            kind: ResourceKind,
            provenance: ResourceProvenance,
            status: ResourceStatus,
            payload: Option<ResourcePayload>,
            failure: Option<ResourceFailure>,
        }

        let entry = SerializedEntry::deserialize(deserializer)?;
        Self::try_new(
            entry.id,
            entry.content_identity,
            entry.kind,
            entry.provenance,
            entry.status,
            entry.payload,
            entry.failure,
        )
        .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceEntryError {
    InvalidState {
        status: ResourceStatus,
        has_payload: bool,
        has_failure: bool,
    },
    InvalidFailure(InvalidResourceFailure),
}

impl fmt::Display for ResourceEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState {
                status,
                has_payload,
                has_failure,
            } => write!(
                formatter,
                "invalid {status:?} resource state (payload: {has_payload}, failure: {has_failure})"
            ),
            Self::InvalidFailure(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ResourceEntryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidState { .. } => None,
            Self::InvalidFailure(error) => Some(error),
        }
    }
}

/// Immutable, deterministically indexed resource state captured for one frame.
///
/// This is neither a cache nor a fetcher. It contains only source content and
/// readiness state already resolved by the upstream resource authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceSnapshot {
    epoch: ResourceEpoch,
    entries: BTreeMap<ResourceId, ResourceEntry>,
}

impl ResourceSnapshot {
    pub fn empty(epoch: ResourceEpoch) -> Self {
        Self {
            epoch,
            entries: BTreeMap::new(),
        }
    }

    pub fn try_new(
        epoch: ResourceEpoch,
        entries: impl IntoIterator<Item = ResourceEntry>,
    ) -> Result<Self, ResourceSnapshotError> {
        let mut indexed = BTreeMap::new();
        for entry in entries {
            entry
                .validate()
                .map_err(|error| ResourceSnapshotError::InvalidEntry {
                    resource_id: entry.id,
                    error,
                })?;
            match indexed.entry(entry.id) {
                Entry::Vacant(slot) => {
                    slot.insert(entry);
                }
                Entry::Occupied(_) => {
                    return Err(ResourceSnapshotError::DuplicateId(entry.id));
                }
            }
        }
        Ok(Self {
            epoch,
            entries: indexed,
        })
    }

    pub fn epoch(&self) -> ResourceEpoch {
        self.epoch
    }

    pub fn get(&self, id: ResourceId) -> Option<&ResourceEntry> {
        self.entries.get(&id)
    }

    pub fn contains(&self, id: ResourceId) -> bool {
        self.entries.contains_key(&id)
    }

    pub fn iter(&self) -> Iter<'_, ResourceId, ResourceEntry> {
        self.entries.iter()
    }

    pub fn entries_with_content_identity<'a>(
        &'a self,
        content_identity: &'a ResourceContentIdentity,
    ) -> impl Iterator<Item = &'a ResourceEntry> + 'a {
        self.entries
            .values()
            .filter(move |entry| entry.content_identity() == content_identity)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn validate(&self) -> Result<(), ResourceSnapshotError> {
        for (indexed_id, entry) in &self.entries {
            if *indexed_id != entry.id {
                return Err(ResourceSnapshotError::MismatchedIndex {
                    indexed_as: *indexed_id,
                    resource_id: entry.id,
                });
            }
            entry
                .validate()
                .map_err(|error| ResourceSnapshotError::InvalidEntry {
                    resource_id: entry.id,
                    error,
                })?;
        }
        Ok(())
    }
}

impl<'a> IntoIterator for &'a ResourceSnapshot {
    type Item = (&'a ResourceId, &'a ResourceEntry);
    type IntoIter = Iter<'a, ResourceId, ResourceEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Serialize for ResourceSnapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut snapshot = serializer.serialize_struct("ResourceSnapshot", 2)?;
        snapshot.serialize_field("epoch", &self.epoch)?;
        snapshot.serialize_field("entries", &self.entries.values().collect::<Vec<_>>())?;
        snapshot.end()
    }
}

impl<'de> Deserialize<'de> for ResourceSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedSnapshot {
            epoch: ResourceEpoch,
            entries: Vec<ResourceEntry>,
        }

        let snapshot = SerializedSnapshot::deserialize(deserializer)?;
        Self::try_new(snapshot.epoch, snapshot.entries).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceSnapshotError {
    DuplicateId(ResourceId),
    MismatchedIndex {
        indexed_as: ResourceId,
        resource_id: ResourceId,
    },
    InvalidEntry {
        resource_id: ResourceId,
        error: ResourceEntryError,
    },
}

impl fmt::Display for ResourceSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(resource_id) => {
                write!(
                    formatter,
                    "resource {} appears more than once",
                    resource_id.0
                )
            }
            Self::MismatchedIndex {
                indexed_as,
                resource_id,
            } => write!(
                formatter,
                "resource {} is indexed as {}",
                resource_id.0, indexed_as.0
            ),
            Self::InvalidEntry { resource_id, error } => {
                write!(formatter, "resource {} is invalid: {error}", resource_id.0)
            }
        }
    }
}

impl std::error::Error for ResourceSnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidEntry { error, .. } => Some(error),
            Self::DuplicateId(_) | Self::MismatchedIndex { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(value: &str) -> ResourceContentIdentity {
        ResourceContentIdentity::try_new(value).unwrap()
    }

    fn provenance() -> ResourceProvenance {
        ResourceProvenance::new(ResourceSource::Memory)
    }

    fn loading(id: u64, content: &str) -> ResourceEntry {
        ResourceEntry::loading(
            ResourceId(id),
            identity(content),
            ResourceKind::Binary,
            provenance(),
        )
    }

    #[test]
    fn snapshot_lookup_and_iteration_are_deterministic() {
        let snapshot = ResourceSnapshot::try_new(
            ResourceEpoch(4),
            [loading(9, "nine"), loading(2, "two"), loading(7, "seven")],
        )
        .unwrap();

        assert_eq!(
            snapshot.iter().map(|(id, _)| id.0).collect::<Vec<_>>(),
            vec![2, 7, 9]
        );
        assert_eq!(
            snapshot
                .get(ResourceId(7))
                .unwrap()
                .content_identity()
                .as_str(),
            "seven"
        );
        assert!(snapshot.get(ResourceId(8)).is_none());
    }

    #[test]
    fn snapshot_serialization_is_independent_of_insertion_order() {
        let ascending = ResourceSnapshot::try_new(
            ResourceEpoch(4),
            [loading(2, "two"), loading(7, "seven"), loading(9, "nine")],
        )
        .unwrap();
        let shuffled = ResourceSnapshot::try_new(
            ResourceEpoch(4),
            [loading(9, "nine"), loading(2, "two"), loading(7, "seven")],
        )
        .unwrap();

        assert_eq!(
            serde_json::to_string(&ascending).unwrap(),
            serde_json::to_string(&shuffled).unwrap()
        );
    }

    #[test]
    fn snapshot_rejects_duplicate_logical_ids() {
        let error = ResourceSnapshot::try_new(
            ResourceEpoch(4),
            [loading(2, "old-content"), loading(2, "new-content")],
        )
        .unwrap_err();

        assert_eq!(error, ResourceSnapshotError::DuplicateId(ResourceId(2)));
    }

    #[test]
    fn ready_state_requires_payload_and_forbids_failure() {
        let error = ResourceEntry::try_new(
            ResourceId(1),
            identity("image-v1"),
            ResourceKind::Image,
            provenance(),
            ResourceStatus::Ready,
            None,
            None,
        )
        .unwrap_err();

        assert_eq!(
            error,
            ResourceEntryError::InvalidState {
                status: ResourceStatus::Ready,
                has_payload: false,
                has_failure: false,
            }
        );

        let failure = ResourceFailure::try_new("decode", "invalid image", false).unwrap();
        let error = ResourceEntry::try_new(
            ResourceId(1),
            identity("image-v1"),
            ResourceKind::Image,
            provenance(),
            ResourceStatus::Ready,
            Some(ResourcePayload::Bytes(vec![1, 2, 3])),
            Some(failure),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ResourceEntryError::InvalidState {
                status: ResourceStatus::Ready,
                has_payload: true,
                has_failure: true,
            }
        ));
    }

    #[test]
    fn failed_state_requires_diagnostic_and_forbids_payload() {
        let error = ResourceEntry::try_new(
            ResourceId(1),
            identity("font-v1"),
            ResourceKind::Font,
            provenance(),
            ResourceStatus::Failed,
            None,
            None,
        )
        .unwrap_err();

        assert_eq!(
            error,
            ResourceEntryError::InvalidState {
                status: ResourceStatus::Failed,
                has_payload: false,
                has_failure: false,
            }
        );

        let failure = ResourceFailure::try_new("read", "source unavailable", true).unwrap();
        let error = ResourceEntry::try_new(
            ResourceId(1),
            identity("font-v1"),
            ResourceKind::Font,
            provenance(),
            ResourceStatus::Failed,
            Some(ResourcePayload::Bytes(vec![1])),
            Some(failure),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ResourceEntryError::InvalidState {
                status: ResourceStatus::Failed,
                has_payload: true,
                has_failure: true,
            }
        ));
    }

    #[test]
    fn deserialization_cannot_bypass_snapshot_invariants() {
        let duplicate = serde_json::json!({
            "epoch": 1,
            "entries": [
                {
                    "id": 3,
                    "content_identity": "content-a",
                    "kind": "Binary",
                    "provenance": {
                        "source": "Memory",
                        "locator": null,
                        "requested_by": null
                    },
                    "status": "Loading",
                    "payload": null,
                    "failure": null
                },
                {
                    "id": 3,
                    "content_identity": "content-b",
                    "kind": "Binary",
                    "provenance": {
                        "source": "Memory",
                        "locator": null,
                        "requested_by": null
                    },
                    "status": "Loading",
                    "payload": null,
                    "failure": null
                }
            ]
        });

        let error = serde_json::from_value::<ResourceSnapshot>(duplicate).unwrap_err();
        assert!(error.to_string().contains("appears more than once"));

        let invalid_state = serde_json::json!({
            "id": 4,
            "content_identity": "content-c",
            "kind": "Image",
            "provenance": {
                "source": "Memory",
                "locator": null,
                "requested_by": null
            },
            "status": "Ready",
            "payload": null,
            "failure": null
        });
        let error = serde_json::from_value::<ResourceEntry>(invalid_state).unwrap_err();
        assert!(error.to_string().contains("invalid Ready resource state"));
    }
}
