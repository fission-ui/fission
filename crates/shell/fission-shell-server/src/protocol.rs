use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

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

impl DomBatch {
    pub fn validate(&self, policy: &WorkerDomPolicy) -> Result<(), WorkerProtocolError> {
        for op in &self.ops {
            op.validate(policy)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkerDomPolicy {
    allowed_nodes: BTreeSet<u64>,
    allowed_semantics: BTreeSet<String>,
    allow_navigation: bool,
    allowed_url_prefixes: Vec<String>,
}

impl WorkerDomPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow_node(mut self, node: u64) -> Self {
        self.allowed_nodes.insert(node);
        self
    }

    pub fn allow_nodes<I>(mut self, nodes: I) -> Self
    where
        I: IntoIterator<Item = u64>,
    {
        self.allowed_nodes.extend(nodes);
        self
    }

    pub fn allow_semantics(mut self, semantics: impl Into<String>) -> Self {
        self.allowed_semantics.insert(semantics.into());
        self
    }

    pub fn allow_semantics_many<I, S>(mut self, semantics: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_semantics
            .extend(semantics.into_iter().map(Into::into));
        self
    }

    pub fn allow_navigation_to_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.allow_navigation = true;
        self.allowed_url_prefixes.push(prefix.into());
        self
    }

    fn can_mutate_node(&self, node: u64) -> bool {
        self.allowed_nodes.contains(&node)
    }

    fn can_mutate_semantics(&self, semantics: &str) -> bool {
        self.allowed_semantics.contains(semantics)
    }

    fn can_navigate_to(&self, url: &str) -> bool {
        self.allow_navigation
            && safe_navigation_url(url)
            && self
                .allowed_url_prefixes
                .iter()
                .any(|prefix| url.starts_with(prefix))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerProtocolError {
    message: String,
}

impl WorkerProtocolError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for WorkerProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for WorkerProtocolError {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DomOp {
    SetText {
        node: u64,
        text: String,
    },
    SetTextBySemantics {
        semantics: String,
        text: String,
    },
    ReplaceChildrenHtmlBySemantics {
        semantics: String,
        html: String,
    },
    SetStylesheet {
        id: String,
        css: String,
    },
    SetAttr {
        node: u64,
        name: String,
        value: String,
    },
    SetAttrBySemantics {
        semantics: String,
        name: String,
        value: String,
    },
    RemoveAttr {
        node: u64,
        name: String,
    },
    RemoveAttrBySemantics {
        semantics: String,
        name: String,
    },
    AddClass {
        node: u64,
        class: String,
    },
    AddClassBySemantics {
        semantics: String,
        class: String,
    },
    RemoveClass {
        node: u64,
        class: String,
    },
    RemoveClassBySemantics {
        semantics: String,
        class: String,
    },
    ToggleClass {
        node: u64,
        class: String,
        enabled: bool,
    },
    ToggleClassBySemantics {
        semantics: String,
        class: String,
        enabled: bool,
    },
    SetStyleVar {
        node: u64,
        name: String,
        value: String,
    },
    SetStyleVarBySemantics {
        semantics: String,
        name: String,
        value: String,
    },
    SetHidden {
        node: u64,
        hidden: bool,
    },
    SetHiddenBySemantics {
        semantics: String,
        hidden: bool,
    },
    SetValue {
        node: u64,
        value: String,
    },
    SetValueBySemantics {
        semantics: String,
        value: String,
    },
    SetChecked {
        node: u64,
        checked: bool,
    },
    SetCheckedBySemantics {
        semantics: String,
        checked: bool,
    },
    Focus {
        node: u64,
    },
    FocusBySemantics {
        semantics: String,
    },
    Blur {
        node: u64,
    },
    BlurBySemantics {
        semantics: String,
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

impl DomOp {
    fn validate(&self, policy: &WorkerDomPolicy) -> Result<(), WorkerProtocolError> {
        match self {
            Self::SetText { node, .. }
            | Self::RemoveAttr { node, .. }
            | Self::AddClass { node, .. }
            | Self::RemoveClass { node, .. }
            | Self::ToggleClass { node, .. }
            | Self::SetHidden { node, .. }
            | Self::SetValue { node, .. }
            | Self::SetChecked { node, .. }
            | Self::Focus { node }
            | Self::Blur { node }
            | Self::ScrollIntoView { node, .. }
            | Self::SetScroll { node, .. } => validate_node(policy, *node),
            Self::SetTextBySemantics { semantics, .. }
            | Self::RemoveAttrBySemantics { semantics, .. }
            | Self::AddClassBySemantics { semantics, .. }
            | Self::RemoveClassBySemantics { semantics, .. }
            | Self::ToggleClassBySemantics { semantics, .. }
            | Self::SetHiddenBySemantics { semantics, .. }
            | Self::SetValueBySemantics { semantics, .. }
            | Self::SetCheckedBySemantics { semantics, .. }
            | Self::FocusBySemantics { semantics }
            | Self::BlurBySemantics { semantics } => validate_semantics(policy, semantics),
            Self::SetAttr { node, name, value } => {
                validate_node(policy, *node)?;
                validate_attr(name, value)
            }
            Self::SetAttrBySemantics {
                semantics,
                name,
                value,
            } => {
                validate_semantics(policy, semantics)?;
                validate_attr(name, value)
            }
            Self::SetStyleVar { node, name, value } => {
                validate_node(policy, *node)?;
                validate_style_var(name, value)
            }
            Self::SetStyleVarBySemantics {
                semantics,
                name,
                value,
            } => {
                validate_semantics(policy, semantics)?;
                validate_style_var(name, value)
            }
            Self::PushHistory { url } | Self::ReplaceHistory { url } => {
                if policy.can_navigate_to(url) {
                    Ok(())
                } else {
                    Err(WorkerProtocolError::new(format!(
                        "worker navigation to `{url}` is not allowed"
                    )))
                }
            }
            Self::Announce { .. } => Ok(()),
            Self::SetStylesheet { id, css } => validate_stylesheet(id, css),
            Self::ReplaceChildrenHtmlBySemantics { semantics, html } => {
                validate_semantics(policy, semantics)?;
                validate_renderer_html(html)
            }
        }
    }
}

fn validate_renderer_html(html: &str) -> Result<(), WorkerProtocolError> {
    if html
        .bytes()
        .any(|byte| byte < 0x20 && !matches!(byte, b'\t' | b'\n' | b'\r'))
    {
        return Err(WorkerProtocolError::new(
            "worker replacement HTML contains control characters",
        ));
    }
    let lower = html.to_ascii_lowercase();
    if lower.contains("<script")
        || lower.contains("javascript:")
        || lower.contains("vbscript:")
        || lower.contains("data:text/html")
        || contains_inline_event_attr(&lower)
    {
        return Err(WorkerProtocolError::new(
            "worker replacement HTML contains executable markup",
        ));
    }
    Ok(())
}

fn contains_inline_event_attr(lower_html: &str) -> bool {
    let bytes = lower_html.as_bytes();
    let mut index = 0;
    while index + 3 < bytes.len() {
        let previous = bytes[index];
        if matches!(previous, b' ' | b'\t' | b'\n' | b'\r')
            && bytes[index + 1] == b'o'
            && bytes[index + 2] == b'n'
        {
            let mut cursor = index + 3;
            while cursor < bytes.len() && bytes[cursor].is_ascii_alphanumeric() {
                cursor += 1;
            }
            while cursor < bytes.len() && matches!(bytes[cursor], b' ' | b'\t' | b'\n' | b'\r') {
                cursor += 1;
            }
            if cursor < bytes.len() && bytes[cursor] == b'=' {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn validate_stylesheet(id: &str, css: &str) -> Result<(), WorkerProtocolError> {
    if id.is_empty()
        || id.len() > 160
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(WorkerProtocolError::new(format!(
            "worker stylesheet id `{id}` is not allowed"
        )));
    }
    if css
        .bytes()
        .any(|byte| byte < 0x20 && !matches!(byte, b'\t' | b'\n' | b'\r'))
    {
        return Err(WorkerProtocolError::new(
            "worker stylesheet CSS contains control characters",
        ));
    }
    Ok(())
}

fn validate_node(policy: &WorkerDomPolicy, node: u64) -> Result<(), WorkerProtocolError> {
    if policy.can_mutate_node(node) {
        Ok(())
    } else {
        Err(WorkerProtocolError::new(format!(
            "worker cannot mutate node `{node}`"
        )))
    }
}

fn validate_semantics(
    policy: &WorkerDomPolicy,
    semantics: &str,
) -> Result<(), WorkerProtocolError> {
    if !safe_semantics_identifier(semantics) {
        return Err(WorkerProtocolError::new(format!(
            "worker semantic target `{semantics}` is not allowed"
        )));
    }
    if policy.can_mutate_semantics(semantics) {
        Ok(())
    } else {
        Err(WorkerProtocolError::new(format!(
            "worker cannot mutate semantic target `{semantics}`"
        )))
    }
}

fn safe_semantics_identifier(semantics: &str) -> bool {
    !semantics.is_empty()
        && semantics.len() <= 160
        && semantics.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.' | b'/')
        })
}

fn validate_attr(name: &str, value: &str) -> Result<(), WorkerProtocolError> {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("on") {
        return Err(WorkerProtocolError::new(format!(
            "worker cannot set event handler attribute `{name}`"
        )));
    }
    if matches!(lower.as_str(), "href" | "src" | "xlink:href") && !safe_navigation_url(value) {
        return Err(WorkerProtocolError::new(format!(
            "worker cannot set unsafe URL attribute `{name}`"
        )));
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(WorkerProtocolError::new(format!(
            "worker attribute `{name}` is not allowed"
        )));
    }
    Ok(())
}

fn validate_style_var(name: &str, value: &str) -> Result<(), WorkerProtocolError> {
    if !name.starts_with("--") {
        return Err(WorkerProtocolError::new(format!(
            "worker style variable `{name}` must start with --"
        )));
    }
    if value
        .bytes()
        .any(|byte| byte < 0x20 && !matches!(byte, b'\t' | b'\n' | b'\r'))
    {
        return Err(WorkerProtocolError::new(
            "worker style variable value contains control characters",
        ));
    }
    Ok(())
}

fn safe_navigation_url(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    if lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("vbscript:")
        || lower.contains('\\')
        || lower.bytes().any(|byte| byte < 0x20)
    {
        return false;
    }
    lower.starts_with('/') && !lower.starts_with("//")
        || lower.starts_with("https://")
        || lower.starts_with("http://")
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BrowserBridgeOutput {
    #[serde(default)]
    pub messages: Vec<WorkerToMain>,
    #[serde(default)]
    pub bindings: Vec<BrowserEventBinding>,
}

impl BrowserBridgeOutput {
    pub fn validate(&self, policy: &WorkerDomPolicy) -> Result<(), WorkerProtocolError> {
        for message in &self.messages {
            if let WorkerToMain::DomBatch(batch) = message {
                batch.validate(policy)?;
            }
        }
        for binding in &self.bindings {
            binding.validate()?;
            validate_semantics(policy, &binding.semantics)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrowserEventBinding {
    pub semantics: String,
    pub event: BrowserEventKind,
    #[serde(default)]
    pub message: Value,
}

impl BrowserEventBinding {
    pub fn validate(&self) -> Result<(), WorkerProtocolError> {
        if safe_semantics_identifier(&self.semantics) {
            Ok(())
        } else {
            Err(WorkerProtocolError::new(format!(
                "browser event binding target `{}` is not allowed",
                self.semantics
            )))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserEventKind {
    Click,
    Input,
    Change,
    Submit,
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
                DomOp::SetTextBySemantics {
                    semantics: "cart-count".into(),
                    text: "1 item".into(),
                },
            ],
        });
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(encoded.contains("dom_batch"));
        let decoded: WorkerToMain = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, message);
    }

    #[test]
    fn worker_dom_policy_rejects_off_tree_and_xss_operations() {
        let policy = WorkerDomPolicy::new()
            .allow_node(42)
            .allow_semantics("cart-count")
            .allow_navigation_to_prefix("/products/");

        let valid = DomBatch {
            sequence: 1,
            transaction_id: None,
            ops: vec![
                DomOp::SetText {
                    node: 42,
                    text: "Safe".into(),
                },
                DomOp::SetAttr {
                    node: 42,
                    name: "aria-label".into(),
                    value: "Safe".into(),
                },
                DomOp::SetStyleVar {
                    node: 42,
                    name: "--accent".into(),
                    value: "#fff".into(),
                },
                DomOp::PushHistory {
                    url: "/products/charizard".into(),
                },
                DomOp::SetTextBySemantics {
                    semantics: "cart-count".into(),
                    text: "1 item".into(),
                },
                DomOp::SetStylesheet {
                    id: "cart-island".into(),
                    css: ".cart{color:red}".into(),
                },
                DomOp::ReplaceChildrenHtmlBySemantics {
                    semantics: "cart-count".into(),
                    html: r#"<div class="cart">1 item</div>"#.into(),
                },
            ],
        };
        assert!(valid.validate(&policy).is_ok());

        let off_tree = DomBatch {
            sequence: 2,
            transaction_id: None,
            ops: vec![DomOp::SetText {
                node: 7,
                text: "No".into(),
            }],
        };
        assert!(off_tree.validate(&policy).is_err());

        let event_handler = DomBatch {
            sequence: 3,
            transaction_id: None,
            ops: vec![DomOp::SetAttr {
                node: 42,
                name: "onclick".into(),
                value: "alert(1)".into(),
            }],
        };
        assert!(event_handler.validate(&policy).is_err());

        let unsafe_url = DomBatch {
            sequence: 4,
            transaction_id: None,
            ops: vec![DomOp::SetAttr {
                node: 42,
                name: "href".into(),
                value: "javascript:alert(1)".into(),
            }],
        };
        assert!(unsafe_url.validate(&policy).is_err());

        let unsafe_navigation = DomBatch {
            sequence: 5,
            transaction_id: None,
            ops: vec![DomOp::PushHistory {
                url: "/admin".into(),
            }],
        };
        assert!(unsafe_navigation.validate(&policy).is_err());

        let unsafe_semantics = DomBatch {
            sequence: 6,
            transaction_id: None,
            ops: vec![DomOp::SetTextBySemantics {
                semantics: "cart count".into(),
                text: "No".into(),
            }],
        };
        assert!(unsafe_semantics.validate(&policy).is_err());

        let off_semantics = DomBatch {
            sequence: 7,
            transaction_id: None,
            ops: vec![DomOp::SetTextBySemantics {
                semantics: "checkout-total".into(),
                text: "No".into(),
            }],
        };
        assert!(off_semantics.validate(&policy).is_err());

        let unsafe_style_id = DomBatch {
            sequence: 8,
            transaction_id: None,
            ops: vec![DomOp::SetStylesheet {
                id: "bad id".into(),
                css: String::new(),
            }],
        };
        assert!(unsafe_style_id.validate(&policy).is_err());

        let unsafe_replacement_html = DomBatch {
            sequence: 9,
            transaction_id: None,
            ops: vec![DomOp::ReplaceChildrenHtmlBySemantics {
                semantics: "cart-count".into(),
                html: r#"<img src="/x" onerror="alert(1)">"#.into(),
            }],
        };
        assert!(unsafe_replacement_html.validate(&policy).is_err());
    }

    #[test]
    fn browser_bridge_output_validates_messages_and_bindings() {
        let output = BrowserBridgeOutput {
            messages: vec![WorkerToMain::DomBatch(DomBatch {
                sequence: 1,
                transaction_id: None,
                ops: vec![DomOp::SetTextBySemantics {
                    semantics: "cart-count".into(),
                    text: "1 item".into(),
                }],
            })],
            bindings: vec![BrowserEventBinding {
                semantics: "cart-add".into(),
                event: BrowserEventKind::Click,
                message: serde_json::json!({ "action": "add" }),
            }],
        };
        let policy = WorkerDomPolicy::new()
            .allow_semantics("cart-count")
            .allow_semantics("cart-add");
        assert!(output.validate(&policy).is_ok());

        let output = BrowserBridgeOutput {
            bindings: vec![BrowserEventBinding {
                semantics: "cart add".into(),
                event: BrowserEventKind::Click,
                message: serde_json::json!({ "action": "add" }),
            }],
            ..Default::default()
        };
        assert!(output.validate(&policy).is_err());
    }
}
