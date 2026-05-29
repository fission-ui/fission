use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheError {
    message: String,
}

impl CacheError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CacheError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CacheKey(String);

impl CacheKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CacheTag(String);

impl CacheTag {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CacheTag {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CacheTag {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheScope {
    Public,
    PrivateSession,
    PrivateUser,
    PrivateTenant,
    NoStore,
}

impl CacheScope {
    pub fn is_publicly_shareable(&self) -> bool {
        matches!(self, Self::Public)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub route_path: String,
    pub content_type: String,
    pub source: String,
}

impl CacheMetadata {
    pub fn full_page(route_path: impl Into<String>) -> Self {
        Self {
            route_path: route_path.into(),
            content_type: "text/html; charset=utf-8".to_string(),
            source: "fission-shell-server".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedPage {
    pub html: String,
    pub css: String,
    pub status: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredJobResult {
    pub job_name: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheValue {
    FullPage(RenderedPage),
    Fragment(String),
    JobResult(StoredJobResult),
    AssetMetadata(BTreeMap<String, String>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: CacheKey,
    pub value: CacheValue,
    pub scope: CacheScope,
    pub created_at: SystemTime,
    pub fresh_until: SystemTime,
    pub stale_until: Option<SystemTime>,
    pub tags: Vec<CacheTag>,
    pub vary: Vec<String>,
    pub content_hash: u64,
    pub metadata: CacheMetadata,
}

impl CacheEntry {
    pub fn full_page(
        key: CacheKey,
        page: RenderedPage,
        scope: CacheScope,
        ttl: Duration,
        stale_while_revalidate: Option<Duration>,
        tags: Vec<CacheTag>,
        metadata: CacheMetadata,
    ) -> Self {
        let created_at = SystemTime::now();
        let fresh_until = created_at + ttl;
        let stale_until = stale_while_revalidate.map(|stale| fresh_until + stale);
        let content_hash = stable_hash(page.html.as_bytes()) ^ stable_hash(page.css.as_bytes());
        Self {
            key,
            value: CacheValue::FullPage(page),
            scope,
            created_at,
            fresh_until,
            stale_until,
            tags,
            vary: Vec::new(),
            content_hash,
            metadata,
        }
    }

    pub fn freshness(&self, now: SystemTime) -> Freshness {
        if now <= self.fresh_until {
            Freshness::Fresh
        } else if self
            .stale_until
            .is_some_and(|stale_until| now <= stale_until)
        {
            Freshness::Stale
        } else {
            Freshness::Expired
        }
    }

    pub fn rendered_page(&self) -> Option<&RenderedPage> {
        match &self.value {
            CacheValue::FullPage(page) => Some(page),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    Stale,
    Expired,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvalidationReport {
    pub removed_keys: usize,
    pub removed_tags: usize,
}

pub trait Cache: Send + Sync + 'static {
    fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError>;
    fn put(&self, entry: CacheEntry) -> Result<(), CacheError>;
    fn remove(&self, key: &CacheKey) -> Result<(), CacheError>;
    fn invalidate_tag(&self, tag: &CacheTag) -> Result<InvalidationReport, CacheError>;

    fn invalidate_tags(&self, tags: &[CacheTag]) -> Result<InvalidationReport, CacheError> {
        let mut out = InvalidationReport::default();
        for tag in tags {
            let report = self.invalidate_tag(tag)?;
            out.removed_keys += report.removed_keys;
            out.removed_tags += report.removed_tags;
        }
        Ok(out)
    }

    fn contains_fresh(&self, key: &CacheKey, now: SystemTime) -> Result<bool, CacheError> {
        Ok(self
            .get(key)?
            .is_some_and(|entry| entry.freshness(now) == Freshness::Fresh))
    }
}

#[derive(Clone, Debug)]
pub struct MokaCacheOptions {
    pub max_capacity: u64,
}

impl Default for MokaCacheOptions {
    fn default() -> Self {
        Self {
            max_capacity: 10_000,
        }
    }
}

pub struct MokaCache {
    inner: moka::sync::Cache<String, CacheEntry>,
    tag_index: Mutex<BTreeMap<String, BTreeSet<String>>>,
}

impl MokaCache {
    pub fn new(options: MokaCacheOptions) -> Self {
        Self {
            inner: moka::sync::Cache::builder()
                .max_capacity(options.max_capacity)
                .build(),
            tag_index: Mutex::new(BTreeMap::new()),
        }
    }
}

impl Default for MokaCache {
    fn default() -> Self {
        Self::new(MokaCacheOptions::default())
    }
}

impl Cache for MokaCache {
    fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError> {
        Ok(self.inner.get(key.as_str()))
    }

    fn put(&self, entry: CacheEntry) -> Result<(), CacheError> {
        if matches!(entry.scope, CacheScope::NoStore) {
            return Ok(());
        }
        let key = entry.key.as_str().to_string();
        self.inner.insert(key.clone(), entry.clone());
        let mut tag_index = self
            .tag_index
            .lock()
            .map_err(|_| CacheError::new("cache tag index lock poisoned"))?;
        for tag in entry.tags {
            tag_index
                .entry(tag.as_str().to_string())
                .or_default()
                .insert(key.clone());
        }
        Ok(())
    }

    fn remove(&self, key: &CacheKey) -> Result<(), CacheError> {
        self.inner.invalidate(key.as_str());
        let mut tag_index = self
            .tag_index
            .lock()
            .map_err(|_| CacheError::new("cache tag index lock poisoned"))?;
        for keys in tag_index.values_mut() {
            keys.remove(key.as_str());
        }
        tag_index.retain(|_, keys| !keys.is_empty());
        Ok(())
    }

    fn invalidate_tag(&self, tag: &CacheTag) -> Result<InvalidationReport, CacheError> {
        let keys = {
            let mut tag_index = self
                .tag_index
                .lock()
                .map_err(|_| CacheError::new("cache tag index lock poisoned"))?;
            tag_index.remove(tag.as_str()).unwrap_or_default()
        };
        for key in &keys {
            self.inner.invalidate(key);
        }
        Ok(InvalidationReport {
            removed_keys: keys.len(),
            removed_tags: usize::from(!keys.is_empty()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheLayerPolicy {
    WriteThrough,
    ReadOnly,
    HotOnly,
}

pub struct CachePipeline {
    layers: Vec<(Arc<dyn Cache>, CacheLayerPolicy)>,
}

#[cfg(feature = "redis")]
pub struct RedisCache {
    client: redis::Client,
    prefix: String,
}

#[cfg(feature = "redis")]
impl RedisCache {
    pub fn new(url: &str, prefix: impl Into<String>) -> Result<Self, CacheError> {
        let client = redis::Client::open(url)
            .map_err(|error| CacheError::new(format!("failed to create redis client: {error}")))?;
        Ok(Self {
            client,
            prefix: prefix.into(),
        })
    }

    fn entry_key(&self, key: &CacheKey) -> String {
        format!("{}:entry:{}", self.prefix, key.as_str())
    }

    fn tag_key(&self, tag: &CacheTag) -> String {
        format!("{}:tag:{}", self.prefix, tag.as_str())
    }
}

#[cfg(feature = "redis")]
impl Cache for RedisCache {
    fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError> {
        let mut conn = self
            .client
            .get_connection()
            .map_err(|error| CacheError::new(format!("failed to connect to redis: {error}")))?;
        let data: Option<Vec<u8>> = redis::Commands::get(&mut conn, self.entry_key(key))
            .map_err(|error| CacheError::new(format!("failed to read redis cache: {error}")))?;
        data.map(|bytes| {
            bincode::deserialize::<CacheEntry>(&bytes).map_err(|error| {
                CacheError::new(format!("failed to decode redis cache entry: {error}"))
            })
        })
        .transpose()
    }

    fn put(&self, entry: CacheEntry) -> Result<(), CacheError> {
        if matches!(entry.scope, CacheScope::NoStore) {
            return Ok(());
        }
        let mut conn = self
            .client
            .get_connection()
            .map_err(|error| CacheError::new(format!("failed to connect to redis: {error}")))?;
        let key = self.entry_key(&entry.key);
        let bytes = bincode::serialize(&entry).map_err(|error| {
            CacheError::new(format!("failed to encode redis cache entry: {error}"))
        })?;
        let ttl_secs = entry
            .stale_until
            .or(Some(entry.fresh_until))
            .and_then(|expires| expires.duration_since(SystemTime::now()).ok())
            .map(|duration| duration.as_secs().max(1))
            .unwrap_or(60);
        let _: () = redis::Commands::set_ex(&mut conn, &key, bytes, ttl_secs)
            .map_err(|error| CacheError::new(format!("failed to write redis cache: {error}")))?;
        for tag in &entry.tags {
            let tag_key = self.tag_key(tag);
            let _: () = redis::Commands::sadd(&mut conn, &tag_key, &key).map_err(|error| {
                CacheError::new(format!("failed to update redis tag index: {error}"))
            })?;
            let _: () =
                redis::Commands::expire(&mut conn, &tag_key, ttl_secs as i64).map_err(|error| {
                    CacheError::new(format!("failed to expire redis tag index: {error}"))
                })?;
        }
        Ok(())
    }

    fn remove(&self, key: &CacheKey) -> Result<(), CacheError> {
        let mut conn = self
            .client
            .get_connection()
            .map_err(|error| CacheError::new(format!("failed to connect to redis: {error}")))?;
        let _: () = redis::Commands::del(&mut conn, self.entry_key(key)).map_err(|error| {
            CacheError::new(format!("failed to remove redis cache entry: {error}"))
        })?;
        Ok(())
    }

    fn invalidate_tag(&self, tag: &CacheTag) -> Result<InvalidationReport, CacheError> {
        let mut conn = self
            .client
            .get_connection()
            .map_err(|error| CacheError::new(format!("failed to connect to redis: {error}")))?;
        let tag_key = self.tag_key(tag);
        let keys: Vec<String> = redis::Commands::smembers(&mut conn, &tag_key)
            .map_err(|error| CacheError::new(format!("failed to read redis tag index: {error}")))?;
        for key in &keys {
            let _: () = redis::Commands::del(&mut conn, key).map_err(|error| {
                CacheError::new(format!("failed to delete redis cache key: {error}"))
            })?;
        }
        let _: () = redis::Commands::del(&mut conn, &tag_key)
            .map_err(|error| CacheError::new(format!("failed to delete redis tag key: {error}")))?;
        Ok(InvalidationReport {
            removed_keys: keys.len(),
            removed_tags: usize::from(!keys.is_empty()),
        })
    }
}

impl CachePipeline {
    pub fn new(layers: Vec<Arc<dyn Cache>>) -> Self {
        Self {
            layers: layers
                .into_iter()
                .map(|layer| (layer, CacheLayerPolicy::WriteThrough))
                .collect(),
        }
    }

    pub fn with_policies(layers: Vec<(Arc<dyn Cache>, CacheLayerPolicy)>) -> Self {
        Self { layers }
    }

    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}

impl Cache for CachePipeline {
    fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError> {
        for (index, (layer, _)) in self.layers.iter().enumerate() {
            if let Some(entry) = layer.get(key)? {
                for (prior, policy) in &self.layers[..index] {
                    if matches!(
                        policy,
                        CacheLayerPolicy::WriteThrough | CacheLayerPolicy::HotOnly
                    ) {
                        prior.put(entry.clone())?;
                    }
                }
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    fn put(&self, entry: CacheEntry) -> Result<(), CacheError> {
        for (layer, policy) in &self.layers {
            if matches!(
                policy,
                CacheLayerPolicy::WriteThrough | CacheLayerPolicy::HotOnly
            ) {
                layer.put(entry.clone())?;
            }
        }
        Ok(())
    }

    fn remove(&self, key: &CacheKey) -> Result<(), CacheError> {
        for (layer, _) in &self.layers {
            layer.remove(key)?;
        }
        Ok(())
    }

    fn invalidate_tag(&self, tag: &CacheTag) -> Result<InvalidationReport, CacheError> {
        let mut out = InvalidationReport::default();
        for (layer, _) in &self.layers {
            let report = layer.invalidate_tag(tag)?;
            out.removed_keys += report.removed_keys;
            out.removed_tags += report.removed_tags;
        }
        Ok(out)
    }
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, tag: &str, ttl: Duration) -> CacheEntry {
        CacheEntry::full_page(
            CacheKey::new(key),
            RenderedPage {
                html: format!("<p>{key}</p>"),
                css: String::new(),
                status: 200,
            },
            CacheScope::Public,
            ttl,
            Some(Duration::from_millis(50)),
            vec![CacheTag::new(tag)],
            CacheMetadata::full_page("/"),
        )
    }

    #[test]
    fn moka_cache_stores_and_invalidates_by_tag() {
        let cache = MokaCache::default();
        let key = CacheKey::new("page:/cards");
        cache
            .put(entry(key.as_str(), "catalog", Duration::from_secs(60)))
            .unwrap();

        assert!(cache.get(&key).unwrap().is_some());
        let report = cache.invalidate_tag(&CacheTag::new("catalog")).unwrap();
        assert_eq!(report.removed_keys, 1);
        assert!(cache.get(&key).unwrap().is_none());
    }

    #[test]
    fn moka_cache_does_not_store_no_store_entries() {
        let cache = MokaCache::default();
        let mut entry = entry("page:/private", "private", Duration::from_secs(60));
        entry.scope = CacheScope::NoStore;
        let key = entry.key.clone();

        cache.put(entry).unwrap();

        assert!(cache.get(&key).unwrap().is_none());
    }

    #[test]
    fn moka_cache_remove_cleans_tag_index() {
        let cache = MokaCache::default();
        let key = CacheKey::new("page:/remove");
        cache
            .put(entry(key.as_str(), "catalog", Duration::from_secs(60)))
            .unwrap();

        cache.remove(&key).unwrap();
        let report = cache.invalidate_tag(&CacheTag::new("catalog")).unwrap();

        assert_eq!(report.removed_keys, 0);
        assert_eq!(report.removed_tags, 0);
    }

    #[test]
    fn cache_entry_reports_fresh_stale_and_expired() {
        let entry = entry("page:/", "home", Duration::from_millis(10));
        assert_eq!(entry.freshness(entry.created_at), Freshness::Fresh);
        assert_eq!(
            entry.freshness(entry.fresh_until + Duration::from_millis(5)),
            Freshness::Stale
        );
        assert_eq!(
            entry.freshness(entry.fresh_until + Duration::from_millis(100)),
            Freshness::Expired
        );
    }

    #[test]
    fn pipeline_promotes_lower_layer_hits_to_hot_layer() {
        let hot = Arc::new(MokaCache::default());
        let shared = Arc::new(MokaCache::default());
        let key = CacheKey::new("page:/promote");
        shared
            .put(entry(key.as_str(), "catalog", Duration::from_secs(60)))
            .unwrap();
        let pipeline = CachePipeline::new(vec![hot.clone(), shared]);

        assert!(pipeline.get(&key).unwrap().is_some());
        assert!(hot.get(&key).unwrap().is_some());
    }

    #[test]
    fn pipeline_respects_read_only_layers_on_put() {
        let read_only = Arc::new(MokaCache::default());
        let writable = Arc::new(MokaCache::default());
        let key = CacheKey::new("page:/readonly");
        let pipeline = CachePipeline::with_policies(vec![
            (read_only.clone(), CacheLayerPolicy::ReadOnly),
            (writable.clone(), CacheLayerPolicy::WriteThrough),
        ]);

        pipeline
            .put(entry(key.as_str(), "catalog", Duration::from_secs(60)))
            .unwrap();

        assert!(read_only.get(&key).unwrap().is_none());
        assert!(writable.get(&key).unwrap().is_some());
    }

    #[test]
    fn pipeline_invalidation_reaches_all_layers() {
        let hot = Arc::new(MokaCache::default());
        let shared = Arc::new(MokaCache::default());
        let key = CacheKey::new("page:/invalidate");
        hot.put(entry(key.as_str(), "catalog", Duration::from_secs(60)))
            .unwrap();
        shared
            .put(entry(key.as_str(), "catalog", Duration::from_secs(60)))
            .unwrap();
        let pipeline = CachePipeline::new(vec![hot.clone(), shared.clone()]);

        let report = pipeline.invalidate_tag(&CacheTag::new("catalog")).unwrap();

        assert_eq!(report.removed_keys, 2);
        assert!(hot.get(&key).unwrap().is_none());
        assert!(shared.get(&key).unwrap().is_none());
    }

    #[cfg(feature = "redis")]
    #[test]
    fn redis_cache_stores_and_invalidates_by_tag_when_available() {
        let Ok(url) = std::env::var("FISSION_REDIS_URL") else {
            return;
        };
        let prefix = format!("fission-test-{}", std::process::id());
        let cache = RedisCache::new(&url, &prefix).unwrap();
        let key = CacheKey::new("page:/redis");
        cache
            .put(entry(
                key.as_str(),
                "redis-catalog",
                Duration::from_secs(60),
            ))
            .unwrap();

        assert!(cache.get(&key).unwrap().is_some());
        let report = cache
            .invalidate_tag(&CacheTag::new("redis-catalog"))
            .unwrap();
        assert_eq!(report.removed_keys, 1);
        assert!(cache.get(&key).unwrap().is_none());
    }
}
