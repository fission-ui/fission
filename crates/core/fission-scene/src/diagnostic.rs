//! Diagnostics and observable scene-pass accounting.

use serde::{Deserialize, Serialize};

use crate::{AssetId, NodeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SceneSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneDiagnostic {
    pub severity: SceneSeverity,
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub node: Option<NodeId>,
    #[serde(default)]
    pub asset: Option<AssetId>,
}

impl SceneDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: SceneSeverity::Error,
            code: code.into(),
            message: message.into(),
            node: None,
            asset: None,
        }
    }

    pub fn for_node(mut self, node: NodeId) -> Self {
        self.node = Some(node);
        self
    }

    pub fn for_asset(mut self, asset: AssetId) -> Self {
        self.asset = Some(asset);
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenePassStats {
    pub considered: u32,
    pub rejected: u32,
    pub culled: u32,
    pub batches: u32,
    pub drawn: u32,
    pub retained_resources: u32,
    pub uploaded_resources: u32,
}
