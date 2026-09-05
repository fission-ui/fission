#[cfg(not(target_arch = "wasm32"))]
use anyhow::{anyhow, Context, Result};
#[cfg(not(target_arch = "wasm32"))]
use base64::Engine;
#[cfg(not(target_arch = "wasm32"))]
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::{json, Value};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::net::TcpListener;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Child, Command, Stdio};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(not(target_arch = "wasm32"))]
use tungstenite::{connect, Message};

#[cfg(not(target_arch = "wasm32"))]
use crate::{
    SelectorCandidate, SelectorFailure, SelectorFailureKind, SelectorQuery, TestCommand,
    TestResponse, VisibilityState,
};

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserSmokeMode {
    Dom,
    FissionCanvas,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct BrowserTestOptions {
    pub url: String,
    pub mode: BrowserSmokeMode,
    pub chrome_path: Option<PathBuf>,
    pub cdp_port: Option<u16>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    /// Maximum time allowed for browser startup/readiness and for each Chrome
    /// DevTools Protocol round trip. LiveTest wait commands retain their own
    /// explicit `timeout_ms`, which controls how long the app state may take
    /// to reach the requested condition after a CDP call succeeds.
    pub timeout_ms: u64,
    pub screenshot_path: Option<PathBuf>,
}

#[cfg(not(target_arch = "wasm32"))]
impl BrowserTestOptions {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            mode: BrowserSmokeMode::Dom,
            chrome_path: None,
            cdp_port: None,
            viewport_width: 1280,
            viewport_height: 900,
            timeout_ms: 60_000,
            screenshot_path: None,
        }
    }

    pub fn fission_canvas(mut self) -> Self {
        self.mode = BrowserSmokeMode::FissionCanvas;
        self
    }

    pub fn screenshot(mut self, path: impl Into<PathBuf>) -> Self {
        self.screenshot_path = Some(path.into());
        self
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrowserSmokeReport {
    pub url: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub renderer: Option<String>,
    pub body_text_len: usize,
    pub screenshot_path: Option<PathBuf>,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn detect_chrome() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("FISSION_CHROME").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }
    for candidate in [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    for candidate in ["google-chrome", "chromium", "chromium-browser", "chrome"] {
        if let Ok(output) = Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {candidate}"))
            .output()
        {
            if output.status.success() {
                let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !value.is_empty() {
                    return Some(PathBuf::from(value));
                }
            }
        }
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_browser_smoke(options: BrowserTestOptions) -> Result<BrowserSmokeReport> {
    let mut controller = BrowserController::launch(options.clone(), false)?;
    if let Some(path) = &options.screenshot_path {
        let bytes = controller.capture_page_screenshot()?;
        write_screenshot(path, &bytes)?;
    }
    Ok(controller.report.clone())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct BrowserController {
    _session: ChromeSession,
    client: CdpClient,
    report: BrowserSmokeReport,
}

#[cfg(not(target_arch = "wasm32"))]
impl BrowserController {
    pub(crate) fn launch(options: BrowserTestOptions, require_live_control: bool) -> Result<Self> {
        let chrome = options
            .chrome_path
            .clone()
            .or_else(detect_chrome)
            .context("Chrome/Chromium was not found; set FISSION_CHROME=/path/to/chrome")?;
        let cdp_port = options.cdp_port.unwrap_or_else(free_port);
        let session = ChromeSession::launch(&chrome, cdp_port, &options)?;
        let ws_url = wait_for_target(
            cdp_port,
            &options.url,
            Duration::from_millis(options.timeout_ms),
        )?;
        let mut client = CdpClient::connect(&ws_url, Duration::from_millis(options.timeout_ms))?;
        client.send("Runtime.enable", json!({}))?;
        client.send("Log.enable", json!({}))?;
        client.send("Page.enable", json!({}))?;
        // Headless Chromium has no desktop clipboard broker. Grant clipboard
        // access to this disposable browser profile so trusted keyboard copy,
        // cut, and paste events exercise the same DOM path as an interactive
        // browser session.
        client.send(
            "Browser.grantPermissions",
            json!({
                "permissions": ["clipboardReadWrite", "clipboardSanitizedWrite"]
            }),
        )?;
        client.send(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": options.viewport_width,
                "height": options.viewport_height,
                "deviceScaleFactor": 1,
                "mobile": false
            }),
        )?;

        let deadline = Instant::now() + Duration::from_millis(options.timeout_ms);
        let mut last_status = None;
        while Instant::now() < deadline {
            client.drain_events(Duration::from_millis(25))?;
            if !client.errors.is_empty() {
                return Err(anyhow!(
                    "browser reported errors:\n{}",
                    client.errors.join("\n")
                ));
            }
            let status = read_runtime_status(&mut client)?;
            let ready = match options.mode {
                BrowserSmokeMode::Dom => status.ready_dom,
                BrowserSmokeMode::FissionCanvas => status.ready_canvas && status.renderer.is_some(),
            } && (!require_live_control || status.test_bridge_ready);
            if ready {
                let report = BrowserSmokeReport {
                    url: options.url.clone(),
                    title: status.title,
                    width: status.width,
                    height: status.height,
                    renderer: status.renderer,
                    body_text_len: status.body_text_len,
                    screenshot_path: options.screenshot_path.clone(),
                };
                return Ok(Self {
                    _session: session,
                    client,
                    report,
                });
            }
            last_status = Some(status);
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(anyhow!(
            "browser{} test timed out for {}; last status: {:?}",
            if require_live_control {
                " live-control"
            } else {
                " smoke"
            },
            options.url,
            last_status
        ))
    }

    pub(crate) fn report(&self) -> BrowserSmokeReport {
        self.report.clone()
    }

    pub(crate) fn evaluate_json(&mut self, expression: &str) -> Result<Value> {
        let result = self.client.send(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true }),
        )?;
        runtime_exception(&result)?;
        result
            .pointer("/result/value")
            .cloned()
            .context("browser evaluation returned no JSON value")
    }

    pub(crate) fn send_test_command(&mut self, command: TestCommand) -> Result<TestResponse> {
        match command {
            TestCommand::Wait { ms } => {
                std::thread::sleep(Duration::from_millis(ms));
                Ok(TestResponse::Ok {})
            }
            TestCommand::WaitForSelector { query, timeout_ms } => {
                self.wait_for_selector(query, timeout_ms, SelectorWaitCondition::Present)
            }
            TestCommand::WaitForVisible { query, timeout_ms } => {
                self.wait_for_selector(query, timeout_ms, SelectorWaitCondition::Visible)
            }
            TestCommand::WaitForEnabled { query, timeout_ms } => {
                self.wait_for_selector(query, timeout_ms, SelectorWaitCondition::Enabled)
            }
            TestCommand::WaitForDisabled { query, timeout_ms } => {
                self.wait_for_selector(query, timeout_ms, SelectorWaitCondition::Disabled)
            }
            TestCommand::WaitForValue {
                query,
                value,
                timeout_ms,
            } => self.wait_for_selector(query, timeout_ms, SelectorWaitCondition::Value(value)),
            TestCommand::WaitForGone { query, timeout_ms } => {
                self.wait_for_selector(query, timeout_ms, SelectorWaitCondition::Gone)
            }
            TestCommand::WaitForText { text, timeout_ms } => self.wait_for_text(&text, timeout_ms),
            TestCommand::WaitForIdle {
                timeout_ms,
                ignore_repeating_motion,
            } => self.wait_for_idle(timeout_ms, ignore_repeating_motion),
            TestCommand::TapSelector { query } => {
                self.selector_action(query.clone(), TestCommand::TapSelector { query })
            }
            TestCommand::ActivateSelector { query } => {
                self.selector_action(query.clone(), TestCommand::ActivateSelector { query })
            }
            TestCommand::FocusSelector { query } => {
                self.selector_action(query.clone(), TestCommand::FocusSelector { query })
            }
            TestCommand::HoverSelector { query } => {
                self.selector_action(query.clone(), TestCommand::HoverSelector { query })
            }
            TestCommand::RightClickSelector { query } => self.right_click_selector(query),
            TestCommand::FillText { query, text } => {
                self.selector_action(query.clone(), TestCommand::FillText { query, text })
            }
            TestCommand::ClearText { query } => {
                self.selector_action(query.clone(), TestCommand::ClearText { query })
            }
            TestCommand::Toggle { query } => {
                self.selector_action(query.clone(), TestCommand::Toggle { query })
            }
            TestCommand::SelectOption { query } => {
                self.selector_action(query.clone(), TestCommand::SelectOption { query })
            }
            TestCommand::Screenshot { path } => {
                let bytes = self.capture_page_screenshot()?;
                write_screenshot(&PathBuf::from(path), &bytes)?;
                Ok(TestResponse::Ok {})
            }
            TestCommand::CaptureScreenshot {} => self.capture_page_response(),
            TestCommand::CaptureAt { ms } => {
                ensure_response_ok(self.send_bridge_command(TestCommand::AdvanceClock { ms })?)?;
                ensure_response_ok(self.send_bridge_command(TestCommand::Pump {})?)?;
                self.capture_page_response()
            }
            TestCommand::PressKey { key, modifiers } => self.press_dom_key(&key, modifiers),
            command => self.send_bridge_command(command),
        }
    }

    fn right_click_selector(&mut self, query: SelectorQuery) -> Result<TestResponse> {
        ensure_response_ok(self.send_bridge_command(TestCommand::ScrollIntoView {
            query: query.clone().include_hidden(),
        })?)?;
        ensure_response_ok(self.send_bridge_command(TestCommand::Pump {})?)?;
        let response = self.send_bridge_command(TestCommand::ResolveSelector { query })?;
        let TestResponse::SelectorResolved { node } = response else {
            return Ok(response);
        };
        let bounds = node.visible_bounds.unwrap_or(node.logical_bounds);
        let (offset_x, offset_y) = self.canvas_viewport_offset()?;
        let x = offset_x + f64::from(bounds.x + bounds.width * 0.5);
        let y = offset_y + f64::from(bounds.y + bounds.height * 0.5);
        self.client.send(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseMoved", "x": x, "y": y }),
        )?;
        self.client.send(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mousePressed",
                "x": x,
                "y": y,
                "button": "right",
                "buttons": 2,
                "clickCount": 1
            }),
        )?;
        self.client.send(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseReleased",
                "x": x,
                "y": y,
                "button": "right",
                "buttons": 0,
                "clickCount": 1
            }),
        )?;
        ensure_response_ok(self.send_bridge_command(TestCommand::Pump {})?)?;
        Ok(TestResponse::Ok {})
    }

    fn press_dom_key(&mut self, key: &str, modifiers: u8) -> Result<TestResponse> {
        let cdp_modifiers = cdp_modifiers(modifiers);
        let (dom_key, code) = dom_key_and_code(key);
        for event_type in ["keyDown", "keyUp"] {
            let commands = if event_type == "keyDown" {
                cdp_editing_commands(key, modifiers)
            } else {
                Vec::new()
            };
            self.client.send(
                "Input.dispatchKeyEvent",
                json!({
                    "type": event_type,
                    "key": dom_key,
                    "code": code,
                    "modifiers": cdp_modifiers,
                    "commands": commands,
                }),
            )?;
        }
        ensure_response_ok(self.send_bridge_command(TestCommand::Pump {})?)?;
        Ok(TestResponse::Ok {})
    }

    fn canvas_viewport_offset(&mut self) -> Result<(f64, f64)> {
        let result = self.client.send(
            "Runtime.evaluate",
            json!({
                "expression": "(() => { const r = document.querySelector('canvas')?.getBoundingClientRect(); return r ? [r.left, r.top] : [0, 0]; })()",
                "returnByValue": true
            }),
        )?;
        runtime_exception(&result)?;
        let values = result
            .pointer("/result/value")
            .and_then(Value::as_array)
            .context("browser returned no canvas offset")?;
        Ok((
            values.first().and_then(Value::as_f64).unwrap_or(0.0),
            values.get(1).and_then(Value::as_f64).unwrap_or(0.0),
        ))
    }

    fn selector_action(
        &mut self,
        query: SelectorQuery,
        command: TestCommand,
    ) -> Result<TestResponse> {
        let scroll = self.send_bridge_command(TestCommand::ScrollIntoView {
            query: query.include_hidden(),
        })?;
        ensure_response_ok(scroll)?;
        ensure_response_ok(self.send_bridge_command(TestCommand::Pump {})?)?;
        self.send_bridge_command(command)
    }

    fn wait_for_selector(
        &mut self,
        query: SelectorQuery,
        timeout_ms: u64,
        condition: SelectorWaitCondition,
    ) -> Result<TestResponse> {
        let started = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        loop {
            let response = self.send_bridge_command(TestCommand::ResolveSelector {
                query: query.clone(),
            })?;
            if selector_wait_matches(&condition, &response) {
                return Ok(TestResponse::Ok {});
            }
            if started.elapsed() >= timeout {
                let candidates = match response {
                    TestResponse::SelectorResolved { node } => vec![SelectorCandidate {
                        node,
                        rejected_reason: Some("wait condition did not pass".into()),
                    }],
                    TestResponse::SelectorError { failure } => failure.candidates,
                    _ => Vec::new(),
                };
                return Ok(TestResponse::SelectorError {
                    failure: SelectorFailure {
                        kind: SelectorFailureKind::Timeout,
                        selector: query,
                        candidates,
                        message: format!(
                            "timed out after {timeout_ms}ms waiting for browser selector"
                        ),
                    },
                });
            }
            let _ = self.send_bridge_command(TestCommand::Pump {})?;
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn wait_for_text(&mut self, text: &str, timeout_ms: u64) -> Result<TestResponse> {
        let started = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        loop {
            match self.send_bridge_command(TestCommand::GetText {})? {
                TestResponse::Text { items }
                    if items.iter().any(|item| item.text.contains(text)) =>
                {
                    return Ok(TestResponse::Ok {});
                }
                TestResponse::Error { message } => {
                    return Ok(TestResponse::Error { message });
                }
                _ => {}
            }
            if started.elapsed() >= timeout {
                return Ok(TestResponse::Error {
                    message: format!(
                        "timed out after {timeout_ms}ms waiting for browser text `{text}`"
                    ),
                });
            }
            let _ = self.send_bridge_command(TestCommand::Pump {})?;
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn wait_for_idle(
        &mut self,
        timeout_ms: u64,
        ignore_repeating_motion: bool,
    ) -> Result<TestResponse> {
        let started = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        loop {
            match self.send_bridge_command(TestCommand::WaitForIdle {
                timeout_ms: 0,
                ignore_repeating_motion,
            })? {
                TestResponse::MotionStatus {
                    finite,
                    repeating,
                    ripples,
                } if finite == 0 && ripples == 0 && (ignore_repeating_motion || repeating == 0) => {
                    return Ok(TestResponse::Ok {});
                }
                TestResponse::Error { message } => {
                    return Ok(TestResponse::Error { message });
                }
                _ => {}
            }
            if started.elapsed() >= timeout {
                return Ok(TestResponse::Error {
                    message: format!(
                        "timed out after {timeout_ms}ms waiting for browser motion to become idle"
                    ),
                });
            }
            std::thread::sleep(Duration::from_millis(8));
        }
    }

    fn send_bridge_command(&mut self, command: TestCommand) -> Result<TestResponse> {
        let command_json = serde_json::to_string(&command)?;
        let argument = serde_json::to_string(&command_json)?;
        let submit = format!("globalThis.__FISSION_TEST__.submit({argument})");
        let result = self.client.send(
            "Runtime.evaluate",
            json!({ "expression": submit, "returnByValue": true }),
        )?;
        runtime_exception(&result)?;
        let request_id = result
            .pointer("/result/value")
            .and_then(Value::as_u64)
            .context("Fission browser test bridge returned no request id")?;

        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            self.client.drain_events(Duration::from_millis(5))?;
            self.fail_on_browser_errors()?;
            let poll = self.client.send(
                "Runtime.evaluate",
                json!({
                    "expression": format!(
                        "globalThis.__FISSION_TEST__.poll({request_id})"
                    ),
                    "returnByValue": true
                }),
            )?;
            runtime_exception(&poll)?;
            if let Some(response_json) = poll.pointer("/result/value").and_then(Value::as_str) {
                return serde_json::from_str(response_json)
                    .context("failed to decode Fission browser test response");
            }
            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for Fission browser test request {request_id}"
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn capture_page_response(&mut self) -> Result<TestResponse> {
        let bytes = self.capture_page_screenshot()?;
        let status = read_runtime_status(&mut self.client)?;
        Ok(TestResponse::Screenshot {
            png_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            width: status.width,
            height: status.height,
        })
    }

    fn capture_page_screenshot(&mut self) -> Result<Vec<u8>> {
        let result = self.client.send(
            "Page.captureScreenshot",
            json!({ "format": "png", "captureBeyondViewport": true }),
        )?;
        let data = result
            .get("data")
            .and_then(Value::as_str)
            .context("Page.captureScreenshot returned no data")?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .context("Chrome returned invalid screenshot base64")
    }

    fn fail_on_browser_errors(&mut self) -> Result<()> {
        if self.client.errors.is_empty() {
            Ok(())
        } else {
            let errors = std::mem::take(&mut self.client.errors);
            Err(anyhow!("browser reported errors:\n{}", errors.join("\n")))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
enum SelectorWaitCondition {
    Present,
    Visible,
    Enabled,
    Disabled,
    Value(String),
    Gone,
}

#[cfg(not(target_arch = "wasm32"))]
fn selector_wait_matches(condition: &SelectorWaitCondition, response: &TestResponse) -> bool {
    match (condition, response) {
        (SelectorWaitCondition::Gone, TestResponse::SelectorError { failure }) => {
            failure.kind == SelectorFailureKind::NoMatch
        }
        (SelectorWaitCondition::Present, TestResponse::SelectorResolved { .. }) => true,
        (SelectorWaitCondition::Visible, TestResponse::SelectorResolved { node }) => {
            node.visibility != VisibilityState::Hidden
        }
        (SelectorWaitCondition::Enabled, TestResponse::SelectorResolved { node }) => !node.disabled,
        (SelectorWaitCondition::Disabled, TestResponse::SelectorResolved { node }) => node.disabled,
        (SelectorWaitCondition::Value(expected), TestResponse::SelectorResolved { node }) => {
            node.value.as_deref() == Some(expected.as_str())
        }
        _ => false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn ensure_response_ok(response: TestResponse) -> Result<()> {
    match response {
        TestResponse::Error { message } => Err(anyhow!(message)),
        TestResponse::SelectorError { failure } => Err(anyhow!(failure.message)),
        _ => Ok(()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn cdp_modifiers(fission: u8) -> u8 {
    let mut cdp = 0;
    if fission & 1 != 0 {
        cdp |= 8;
    }
    if fission & 2 != 0 {
        cdp |= 1;
    }
    if fission & 4 != 0 {
        cdp |= 2;
    }
    if fission & 8 != 0 {
        cdp |= 4;
    }
    cdp
}

#[cfg(not(target_arch = "wasm32"))]
fn cdp_editing_commands(key: &str, modifiers: u8) -> Vec<&'static str> {
    let primary = modifiers & (4 | 8) != 0;
    let has_alt = modifiers & 2 != 0;
    if !primary || has_alt {
        return Vec::new();
    }
    match key.to_ascii_lowercase().as_str() {
        "a" => vec!["selectAll"],
        "c" => vec!["copy"],
        "v" => vec!["paste"],
        "x" => vec!["cut"],
        _ => Vec::new(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn dom_key_and_code(key: &str) -> (String, String) {
    let named = match key {
        "Left" => Some(("ArrowLeft", "ArrowLeft")),
        "Right" => Some(("ArrowRight", "ArrowRight")),
        "Up" => Some(("ArrowUp", "ArrowUp")),
        "Down" => Some(("ArrowDown", "ArrowDown")),
        "Space" => Some((" ", "Space")),
        "Esc" => Some(("Escape", "Escape")),
        "Enter" | "Escape" | "Tab" | "Backspace" | "Delete" | "Home" | "End" | "PageUp"
        | "PageDown" => Some((key, key)),
        _ => None,
    };
    if let Some((dom_key, code)) = named {
        return (dom_key.to_owned(), code.to_owned());
    }
    let mut chars = key.chars();
    if let (Some(ch), None) = (chars.next(), chars.next()) {
        let code = if ch.is_ascii_alphabetic() {
            format!("Key{}", ch.to_ascii_uppercase())
        } else if ch.is_ascii_digit() {
            format!("Digit{ch}")
        } else {
            String::new()
        };
        (ch.to_string(), code)
    } else {
        (key.to_owned(), key.to_owned())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn runtime_exception(result: &Value) -> Result<()> {
    if let Some(details) = result.get("exceptionDetails") {
        Err(anyhow!("browser runtime evaluation failed: {details}"))
    } else {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct RuntimeStatus {
    ready_dom: bool,
    ready_canvas: bool,
    title: String,
    width: u32,
    height: u32,
    body_text_len: usize,
    renderer: Option<String>,
    test_bridge_ready: bool,
}

#[cfg(not(target_arch = "wasm32"))]
fn read_runtime_status(client: &mut CdpClient) -> Result<RuntimeStatus> {
    let expression = r#"(() => {
      const body = document.body;
      const canvas = document.querySelector('canvas');
      const rect = canvas ? canvas.getBoundingClientRect() : { width: 0, height: 0 };
      const renderer = globalThis.__FISSION_RENDERER_INFO ?? null;
      return {
        ready_dom: document.readyState === 'complete' && !!body && body.innerText.trim().length > 0,
        ready_canvas: !!canvas && rect.width > 0 && rect.height > 0,
        title: document.title || '',
        width: Math.round(rect.width || window.innerWidth || 0),
        height: Math.round(rect.height || window.innerHeight || 0),
        body_text_len: body ? body.innerText.trim().length : 0,
        renderer: renderer ? renderer.active : null,
        test_bridge_ready: !!globalThis.__FISSION_TEST__
          && typeof globalThis.__FISSION_TEST__.submit === 'function'
          && typeof globalThis.__FISSION_TEST__.poll === 'function',
      };
    })()"#;
    let result = client.send(
        "Runtime.evaluate",
        json!({ "expression": expression, "returnByValue": true }),
    )?;
    if let Some(details) = result.get("exceptionDetails") {
        return Err(anyhow!("runtime evaluation failed: {details}"));
    }
    let value = result
        .get("result")
        .and_then(|result| result.get("value"))
        .cloned()
        .context("Runtime.evaluate returned no value")?;
    serde_json::from_value(value).context("failed to decode browser runtime status")
}

#[cfg(not(target_arch = "wasm32"))]
fn write_screenshot(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(not(target_arch = "wasm32"))]
fn wait_for_target(cdp_port: u16, expected_url: &str, timeout: Duration) -> Result<String> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    while Instant::now() < deadline {
        match ureq::get(&format!("http://127.0.0.1:{cdp_port}/json/list")).call() {
            Ok(response) => {
                let targets: Value = response.into_json()?;
                if let Some(target) = targets.as_array().and_then(|items| {
                    items.iter().find(|entry| {
                        entry.get("type").and_then(Value::as_str) == Some("page")
                            && entry
                                .get("url")
                                .and_then(Value::as_str)
                                .is_some_and(|url| url.starts_with(expected_url))
                    })
                }) {
                    if let Some(url) = target.get("webSocketDebuggerUrl").and_then(Value::as_str) {
                        return Ok(url.to_string());
                    }
                }
            }
            Err(error) => last_error = Some(error.to_string()),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(anyhow!(
        "Chrome CDP target did not become ready for {expected_url}: {}",
        last_error.unwrap_or_else(|| "no matching target".to_string())
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("failed to allocate local port")
        .local_addr()
        .expect("failed to read local port")
        .port()
}

#[cfg(not(target_arch = "wasm32"))]
struct ChromeSession {
    child: Option<Child>,
    profile_dir: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl ChromeSession {
    fn launch(chrome: &PathBuf, cdp_port: u16, options: &BrowserTestOptions) -> Result<Self> {
        let profile_dir = std::env::temp_dir().join(format!(
            "fission-cdp-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&profile_dir)?;
        let child = Command::new(chrome)
            .arg("--headless=new")
            .arg("--enable-unsafe-webgpu")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg(format!("--remote-debugging-port={cdp_port}"))
            .arg(format!("--user-data-dir={}", profile_dir.display()))
            .arg(format!(
                "--window-size={},{}",
                options.viewport_width, options.viewport_height
            ))
            .arg(&options.url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to start {}", chrome.display()))?;
        Ok(Self {
            child: Some(child),
            profile_dir,
        })
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for ChromeSession {
    fn drop(&mut self) {
        self.kill();
        let _ = fs::remove_dir_all(&self.profile_dir);
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct CdpClient {
    socket: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    next_id: u64,
    backlog: VecDeque<Value>,
    errors: Vec<String>,
    command_timeout: Duration,
}

#[cfg(not(target_arch = "wasm32"))]
impl CdpClient {
    fn connect(ws_url: &str, command_timeout: Duration) -> Result<Self> {
        let (mut socket, _) =
            connect(ws_url).context("failed to connect to Chrome CDP websocket")?;
        if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
            stream.set_read_timeout(Some(Duration::from_millis(100)))?;
        }
        Ok(Self {
            socket,
            next_id: 1,
            backlog: VecDeque::new(),
            errors: Vec::new(),
            command_timeout,
        })
    }

    fn send(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.socket.send(Message::Text(serde_json::to_string(
            &json!({ "id": id, "method": method, "params": params }),
        )?))?;
        let deadline = Instant::now() + self.command_timeout;
        loop {
            if let Some(message) = self.backlog.pop_front() {
                if message.get("id").and_then(Value::as_u64) == Some(id) {
                    return Self::command_result(method, message);
                }
                self.handle_event(&message);
                continue;
            }
            if Instant::now() >= deadline {
                return Err(anyhow!("CDP command timed out: {method}"));
            }
            let message = match self.socket.read() {
                Ok(message) => message,
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let text = match message {
                Message::Text(text) => text,
                Message::Binary(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Message::Ping(_) | Message::Pong(_) => continue,
                Message::Close(_) => return Err(anyhow!("CDP websocket closed")),
                Message::Frame(_) => continue,
            };
            let value: Value = serde_json::from_str(&text)?;
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                return Self::command_result(method, value);
            }
            self.handle_event(&value);
        }
    }

    fn drain_events(&mut self, budget: Duration) -> Result<()> {
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    let value: Value = serde_json::from_str(&text)?;
                    if value.get("id").is_some() {
                        self.backlog.push_back(value);
                    } else {
                        self.handle_event(&value);
                    }
                }
                Ok(Message::Binary(bytes)) => {
                    let value: Value = serde_json::from_slice(&bytes)?;
                    if value.get("id").is_some() {
                        self.backlog.push_back(value);
                    } else {
                        self.handle_event(&value);
                    }
                }
                Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {}
                Ok(Message::Close(_)) => return Err(anyhow!("CDP websocket closed")),
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(tungstenite::Error::ConnectionClosed) => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn command_result(method: &str, message: Value) -> Result<Value> {
        if let Some(error) = message.get("error") {
            return Err(anyhow!("{method}: {error}"));
        }
        Ok(message.get("result").cloned().unwrap_or_else(|| json!({})))
    }

    fn handle_event(&mut self, message: &Value) {
        match message.get("method").and_then(Value::as_str) {
            Some("Runtime.exceptionThrown") => self.errors.push(format!(
                "runtime exception: {}",
                message
                    .pointer("/params/exceptionDetails/exception/description")
                    .or_else(|| message.pointer("/params/exceptionDetails/text"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            )),
            Some("Runtime.consoleAPICalled") => {
                let level = message.pointer("/params/type").and_then(Value::as_str);
                if matches!(level, Some("error" | "assert")) {
                    self.errors
                        .push(format!("console.{}: {}", level.unwrap(), message));
                }
            }
            Some("Log.entryAdded") => {
                if message
                    .pointer("/params/entry/level")
                    .and_then(Value::as_str)
                    == Some("error")
                {
                    let text = message
                        .pointer("/params/entry/text")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown browser log error");
                    let url = message
                        .pointer("/params/entry/url")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown URL");
                    if !text.contains("/__fission/renderer") && !url.contains("/__fission/renderer")
                    {
                        self.errors
                            .push(format!("browser log error at {url}: {text}"));
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, SemanticNode};

    fn resolved_node(
        visibility: VisibilityState,
        disabled: bool,
        value: Option<&str>,
    ) -> TestResponse {
        TestResponse::SelectorResolved {
            node: SemanticNode {
                identifier: Some("test.field".into()),
                widget_id: "1".into(),
                stable_node_id: "node-1".into(),
                parent: None,
                children: Vec::new(),
                role: "text_input".into(),
                label: Some("Field".into()),
                value: value.map(str::to_owned),
                value_present: value.is_some(),
                focusable: true,
                disabled,
                read_only: false,
                checked: None,
                actions: vec!["focus".into()],
                text_selection: None,
                masked: false,
                scrollable_x: false,
                scrollable_y: false,
                logical_bounds: Bounds {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 40.0,
                },
                visible_bounds: Some(Bounds {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 40.0,
                }),
                visibility,
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 40.0,
            },
        }
    }

    #[test]
    fn browser_selector_waits_use_native_semantics() {
        let visible = resolved_node(VisibilityState::FullyVisible, false, Some("ready"));
        assert!(selector_wait_matches(
            &SelectorWaitCondition::Present,
            &visible
        ));
        assert!(selector_wait_matches(
            &SelectorWaitCondition::Visible,
            &visible
        ));
        assert!(selector_wait_matches(
            &SelectorWaitCondition::Enabled,
            &visible
        ));
        assert!(selector_wait_matches(
            &SelectorWaitCondition::Value("ready".into()),
            &visible
        ));
        assert!(!selector_wait_matches(
            &SelectorWaitCondition::Value("pending".into()),
            &visible
        ));

        let disabled = resolved_node(VisibilityState::PartiallyVisible, true, None);
        assert!(selector_wait_matches(
            &SelectorWaitCondition::Disabled,
            &disabled
        ));

        let gone = TestResponse::SelectorError {
            failure: SelectorFailure {
                kind: SelectorFailureKind::NoMatch,
                selector: SelectorQuery::label("missing"),
                candidates: Vec::new(),
                message: "not found".into(),
            },
        };
        assert!(selector_wait_matches(&SelectorWaitCondition::Gone, &gone));
    }

    #[test]
    fn browser_status_requires_explicit_live_bridge_signal() {
        let status: RuntimeStatus = serde_json::from_value(serde_json::json!({
            "ready_dom": true,
            "ready_canvas": true,
            "title": "Fission",
            "width": 1280,
            "height": 900,
            "body_text_len": 0,
            "renderer": "webgpu-vello",
            "test_bridge_ready": true
        }))
        .expect("decode browser status");

        assert!(status.test_bridge_ready);
        assert_eq!(status.renderer.as_deref(), Some("webgpu-vello"));
    }

    #[test]
    fn browser_key_events_translate_fission_modifiers_to_cdp() {
        assert_eq!(cdp_modifiers(1), 8); // Shift
        assert_eq!(cdp_modifiers(2), 1); // Alt
        assert_eq!(cdp_modifiers(4), 2); // Control
        assert_eq!(cdp_modifiers(8), 4); // Meta
        assert_eq!(cdp_modifiers(1 | 4 | 8), 8 | 2 | 4);
    }

    #[test]
    fn browser_primary_shortcuts_request_native_editing_commands() {
        assert_eq!(cdp_editing_commands("a", 4), vec!["selectAll"]);
        assert_eq!(cdp_editing_commands("C", 8), vec!["copy"]);
        assert_eq!(cdp_editing_commands("v", 4), vec!["paste"]);
        assert_eq!(cdp_editing_commands("x", 4), vec!["cut"]);
        assert!(cdp_editing_commands("v", 4 | 2).is_empty());
        assert!(cdp_editing_commands("v", 0).is_empty());
    }

    #[test]
    fn browser_key_events_use_dom_key_names() {
        assert_eq!(
            dom_key_and_code("Left"),
            ("ArrowLeft".into(), "ArrowLeft".into())
        );
        assert_eq!(dom_key_and_code("a"), ("a".into(), "KeyA".into()));
        assert_eq!(dom_key_and_code("7"), ("7".into(), "Digit7".into()));
    }
}
