use fission_core::{ActionEnvelope, ActionInput, WidgetId};
use fission_ir::CoreIR;
use serde::{Deserialize, Serialize};

pub const APP_WORKER_PROTOCOL_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRequest {
    pub request_id: u64,
    pub command: WorkerCommand,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerCommand {
    Handshake,
    Snapshot,
    Restore { snapshot: StateSnapshot },
    Build,
    Dispatch { dispatch: AppAction },
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub request_id: u64,
    pub result: Result<WorkerOutput, WorkerError>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOutput {
    Handshake(WorkerHandshake),
    Snapshot(StateSnapshot),
    Restored,
    Frame(AppFrame),
    Dispatched(DispatchResult),
    ShuttingDown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHandshake {
    pub protocol_version: u32,
    pub generation: u64,
    pub application_id: String,
    pub state_schema: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub application_id: String,
    pub schema: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppFrame {
    pub application_name: String,
    pub route: Option<String>,
    pub generation: u64,
    pub ir: CoreIR,
}

impl AppFrame {
    pub fn validate(&self) -> Result<(), WorkerError> {
        let root = self
            .ir
            .root
            .ok_or_else(|| WorkerError::new("invalid_frame", "Core IR has no root"))?;
        if !self.ir.nodes.contains_key(&root) {
            return Err(WorkerError::new(
                "invalid_frame",
                format!("Core IR root {root} is missing"),
            ));
        }
        for node in self.ir.nodes.values() {
            for child in &node.children {
                if !self.ir.nodes.contains_key(child) {
                    return Err(WorkerError::new(
                        "invalid_frame",
                        format!("Core IR node {} references missing child {child}", node.id),
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn action_ids(&self) -> Vec<u128> {
        let mut ids = self
            .ir
            .nodes
            .values()
            .filter_map(|node| match &node.op {
                fission_ir::Op::Semantics(semantics) => Some(
                    semantics
                        .actions
                        .entries
                        .iter()
                        .filter(|entry| entry.action_id != 0)
                        .map(|entry| entry.action_id),
                ),
                _ => None,
            })
            .flatten()
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppAction {
    pub action: ActionEnvelope,
    pub target: WidgetId,
    pub input: Vec<u8>,
}

impl AppAction {
    pub fn new(action: ActionEnvelope, target: WidgetId, input: &ActionInput) -> Self {
        Self {
            action,
            target,
            input: input
                .unscoped()
                .encode_opaque()
                .expect("Fission action input must remain serializable"),
        }
    }

    pub fn decode_input(&self) -> Result<ActionInput, WorkerError> {
        ActionInput::decode_opaque(&self.input).map_err(|error| {
            WorkerError::new(
                "invalid_action_input",
                format!("action input could not be decoded: {error}"),
            )
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DispatchResult {
    pub state_changed: bool,
    pub frame: AppFrame,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerError {
    pub code: String,
    pub message: String,
}

impl WorkerError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

pub fn encode_line<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    Ok(line)
}

pub fn decode_line<T: for<'de> Deserialize<'de>>(line: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(line)
}
