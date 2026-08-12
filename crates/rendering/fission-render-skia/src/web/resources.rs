use std::collections::BTreeMap;
use std::fmt;

use fission_render::resource::{
    ResourceEntry, ResourceId, ResourceKind, ResourcePayload, ResourceSnapshot, ResourceStatus,
};
use fission_skia_sys::web::{
    ResourceBatch, ResourceHandle, ResourceKind as WireResourceKind, ResourceOperation,
    ResourceUpdate,
};

const MAX_RESOURCE_SLOTS: u32 = 65_536;
const CONTENT_ID_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const CONTENT_ID_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ResourceMapError {
    ZeroEpoch,
    StaleEpoch { current: u64, received: u64 },
    ReusedEpoch { epoch: u64 },
    SlotExhausted,
    GenerationExhausted { slot: u32 },
}

impl fmt::Display for ResourceMapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroEpoch => formatter.write_str("CanvasKit resource epochs must be non-zero"),
            Self::StaleEpoch { current, received } => write!(
                formatter,
                "CanvasKit resource epoch {received} is older than committed epoch {current}"
            ),
            Self::ReusedEpoch { epoch } => write!(
                formatter,
                "CanvasKit resource epoch {epoch} was reused with different resource content"
            ),
            Self::SlotExhausted => write!(
                formatter,
                "CanvasKit exhausted its bounded table of {MAX_RESOURCE_SLOTS} resource slots"
            ),
            Self::GenerationExhausted { slot } => write!(
                formatter,
                "CanvasKit resource slot {slot} exhausted its generation counter"
            ),
        }
    }
}

impl std::error::Error for ResourceMapError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveResource {
    handle: ResourceHandle,
    kind: WireResourceKind,
    content_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SlotState {
    generation: u32,
    resource_id: Option<ResourceId>,
}

#[derive(Debug, Clone)]
pub(super) struct ResourceMap {
    epoch: u64,
    live: BTreeMap<ResourceId, LiveResource>,
    slots: BTreeMap<u32, SlotState>,
}

impl Default for ResourceMap {
    fn default() -> Self {
        Self {
            epoch: 0,
            live: BTreeMap::new(),
            slots: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ResourcePlan {
    pub(super) batch: ResourceBatch,
    pub(super) uploaded_bytes: u64,
    next: ResourceMap,
}

impl ResourcePlan {
    /// Resolves against the complete post-commit table represented by this
    /// plan. Frame compilation must use this view so first-use resources can be
    /// referenced before the batch Ack is committed locally.
    pub(super) fn handle(&self, resource_id: ResourceId) -> Option<ResourceHandle> {
        self.next.handle(resource_id)
    }
}

#[derive(Debug, Clone)]
struct DesiredResource {
    kind: WireResourceKind,
    content_id: u64,
    bytes: Vec<u8>,
}

impl ResourceMap {
    pub(super) fn epoch(&self) -> u64 {
        self.epoch
    }

    pub(super) fn live_count(&self) -> usize {
        self.live.len()
    }

    pub(super) fn handle(&self, resource_id: ResourceId) -> Option<ResourceHandle> {
        self.live.get(&resource_id).map(|resource| resource.handle)
    }

    pub(super) fn handle_after(
        &self,
        plan: Option<&ResourcePlan>,
        resource_id: ResourceId,
    ) -> Option<ResourceHandle> {
        plan.map_or_else(|| self.handle(resource_id), |plan| plan.handle(resource_id))
    }

    /// Build the complete next table without changing committed state.
    ///
    /// The caller commits the returned plan only after the browser has Acked
    /// its `ResourceBatch`. A host Error therefore leaves slots, generations,
    /// and the committed epoch unchanged and makes an exact retry possible.
    pub(super) fn plan(
        &self,
        snapshot: &ResourceSnapshot,
    ) -> Result<Option<ResourcePlan>, ResourceMapError> {
        let received_epoch = snapshot.epoch().0;
        if received_epoch == 0 {
            return Err(ResourceMapError::ZeroEpoch);
        }

        let desired = desired_resources(snapshot);
        if received_epoch < self.epoch {
            return Err(ResourceMapError::StaleEpoch {
                current: self.epoch,
                received: received_epoch,
            });
        }
        if received_epoch == self.epoch {
            return if self.matches(&desired) {
                Ok(None)
            } else {
                Err(ResourceMapError::ReusedEpoch {
                    epoch: received_epoch,
                })
            };
        }

        let mut next = self.clone();
        let mut updates = Vec::new();

        let removed = self
            .live
            .keys()
            .filter(|resource_id| !desired.contains_key(resource_id))
            .copied()
            .collect::<Vec<_>>();
        let changed = self
            .live
            .iter()
            .filter_map(|(resource_id, live)| {
                desired
                    .get(resource_id)
                    .filter(|desired| {
                        desired.kind != live.kind || desired.content_id != live.content_id
                    })
                    .map(|_| *resource_id)
            })
            .collect::<Vec<_>>();

        // Release every old object before a later update reuses its slot. The
        // wire-side protocol applies this vector atomically in order.
        for resource_id in removed.iter().chain(changed.iter()) {
            let live = next
                .live
                .remove(resource_id)
                .expect("removed and changed resources were selected from the live map");
            updates.push(release_update(live.handle, live.kind));
            next.slots
                .get_mut(&live.handle.slot)
                .expect("every live resource owns an allocated slot")
                .resource_id = None;
        }

        // A logical resource keeps its slot when immutable content changes,
        // but receives a new generation so stale command handles cannot alias
        // the replacement object.
        for resource_id in &changed {
            let previous = self
                .live
                .get(resource_id)
                .expect("changed resources were selected from the live map");
            let desired_resource = desired
                .get(resource_id)
                .expect("changed resources remain in the desired map");
            let generation = previous.handle.generation.checked_add(1).ok_or(
                ResourceMapError::GenerationExhausted {
                    slot: previous.handle.slot,
                },
            )?;
            let handle = ResourceHandle {
                slot: previous.handle.slot,
                generation,
            };
            next.install(*resource_id, handle, desired_resource, &mut updates);
        }

        for (resource_id, desired_resource) in &desired {
            if self.live.contains_key(resource_id) {
                continue;
            }
            let handle = next.allocate_handle()?;
            next.install(*resource_id, handle, desired_resource, &mut updates);
        }

        next.epoch = received_epoch;
        let uploaded_bytes = updates
            .iter()
            .filter(|update| update.operation == ResourceOperation::Upsert)
            .fold(0_u64, |total, update| {
                total.saturating_add(update.bytes.len() as u64)
            });

        Ok(Some(ResourcePlan {
            batch: ResourceBatch {
                resource_epoch: received_epoch,
                updates,
            },
            uploaded_bytes,
            next,
        }))
    }

    pub(super) fn commit(&mut self, plan: ResourcePlan) {
        *self = plan.next;
    }

    fn matches(&self, desired: &BTreeMap<ResourceId, DesiredResource>) -> bool {
        self.live.len() == desired.len()
            && desired.iter().all(|(resource_id, desired)| {
                self.live.get(resource_id).is_some_and(|live| {
                    live.kind == desired.kind && live.content_id == desired.content_id
                })
            })
    }

    fn allocate_handle(&mut self) -> Result<ResourceHandle, ResourceMapError> {
        if let Some((&slot, state)) = self
            .slots
            .iter_mut()
            .find(|(_, state)| state.resource_id.is_none())
        {
            let generation = state
                .generation
                .checked_add(1)
                .ok_or(ResourceMapError::GenerationExhausted { slot })?;
            state.generation = generation;
            return Ok(ResourceHandle { slot, generation });
        }

        let slot = self
            .slots
            .last_key_value()
            .map_or(1, |(slot, _)| slot.saturating_add(1));
        if slot == 0 || slot > MAX_RESOURCE_SLOTS {
            return Err(ResourceMapError::SlotExhausted);
        }
        self.slots.insert(
            slot,
            SlotState {
                generation: 1,
                resource_id: None,
            },
        );
        Ok(ResourceHandle {
            slot,
            generation: 1,
        })
    }

    fn install(
        &mut self,
        resource_id: ResourceId,
        handle: ResourceHandle,
        desired: &DesiredResource,
        updates: &mut Vec<ResourceUpdate>,
    ) {
        let slot = self
            .slots
            .get_mut(&handle.slot)
            .expect("a resource handle is allocated before installation");
        slot.generation = handle.generation;
        slot.resource_id = Some(resource_id);
        self.live.insert(
            resource_id,
            LiveResource {
                handle,
                kind: desired.kind,
                content_id: desired.content_id,
            },
        );
        updates.push(ResourceUpdate {
            handle,
            operation: ResourceOperation::Upsert,
            kind: desired.kind,
            content_id: desired.content_id,
            bytes: desired.bytes.clone(),
        });
    }
}

fn desired_resources(snapshot: &ResourceSnapshot) -> BTreeMap<ResourceId, DesiredResource> {
    snapshot
        .iter()
        .filter_map(|(resource_id, entry)| {
            // CanvasKit's Web SVG contract is backend-neutral geometry. Raw
            // SVG source must never reach the JavaScript resource decoder,
            // which intentionally rejects it rather than claiming SkSVGDOM.
            (entry.status() == ResourceStatus::Ready && entry.kind() != &ResourceKind::Svg).then(
                || {
                    let bytes = payload_bytes(
                        entry
                            .payload()
                            .expect("a validated ready resource always owns a payload"),
                    );
                    let kind = wire_kind(entry.kind());
                    let content_id = deterministic_content_id(entry, &bytes);
                    (
                        *resource_id,
                        DesiredResource {
                            kind,
                            content_id,
                            bytes,
                        },
                    )
                },
            )
        })
        .collect()
}

fn payload_bytes(payload: &ResourcePayload) -> Vec<u8> {
    match payload {
        ResourcePayload::Bytes(bytes) => bytes.clone(),
        ResourcePayload::Text(text) => text.as_bytes().to_vec(),
    }
}

fn wire_kind(kind: &ResourceKind) -> WireResourceKind {
    match kind {
        ResourceKind::Image => WireResourceKind::Image,
        ResourceKind::Svg => WireResourceKind::Svg,
        ResourceKind::Font => WireResourceKind::Font,
        ResourceKind::Text => WireResourceKind::Text,
        ResourceKind::Binary | ResourceKind::Custom(_) => WireResourceKind::Binary,
    }
}

fn release_update(handle: ResourceHandle, kind: WireResourceKind) -> ResourceUpdate {
    ResourceUpdate {
        handle,
        operation: ResourceOperation::Release,
        kind,
        content_id: 0,
        bytes: Vec::new(),
    }
}

/// Produce a target-independent content identifier rather than relying on
/// Rust's process-randomized `Hash` implementation.
fn deterministic_content_id(entry: &ResourceEntry, bytes: &[u8]) -> u64 {
    let mut hash = StableHasher::new();
    hash.bytes(b"fission-canvaskit-resource-v1\0");
    match entry.kind() {
        ResourceKind::Image => hash.byte(1),
        ResourceKind::Svg => hash.byte(2),
        ResourceKind::Font => hash.byte(3),
        ResourceKind::Text => hash.byte(4),
        ResourceKind::Binary => hash.byte(5),
        ResourceKind::Custom(name) => {
            hash.byte(6);
            hash.sized_bytes(name.as_bytes());
        }
    }
    hash.sized_bytes(entry.content_identity().as_str().as_bytes());
    hash.sized_bytes(bytes);
    let value = hash.finish();
    if value == 0 {
        1
    } else {
        value
    }
}

struct StableHasher(u64);

impl StableHasher {
    const fn new() -> Self {
        Self(CONTENT_ID_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(CONTENT_ID_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.byte(byte);
        }
    }

    fn sized_bytes(&mut self, bytes: &[u8]) {
        self.bytes(&(bytes.len() as u64).to_le_bytes());
        self.bytes(bytes);
    }

    const fn finish(self) -> u64 {
        self.0
    }
}
