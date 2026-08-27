//! Automated UI testing client and protocol for Fission applications.
//!
//! This crate provides the JSON protocol types shared by the test client and
//! platform shells, plus a [`LiveTestClient`] that drives a running native or
//! Web Fission application.
//!
//! # Architecture
//!
//! Native applications expose the loopback test-control server. Web
//! applications built through `fission test --target web` expose a test-only
//! in-page bridge driven through Chromium.

#[cfg(not(target_arch = "wasm32"))]
use anyhow::{anyhow, Result};
#[cfg(not(target_arch = "wasm32"))]
use base64::Engine;
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
mod developer;
#[cfg(not(target_arch = "wasm32"))]
pub use developer::{
    developer_session_directory, developer_session_path, DeveloperSessionClient,
    DeveloperSessionDescriptor, DeveloperSessionStatus, ReloadOutcome,
    DEVELOPER_SESSION_PROTOCOL_VERSION,
};

pub mod golden;
pub use golden::{compare_png_to_golden, GoldenOptions, GoldenReport};

#[cfg(not(target_arch = "wasm32"))]
pub mod browser;
#[cfg(not(target_arch = "wasm32"))]
pub use browser::{
    detect_chrome, run_browser_smoke, BrowserSmokeMode, BrowserSmokeReport, BrowserTestOptions,
};

// --- Protocol types (shared between client and server) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestPointerKind {
    Mouse,
    Touch,
    Stylus,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestPointerPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestScrollDeltaMode {
    Line,
    Pixel,
}

/// A command sent from the test client to the running application.
///
/// Serialized with `#[serde(tag = "cmd")]`. See the crate-level docs for
/// the full command reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum TestCommand {
    Tap {
        x: f32,
        y: f32,
    },
    Drag {
        start_x: f32,
        start_y: f32,
        end_x: f32,
        end_y: f32,
        steps: u32,
    },
    PointerDown {
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    },
    PointerMove {
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    },
    PointerUp {
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    },
    PointerCancel {
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    },
    PointerScroll {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        delta_mode: TestScrollDeltaMode,
        phase: TestPointerPhase,
        modifiers: u8,
    },
    Magnify {
        x: f32,
        y: f32,
        scale_factor: f32,
        phase: TestPointerPhase,
        modifiers: u8,
    },
    TapText {
        text: String,
    },
    ResolveSelector {
        query: SelectorQuery,
    },
    TapSelector {
        query: SelectorQuery,
    },
    ActivateSelector {
        query: SelectorQuery,
    },
    FocusSelector {
        query: SelectorQuery,
    },
    HoverSelector {
        query: SelectorQuery,
    },
    RightClickSelector {
        query: SelectorQuery,
    },
    ScrollIntoView {
        query: SelectorQuery,
    },
    FillText {
        query: SelectorQuery,
        text: String,
    },
    ClearText {
        query: SelectorQuery,
    },
    Toggle {
        query: SelectorQuery,
    },
    SelectOption {
        query: SelectorQuery,
    },
    WaitForSelector {
        query: SelectorQuery,
        timeout_ms: u64,
    },
    WaitForVisible {
        query: SelectorQuery,
        timeout_ms: u64,
    },
    WaitForEnabled {
        query: SelectorQuery,
        timeout_ms: u64,
    },
    WaitForDisabled {
        query: SelectorQuery,
        timeout_ms: u64,
    },
    WaitForValue {
        query: SelectorQuery,
        value: String,
        timeout_ms: u64,
    },
    WaitForText {
        text: String,
        timeout_ms: u64,
    },
    WaitForGone {
        query: SelectorQuery,
        timeout_ms: u64,
    },
    Scroll {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
    },
    ExternalFileHover {
        x: f32,
        y: f32,
        paths: Vec<String>,
    },
    ExternalFileDrop {
        x: f32,
        y: f32,
        paths: Vec<String>,
    },
    ExternalFileCancel {},
    TypeText {
        text: String,
    },
    ImePreedit {
        text: String,
        cursor_start: Option<usize>,
        cursor_end: Option<usize>,
    },
    ImeCommit {
        text: String,
    },
    ImeCancel {},
    PressKey {
        key: String,
        modifiers: u8,
    },
    Screenshot {
        path: String,
    },
    CaptureScreenshot {},
    PauseAnimations {},
    ResumeAnimations {},
    AdvanceClock {
        ms: u64,
    },
    CaptureAt {
        ms: u64,
    },
    WaitForIdle {
        timeout_ms: u64,
        ignore_repeating_motion: bool,
    },
    GetText {},
    GetTree {},
    Wait {
        ms: u64,
    },
    Pump {},
    Quit {},
    // NEW: simulate real winit-level events for realistic testing
    SimulateMouseMove {
        x: f32,
        y: f32,
    },
    SimulateRightClick {
        x: f32,
        y: f32,
    },
    SimulateResize {
        /// Target logical viewport width in test-space pixels.
        width: u32,
        /// Target logical viewport height in test-space pixels.
        height: u32,
    },
}

/// Events injected into the winit event loop via `EventLoopProxy`.
///
/// Input-simulation variants (`MouseMove`, `MouseDown`, etc.) travel through
/// the **same** `Event::UserEvent` → handler path as real `WindowEvent`s, so
/// test code exercises identical code paths as real user interaction.
///
/// Query / control variants (`Screenshot`, `GetText`, etc.) also go through
/// the proxy so the main loop can respond via a dedicated response channel.
#[derive(Debug, Clone)]
pub enum TestEvent {
    // --- Input simulation (mirrors winit WindowEvents) ---
    MouseMove {
        x: f32,
        y: f32,
    },
    MouseDown {
        x: f32,
        y: f32,
        button: u8,
    }, // 0=left, 1=right, 2=middle
    MouseUp {
        x: f32,
        y: f32,
        button: u8,
    },
    PointerDown {
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    },
    PointerMove {
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    },
    PointerUp {
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    },
    PointerCancel {
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    },
    KeyDown {
        key_code: String,
        modifiers: u8,
    },
    KeyUp {
        key_code: String,
        modifiers: u8,
    },
    TextInput {
        text: String,
    },
    ImePreedit {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    ImeCommit {
        text: String,
    },
    ImeCancel,
    Scroll {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
    },
    PointerScroll {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        delta_mode: TestScrollDeltaMode,
        phase: TestPointerPhase,
        modifiers: u8,
    },
    Magnify {
        x: f32,
        y: f32,
        scale_factor: f32,
        phase: TestPointerPhase,
        modifiers: u8,
    },
    ExternalFileHover {
        x: f32,
        y: f32,
        paths: Vec<String>,
    },
    ExternalFileDrop {
        x: f32,
        y: f32,
        paths: Vec<String>,
    },
    ExternalFileCancel,
    Resize {
        width: u32,
        height: u32,
    },
    // --- Queries / control (need response channel) ---
    Screenshot {
        path: String,
        response_tx: TestResponseSender,
    },
    CaptureScreenshot {
        response_tx: TestResponseSender,
    },
    PauseAnimations {
        response_tx: TestResponseSender,
    },
    ResumeAnimations {
        response_tx: TestResponseSender,
    },
    AdvanceClock {
        ms: u64,
        response_tx: TestResponseSender,
    },
    CaptureAt {
        ms: u64,
        response_tx: TestResponseSender,
    },
    MotionStatus {
        response_tx: TestResponseSender,
    },
    GetText {
        response_tx: TestResponseSender,
    },
    GetTree {
        response_tx: TestResponseSender,
    },
    Pump {
        response_tx: TestResponseSender,
    },
    Wake,
    Quit,
    /// Internal: TapText resolves a text label to coordinates; the server
    /// injects this so the main loop can do the lookup with access to the IR.
    TapText {
        text: String,
        response_tx: TestResponseSender,
    },
    ResolveSelector {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    TapSelector {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    ActivateSelector {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    FocusSelector {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    HoverSelector {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    RightClickSelector {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    ScrollIntoView {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    FillText {
        query: SelectorQuery,
        text: String,
        response_tx: TestResponseSender,
    },
    ClearText {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    Toggle {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    SelectOption {
        query: SelectorQuery,
        response_tx: TestResponseSender,
    },
    /// Internal: Wait is handled server-side (sleep) then responds.
    Wait {
        ms: u64,
        response_tx: TestResponseSender,
    },
}

/// A high-level selector for resolving semantic nodes without manual coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Selector {
    /// Match [`fission_ir::Semantics::identifier`].
    SemanticIdentifier { identifier: String },
    /// Match a stable widget id. Accepts a 32-character raw id or an explicit id key.
    WidgetId { widget_id: String },
    /// Match a stable test identifier. This currently aliases semantic identifier.
    TestId { test_id: String },
    /// Match an accessibility identifier. This currently aliases semantic identifier.
    AccessibilityIdentifier { identifier: String },
    /// Match by role and label.
    RoleLabel { role: String, label: String },
    /// Match by label only.
    Label { label: String },
}

impl Selector {
    pub fn semantic_identifier(identifier: impl Into<String>) -> Self {
        Self::SemanticIdentifier {
            identifier: identifier.into(),
        }
    }

    pub fn widget_id(widget_id: impl Into<String>) -> Self {
        Self::WidgetId {
            widget_id: widget_id.into(),
        }
    }

    pub fn test_id(test_id: impl Into<String>) -> Self {
        Self::TestId {
            test_id: test_id.into(),
        }
    }

    pub fn accessibility_identifier(identifier: impl Into<String>) -> Self {
        Self::AccessibilityIdentifier {
            identifier: identifier.into(),
        }
    }

    pub fn role_label(role: impl Into<String>, label: impl Into<String>) -> Self {
        Self::RoleLabel {
            role: role.into(),
            label: label.into(),
        }
    }

    pub fn label(label: impl Into<String>) -> Self {
        Self::Label {
            label: label.into(),
        }
    }
}

/// A selector query with optional scoping and duplicate disambiguation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectorQuery {
    pub selector: Selector,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Box<SelectorQuery>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    #[serde(default)]
    pub include_hidden: bool,
}

impl SelectorQuery {
    pub fn new(selector: Selector) -> Self {
        Self {
            selector,
            scope: None,
            index: None,
            include_hidden: false,
        }
    }

    pub fn semantic_identifier(identifier: impl Into<String>) -> Self {
        Self::new(Selector::semantic_identifier(identifier))
    }

    pub fn widget_id(widget_id: impl Into<String>) -> Self {
        Self::new(Selector::widget_id(widget_id))
    }

    pub fn test_id(test_id: impl Into<String>) -> Self {
        Self::new(Selector::test_id(test_id))
    }

    pub fn accessibility_identifier(identifier: impl Into<String>) -> Self {
        Self::new(Selector::accessibility_identifier(identifier))
    }

    pub fn role_label(role: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(Selector::role_label(role, label))
    }

    pub fn label(label: impl Into<String>) -> Self {
        Self::new(Selector::label(label))
    }

    pub fn scoped(mut self, scope: SelectorQuery) -> Self {
        self.scope = Some(Box::new(scope));
        self
    }

    pub fn index(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }

    pub fn include_hidden(mut self) -> Self {
        self.include_hidden = true;
        self
    }
}

/// A logical rectangle in test-space pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// How much of a semantic node is visible after viewport and clipping are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisibilityState {
    FullyVisible,
    PartiallyVisible,
    Hidden,
}

/// Machine-readable selector failure category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectorFailureKind {
    NoMatch,
    Ambiguous,
    FoundButNotVisible,
    Disabled,
    ReadOnly,
    UnsupportedAction,
    Timeout,
    StaleFrame,
}

/// A candidate considered while resolving a selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorCandidate {
    pub node: SemanticNode,
    pub rejected_reason: Option<String>,
}

/// Detailed selector failure response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorFailure {
    pub kind: SelectorFailureKind,
    pub selector: SelectorQuery,
    pub candidates: Vec<SelectorCandidate>,
    pub message: String,
}

/// A visible text element with its bounding rectangle, in logical test-space
/// pixels, returned by [`TestCommand::GetText`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextItem {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A node in the semantic accessibility tree, returned by [`TestCommand::GetTree`].
/// Bounding rectangles are expressed in logical test-space pixels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticNode {
    pub identifier: Option<String>,
    pub widget_id: String,
    pub stable_node_id: String,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub role: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub value_present: bool,
    pub focusable: bool,
    pub disabled: bool,
    pub read_only: bool,
    pub checked: Option<bool>,
    pub actions: Vec<String>,
    pub text_selection: Option<(usize, usize)>,
    pub masked: bool,
    pub scrollable_x: bool,
    pub scrollable_y: bool,
    pub logical_bounds: Bounds,
    pub visible_bounds: Option<Bounds>,
    pub visibility: VisibilityState,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The response from the application to a [`TestCommand`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum TestResponse {
    Ok {},
    Text {
        items: Vec<TextItem>,
    },
    Tree {
        nodes: Vec<SemanticNode>,
    },
    Screenshot {
        png_base64: String,
        /// PNG width in logical test-space pixels.
        width: u32,
        /// PNG height in logical test-space pixels.
        height: u32,
    },
    MotionStatus {
        finite: usize,
        repeating: usize,
        ripples: usize,
    },
    SelectorResolved {
        node: SemanticNode,
    },
    SelectorError {
        failure: SelectorFailure,
    },
    Error {
        message: String,
    },
}

/// Per-command response channel used by the shell event loop.
pub type TestResponseSender = std::sync::mpsc::Sender<TestResponse>;

// --- Client ---

/// An HTTP client that drives a running Fission application for automated UI testing.
///
/// Connect to a running application via [`LiveTestClient::connect(port)`]. The
/// application must have been started with `FISSION_TEST_CONTROL_PORT=<port>`.
///
/// # Example
///
/// ```rust,ignore
/// let client = LiveTestClient::connect(9876);
/// client.wait_for_ready(5000).unwrap();
/// client.tap_text("Submit").unwrap();
/// client.assert_text_visible("Success").unwrap();
/// client.screenshot("/tmp/result.png").unwrap();
/// client.quit().unwrap();
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub struct LiveTestClient {
    transport: LiveTestTransport,
    scope: Option<SelectorQuery>,
}

#[cfg(not(target_arch = "wasm32"))]
enum LiveTestTransport {
    Http {
        base_url: String,
        bearer_token: Option<String>,
    },
    Browser(std::sync::Mutex<browser::BrowserController>),
}

#[cfg(not(target_arch = "wasm32"))]
impl LiveTestClient {
    pub fn connect(port: u16) -> Self {
        Self {
            transport: LiveTestTransport::Http {
                base_url: format!("http://127.0.0.1:{port}"),
                bearer_token: None,
            },
            scope: None,
        }
    }

    /// Connects to a loopback test host protected by a per-session capability.
    pub fn connect_authenticated(port: u16, bearer_token: impl Into<String>) -> Self {
        Self {
            transport: LiveTestTransport::Http {
                base_url: format!("http://127.0.0.1:{port}"),
                bearer_token: Some(bearer_token.into()),
            },
            scope: None,
        }
    }

    /// Restricts this client to one semantic subtree.
    pub fn with_scope(mut self, scope: SelectorQuery) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Launches Chromium and connects to a Web application built with
    /// Fission's test-only browser bridge.
    pub fn launch_browser(options: BrowserTestOptions) -> Result<Self> {
        let controller = browser::BrowserController::launch(options, true)?;
        Ok(Self {
            transport: LiveTestTransport::Browser(std::sync::Mutex::new(controller)),
            scope: None,
        })
    }

    /// Returns browser readiness details for a Web client.
    pub fn browser_report(&self) -> Option<BrowserSmokeReport> {
        match &self.transport {
            LiveTestTransport::Http { .. } => None,
            LiveTestTransport::Browser(controller) => {
                controller.lock().ok().map(|controller| controller.report())
            }
        }
    }

    /// Evaluates a JSON-returning browser expression in a launched Web test.
    ///
    /// This is intended for host-boundary conformance that cannot be expressed
    /// as a Fission semantic command, such as checking the hidden native text
    /// control, browser history, or platform DOM attributes.
    pub fn browser_evaluate_json(&self, expression: &str) -> Result<serde_json::Value> {
        match &self.transport {
            LiveTestTransport::Browser(controller) => controller
                .lock()
                .map_err(|_| anyhow!("browser test controller lock is poisoned"))?
                .evaluate_json(expression),
            LiveTestTransport::Http { .. } => Err(anyhow!(
                "browser evaluation requires LiveTestClient::launch_browser"
            )),
        }
    }

    pub fn wait_for_ready(&self, timeout_ms: u64) -> Result<()> {
        let LiveTestTransport::Http {
            base_url,
            bearer_token,
        } = &self.transport
        else {
            return Ok(());
        };
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        loop {
            let mut request = ureq::get(&format!("{base_url}/health"));
            if let Some(token) = bearer_token {
                request = request.set("Authorization", &format!("Bearer {token}"));
            }
            match request.call() {
                Ok(_) => return Ok(()),
                Err(_) => {
                    if start.elapsed() > timeout {
                        return Err(anyhow!("timed out waiting for test server"));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }

    fn send(&self, cmd: TestCommand) -> Result<TestResponse> {
        let response = match &self.transport {
            LiveTestTransport::Http {
                base_url,
                bearer_token,
            } => {
                let body = serde_json::to_string(&cmd)?;
                let mut request =
                    ureq::post(&format!("{base_url}/cmd")).set("Content-Type", "application/json");
                if let Some(token) = bearer_token {
                    request = request.set("Authorization", &format!("Bearer {token}"));
                }
                let response = request
                    .send_string(&body)
                    .map_err(|error| anyhow!("request failed: {error}"))?;
                serde_json::from_str(&response.into_string()?)?
            }
            LiveTestTransport::Browser(controller) => controller
                .lock()
                .map_err(|_| anyhow!("browser test controller lock is poisoned"))?
                .send_test_command(cmd)?,
        };
        if let TestResponse::Error { message } = &response {
            return Err(anyhow!("test host error: {message}"));
        }
        if let TestResponse::SelectorError { failure } = &response {
            return Err(anyhow!("selector error: {}", failure.message));
        }
        Ok(response)
    }

    fn scoped_query(&self, query: SelectorQuery) -> SelectorQuery {
        if query.scope.is_some() {
            query
        } else if let Some(scope) = &self.scope {
            query.scoped(scope.clone())
        } else {
            query
        }
    }

    fn scope_node(&self) -> Result<Option<SemanticNode>> {
        let Some(scope) = &self.scope else {
            return Ok(None);
        };
        match self.send(TestCommand::ResolveSelector {
            query: scope.clone(),
        })? {
            TestResponse::SelectorResolved { node } => Ok(Some(node)),
            other => Err(anyhow!("unexpected scope response: {other:?}")),
        }
    }

    fn translated_point(&self, x: f32, y: f32) -> Result<(f32, f32)> {
        Ok(self.scope_node()?.map_or((x, y), |scope| {
            (x + scope.logical_bounds.x, y + scope.logical_bounds.y)
        }))
    }

    pub fn tap(&self, x: f32, y: f32) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::Tap { x, y })?;
        Ok(())
    }

    pub fn tap_text(&self, text: &str) -> Result<()> {
        if self.scope.is_some() {
            self.pump()?;
            let item = self
                .get_text()?
                .into_iter()
                .find(|item| item.text == text)
                .or_else(|| {
                    self.get_text()
                        .ok()?
                        .into_iter()
                        .find(|item| item.text.contains(text))
                })
                .ok_or_else(|| anyhow!("text `{text}` was not found in the test surface"))?;
            self.tap(item.x + item.width / 2.0, item.y + item.height / 2.0)?;
            self.pump()?;
            return Ok(());
        }
        // Pump first to ensure layout positions are current
        self.pump()?;
        self.send(TestCommand::TapText {
            text: text.to_string(),
        })?;
        // Pump after to render the result of the tap
        self.pump()?;
        Ok(())
    }

    pub fn tap_text_without_pump(&self, text: &str) -> Result<()> {
        self.send(TestCommand::TapText {
            text: text.to_string(),
        })?;
        Ok(())
    }

    pub fn resolve_selector(&self, query: SelectorQuery) -> Result<SemanticNode> {
        match self.send(TestCommand::ResolveSelector {
            query: self.scoped_query(query),
        })? {
            TestResponse::SelectorResolved { node } => Ok(node),
            other => Err(anyhow!(
                "unexpected response to ResolveSelector: {:?}",
                other
            )),
        }
    }

    pub fn scroll_into_view(&self, query: SelectorQuery) -> Result<SemanticNode> {
        let node = match self.send(TestCommand::ScrollIntoView {
            query: self.scoped_query(query),
        })? {
            TestResponse::SelectorResolved { node } => node,
            other => {
                return Err(anyhow!(
                    "unexpected response to ScrollIntoView: {:?}",
                    other
                ))
            }
        };
        self.pump()?;
        Ok(node)
    }

    pub fn tap_selector(&self, query: SelectorQuery) -> Result<()> {
        self.pump()?;
        self.send(TestCommand::TapSelector {
            query: self.scoped_query(query),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn tap_semantic_identifier(&self, identifier: &str) -> Result<()> {
        self.tap_selector(SelectorQuery::semantic_identifier(identifier))
    }

    pub fn activate_selector(&self, query: SelectorQuery) -> Result<()> {
        self.pump()?;
        self.send(TestCommand::ActivateSelector {
            query: self.scoped_query(query),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn focus_selector(&self, query: SelectorQuery) -> Result<()> {
        self.pump()?;
        self.send(TestCommand::FocusSelector {
            query: self.scoped_query(query),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn hover_selector(&self, query: SelectorQuery) -> Result<()> {
        self.pump()?;
        self.send(TestCommand::HoverSelector {
            query: self.scoped_query(query),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn right_click_selector(&self, query: SelectorQuery) -> Result<()> {
        self.pump()?;
        self.send(TestCommand::RightClickSelector {
            query: self.scoped_query(query),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn fill_text_selector(&self, query: SelectorQuery, text: &str) -> Result<()> {
        self.pump()?;
        self.send(TestCommand::FillText {
            query: self.scoped_query(query),
            text: text.to_string(),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn fill_text_semantic_identifier(&self, identifier: &str, text: &str) -> Result<()> {
        self.fill_text_selector(SelectorQuery::semantic_identifier(identifier), text)
    }

    pub fn clear_text_selector(&self, query: SelectorQuery) -> Result<()> {
        self.pump()?;
        self.send(TestCommand::ClearText {
            query: self.scoped_query(query),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn toggle_selector(&self, query: SelectorQuery) -> Result<()> {
        self.pump()?;
        self.send(TestCommand::Toggle {
            query: self.scoped_query(query),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn select_option(&self, query: SelectorQuery) -> Result<()> {
        self.pump()?;
        self.send(TestCommand::SelectOption {
            query: self.scoped_query(query),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn wait_for_selector(&self, query: SelectorQuery, timeout_ms: u64) -> Result<()> {
        self.send(TestCommand::WaitForSelector {
            query: self.scoped_query(query),
            timeout_ms,
        })?;
        Ok(())
    }

    pub fn wait_for_visible(&self, query: SelectorQuery, timeout_ms: u64) -> Result<()> {
        self.send(TestCommand::WaitForVisible {
            query: self.scoped_query(query),
            timeout_ms,
        })?;
        Ok(())
    }

    pub fn wait_for_enabled(&self, query: SelectorQuery, timeout_ms: u64) -> Result<()> {
        self.send(TestCommand::WaitForEnabled {
            query: self.scoped_query(query),
            timeout_ms,
        })?;
        Ok(())
    }

    pub fn wait_for_disabled(&self, query: SelectorQuery, timeout_ms: u64) -> Result<()> {
        self.send(TestCommand::WaitForDisabled {
            query: self.scoped_query(query),
            timeout_ms,
        })?;
        Ok(())
    }

    pub fn wait_for_value(&self, query: SelectorQuery, value: &str, timeout_ms: u64) -> Result<()> {
        self.send(TestCommand::WaitForValue {
            query: self.scoped_query(query),
            value: value.to_string(),
            timeout_ms,
        })?;
        Ok(())
    }

    pub fn wait_for_text(&self, text: &str, timeout_ms: u64) -> Result<()> {
        if self.scope.is_some() {
            let started = std::time::Instant::now();
            let timeout = std::time::Duration::from_millis(timeout_ms);
            loop {
                if self.get_text()?.iter().any(|item| item.text.contains(text)) {
                    return Ok(());
                }
                if started.elapsed() >= timeout {
                    return Err(anyhow!("timed out waiting for text `{text}`"));
                }
                self.pump()?;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        self.send(TestCommand::WaitForText {
            text: text.to_string(),
            timeout_ms,
        })?;
        Ok(())
    }

    pub fn wait_for_gone(&self, query: SelectorQuery, timeout_ms: u64) -> Result<()> {
        self.send(TestCommand::WaitForGone {
            query: self.scoped_query(query),
            timeout_ms,
        })?;
        Ok(())
    }

    pub fn drag(
        &self,
        start_x: f32,
        start_y: f32,
        end_x: f32,
        end_y: f32,
        steps: u32,
    ) -> Result<()> {
        let (start_x, start_y) = self.translated_point(start_x, start_y)?;
        let (end_x, end_y) = self.translated_point(end_x, end_y)?;
        self.send(TestCommand::Drag {
            start_x,
            start_y,
            end_x,
            end_y,
            steps,
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn pointer_down(
        &self,
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    ) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::PointerDown {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        })?;
        Ok(())
    }

    pub fn pointer_move(
        &self,
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    ) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::PointerMove {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        })?;
        Ok(())
    }

    pub fn pointer_up(
        &self,
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    ) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::PointerUp {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        })?;
        Ok(())
    }

    pub fn pointer_cancel(
        &self,
        pointer_id: u64,
        kind: TestPointerKind,
        x: f32,
        y: f32,
        modifiers: u8,
    ) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::PointerCancel {
            pointer_id,
            kind,
            x,
            y,
            modifiers,
        })?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pointer_scroll(
        &self,
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        delta_mode: TestScrollDeltaMode,
        phase: TestPointerPhase,
        modifiers: u8,
    ) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::PointerScroll {
            x,
            y,
            dx,
            dy,
            delta_mode,
            phase,
            modifiers,
        })?;
        Ok(())
    }

    pub fn magnify(
        &self,
        x: f32,
        y: f32,
        scale_factor: f32,
        phase: TestPointerPhase,
        modifiers: u8,
    ) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::Magnify {
            x,
            y,
            scale_factor,
            phase,
            modifiers,
        })?;
        Ok(())
    }

    pub fn external_file_hover(&self, x: f32, y: f32, paths: impl Into<Vec<String>>) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::ExternalFileHover {
            x,
            y,
            paths: paths.into(),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn external_file_drop(&self, x: f32, y: f32, paths: impl Into<Vec<String>>) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::ExternalFileDrop {
            x,
            y,
            paths: paths.into(),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn external_file_cancel(&self) -> Result<()> {
        self.send(TestCommand::ExternalFileCancel {})?;
        self.pump()?;
        Ok(())
    }

    pub fn scroll(&self, x: f32, y: f32, dx: f32, dy: f32) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::Scroll { x, y, dx, dy })?;
        Ok(())
    }

    pub fn press_key(&self, key: &str, modifiers: u8) -> Result<()> {
        self.send(TestCommand::PressKey {
            key: key.to_string(),
            modifiers,
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn type_text(&self, text: &str) -> Result<()> {
        self.send(TestCommand::TypeText {
            text: text.to_string(),
        })?;
        Ok(())
    }

    pub fn ime_preedit(&self, text: &str, cursor: Option<(usize, usize)>) -> Result<()> {
        self.send(TestCommand::ImePreedit {
            text: text.to_string(),
            cursor_start: cursor.map(|range| range.0),
            cursor_end: cursor.map(|range| range.1),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn ime_commit(&self, text: &str) -> Result<()> {
        self.send(TestCommand::ImeCommit {
            text: text.to_string(),
        })?;
        self.pump()?;
        Ok(())
    }

    pub fn ime_cancel(&self) -> Result<()> {
        self.send(TestCommand::ImeCancel {})?;
        self.pump()?;
        Ok(())
    }

    pub fn screenshot(&self, path: &str) -> Result<()> {
        std::fs::write(path, self.capture_screenshot_png()?)?;
        Ok(())
    }

    /// Captures the current frame as encoded PNG bytes.
    pub fn capture_screenshot_png(&self) -> Result<Vec<u8>> {
        let png = screenshot_bytes(self.send(TestCommand::CaptureScreenshot {})?)?;
        self.crop_to_scope(png)
    }

    /// Compares the current frame to a golden image and writes an optional heatmap.
    pub fn compare_golden(
        &self,
        golden_path: impl AsRef<std::path::Path>,
        diff_path: Option<impl AsRef<std::path::Path>>,
        options: GoldenOptions,
    ) -> Result<GoldenReport> {
        let report = compare_png_to_golden(
            &self.capture_screenshot_png()?,
            golden_path,
            diff_path,
            options,
        )?;
        if !report.passed(options) {
            return Err(anyhow!(
                "golden comparison changed {:.4}% of pixels (allowed {:.4}%)",
                report.changed_percent,
                options.max_changed_percent
            ));
        }
        Ok(report)
    }

    /// Freezes the animation clock while leaving production motion declarations active.
    pub fn pause_animations(&self) -> Result<()> {
        self.send(TestCommand::PauseAnimations {})?;
        Ok(())
    }

    /// Resumes advancing animations from the currently frozen clock value.
    pub fn resume_animations(&self) -> Result<()> {
        self.send(TestCommand::ResumeAnimations {})?;
        Ok(())
    }

    /// Deterministically advances the application and motion clock.
    pub fn advance_clock(&self, ms: u64) -> Result<()> {
        self.send(TestCommand::AdvanceClock { ms })?;
        self.pump()
    }

    /// Advances the clock and captures the resulting frame to `path`.
    pub fn capture_at(&self, ms: u64, path: &str) -> Result<()> {
        std::fs::write(path, self.capture_at_png(ms)?)?;
        Ok(())
    }

    /// Advances the clock and returns the resulting frame as encoded PNG bytes.
    pub fn capture_at_png(&self, ms: u64) -> Result<Vec<u8>> {
        let png = screenshot_bytes(self.send(TestCommand::CaptureAt { ms })?)?;
        self.crop_to_scope(png)
    }

    /// Waits for finite motion to settle, optionally ignoring repeating motion.
    pub fn wait_for_idle(&self, timeout_ms: u64, ignore_repeating_motion: bool) -> Result<()> {
        self.send(TestCommand::WaitForIdle {
            timeout_ms,
            ignore_repeating_motion,
        })?;
        Ok(())
    }

    pub fn get_text(&self) -> Result<Vec<TextItem>> {
        match self.send(TestCommand::GetText {})? {
            TestResponse::Text { mut items } => {
                if let Some(scope) = self.scope_node()? {
                    let bounds = scope.visible_bounds.unwrap_or(scope.logical_bounds);
                    items.retain(|item| {
                        rectangles_intersect(
                            Bounds {
                                x: item.x,
                                y: item.y,
                                width: item.width,
                                height: item.height,
                            },
                            bounds,
                        )
                    });
                    for item in &mut items {
                        item.x -= bounds.x;
                        item.y -= bounds.y;
                    }
                }
                Ok(items)
            }
            other => Err(anyhow!("unexpected response: {:?}", other)),
        }
    }

    pub fn get_tree(&self) -> Result<Vec<SemanticNode>> {
        match self.send(TestCommand::GetTree {})? {
            TestResponse::Tree { mut nodes } => {
                let Some(scope) = self.scope_node()? else {
                    return Ok(nodes);
                };
                let scope_id = scope.stable_node_id.clone();
                let mut retained = std::collections::HashSet::from([scope_id.clone()]);
                loop {
                    let before = retained.len();
                    for node in &nodes {
                        if node
                            .parent
                            .as_ref()
                            .is_some_and(|parent| retained.contains(parent))
                        {
                            retained.insert(node.stable_node_id.clone());
                        }
                    }
                    if retained.len() == before {
                        break;
                    }
                }
                let origin = scope.visible_bounds.unwrap_or(scope.logical_bounds);
                nodes.retain(|node| {
                    node.stable_node_id != scope_id && retained.contains(&node.stable_node_id)
                });
                for node in &mut nodes {
                    if node.parent.as_deref() == Some(scope_id.as_str()) {
                        node.parent = None;
                    }
                    node.logical_bounds.x -= origin.x;
                    node.logical_bounds.y -= origin.y;
                    if let Some(bounds) = &mut node.visible_bounds {
                        bounds.x -= origin.x;
                        bounds.y -= origin.y;
                    }
                    node.x -= origin.x;
                    node.y -= origin.y;
                }
                Ok(nodes)
            }
            other => Err(anyhow!("unexpected response: {:?}", other)),
        }
    }

    pub fn wait(&self, ms: u64) -> Result<()> {
        self.send(TestCommand::Wait { ms })?;
        Ok(())
    }

    pub fn pump(&self) -> Result<()> {
        self.send(TestCommand::Pump {})?;
        Ok(())
    }

    pub fn quit(&self) -> Result<()> {
        let _ = self.send(TestCommand::Quit {});
        Ok(())
    }

    // --- NEW: simulate real winit-level events ---

    /// Simulate a mouse move to (x, y) — goes through the real CursorMoved path.
    pub fn simulate_mouse_move(&self, x: f32, y: f32) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::SimulateMouseMove { x, y })?;
        Ok(())
    }

    /// Simulate a right-click at (x, y) — move + down + up with right button.
    pub fn right_click(&self, x: f32, y: f32) -> Result<()> {
        let (x, y) = self.translated_point(x, y)?;
        self.send(TestCommand::SimulateRightClick { x, y })?;
        Ok(())
    }

    /// Simulate a window resize in logical test-space pixels.
    pub fn simulate_resize(&self, width: u32, height: u32) -> Result<()> {
        let (width, height) = if let Some(scope) = self.scope_node()? {
            let png = screenshot_bytes(self.send(TestCommand::CaptureScreenshot {})?)?;
            let image = image::load_from_memory(&png)?.to_rgba8();
            let extra_width = image
                .width()
                .saturating_sub(scope.logical_bounds.width.round() as u32);
            let extra_height = image
                .height()
                .saturating_sub(scope.logical_bounds.height.round() as u32);
            (
                width.saturating_add(extra_width),
                height.saturating_add(extra_height),
            )
        } else {
            (width, height)
        };
        self.send(TestCommand::SimulateResize { width, height })?;
        Ok(())
    }

    fn crop_to_scope(&self, png: Vec<u8>) -> Result<Vec<u8>> {
        let Some(scope) = self.scope_node()? else {
            return Ok(png);
        };
        let bounds = scope.visible_bounds.unwrap_or(scope.logical_bounds);
        let image = image::load_from_memory(&png)?.to_rgba8();
        let x = bounds.x.max(0.0).round() as u32;
        let y = bounds.y.max(0.0).round() as u32;
        let width = (bounds.width.max(1.0).round() as u32).min(image.width().saturating_sub(x));
        let height = (bounds.height.max(1.0).round() as u32).min(image.height().saturating_sub(y));
        if width == 0 || height == 0 {
            return Err(anyhow!("test surface is outside the captured frame"));
        }
        let cropped = image::imageops::crop_imm(&image, x, y, width, height).to_image();
        let mut encoded = Vec::new();
        image::DynamicImage::ImageRgba8(cropped).write_to(
            &mut std::io::Cursor::new(&mut encoded),
            image::ImageFormat::Png,
        )?;
        Ok(encoded)
    }

    // --- High-level helpers ---

    pub fn tap_text_and_wait(&self, text: &str, ms: u64) -> Result<()> {
        self.tap_text(text)?;
        self.wait(ms)?;
        Ok(())
    }

    pub fn assert_text_visible(&self, needle: &str) -> Result<()> {
        let items = self.get_text()?;
        let found = items.iter().any(|t| t.text.contains(needle));
        if !found {
            let all: Vec<&str> = items.iter().map(|t| t.text.as_str()).collect();
            return Err(anyhow!(
                "expected '{}' to be visible, found: {:?}",
                needle,
                &all[..all.len().min(20)]
            ));
        }
        Ok(())
    }

    pub fn assert_text_not_visible(&self, needle: &str) -> Result<()> {
        let items = self.get_text()?;
        let found = items.iter().any(|t| t.text.contains(needle));
        if found {
            return Err(anyhow!("expected '{}' to NOT be visible", needle));
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn screenshot_bytes(response: TestResponse) -> Result<Vec<u8>> {
    match response {
        TestResponse::Screenshot {
            png_base64,
            width: _,
            height: _,
        } => base64::engine::general_purpose::STANDARD
            .decode(png_base64)
            .map_err(|error| anyhow!("invalid screenshot payload: {error}")),
        other => Err(anyhow!("expected screenshot response, received {other:?}")),
    }
}

fn rectangles_intersect(left: Bounds, right: Bounds) -> bool {
    left.x < right.x + right.width
        && left.x + left.width > right.x
        && left.y < right.y + right.height
        && left.y + left.height > right.y
}

#[cfg(test)]
mod tests {
    use super::{TestCommand, TestPointerKind, TestPointerPhase, TestScrollDeltaMode};

    #[test]
    fn deterministic_motion_commands_have_stable_wire_shapes() {
        assert_eq!(
            serde_json::to_value(TestCommand::PauseAnimations {}).expect("serialize pause"),
            serde_json::json!({ "cmd": "PauseAnimations" })
        );
        assert_eq!(
            serde_json::to_value(TestCommand::AdvanceClock { ms: 160 })
                .expect("serialize clock advance"),
            serde_json::json!({ "cmd": "AdvanceClock", "ms": 160 })
        );
        assert_eq!(
            serde_json::to_value(TestCommand::WaitForIdle {
                timeout_ms: 2_000,
                ignore_repeating_motion: true,
            })
            .expect("serialize idle wait"),
            serde_json::json!({
                "cmd": "WaitForIdle",
                "timeout_ms": 2_000,
                "ignore_repeating_motion": true
            })
        );
    }

    #[test]
    fn multi_pointer_commands_have_explicit_wire_metadata() {
        let pointer = TestCommand::PointerDown {
            pointer_id: 7,
            kind: TestPointerKind::Touch,
            x: 12.0,
            y: 24.0,
            modifiers: 0,
        };
        assert_eq!(
            serde_json::to_value(pointer).unwrap(),
            serde_json::json!({
                "cmd": "PointerDown",
                "pointer_id": 7,
                "kind": "touch",
                "x": 12.0,
                "y": 24.0,
                "modifiers": 0
            })
        );

        let scroll = TestCommand::PointerScroll {
            x: 5.0,
            y: 6.0,
            dx: 1.0,
            dy: -2.0,
            delta_mode: TestScrollDeltaMode::Pixel,
            phase: TestPointerPhase::Moved,
            modifiers: 4,
        };
        let value = serde_json::to_value(scroll).unwrap();
        assert_eq!(value["delta_mode"], "pixel");
        assert_eq!(value["phase"], "moved");
        assert_eq!(value["modifiers"], 4);
    }
}
