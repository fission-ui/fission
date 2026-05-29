use crate::{CacheScope, CacheTag};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebRouteMode {
    Static,
    Revalidated(RevalidationPolicy),
    Server(ServerRenderPolicy),
    ServerPrivate(ServerPrivatePolicy),
    ClientApp(ClientAppPolicy),
}

impl WebRouteMode {
    pub fn cache_scope(&self) -> CacheScope {
        match self {
            Self::Static | Self::Revalidated(_) | Self::Server(_) | Self::ClientApp(_) => {
                CacheScope::Public
            }
            Self::ServerPrivate(policy) => policy.scope.clone(),
        }
    }

    pub fn revalidation(&self) -> Option<&RevalidationPolicy> {
        match self {
            Self::Revalidated(policy) => Some(policy),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevalidationPolicy {
    pub ttl: Duration,
    pub stale_while_revalidate: Option<Duration>,
    pub tags: Vec<CacheTag>,
    pub vary: Vec<String>,
}

impl RevalidationPolicy {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            stale_while_revalidate: None,
            tags: Vec::new(),
            vary: Vec::new(),
        }
    }

    pub fn stale_while_revalidate(mut self, duration: Duration) -> Self {
        self.stale_while_revalidate = Some(duration);
        self
    }

    pub fn tag(mut self, tag: impl Into<CacheTag>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn tags<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<CacheTag>,
    {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    pub fn vary(mut self, field: impl Into<String>) -> Self {
        self.vary.push(field.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerRenderPolicy {
    pub cache_scope: Option<CacheScope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerPrivatePolicy {
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
pub struct ClientAppPolicy {
    pub preload: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerResourcePolicy {
    Blocking,
    Deferred,
    IslandOnly,
    NoServerExecution,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressiveWorker {
    pub id: String,
    pub artifact: String,
    pub root_node_id: Option<String>,
    pub description: Option<String>,
}

impl ProgressiveWorker {
    pub fn new(id: impl Into<String>, artifact: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            artifact: artifact.into(),
            root_node_id: None,
            description: None,
        }
    }

    pub fn root_node_id(mut self, id: impl Into<String>) -> Self {
        self.root_node_id = Some(id.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmIsland {
    pub id: String,
    pub artifact: String,
    pub mount_id: String,
    pub description: Option<String>,
}

impl WasmIsland {
    pub fn new(
        id: impl Into<String>,
        artifact: impl Into<String>,
        mount_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            artifact: artifact.into(),
            mount_id: mount_id.into(),
            description: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebRoute {
    pub path: String,
    pub title: String,
    pub description: Option<String>,
    pub mode: WebRouteMode,
    pub workers: Vec<ProgressiveWorker>,
    pub islands: Vec<WasmIsland>,
}
