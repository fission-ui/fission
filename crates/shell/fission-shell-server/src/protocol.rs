use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Messages sent by the browser main thread to a progressive worker.
pub enum MainToWorker {
    /// Initializes a newly created worker instance.
    Boot(WorkerBoot),
    /// Delivers a sanitized DOM event.
    Event(WorkerDomEvent),
    /// Reports the worker-owned viewport size.
    Resize(WorkerResize),
    /// Reports page visibility.
    VisibilityChanged {
        /// Whether the document is currently visible.
        visible: bool,
    },
    /// Reports a theme selection change.
    ThemeChanged {
        /// Stable theme identifier selected by the page.
        theme_id: String,
    },
    /// Reports a locale selection change.
    LocaleChanged {
        /// BCP-47-like locale identifier selected by the page.
        locale: String,
    },
    /// Completes an earlier host request.
    Response(WorkerResponse),
    /// Requests orderly worker shutdown.
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Initialization data supplied to one progressive worker instance.
pub struct WorkerBoot {
    /// Version of this serialized protocol.
    pub protocol_version: u16,
    /// Unique identifier for this concrete worker instance.
    pub worker_instance_id: String,
    /// Route declaration that created the worker.
    pub route_id: String,
    /// Public base URL used to resolve application requests.
    pub base_url: String,
    /// DOM root the worker is authorized to manage.
    pub root_node_id: String,
    /// Active locale identifier.
    pub locale: String,
    /// Active theme identifier.
    pub theme_id: String,
    /// Host capabilities enabled for this worker.
    pub feature_flags: Vec<String>,
    #[serde(default)]
    /// Application-defined initialization properties.
    pub props: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Sanitized browser event delivered to a progressive worker.
pub struct WorkerDomEvent {
    /// Monotonic event sequence within this worker instance.
    pub sequence: u64,
    /// Numeric DOM target authorized by the worker policy.
    pub target_node_id: u64,
    /// Browser event name, such as `click` or `input`.
    pub event_kind: String,
    #[serde(default)]
    /// Current control value for value-bearing events.
    pub value: Option<String>,
    #[serde(default)]
    /// Active keyboard modifier names.
    pub modifiers: Vec<String>,
    #[serde(default)]
    /// Pointer coordinates and button for pointer events.
    pub pointer: Option<WorkerPointer>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Pointer details attached to a worker DOM event.
pub struct WorkerPointer {
    /// Horizontal coordinate relative to the worker root.
    pub x: f64,
    /// Vertical coordinate relative to the worker root.
    pub y: f64,
    /// Browser button number, when applicable.
    pub button: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Current browser viewport information sent to a worker.
pub struct WorkerResize {
    /// CSS-pixel width.
    pub width: f64,
    /// CSS-pixel height.
    pub height: f64,
    /// Device pixels per CSS pixel.
    pub scale_factor: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Host response to a worker's capability request.
pub struct WorkerResponse {
    /// Identifier copied from the corresponding request.
    pub request_id: u64,
    /// Whether the request completed successfully.
    pub ok: bool,
    #[serde(default)]
    /// Capability-specific success value.
    pub payload: Option<Value>,
    #[serde(default)]
    /// Failure diagnostic when `ok` is false.
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Messages emitted by a progressive worker for the browser main thread.
pub enum WorkerToMain {
    /// Signals that boot completed and events may be delivered.
    Ready,
    /// Requests a validated batch of DOM mutations.
    DomBatch(DomBatch),
    /// Requests a host/browser capability.
    Request(WorkerRequest),
    /// Requests browser navigation.
    Navigate(NavigateRequest),
    /// Emits a developer diagnostic.
    Log(WorkerLog),
    /// Reports an unhandled worker failure.
    Error(WorkerError),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Ordered DOM operations committed as one worker update.
pub struct DomBatch {
    /// Monotonic batch sequence within this worker instance.
    pub sequence: u64,
    #[serde(default)]
    /// Optional application transaction identifier for diagnostics.
    pub transaction_id: Option<String>,
    /// Operations applied in declaration order after validation.
    pub ops: Vec<DomOp>,
}

impl DomBatch {
    /// Validates every operation against the worker's authority policy.
    pub fn validate(&self, policy: &WorkerDomPolicy) -> Result<(), WorkerProtocolError> {
        for op in &self.ops {
            op.validate(policy)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Explicit authority granted to a progressive worker over page DOM and navigation.
pub struct WorkerDomPolicy {
    allowed_nodes: BTreeSet<u64>,
    allowed_semantics: BTreeSet<String>,
    allow_navigation: bool,
    allowed_url_prefixes: Vec<String>,
}

impl WorkerDomPolicy {
    /// Creates a deny-by-default policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Authorizes mutation of one numeric DOM node.
    pub fn allow_node(mut self, node: u64) -> Self {
        self.allowed_nodes.insert(node);
        self
    }

    /// Authorizes mutation of multiple numeric DOM nodes.
    pub fn allow_nodes<I>(mut self, nodes: I) -> Self
    where
        I: IntoIterator<Item = u64>,
    {
        self.allowed_nodes.extend(nodes);
        self
    }

    /// Authorizes one stable semantic target.
    pub fn allow_semantics(mut self, semantics: impl Into<String>) -> Self {
        self.allowed_semantics.insert(semantics.into());
        self
    }

    /// Authorizes multiple stable semantic targets.
    pub fn allow_semantics_many<I, S>(mut self, semantics: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_semantics
            .extend(semantics.into_iter().map(Into::into));
        self
    }

    /// Authorizes navigation only to URLs beginning with `prefix`.
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
/// Rejection returned when a worker message exceeds its declared authority or
/// contains unsafe browser content.
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
/// Validated browser DOM operation requested by a progressive worker.
///
/// Numeric-node variants target a host-assigned node ID. `BySemantics`
/// variants target a stable semantic name. Both forms require explicit
/// authorization in [`WorkerDomPolicy`]; URL, attribute, CSS, and replacement
/// HTML values receive additional safety validation before execution.
pub enum DomOp {
    /// Replaces a node's text content.
    SetText {
        /// Authorized numeric target.
        node: u64,
        /// Literal text content.
        text: String,
    },
    /// Replaces text at a semantic target.
    SetTextBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// Literal text content.
        text: String,
    },
    /// Replaces a semantic target's children with sanitized renderer HTML.
    ReplaceChildrenHtmlBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// Non-executable HTML fragment.
        html: String,
    },
    /// Creates or replaces a worker-owned stylesheet.
    SetStylesheet {
        /// Safe stylesheet element identifier.
        id: String,
        /// Complete CSS text.
        css: String,
    },
    /// Sets a validated attribute on a numeric target.
    SetAttr {
        /// Authorized numeric target.
        node: u64,
        /// Attribute name; inline event handlers are rejected.
        name: String,
        /// Attribute value; URL-bearing attributes require safe URLs.
        value: String,
    },
    /// Sets a validated attribute on a semantic target.
    SetAttrBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// Attribute name; inline event handlers are rejected.
        name: String,
        /// Attribute value; URL-bearing attributes require safe URLs.
        value: String,
    },
    /// Removes an attribute from a numeric target.
    RemoveAttr {
        /// Authorized numeric target.
        node: u64,
        /// Attribute name to remove.
        name: String,
    },
    /// Removes an attribute from a semantic target.
    RemoveAttrBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// Attribute name to remove.
        name: String,
    },
    /// Adds a CSS class to a numeric target.
    AddClass {
        /// Authorized numeric target.
        node: u64,
        /// Class token to add.
        class: String,
    },
    /// Adds a CSS class to a semantic target.
    AddClassBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// Class token to add.
        class: String,
    },
    /// Removes a CSS class from a numeric target.
    RemoveClass {
        /// Authorized numeric target.
        node: u64,
        /// Class token to remove.
        class: String,
    },
    /// Removes a CSS class from a semantic target.
    RemoveClassBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// Class token to remove.
        class: String,
    },
    /// Sets whether a numeric target contains a CSS class.
    ToggleClass {
        /// Authorized numeric target.
        node: u64,
        /// Class token to update.
        class: String,
        /// `true` adds the class and `false` removes it.
        enabled: bool,
    },
    /// Sets whether a semantic target contains a CSS class.
    ToggleClassBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// Class token to update.
        class: String,
        /// `true` adds the class and `false` removes it.
        enabled: bool,
    },
    /// Sets a CSS custom property on a numeric target.
    SetStyleVar {
        /// Authorized numeric target.
        node: u64,
        /// Custom property name beginning with `--`.
        name: String,
        /// Property value without control characters.
        value: String,
    },
    /// Sets a CSS custom property on a semantic target.
    SetStyleVarBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// Custom property name beginning with `--`.
        name: String,
        /// Property value without control characters.
        value: String,
    },
    /// Updates the hidden state of a numeric target.
    SetHidden {
        /// Authorized numeric target.
        node: u64,
        /// Whether the element is hidden.
        hidden: bool,
    },
    /// Updates the hidden state of a semantic target.
    SetHiddenBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// Whether the element is hidden.
        hidden: bool,
    },
    /// Replaces the form value of a numeric target.
    SetValue {
        /// Authorized numeric target.
        node: u64,
        /// New form-control value.
        value: String,
    },
    /// Replaces the form value of a semantic target.
    SetValueBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// New form-control value.
        value: String,
    },
    /// Updates the checked state of a numeric target.
    SetChecked {
        /// Authorized numeric target.
        node: u64,
        /// New checked state.
        checked: bool,
    },
    /// Updates the checked state of a semantic target.
    SetCheckedBySemantics {
        /// Authorized semantic target.
        semantics: String,
        /// New checked state.
        checked: bool,
    },
    /// Moves browser focus to a numeric target.
    Focus {
        /// Authorized numeric target.
        node: u64,
    },
    /// Moves browser focus to a semantic target.
    FocusBySemantics {
        /// Authorized semantic target.
        semantics: String,
    },
    /// Removes browser focus from a numeric target.
    Blur {
        /// Authorized numeric target.
        node: u64,
    },
    /// Removes browser focus from a semantic target.
    BlurBySemantics {
        /// Authorized semantic target.
        semantics: String,
    },
    /// Scrolls a numeric target into its nearest scroll container.
    ScrollIntoView {
        /// Authorized numeric target.
        node: u64,
        /// Alignment within the visible scroll area.
        block: ScrollBlock,
    },
    /// Sets absolute scroll offsets on a numeric target.
    SetScroll {
        /// Authorized numeric target.
        node: u64,
        /// Horizontal CSS-pixel offset.
        x: f64,
        /// Vertical CSS-pixel offset.
        y: f64,
    },
    /// Pushes a validated URL into browser history.
    PushHistory {
        /// URL allowed by the worker navigation policy.
        url: String,
    },
    /// Replaces the current browser history entry with a validated URL.
    ReplaceHistory {
        /// URL allowed by the worker navigation policy.
        url: String,
    },
    /// Announces text through an ARIA live region.
    Announce {
        /// Interruption priority for assistive technology.
        politeness: AriaPoliteness,
        /// Human-readable announcement.
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
/// Vertical alignment used by a worker scroll-into-view request.
pub enum ScrollBlock {
    /// Align the element with the start of the scroll viewport.
    Start,
    /// Center the element in the scroll viewport.
    Center,
    /// Align the element with the end of the scroll viewport.
    End,
    /// Perform the smallest scroll needed to reveal the element.
    Nearest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Priority for text announced through an ARIA live region.
pub enum AriaPoliteness {
    /// Wait for the current assistive-technology announcement to finish.
    Polite,
    /// Interrupt with time-sensitive information.
    Assertive,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Capability request sent by a progressive worker to the main thread.
pub struct WorkerRequest {
    /// Identifier copied into the eventual response.
    pub request_id: u64,
    /// Browser or server capability being requested.
    pub kind: WorkerRequestKind,
    #[serde(default)]
    /// Capability-specific request value.
    pub payload: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Capabilities a worker may request from the browser main thread.
pub enum WorkerRequestKind {
    /// Submit a signed Fission action to the SSR endpoint.
    FetchServerAction,
    /// Read a browser local-storage key.
    ReadLocalStorage,
    /// Write a browser local-storage key.
    WriteLocalStorage,
    /// Remove a browser local-storage key.
    RemoveLocalStorage,
    /// Read a browser session-storage key.
    ReadSessionStorage,
    /// Write a browser session-storage key.
    WriteSessionStorage,
    /// Write plain text to the system clipboard.
    ClipboardWriteText,
    /// Read plain text from the system clipboard.
    ClipboardReadText,
    /// Read the current browser location.
    CurrentLocation,
    /// Read the current document visibility state.
    DocumentVisibility,
    /// Evaluate a browser media query.
    MatchMedia,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Browser navigation requested by a progressive worker.
pub struct NavigateRequest {
    /// Destination URL.
    pub url: String,
    /// History/document transition to perform.
    pub mode: NavigateMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Browser navigation behavior requested by a worker.
pub enum NavigateMode {
    /// Push a new same-document history entry.
    Push,
    /// Replace the current same-document history entry.
    Replace,
    /// Navigate the complete browser document.
    FullDocument,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Structured developer log emitted by a progressive worker.
pub struct WorkerLog {
    /// Severity used by browser logging and diagnostics.
    pub level: WorkerLogLevel,
    /// Human-readable message.
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Severity of a progressive worker log record.
pub enum WorkerLogLevel {
    /// Fine-grained execution trace.
    Trace,
    /// Developer debugging detail.
    Debug,
    /// Normal lifecycle information.
    Info,
    /// Recoverable or suspicious condition.
    Warn,
    /// Failed worker operation.
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Unhandled failure reported by a progressive worker.
pub struct WorkerError {
    /// Human-readable failure message.
    pub message: String,
    /// Optional JavaScript stack trace.
    pub stack: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
/// Complete messages and event bindings produced by a browser worker bridge.
pub struct BrowserBridgeOutput {
    #[serde(default)]
    /// Worker messages to process in declaration order.
    pub messages: Vec<WorkerToMain>,
    #[serde(default)]
    /// DOM events the browser should route back to the worker.
    pub bindings: Vec<BrowserEventBinding>,
}

impl BrowserBridgeOutput {
    /// Validates DOM operations, semantic targets, and event bindings.
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
/// Declarative DOM event subscription installed for a worker.
pub struct BrowserEventBinding {
    /// Authorized semantic target on which to listen.
    pub semantics: String,
    /// Browser event forwarded to the worker.
    pub event: BrowserEventKind,
    #[serde(default)]
    /// Application-defined message included with each forwarded event.
    pub message: Value,
}

impl BrowserEventBinding {
    /// Validates that the semantic target has a safe identifier shape.
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
/// Browser events supported by declarative worker bindings.
pub enum BrowserEventKind {
    /// Pointer or keyboard activation.
    Click,
    /// Incremental form-control value change.
    Input,
    /// Committed form-control value change.
    Change,
    /// Form submission.
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
