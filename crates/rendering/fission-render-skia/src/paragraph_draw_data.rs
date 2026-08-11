//! Shared ownership for immutable paragraph paint resources.
//!
//! Layout remains the sole geometry authority. This registry only associates
//! that exact [`ParagraphResult`] with backend-owned immutable draw data and
//! keeps per-frame resources alive while native execution is in flight.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use fission_ir::WidgetId;
use fission_layout::{ParagraphCacheKey, ParagraphDrawDataId, ParagraphResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParagraphDrawDataBudget {
    pub(crate) max_entries: NonZeroUsize,
    pub(crate) max_bytes: NonZeroUsize,
}

impl ParagraphDrawDataBudget {
    pub(crate) const fn new(max_entries: NonZeroUsize, max_bytes: NonZeroUsize) -> Self {
        Self {
            max_entries,
            max_bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ParagraphDrawDataUsage {
    pub(crate) entries: usize,
    pub(crate) bytes: usize,
}

/// One profile-local registry shared by its paragraph engine and renderer.
///
/// Identifiers are never reused during the registry's lifetime. Entries are
/// removed only after layout supplies the complete set of published results
/// that can be painted. A frame binding clones the resource's [`Arc`], so a
/// later sweep cannot invalidate work already submitted for compilation.
pub(crate) struct ParagraphDrawDataRegistry<T> {
    state: Mutex<RegistryState<T>>,
    budget: ParagraphDrawDataBudget,
}

impl<T> ParagraphDrawDataRegistry<T> {
    pub(crate) fn new(budget: ParagraphDrawDataBudget) -> Self {
        Self {
            state: Mutex::new(RegistryState {
                next_id: Some(1),
                by_id: HashMap::new(),
                by_cache_key: HashMap::new(),
                used_bytes: 0,
            }),
            budget,
        }
    }

    /// Registers immutable draw data produced alongside the exact geometry.
    ///
    /// A cache-key hit reuses the already registered picture. Budget pressure
    /// fails explicitly; registration never evicts a draw-data identifier that
    /// may still be present in a published paragraph result.
    pub(crate) fn register(
        &self,
        cache_key: ParagraphCacheKey,
        data: Arc<T>,
        approximate_bytes: usize,
    ) -> Result<ParagraphDrawDataId, ParagraphDrawDataError> {
        let mut state = self.state.lock().unwrap();
        if let Some(id) = state.by_cache_key.get(&cache_key).copied() {
            return Ok(id);
        }

        let requested_entries =
            state
                .by_id
                .len()
                .checked_add(1)
                .ok_or(ParagraphDrawDataError::AccountingOverflow {
                    field: "entry count",
                })?;
        let requested_bytes = state.used_bytes.checked_add(approximate_bytes).ok_or(
            ParagraphDrawDataError::AccountingOverflow {
                field: "byte count",
            },
        )?;
        if requested_entries > self.budget.max_entries.get()
            || requested_bytes > self.budget.max_bytes.get()
        {
            return Err(ParagraphDrawDataError::BudgetExceeded {
                requested: ParagraphDrawDataUsage {
                    entries: requested_entries,
                    bytes: requested_bytes,
                },
                budget: self.budget,
            });
        }

        let raw_id = state
            .next_id
            .ok_or(ParagraphDrawDataError::IdentifierExhausted)?;
        state.next_id = raw_id.checked_add(1);
        let id = ParagraphDrawDataId::new(raw_id);
        state.by_cache_key.insert(cache_key, id);
        state.by_id.insert(
            id,
            DrawDataEntry {
                cache_key,
                approximate_bytes,
                data,
            },
        );
        state.used_bytes = requested_bytes;
        Ok(id)
    }

    /// Resolves and verifies the draw data named by an authoritative result.
    pub(crate) fn resolve(
        &self,
        result: &ParagraphResult,
    ) -> Result<Arc<T>, ParagraphDrawDataError> {
        let id = result
            .draw_data()
            .ok_or(ParagraphDrawDataError::MissingDrawData)?;
        let state = self.state.lock().unwrap();
        let entry = state
            .by_id
            .get(&id)
            .ok_or(ParagraphDrawDataError::UnknownIdentifier { id })?;
        if entry.cache_key != result.cache_key() {
            return Err(ParagraphDrawDataError::CacheKeyMismatch {
                id,
                expected: entry.cache_key,
                actual: result.cache_key(),
            });
        }
        Ok(Arc::clone(&entry.data))
    }

    /// Retires everything except the complete set of currently published
    /// paragraph results. Validation is transactional: an invalid live set
    /// leaves the registry unchanged.
    pub(crate) fn retain_results<'a>(
        &self,
        results: impl IntoIterator<Item = &'a ParagraphResult>,
    ) -> Result<ParagraphDrawDataUsage, ParagraphDrawDataError> {
        let requested = results
            .into_iter()
            .map(|result| {
                result
                    .draw_data()
                    .map(|id| (id, result.cache_key()))
                    .ok_or(ParagraphDrawDataError::MissingDrawData)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut state = self.state.lock().unwrap();
        for (id, cache_key) in &requested {
            let entry = state
                .by_id
                .get(id)
                .ok_or(ParagraphDrawDataError::UnknownIdentifier { id: *id })?;
            if entry.cache_key != *cache_key {
                return Err(ParagraphDrawDataError::CacheKeyMismatch {
                    id: *id,
                    expected: entry.cache_key,
                    actual: *cache_key,
                });
            }
        }

        let live = requested
            .into_iter()
            .map(|(id, _)| id)
            .collect::<HashSet<_>>();
        let removed = state
            .by_id
            .iter()
            .filter_map(|(id, entry)| {
                (!live.contains(id)).then_some((*id, entry.approximate_bytes))
            })
            .collect::<Vec<_>>();
        for (id, bytes) in removed {
            let entry = state
                .by_id
                .remove(&id)
                .expect("draw-data entry selected for removal must exist");
            state.by_cache_key.remove(&entry.cache_key);
            state.used_bytes = state
                .used_bytes
                .checked_sub(bytes)
                .expect("registered draw-data byte accounting cannot underflow");
        }
        Ok(state.usage())
    }

    /// Binds exact node results to immutable resources for one frame.
    pub(crate) fn bind_frame(
        &self,
        results: impl IntoIterator<Item = (WidgetId, Arc<ParagraphResult>)>,
    ) -> Result<ParagraphFrameDrawData<T>, ParagraphDrawDataError> {
        let state = self.state.lock().unwrap();
        let mut by_node = HashMap::new();
        for (node_id, result) in results {
            let id = result
                .draw_data()
                .ok_or(ParagraphDrawDataError::MissingNodeDrawData { node_id })?;
            let entry = state
                .by_id
                .get(&id)
                .ok_or(ParagraphDrawDataError::UnknownNodeIdentifier { node_id, id })?;
            if entry.cache_key != result.cache_key() {
                return Err(ParagraphDrawDataError::NodeCacheKeyMismatch {
                    node_id,
                    id,
                    expected: entry.cache_key,
                    actual: result.cache_key(),
                });
            }
            if by_node
                .insert(
                    node_id,
                    BoundParagraphDrawData {
                        id,
                        result,
                        data: Arc::clone(&entry.data),
                    },
                )
                .is_some()
            {
                return Err(ParagraphDrawDataError::DuplicateNode { node_id });
            }
        }
        Ok(ParagraphFrameDrawData { by_node })
    }

    pub(crate) fn usage(&self) -> ParagraphDrawDataUsage {
        self.state.lock().unwrap().usage()
    }

    /// Drops registry ownership without reusing any retired identifier. Draw
    /// data already cloned into a frame binding remains alive until that frame
    /// finishes.
    pub(crate) fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.by_id.clear();
        state.by_cache_key.clear();
        state.used_bytes = 0;
    }
}

struct RegistryState<T> {
    next_id: Option<u64>,
    by_id: HashMap<ParagraphDrawDataId, DrawDataEntry<T>>,
    by_cache_key: HashMap<ParagraphCacheKey, ParagraphDrawDataId>,
    used_bytes: usize,
}

impl<T> RegistryState<T> {
    fn usage(&self) -> ParagraphDrawDataUsage {
        ParagraphDrawDataUsage {
            entries: self.by_id.len(),
            bytes: self.used_bytes,
        }
    }
}

struct DrawDataEntry<T> {
    cache_key: ParagraphCacheKey,
    approximate_bytes: usize,
    data: Arc<T>,
}

/// Immutable draw resources pinned for one compiled frame.
pub(crate) struct ParagraphFrameDrawData<T> {
    by_node: HashMap<WidgetId, BoundParagraphDrawData<T>>,
}

impl<T> ParagraphFrameDrawData<T> {
    pub(crate) fn get(&self, node_id: WidgetId) -> Option<&BoundParagraphDrawData<T>> {
        self.by_node.get(&node_id)
    }

    pub(crate) fn len(&self) -> usize {
        self.by_node.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.by_node.is_empty()
    }
}

pub(crate) struct BoundParagraphDrawData<T> {
    pub(crate) id: ParagraphDrawDataId,
    pub(crate) result: Arc<ParagraphResult>,
    pub(crate) data: Arc<T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParagraphDrawDataError {
    MissingDrawData,
    MissingNodeDrawData {
        node_id: WidgetId,
    },
    UnknownIdentifier {
        id: ParagraphDrawDataId,
    },
    UnknownNodeIdentifier {
        node_id: WidgetId,
        id: ParagraphDrawDataId,
    },
    CacheKeyMismatch {
        id: ParagraphDrawDataId,
        expected: ParagraphCacheKey,
        actual: ParagraphCacheKey,
    },
    NodeCacheKeyMismatch {
        node_id: WidgetId,
        id: ParagraphDrawDataId,
        expected: ParagraphCacheKey,
        actual: ParagraphCacheKey,
    },
    DuplicateNode {
        node_id: WidgetId,
    },
    BudgetExceeded {
        requested: ParagraphDrawDataUsage,
        budget: ParagraphDrawDataBudget,
    },
    AccountingOverflow {
        field: &'static str,
    },
    IdentifierExhausted,
}

impl fmt::Display for ParagraphDrawDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDrawData => formatter.write_str("paragraph result has no draw data"),
            Self::MissingNodeDrawData { node_id } => {
                write!(formatter, "paragraph node {node_id} has no draw data")
            }
            Self::UnknownIdentifier { id } => {
                write!(formatter, "paragraph draw-data identifier {} is stale", id.get())
            }
            Self::UnknownNodeIdentifier { node_id, id } => write!(
                formatter,
                "paragraph node {node_id} refers to stale draw-data identifier {}",
                id.get()
            ),
            Self::CacheKeyMismatch {
                id,
                expected,
                actual,
            } => write!(
                formatter,
                "paragraph draw-data identifier {} belongs to cache key {}, not {}",
                id.get(),
                expected.get(),
                actual.get()
            ),
            Self::NodeCacheKeyMismatch {
                node_id,
                id,
                expected,
                actual,
            } => write!(
                formatter,
                "paragraph node {node_id} draw-data identifier {} belongs to cache key {}, not {}",
                id.get(),
                expected.get(),
                actual.get()
            ),
            Self::DuplicateNode { node_id } => {
                write!(formatter, "paragraph node {node_id} was bound more than once")
            }
            Self::BudgetExceeded { requested, budget } => write!(
                formatter,
                "paragraph draw-data registry needs {} entries and {} bytes, exceeding its {} entry / {} byte budget",
                requested.entries,
                requested.bytes,
                budget.max_entries,
                budget.max_bytes
            ),
            Self::AccountingOverflow { field } => {
                write!(formatter, "paragraph draw-data {field} overflowed usize")
            }
            Self::IdentifierExhausted => {
                formatter.write_str("paragraph draw-data identifiers are exhausted")
            }
        }
    }
}

impl std::error::Error for ParagraphDrawDataError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use fission_ir::op::{Color, TextDirection, TextParagraphStyle, TextStyle};
    use fission_layout::{
        LayoutSize, ParagraphCapabilities, ParagraphDescription, ParagraphGeometry,
        ParagraphStyleRun, Utf8Range,
    };

    use super::*;

    #[derive(Debug)]
    struct Resource {
        drops: Arc<AtomicUsize>,
    }

    impl Drop for Resource {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn budget(entries: usize, bytes: usize) -> ParagraphDrawDataBudget {
        ParagraphDrawDataBudget::new(
            NonZeroUsize::new(entries).unwrap(),
            NonZeroUsize::new(bytes).unwrap(),
        )
    }

    fn result(cache_key: u128, draw_data: Option<ParagraphDrawDataId>) -> Arc<ParagraphResult> {
        let text = "text";
        let mut description = ParagraphDescription::new(
            text,
            vec![ParagraphStyleRun::new(
                Utf8Range::from_byte_offsets(0, text.len()).unwrap(),
                TextStyle {
                    font_size: 14.0,
                    color: Color::BLACK,
                    underline: false,
                    font_family: None,
                    locale: None,
                    font_weight: 400,
                    font_style: Default::default(),
                    line_height: None,
                    letter_spacing: 0.0,
                    background_color: None,
                },
            )],
            TextParagraphStyle::default(),
            Some(100.0),
        );
        description.paragraph_style.text_direction = TextDirection::Ltr;
        let result = ParagraphResult::new(
            &description,
            ParagraphCacheKey::new(cache_key),
            ParagraphCapabilities::NONE,
            ParagraphGeometry::new(LayoutSize::new(40.0, 20.0)),
            Vec::new(),
        )
        .unwrap();
        Arc::new(match draw_data {
            Some(id) => result.with_draw_data(id),
            None => result,
        })
    }

    #[test]
    fn cache_key_reuses_one_registered_resource() {
        let registry = ParagraphDrawDataRegistry::new(budget(2, 128));
        let drops = Arc::new(AtomicUsize::new(0));
        let first = Arc::new(Resource {
            drops: Arc::clone(&drops),
        });
        let second = Arc::new(Resource {
            drops: Arc::clone(&drops),
        });

        let first_id = registry
            .register(ParagraphCacheKey::new(1), Arc::clone(&first), 32)
            .unwrap();
        let second_id = registry
            .register(ParagraphCacheKey::new(1), second, 64)
            .unwrap();

        assert_eq!(first_id, second_id);
        assert_eq!(
            registry.usage(),
            ParagraphDrawDataUsage {
                entries: 1,
                bytes: 32
            }
        );
        assert!(Arc::ptr_eq(
            &registry.resolve(&result(1, Some(first_id))).unwrap(),
            &first
        ));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn budget_pressure_fails_without_evicting_published_identifiers() {
        let registry = ParagraphDrawDataRegistry::new(budget(1, 32));
        let first_id = registry
            .register(
                ParagraphCacheKey::new(1),
                Arc::new(Resource {
                    drops: Arc::new(AtomicUsize::new(0)),
                }),
                32,
            )
            .unwrap();

        assert!(matches!(
            registry.register(
                ParagraphCacheKey::new(2),
                Arc::new(Resource {
                    drops: Arc::new(AtomicUsize::new(0)),
                }),
                1,
            ),
            Err(ParagraphDrawDataError::BudgetExceeded { .. })
        ));
        assert!(registry.resolve(&result(1, Some(first_id))).is_ok());
    }

    #[test]
    fn retaining_published_results_retires_measurement_probes() {
        let registry = ParagraphDrawDataRegistry::new(budget(3, 96));
        let drops = Arc::new(AtomicUsize::new(0));
        let register = |key| {
            registry
                .register(
                    ParagraphCacheKey::new(key),
                    Arc::new(Resource {
                        drops: Arc::clone(&drops),
                    }),
                    32,
                )
                .unwrap()
        };
        let probe = register(1);
        let published = register(2);
        let published_result = result(2, Some(published));

        assert_eq!(
            registry
                .retain_results([published_result.as_ref()])
                .unwrap(),
            ParagraphDrawDataUsage {
                entries: 1,
                bytes: 32
            }
        );
        assert!(matches!(
            registry.resolve(&result(1, Some(probe))),
            Err(ParagraphDrawDataError::UnknownIdentifier { .. })
        ));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn frame_binding_pins_exact_result_and_resource_across_registry_clear() {
        let registry = ParagraphDrawDataRegistry::new(budget(1, 64));
        let drops = Arc::new(AtomicUsize::new(0));
        let resource = Arc::new(Resource {
            drops: Arc::clone(&drops),
        });
        let id = registry
            .register(ParagraphCacheKey::new(9), Arc::clone(&resource), 64)
            .unwrap();
        let result = result(9, Some(id));
        let node_id = WidgetId::explicit("paragraph");
        let frame = registry
            .bind_frame([(node_id, Arc::clone(&result))])
            .unwrap();

        registry.clear();
        drop(resource);
        let bound = frame.get(node_id).unwrap();
        assert_eq!(bound.id, id);
        assert!(Arc::ptr_eq(&bound.result, &result));
        assert_eq!(frame.len(), 1);
        assert!(!frame.is_empty());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(frame);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn stale_mismatched_and_duplicate_bindings_fail_explicitly() {
        let registry = ParagraphDrawDataRegistry::new(budget(1, 64));
        let id = registry
            .register(
                ParagraphCacheKey::new(1),
                Arc::new(Resource {
                    drops: Arc::new(AtomicUsize::new(0)),
                }),
                16,
            )
            .unwrap();
        let node = WidgetId::explicit("paragraph");

        assert!(matches!(
            registry.bind_frame([(node, result(2, Some(id)))]),
            Err(ParagraphDrawDataError::NodeCacheKeyMismatch { .. })
        ));
        assert!(matches!(
            registry.bind_frame([(node, result(1, Some(ParagraphDrawDataId::new(999))))]),
            Err(ParagraphDrawDataError::UnknownNodeIdentifier { .. })
        ));
        assert!(matches!(
            registry.bind_frame([(node, result(1, Some(id))), (node, result(1, Some(id)))]),
            Err(ParagraphDrawDataError::DuplicateNode { .. })
        ));
    }

    #[test]
    fn invalid_retention_set_is_transactional() {
        let registry = ParagraphDrawDataRegistry::new(budget(1, 64));
        let id = registry
            .register(
                ParagraphCacheKey::new(1),
                Arc::new(Resource {
                    drops: Arc::new(AtomicUsize::new(0)),
                }),
                16,
            )
            .unwrap();
        let before = registry.usage();

        assert!(matches!(
            registry.retain_results([result(2, Some(id)).as_ref()]),
            Err(ParagraphDrawDataError::CacheKeyMismatch { .. })
        ));
        assert_eq!(registry.usage(), before);
        assert!(registry.resolve(&result(1, Some(id))).is_ok());
    }

    #[test]
    fn registry_and_frame_resources_are_send_and_sync_when_payload_is() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<ParagraphDrawDataRegistry<Resource>>();
        assert_send_sync::<ParagraphFrameDrawData<Resource>>();
    }
}
