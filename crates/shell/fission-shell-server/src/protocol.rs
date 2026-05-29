use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MainToWorker {
    Boot(WorkerBoot),
    Event(WorkerDomEvent),
    Resize(WorkerResize),
    VisibilityChanged { visible: bool },
    ThemeChanged { theme_id: String },
    LocaleChanged { locale: String },
    Response(WorkerResponse),
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerBoot {
    pub protocol_version: u16,
    pub worker_instance_id: String,
    pub route_id: String,
    pub base_url: String,
    pub root_node_id: String,
    pub locale: String,
    pub theme_id: String,
    pub feature_flags: Vec<String>,
    #[serde(default)]
    pub props: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerDomEvent {
    pub sequence: u64,
    pub target_node_id: u64,
    pub event_kind: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub modifiers: Vec<String>,
    #[serde(default)]
    pub pointer: Option<WorkerPointer>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerPointer {
    pub x: f64,
    pub y: f64,
    pub button: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerResize {
    pub width: f64,
    pub height: f64,
    pub scale_factor: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub request_id: u64,
    pub ok: bool,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerToMain {
    Ready,
    DomBatch(DomBatch),
    Request(WorkerRequest),
    Navigate(NavigateRequest),
    Log(WorkerLog),
    Error(WorkerError),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DomBatch {
    pub sequence: u64,
    #[serde(default)]
    pub transaction_id: Option<String>,
    pub ops: Vec<DomOp>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DomOp {
    SetText {
        node: u64,
        text: String,
    },
    SetAttr {
        node: u64,
        name: String,
        value: String,
    },
    RemoveAttr {
        node: u64,
        name: String,
    },
    AddClass {
        node: u64,
        class: String,
    },
    RemoveClass {
        node: u64,
        class: String,
    },
    ToggleClass {
        node: u64,
        class: String,
        enabled: bool,
    },
    SetStyleVar {
        node: u64,
        name: String,
        value: String,
    },
    SetHidden {
        node: u64,
        hidden: bool,
    },
    SetValue {
        node: u64,
        value: String,
    },
    SetChecked {
        node: u64,
        checked: bool,
    },
    Focus {
        node: u64,
    },
    Blur {
        node: u64,
    },
    ScrollIntoView {
        node: u64,
        block: ScrollBlock,
    },
    SetScroll {
        node: u64,
        x: f64,
        y: f64,
    },
    PushHistory {
        url: String,
    },
    ReplaceHistory {
        url: String,
    },
    Announce {
        politeness: AriaPoliteness,
        text: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollBlock {
    Start,
    Center,
    End,
    Nearest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AriaPoliteness {
    Polite,
    Assertive,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerRequest {
    pub request_id: u64,
    pub kind: WorkerRequestKind,
    #[serde(default)]
    pub payload: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerRequestKind {
    FetchServerAction,
    ReadLocalStorage,
    WriteLocalStorage,
    RemoveLocalStorage,
    ReadSessionStorage,
    WriteSessionStorage,
    ClipboardWriteText,
    ClipboardReadText,
    CurrentLocation,
    DocumentVisibility,
    MatchMedia,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigateRequest {
    pub url: String,
    pub mode: NavigateMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigateMode {
    Push,
    Replace,
    FullDocument,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLog {
    pub level: WorkerLogLevel,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerError {
    pub message: String,
    pub stack: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_protocol_round_trips_dom_batches() {
        let message = WorkerToMain::DomBatch(DomBatch {
            sequence: 7,
            transaction_id: Some("nav".into()),
            ops: vec![
                DomOp::SetHidden {
                    node: 42,
                    hidden: false,
                },
                DomOp::AddClass {
                    node: 42,
                    class: "open".into(),
                },
            ],
        });
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(encoded.contains("dom_batch"));
        let decoded: WorkerToMain = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, message);
    }
}
