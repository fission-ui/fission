use std::collections::btree_map::{Entry, Iter};
use std::collections::BTreeMap;
use std::fmt;

use serde::de;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::capabilities::ExternalSurfaceTransport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExternalSurfaceSlotId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExternalProducerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExternalFrameId(pub u64);

/// Producer-local handle resolved by the platform/backend adapter.
///
/// It is deliberately opaque to frame compilation. The binding transport says
/// what kind of object it identifies; placement remains solely in `DisplayOp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExternalFrameToken(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExternalProducerKind {
    Video,
    WebView,
    ThreeD,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalFrameState {
    Pending,
    Ready,
    Failed,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalColorSpace {
    Srgb,
    DisplayP3,
    LinearSrgb,
    Rec2020,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalAlphaType {
    Opaque,
    Premultiplied,
    Unpremultiplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalOwnership {
    BorrowedForFrame,
    Shared,
    Transferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalSynchronization {
    None,
    /// Adapter-resolved fence token that must be acquired before sampling.
    Fence {
        acquire: u64,
        release: Option<u64>,
    },
    /// Monotonic timeline token and value.
    Timeline {
        token: u64,
        value: u64,
    },
}

/// Producer state for one externally rendered slot.
///
/// Bounds, z-order, clipping, transforms, and opacity are intentionally absent:
/// the matching `DisplayOp::DrawSurface` is their sole authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExternalSurfaceBinding {
    pub slot_id: ExternalSurfaceSlotId,
    pub producer_id: ExternalProducerId,
    pub producer_kind: ExternalProducerKind,
    pub frame_id: ExternalFrameId,
    pub frame_token: Option<ExternalFrameToken>,
    pub state: ExternalFrameState,
    pub transport: ExternalSurfaceTransport,
    pub color_space: ExternalColorSpace,
    pub alpha_type: ExternalAlphaType,
    pub ownership: ExternalOwnership,
    pub synchronization: ExternalSynchronization,
    pub zero_copy: bool,
    pub damaged: bool,
}

impl ExternalSurfaceBinding {
    /// Validate state that can be proven without resolving the opaque producer
    /// handle in a host or backend adapter.
    pub fn validate(&self) -> Result<(), ExternalSurfaceBindingError> {
        let image_transport = matches!(
            self.transport,
            ExternalSurfaceTransport::CpuImage
                | ExternalSurfaceTransport::NativeImage
                | ExternalSurfaceTransport::GpuImage
        );
        if self.state == ExternalFrameState::Ready && image_transport && self.frame_token.is_none()
        {
            return Err(ExternalSurfaceBindingError::ReadyImageWithoutToken {
                transport: self.transport,
            });
        }
        if self.transport == ExternalSurfaceTransport::DirectTarget && self.zero_copy {
            return Err(ExternalSurfaceBindingError::DirectTargetClaimsZeroCopy);
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for ExternalSurfaceBinding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedBinding {
            slot_id: ExternalSurfaceSlotId,
            producer_id: ExternalProducerId,
            producer_kind: ExternalProducerKind,
            frame_id: ExternalFrameId,
            frame_token: Option<ExternalFrameToken>,
            state: ExternalFrameState,
            transport: ExternalSurfaceTransport,
            color_space: ExternalColorSpace,
            alpha_type: ExternalAlphaType,
            ownership: ExternalOwnership,
            synchronization: ExternalSynchronization,
            zero_copy: bool,
            damaged: bool,
        }

        let serialized = SerializedBinding::deserialize(deserializer)?;
        let binding = Self {
            slot_id: serialized.slot_id,
            producer_id: serialized.producer_id,
            producer_kind: serialized.producer_kind,
            frame_id: serialized.frame_id,
            frame_token: serialized.frame_token,
            state: serialized.state,
            transport: serialized.transport,
            color_space: serialized.color_space,
            alpha_type: serialized.alpha_type,
            ownership: serialized.ownership,
            synchronization: serialized.synchronization,
            zero_copy: serialized.zero_copy,
            damaged: serialized.damaged,
        };
        binding.validate().map_err(de::Error::custom)?;
        Ok(binding)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSurfaceBindingError {
    ReadyImageWithoutToken { transport: ExternalSurfaceTransport },
    DirectTargetClaimsZeroCopy,
}

impl fmt::Display for ExternalSurfaceBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadyImageWithoutToken { transport } => write!(
                formatter,
                "ready {transport:?} external surface has no frame token"
            ),
            Self::DirectTargetClaimsZeroCopy => formatter
                .write_str("direct-target external surface cannot claim zero-copy composition"),
        }
    }
}

impl std::error::Error for ExternalSurfaceBindingError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidExternalSurfaceBinding {
    pub slot_id: ExternalSurfaceSlotId,
    pub error: ExternalSurfaceBindingError,
}

impl fmt::Display for InvalidExternalSurfaceBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "external surface slot {} is invalid: {}",
            self.slot_id.0, self.error
        )
    }
}

impl std::error::Error for InvalidExternalSurfaceBinding {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalSurfaceBindings {
    bindings: BTreeMap<ExternalSurfaceSlotId, ExternalSurfaceBinding>,
}

impl ExternalSurfaceBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_new(
        bindings: impl IntoIterator<Item = ExternalSurfaceBinding>,
    ) -> Result<Self, DuplicateExternalSurfaceBinding> {
        let mut indexed = Self::new();
        for binding in bindings {
            indexed.insert(binding)?;
        }
        Ok(indexed)
    }

    pub fn insert(
        &mut self,
        binding: ExternalSurfaceBinding,
    ) -> Result<(), DuplicateExternalSurfaceBinding> {
        match self.bindings.entry(binding.slot_id) {
            Entry::Vacant(entry) => {
                entry.insert(binding);
                Ok(())
            }
            Entry::Occupied(_) => Err(DuplicateExternalSurfaceBinding(binding.slot_id)),
        }
    }

    pub fn get(&self, slot_id: ExternalSurfaceSlotId) -> Option<&ExternalSurfaceBinding> {
        self.bindings.get(&slot_id)
    }

    pub fn contains(&self, slot_id: ExternalSurfaceSlotId) -> bool {
        self.bindings.contains_key(&slot_id)
    }

    pub fn iter(&self) -> Iter<'_, ExternalSurfaceSlotId, ExternalSurfaceBinding> {
        self.bindings.iter()
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Validate bindings in stable slot order so the first reported failure is
    /// deterministic across producers and serialization round trips.
    pub fn validate(&self) -> Result<(), InvalidExternalSurfaceBinding> {
        for (slot_id, binding) in &self.bindings {
            binding
                .validate()
                .map_err(|error| InvalidExternalSurfaceBinding {
                    slot_id: *slot_id,
                    error,
                })?;
        }
        Ok(())
    }
}

impl Serialize for ExternalSurfaceBindings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut bindings = serializer.serialize_struct("ExternalSurfaceBindings", 1)?;
        bindings.serialize_field("bindings", &self.bindings.values().collect::<Vec<_>>())?;
        bindings.end()
    }
}

impl<'de> Deserialize<'de> for ExternalSurfaceBindings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedBindings {
            bindings: Vec<ExternalSurfaceBinding>,
        }

        let bindings = SerializedBindings::deserialize(deserializer)?;
        Self::try_new(bindings.bindings).map_err(de::Error::custom)
    }
}

impl<'a> IntoIterator for &'a ExternalSurfaceBindings {
    type Item = (&'a ExternalSurfaceSlotId, &'a ExternalSurfaceBinding);
    type IntoIter = Iter<'a, ExternalSurfaceSlotId, ExternalSurfaceBinding>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateExternalSurfaceBinding(pub ExternalSurfaceSlotId);

impl fmt::Display for DuplicateExternalSurfaceBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let slot_id = self.0;
        write!(
            formatter,
            "external surface slot {} is already bound",
            slot_id.0
        )
    }
}

impl std::error::Error for DuplicateExternalSurfaceBinding {}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(slot_id: u64) -> ExternalSurfaceBinding {
        ExternalSurfaceBinding {
            slot_id: ExternalSurfaceSlotId(slot_id),
            producer_id: ExternalProducerId(10),
            producer_kind: ExternalProducerKind::Video,
            frame_id: ExternalFrameId(20),
            frame_token: Some(ExternalFrameToken(30)),
            state: ExternalFrameState::Ready,
            transport: ExternalSurfaceTransport::GpuImage,
            color_space: ExternalColorSpace::Srgb,
            alpha_type: ExternalAlphaType::Opaque,
            ownership: ExternalOwnership::BorrowedForFrame,
            synchronization: ExternalSynchronization::None,
            zero_copy: true,
            damaged: true,
        }
    }

    #[test]
    fn bindings_reject_a_second_authority_for_the_same_slot() {
        let mut bindings = ExternalSurfaceBindings::new();
        bindings.insert(binding(1)).unwrap();

        let duplicate = bindings.insert(binding(1)).unwrap_err();

        assert_eq!(
            duplicate,
            DuplicateExternalSurfaceBinding(ExternalSurfaceSlotId(1))
        );
        assert_eq!(bindings.len(), 1);
    }

    #[test]
    fn bindings_round_trip_in_deterministic_slot_order() {
        let bindings = ExternalSurfaceBindings::try_new([binding(9), binding(2)]).unwrap();

        let json = serde_json::to_string(&bindings).unwrap();
        let first_slot = json.find("\"slot_id\":2").unwrap();
        let second_slot = json.find("\"slot_id\":9").unwrap();
        assert!(first_slot < second_slot);

        let decoded: ExternalSurfaceBindings = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, bindings);
    }

    #[test]
    fn deserialization_rejects_duplicate_slot_authority() {
        let first = serde_json::to_value(binding(4)).unwrap();
        let json = serde_json::json!({ "bindings": [first.clone(), first] });

        let error = serde_json::from_value::<ExternalSurfaceBindings>(json)
            .unwrap_err()
            .to_string();

        assert!(error.contains("external surface slot 4 is already bound"));
    }

    #[test]
    fn ready_image_transport_requires_an_opaque_frame_token() {
        let mut invalid = binding(4);
        invalid.frame_token = None;

        assert_eq!(
            invalid.validate(),
            Err(ExternalSurfaceBindingError::ReadyImageWithoutToken {
                transport: ExternalSurfaceTransport::GpuImage,
            })
        );

        let serialized = serde_json::to_value(&invalid).unwrap();
        let error = serde_json::from_value::<ExternalSurfaceBinding>(serialized).unwrap_err();
        assert!(error.to_string().contains("has no frame token"));
    }

    #[test]
    fn direct_target_cannot_misrepresent_itself_as_zero_copy_composition() {
        let mut invalid = binding(8);
        invalid.transport = ExternalSurfaceTransport::DirectTarget;

        assert_eq!(
            invalid.validate(),
            Err(ExternalSurfaceBindingError::DirectTargetClaimsZeroCopy)
        );
    }

    #[test]
    fn binding_validation_reports_the_lowest_invalid_slot_first() {
        let mut high = binding(9);
        high.frame_token = None;
        let mut low = binding(2);
        low.frame_token = None;
        let bindings = ExternalSurfaceBindings::try_new([high, low]).unwrap();

        let error = bindings.validate().unwrap_err();

        assert_eq!(error.slot_id, ExternalSurfaceSlotId(2));
    }
}
