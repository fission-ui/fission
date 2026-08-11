use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use std::sync::Mutex;

#[cfg(not(target_arch = "wasm32"))]
use moka::notification::RemovalCause;
#[cfg(not(target_arch = "wasm32"))]
use moka::sync::Cache;

type Weigher<V> = Arc<dyn Fn(&V) -> u32 + Send + Sync>;
type SizeEvictionListener = Arc<dyn Fn() + Send + Sync>;

pub struct ImageCacheStore<V>
where
    V: Clone + Send + Sync + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    inner: Cache<String, V>,
    #[cfg(target_arch = "wasm32")]
    inner: Mutex<WasmCache<V>>,
}

#[cfg(target_arch = "wasm32")]
struct WasmCache<V> {
    entries: HashMap<String, WasmEntry<V>>,
    max_weight: u64,
    total_weight: u64,
    next_sequence: u64,
    weigher: Weigher<V>,
    on_size_eviction: SizeEvictionListener,
}

#[cfg(target_arch = "wasm32")]
struct WasmEntry<V> {
    value: V,
    weight: u32,
    sequence: u64,
}

impl<V> ImageCacheStore<V>
where
    V: Clone + Send + Sync + 'static,
{
    pub fn new(
        name: &'static str,
        max_weight: u64,
        weigher: impl Fn(&V) -> u32 + Send + Sync + 'static,
        on_size_eviction: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let weigher: Weigher<V> = Arc::new(weigher);
        let on_size_eviction: SizeEvictionListener = Arc::new(on_size_eviction);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let entry_weigher = Arc::clone(&weigher);
            let eviction_listener = Arc::clone(&on_size_eviction);
            Self {
                inner: Cache::<String, V>::builder()
                    .name(name)
                    .max_capacity(max_weight)
                    .weigher(move |_, value| entry_weigher(value))
                    .eviction_listener(move |_, _, cause| {
                        if matches!(cause, RemovalCause::Size) {
                            eviction_listener();
                        }
                    })
                    .build(),
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = name;
            Self {
                inner: Mutex::new(WasmCache {
                    entries: HashMap::new(),
                    max_weight,
                    total_weight: 0,
                    next_sequence: 0,
                    weigher,
                    on_size_eviction,
                }),
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.get(key)
        }

        #[cfg(target_arch = "wasm32")]
        {
            let mut cache = self.inner.lock().expect("image cache lock poisoned");
            cache.next_sequence = cache.next_sequence.wrapping_add(1);
            let sequence = cache.next_sequence;
            let entry = cache.entries.get_mut(key)?;
            entry.sequence = sequence;
            Some(entry.value.clone())
        }
    }

    pub fn insert(&self, key: String, value: V) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.insert(key, value);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let mut cache = self.inner.lock().expect("image cache lock poisoned");
            let weight = (cache.weigher)(&value);
            if let Some(previous) = cache.entries.remove(&key) {
                cache.total_weight = cache
                    .total_weight
                    .saturating_sub(u64::from(previous.weight));
            }
            cache.next_sequence = cache.next_sequence.wrapping_add(1);
            let sequence = cache.next_sequence;
            cache.entries.insert(
                key,
                WasmEntry {
                    value,
                    weight,
                    sequence,
                },
            );
            cache.total_weight = cache.total_weight.saturating_add(u64::from(weight));

            while cache.total_weight > cache.max_weight && cache.entries.len() > 1 {
                let Some(oldest_key) = cache
                    .entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.sequence)
                    .map(|(key, _)| key.clone())
                else {
                    break;
                };
                if let Some(removed) = cache.entries.remove(&oldest_key) {
                    cache.total_weight =
                        cache.total_weight.saturating_sub(u64::from(removed.weight));
                    (cache.on_size_eviction)();
                }
            }
        }
    }

    pub fn invalidate(&self, key: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.invalidate(key);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let mut cache = self.inner.lock().expect("image cache lock poisoned");
            if let Some(removed) = cache.entries.remove(key) {
                cache.total_weight = cache.total_weight.saturating_sub(u64::from(removed.weight));
            }
        }
    }

    /// Invalidates every derived image while preserving the cache's configured
    /// budget and eviction policy for subsequent inserts.
    pub fn invalidate_all(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.invalidate_all();
        }

        #[cfg(target_arch = "wasm32")]
        {
            let mut cache = self.inner.lock().expect("image cache lock poisoned");
            cache.entries.clear();
            cache.total_weight = 0;
        }
    }

    pub fn values(&self) -> Vec<V> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.iter().map(|entry| entry.1).collect()
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.inner
                .lock()
                .expect("image cache lock poisoned")
                .entries
                .values()
                .map(|entry| entry.value.clone())
                .collect()
        }
    }

    pub fn entry_count(&self) -> u64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.entry_count()
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.inner
                .lock()
                .expect("image cache lock poisoned")
                .entries
                .len() as u64
        }
    }

    pub fn weighted_size(&self) -> u64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.weighted_size()
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.inner
                .lock()
                .expect("image cache lock poisoned")
                .total_weight
        }
    }

    pub fn run_pending_tasks(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.inner.run_pending_tasks();
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::ImageCacheStore;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn wasm_cache_evicts_least_recently_used_entries_to_its_weight_budget() {
        let evictions = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&evictions);
        let cache = ImageCacheStore::new(
            "test",
            5,
            |value: &u32| *value,
            move || {
                observed.fetch_add(1, Ordering::Relaxed);
            },
        );

        cache.insert("first".into(), 3);
        cache.insert("second".into(), 2);
        assert_eq!(cache.get("first"), Some(3));
        cache.insert("third".into(), 2);

        assert_eq!(cache.get("second"), None);
        assert_eq!(cache.get("first"), Some(3));
        assert_eq!(cache.get("third"), Some(2));
        assert_eq!(cache.weighted_size(), 5);
        assert_eq!(evictions.load(Ordering::Relaxed), 1);
    }
}
