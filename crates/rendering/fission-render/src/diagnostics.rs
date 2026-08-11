use serde::{Deserialize, Serialize};

use crate::capabilities::BackendIdentity;
use crate::frame::FrameId;
use crate::surface::SessionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticCategory {
    Capability,
    Lifecycle,
    Surface,
    Device,
    Resource,
    ExternalSurface,
    Performance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticProvenance {
    pub frame_id: Option<FrameId>,
    pub node_id: Option<fission_ir::WidgetId>,
    pub operation_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendDiagnostic {
    pub severity: DiagnosticSeverity,
    pub category: DiagnosticCategory,
    pub code: String,
    pub message: String,
    pub provenance: Option<DiagnosticProvenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BackendCounters {
    pub frames_rendered: u64,
    pub frames_presented: u64,
    pub surface_recoveries: u64,
    pub device_recoveries: u64,
    pub dropped_frames: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheDiagnostics {
    pub name: String,
    pub entries: u64,
    pub used_bytes: u64,
    pub budget_bytes: Option<u64>,
    pub evictions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendDiagnostics {
    pub identity: BackendIdentity,
    pub session_state: SessionState,
    pub counters: BackendCounters,
    pub caches: Vec<CacheDiagnostics>,
    pub recent_events: Vec<BackendDiagnostic>,
}

impl BackendDiagnostics {
    pub fn new(identity: BackendIdentity, session_state: SessionState) -> Self {
        Self {
            identity,
            session_state,
            counters: BackendCounters::default(),
            caches: Vec::new(),
            recent_events: Vec::new(),
        }
    }
}
