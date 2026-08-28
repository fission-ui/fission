use crate::{CacheScope, CacheTag};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
/// Rendering and caching strategy for an SSR route.
pub enum WebRouteMode {
    /// Render immutable output suitable for indefinite public caching.
    Static,
    /// Serve cached output and regenerate it according to a policy.
    Revalidated(RevalidationPolicy),
    /// Render each request, optionally using the configured public cache scope.
    Server(ServerRenderPolicy),
    /// Render request-specific output that must not enter a shared public cache.
    ServerPrivate(ServerPrivatePolicy),
    /// Serve a browser-owned client application at this route.
    ClientApp(ClientAppPolicy),
}

impl WebRouteMode {
    /// Returns the effective cache isolation required by this mode.
    pub fn cache_scope(&self) -> CacheScope {
        match self {
            Self::Static | Self::Revalidated(_) | Self::Server(_) | Self::ClientApp(_) => {
                CacheScope::Public
            }
            Self::ServerPrivate(policy) => policy.scope.clone(),
        }
    }

    /// Returns the revalidation policy when this is a revalidated route.
    pub fn revalidation(&self) -> Option<&RevalidationPolicy> {
        match self {
            Self::Revalidated(policy) => Some(policy),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Lifetime, stale-serving, tagging, and variation rules for cached output.
pub struct RevalidationPolicy {
    /// Time for which newly generated output is fresh.
    pub ttl: Duration,
    /// Optional additional interval during which stale output may be served.
    pub stale_while_revalidate: Option<Duration>,
    /// Tags through which related cache entries can be invalidated together.
    pub tags: Vec<CacheTag>,
    /// Request field names incorporated into the cache key.
    pub vary: Vec<String>,
}

impl RevalidationPolicy {
    /// Creates a policy with a fresh TTL and no stale window, tags, or variance.
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            stale_while_revalidate: None,
            tags: Vec::new(),
            vary: Vec::new(),
        }
    }

    /// Allows stale content to be served for `duration` while it is refreshed.
    pub fn stale_while_revalidate(mut self, duration: Duration) -> Self {
        self.stale_while_revalidate = Some(duration);
        self
    }

    /// Adds one invalidation tag.
    pub fn tag(mut self, tag: impl Into<CacheTag>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Adds multiple invalidation tags.
    pub fn tags<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<CacheTag>,
    {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Adds one request field to the cache-key variance set.
    pub fn vary(mut self, field: impl Into<String>) -> Self {
        self.vary.push(field.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Per-request rendering policy for publicly shareable routes.
pub struct ServerRenderPolicy {
    /// Optional explicit cache scope; `None` defers to server configuration.
    pub cache_scope: Option<CacheScope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Cache isolation policy for request-specific routes.
pub struct ServerPrivatePolicy {
    /// Private scope, normally per session or per authenticated user.
    pub scope: CacheScope,
}

impl Default for ServerPrivatePolicy {
    fn default() -> Self {
        Self {
            scope: CacheScope::PrivateSession,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Browser application boot policy for a client-owned route.
pub struct ClientAppPolicy {
    /// Whether the browser artifact should be preloaded by the document.
    pub preload: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Determines where and when a route resource is executed.
pub enum ServerResourcePolicy {
    /// Complete the resource before producing the server response.
    Blocking,
    /// Produce initial output while the resource completes later.
    Deferred,
    /// Execute only inside a browser island.
    IslandOnly,
    /// Never execute this resource during SSR.
    NoServerExecution,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Browser worker attached to a progressively enhanced route.
pub struct ProgressiveWorker {
    /// Stable worker identifier within the route.
    pub id: String,
    /// URL or generated artifact path loaded by the browser.
    pub artifact: String,
    /// Optional exported entry function.
    pub entry: Option<String>,
    /// Optional DOM node whose subtree the worker owns.
    pub root_node_id: Option<String>,
    /// Human-readable purpose shown by tooling.
    pub description: Option<String>,
}

impl ProgressiveWorker {
    /// Creates a worker declaration with an identifier and artifact path.
    pub fn new(id: impl Into<String>, artifact: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            artifact: artifact.into(),
            entry: None,
            root_node_id: None,
            description: None,
        }
    }

    /// Selects the artifact's exported entry function.
    pub fn entry(mut self, entry: impl Into<String>) -> Self {
        self.entry = Some(entry.into());
        self
    }

    /// Limits the worker to a specific rendered DOM root.
    pub fn root_node_id(mut self, id: impl Into<String>) -> Self {
        self.root_node_id = Some(id.into());
        self
    }

    /// Adds a tooling-facing description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Independently hydrated browser component embedded in an SSR document.
pub struct WasmIsland {
    /// Stable island identifier within the route.
    pub id: String,
    /// URL or generated artifact path loaded by the browser.
    pub artifact: String,
    /// Optional exported entry function.
    pub entry: Option<String>,
    /// DOM element ID where the island mounts.
    pub mount_id: String,
    /// Human-readable purpose shown by tooling.
    pub description: Option<String>,
}

impl WasmIsland {
    /// Creates an island declaration and its mount-point identifier.
    pub fn new(
        id: impl Into<String>,
        artifact: impl Into<String>,
        mount_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            artifact: artifact.into(),
            entry: None,
            mount_id: mount_id.into(),
            description: None,
        }
    }

    /// Selects the artifact's exported entry function.
    pub fn entry(mut self, entry: impl Into<String>) -> Self {
        self.entry = Some(entry.into());
        self
    }

    /// Adds a tooling-facing description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Public metadata and execution policy for one registered SSR route.
pub struct WebRoute {
    /// Normalized route pattern or path.
    pub path: String,
    /// Default browser-document title.
    pub title: String,
    /// Optional default document description.
    pub description: Option<String>,
    /// Rendering and caching mode.
    pub mode: WebRouteMode,
    /// Progressive workers attached to the route.
    pub workers: Vec<ProgressiveWorker>,
    /// Hydrated WebAssembly islands attached to the route.
    pub islands: Vec<WasmIsland>,
    /// Serialized JSON-LD documents emitted into the page.
    pub structured_data: Vec<String>,
}
