use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use fission_ir::op::ImageSource;
use fission_ir::{CoreIR, Op, PaintOp, WidgetId};
use fission_render::frame::ResourceEpoch;
use fission_render::resource::{
    resolved_resource_content_identity, unresolved_resource_content_identity,
    ResourceContentIdentity, ResourceEntry, ResourceFailure, ResourceId, ResourceKind,
    ResourcePayload, ResourceProvenance, ResourceSnapshot, ResourceSource,
};

use super::{take_monotonic, FrameSubmissionError};

mod loader;
#[cfg(test)]
mod tests;

use loader::{LoadFailure, LoadOutcome, PendingLoad, PlatformSourceLoader, SourceLoader};

pub(super) type ResourceWake = Arc<dyn Fn() + Send + Sync + 'static>;

/// Assigns backend-neutral resource identities and owns source acquisition.
///
/// Renderer implementations receive immutable source data through each
/// [`ResourceSnapshot`]. They never open paths or fetch URLs themselves.
#[derive(Clone)]
pub(super) struct FrameResourceRegistry {
    next_id: u64,
    image_ids: BTreeMap<WidgetId, ResourceId>,
    authority: ResourceAuthority,
}

impl fmt::Debug for FrameResourceRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FrameResourceRegistry")
            .field("next_id", &self.next_id)
            .field("logical_resources", &self.image_ids.len())
            .field("authority", &self.authority)
            .finish()
    }
}

impl Default for FrameResourceRegistry {
    fn default() -> Self {
        Self::with_loader(Arc::new(PlatformSourceLoader))
    }
}

impl FrameResourceRegistry {
    fn with_loader(loader: Arc<dyn SourceLoader>) -> Self {
        Self {
            next_id: 1,
            image_ids: BTreeMap::new(),
            authority: ResourceAuthority::new(loader),
        }
    }

    fn image_id(&mut self, node_id: WidgetId) -> Result<ResourceId, FrameSubmissionError> {
        if let Some(id) = self.image_ids.get(&node_id) {
            return Ok(*id);
        }

        let id = ResourceId(take_monotonic(&mut self.next_id, "resource id")?);
        self.image_ids.insert(node_id, id);
        Ok(id)
    }

    /// Monotonically changes after an asynchronous source reaches a terminal
    /// state. The host uses this to invalidate paint and submit a new snapshot.
    pub(super) fn generation(&self) -> u64 {
        self.authority.generation()
    }

    pub(super) fn has_pending(&self) -> bool {
        self.authority.has_pending()
    }

    /// Installs the event-loop wake used after an asynchronous completion.
    /// Replacing the callback is safe during host recreation.
    pub(super) fn install_wake(&self, wake: ResourceWake) {
        self.authority.install_wake(wake);
    }
}

#[derive(Clone)]
struct ResourceAuthority {
    inner: Arc<AuthorityInner>,
}

struct AuthorityInner {
    state: Mutex<AuthorityState>,
    generation: AtomicU64,
    wake: Mutex<Option<ResourceWake>>,
    loader: Arc<dyn SourceLoader>,
}

struct AuthorityState {
    next_ticket: u64,
    sources: BTreeMap<AcquisitionKey, SourceRecord>,
}

impl AuthorityState {
    fn initialized() -> Self {
        Self {
            next_ticket: 1,
            sources: BTreeMap::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AcquisitionKey {
    kind: ResourceKind,
    request_fingerprint: String,
}

#[derive(Clone)]
struct SourceRecord {
    ticket: Option<u64>,
    state: SourceState,
}

#[derive(Clone)]
enum SourceState {
    Loading {
        content_identity: ResourceContentIdentity,
    },
    Ready {
        content_identity: ResourceContentIdentity,
        payload: ResourcePayload,
    },
    Failed {
        content_identity: ResourceContentIdentity,
        failure: ResourceFailure,
    },
}

impl fmt::Debug for ResourceAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = lock_unpoisoned(&self.inner.state);
        let pending = state
            .sources
            .values()
            .filter(|record| matches!(record.state, SourceState::Loading { .. }))
            .count();
        formatter
            .debug_struct("ResourceAuthority")
            .field("sources", &state.sources.len())
            .field("pending", &pending)
            .field("generation", &self.generation())
            .finish()
    }
}

impl ResourceAuthority {
    fn new(loader: Arc<dyn SourceLoader>) -> Self {
        Self {
            inner: Arc::new(AuthorityInner {
                state: Mutex::new(AuthorityState::initialized()),
                generation: AtomicU64::new(0),
                wake: Mutex::new(None),
                loader,
            }),
        }
    }

    fn synchronize(&self, requests: &[RequestedResource]) -> Result<(), FrameSubmissionError> {
        let active = requests
            .iter()
            .map(|request| request.key.clone())
            .collect::<BTreeSet<_>>();
        let mut pending = Vec::new();
        {
            let mut state = lock_unpoisoned(&self.inner.state);
            // Source bytes are frame-runtime state, not a permanent global
            // cache. Removing inactive records also makes stale completions
            // harmless and bounds retained source data to live requests.
            state.sources.retain(|key, _| active.contains(key));

            for request in requests {
                let key = request.key.clone();
                if state.sources.contains_key(&key) {
                    continue;
                }

                if let Some(source_state) = immediate_state(&request.source, &request.kind) {
                    state.sources.insert(
                        key,
                        SourceRecord {
                            ticket: None,
                            state: source_state,
                        },
                    );
                    continue;
                }

                let ticket = take_monotonic(&mut state.next_ticket, "resource request ticket")?;
                let content_identity =
                    unresolved_resource_content_identity(&request.kind, &request.source);
                state.sources.insert(
                    key.clone(),
                    SourceRecord {
                        ticket: Some(ticket),
                        state: SourceState::Loading { content_identity },
                    },
                );
                pending.push(PendingLoad {
                    key,
                    ticket,
                    source: request.source.clone(),
                });
            }
        }

        for load in pending {
            self.start(load);
        }
        Ok(())
    }

    fn start(&self, pending: PendingLoad) {
        let key = pending.key.clone();
        let ticket = pending.ticket;
        let source = pending.source.clone();
        let authority = self.clone();
        let completion_source = source.clone();
        let completion =
            Box::new(move |outcome| authority.complete(key, ticket, &completion_source, outcome));
        if let Err(failure) = self.inner.loader.start(pending.source, completion) {
            self.complete(
                pending.key,
                pending.ticket,
                &source,
                LoadOutcome::Failed(failure),
            );
        }
    }

    fn complete(
        &self,
        key: AcquisitionKey,
        ticket: u64,
        source: &ImageSource,
        outcome: LoadOutcome,
    ) {
        let changed = {
            let mut state = lock_unpoisoned(&self.inner.state);
            let Some(record) = state.sources.get_mut(&key) else {
                return;
            };
            if record.ticket != Some(ticket) {
                // A removed and later re-added request has a newer ticket.
                // Its state is authoritative and must not be overwritten by
                // a stale worker completion.
                return;
            }

            record.ticket = None;
            record.state = match outcome {
                LoadOutcome::Ready(bytes) => SourceState::Ready {
                    content_identity: resolved_resource_content_identity(&key.kind, source, &bytes),
                    payload: ResourcePayload::Bytes(bytes),
                },
                LoadOutcome::Failed(failure) => SourceState::Failed {
                    content_identity: unresolved_resource_content_identity(&key.kind, source),
                    failure: resource_failure(failure),
                },
            };
            true
        };

        if changed {
            let _ =
                self.inner
                    .generation
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                        value.checked_add(1)
                    });
            let wake = { lock_unpoisoned(&self.inner.wake).clone() };
            if let Some(wake) = wake {
                wake();
            }
        }
    }

    fn states(&self, requests: &[RequestedResource]) -> Vec<SourceState> {
        let state = lock_unpoisoned(&self.inner.state);
        requests
            .iter()
            .map(|request| {
                state
                    .sources
                    .get(&request.key)
                    .expect("resource synchronization creates every requested source")
                    .state
                    .clone()
            })
            .collect()
    }

    fn generation(&self) -> u64 {
        self.inner.generation.load(Ordering::Acquire)
    }

    fn has_pending(&self) -> bool {
        lock_unpoisoned(&self.inner.state)
            .sources
            .values()
            .any(|record| matches!(record.state, SourceState::Loading { .. }))
    }

    fn install_wake(&self, wake: ResourceWake) {
        *lock_unpoisoned(&self.inner.wake) = Some(wake);
    }
}

#[derive(Clone)]
struct RequestedResource {
    node_id: WidgetId,
    source: ImageSource,
    kind: ResourceKind,
    key: AcquisitionKey,
}

impl RequestedResource {
    fn new(node_id: WidgetId, source: ImageSource, kind: ResourceKind) -> Self {
        let key = AcquisitionKey {
            kind: kind.clone(),
            request_fingerprint: request_fingerprint(&source),
        };
        Self {
            node_id,
            source,
            kind,
            key,
        }
    }

    fn provenance(&self) -> ResourceProvenance {
        let (source, locator) = match &self.source {
            ImageSource::Asset { path } => (ResourceSource::Asset, Some(path.clone())),
            ImageSource::File { path } => (ResourceSource::File, Some(path.clone())),
            ImageSource::Network { url, .. } => {
                (ResourceSource::Network, Some(redacted_network_locator(url)))
            }
            ImageSource::Memory { mime_type, .. } => (ResourceSource::Memory, mime_type.clone()),
            ImageSource::SvgText { .. } => {
                (ResourceSource::Embedded, Some("inline-svg".to_string()))
            }
        };
        ResourceProvenance {
            source,
            locator,
            requested_by: Some(self.node_id),
        }
    }
}

pub(super) fn build_resource_snapshot(
    epoch: ResourceEpoch,
    ir: &CoreIR,
    registry: &mut FrameResourceRegistry,
) -> Result<ResourceSnapshot, FrameSubmissionError> {
    let requests = collect_requests(ir);
    registry.authority.synchronize(&requests)?;
    let states = registry.authority.states(&requests);

    let mut entries = Vec::with_capacity(requests.len());
    for (request, state) in requests.into_iter().zip(states) {
        let id = registry.image_id(request.node_id)?;
        let provenance = request.provenance();
        let entry = match state {
            SourceState::Loading { content_identity } => {
                ResourceEntry::loading(id, content_identity, request.kind, provenance)
            }
            SourceState::Ready {
                content_identity,
                payload,
            } => ResourceEntry::ready(id, content_identity, request.kind, provenance, payload),
            SourceState::Failed {
                content_identity,
                failure,
            } => ResourceEntry::failed(id, content_identity, request.kind, provenance, failure),
        };
        entries.push(entry);
    }

    Ok(ResourceSnapshot::try_new(epoch, entries)
        .expect("the frame resource registry assigns unique, valid resource entries"))
}

fn collect_requests(ir: &CoreIR) -> Vec<RequestedResource> {
    let mut requests = ir
        .nodes
        .iter()
        .filter_map(|(&node_id, node)| match &node.op {
            Op::Paint(PaintOp::DrawImage { request, .. }) => Some(RequestedResource::new(
                node_id,
                request.source.clone(),
                if matches!(&request.source, ImageSource::SvgText { .. }) {
                    ResourceKind::Svg
                } else {
                    ResourceKind::Image
                },
            )),
            Op::Paint(PaintOp::DrawSvg { content, .. }) => Some(RequestedResource::new(
                node_id,
                ImageSource::SvgText {
                    content: content.clone(),
                },
                ResourceKind::Svg,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    requests.sort_unstable_by_key(|request| request.node_id);
    requests
}

fn immediate_state(source: &ImageSource, kind: &ResourceKind) -> Option<SourceState> {
    match source {
        ImageSource::Memory { bytes, .. } => Some(if bytes.len() > loader::MAX_SOURCE_BYTES {
            SourceState::Failed {
                content_identity: unresolved_resource_content_identity(kind, source),
                failure: resource_failure(LoadFailure::too_large()),
            }
        } else {
            SourceState::Ready {
                content_identity: resolved_resource_content_identity(kind, source, bytes),
                payload: ResourcePayload::Bytes(bytes.clone()),
            }
        }),
        ImageSource::SvgText { content } => Some(if content.len() > loader::MAX_SOURCE_BYTES {
            SourceState::Failed {
                content_identity: unresolved_resource_content_identity(kind, source),
                failure: resource_failure(LoadFailure::too_large()),
            }
        } else {
            SourceState::Ready {
                content_identity: resolved_resource_content_identity(
                    kind,
                    source,
                    content.as_bytes(),
                ),
                payload: ResourcePayload::Text(content.clone()),
            }
        }),
        ImageSource::Asset { .. } | ImageSource::File { .. } | ImageSource::Network { .. } => None,
    }
}

fn request_fingerprint(source: &ImageSource) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"fission-resource-request-v1\0");
    hasher.update(source.stable_identity().as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn resource_failure(failure: LoadFailure) -> ResourceFailure {
    ResourceFailure::try_new(failure.code, failure.message, failure.retryable)
        .expect("resource loader failures have bounded non-empty fields")
}

/// Preserve enough of a URL to diagnose the source without transporting
/// query credentials, fragments, or authority userinfo with the frame.
fn redacted_network_locator(url: &str) -> String {
    const REDACTED: &str = "<redacted>";
    let suffix_start = url
        .find(|character| matches!(character, '?' | '#'))
        .unwrap_or(url.len());
    let mut base = url[..suffix_start].to_string();

    if let Some(scheme_end) = base.find("://") {
        let authority_start = scheme_end + 3;
        let authority_end = base[authority_start..]
            .find('/')
            .map_or(base.len(), |offset| authority_start + offset);
        if let Some(at_offset) = base[authority_start..authority_end].rfind('@') {
            let at = authority_start + at_offset;
            base.replace_range(authority_start..at, REDACTED);
        }
    }
    if suffix_start < url.len() {
        base.push_str("?<redacted>");
    }
    base
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
