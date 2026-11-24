#[cfg(not(target_arch = "wasm32"))]
use anyhow::{anyhow, Context, Result};
#[cfg(not(target_arch = "wasm32"))]
use base64::Engine;
#[cfg(not(target_arch = "wasm32"))]
use fission_command_process::SupervisedChild;
#[cfg(not(target_arch = "wasm32"))]
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::{json, Value};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::net::TcpListener;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Command, Stdio};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
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
mod site;
#[cfg(not(target_arch = "wasm32"))]
mod viewport;

#[cfg(not(target_arch = "wasm32"))]
pub mod review;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserSmokeMode {
    Dom,
    Website,
    FissionCanvas,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct BrowserTestOptions {
    pub url: String,
    pub mode: BrowserSmokeMode,
    pub chrome_path: Option<PathBuf>,
    pub cdp_port: Option<u16>,
    /// CSS pixels, 1..=8192 per axis and at most 16,777,216 pixels total.
    /// Device scale and mobile emulation are preserved across in-session resizes.
    pub viewport_width: u32,
    pub viewport_height: u32,
    /// Browser device-pixel ratio used for input and rendering emulation.
    pub device_scale_factor: f64,
    /// Enables Chromium's mobile viewport and touch-device behaviour.
    pub mobile: bool,
    pub prefers_reduced_motion: bool,
    /// Startup budget; each in-session resize/capture clamps this to 1..=60,000 ms.
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
            device_scale_factor: 1.0,
            mobile: false,
            prefers_reduced_motion: false,
            timeout_ms: 60_000,
            screenshot_path: None,
        }
    }

    pub fn fission_canvas(mut self) -> Self {
        self.mode = BrowserSmokeMode::FissionCanvas;
        self
    }

    /// Drives any rendered website through its semantic DOM and real Chromium input.
    pub fn website(mut self) -> Self {
        self.mode = BrowserSmokeMode::Website;
        self
    }

    pub fn screenshot(mut self, path: impl Into<PathBuf>) -> Self {
        self.screenshot_path = Some(path.into());
        self
    }

    /// Emulates the user's reduced-motion preference in the launched browser.
    pub fn reduced_motion(mut self) -> Self {
        self.prefers_reduced_motion = true;
        self
    }

    /// Emulates a mobile browser viewport at the supplied device-pixel ratio.
    pub fn mobile(mut self, device_scale_factor: f64) -> Self {
        self.device_scale_factor = device_scale_factor;
        self.mobile = true;
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
    options: BrowserTestOptions,
    require_frame: bool,
    viewport_after: Option<u64>,
    bridge_ready_navigation_count: Option<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
impl BrowserController {
    pub(crate) fn launch(options: BrowserTestOptions, require_live_control: bool) -> Result<Self> {
        viewport::validate_dimensions(options.viewport_width, options.viewport_height)?;
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
        let mut client = CdpClient::connect(&ws_url)?;
        client.send("Runtime.enable", json!({}))?;
        client.send("Log.enable", json!({}))?;
        client.send("Page.enable", json!({}))?;
        client.send("Network.enable", json!({}))?;
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
                "deviceScaleFactor": options.device_scale_factor,
                "mobile": options.mobile
            }),
        )?;
        if options.prefers_reduced_motion {
            client.send(
                "Emulation.setEmulatedMedia",
                json!({
                    "features": [{
                        "name": "prefers-reduced-motion",
                        "value": "reduce"
                    }]
                }),
            )?;
        }

        let deadline = Instant::now() + Duration::from_millis(options.timeout_ms);
        let mut last_status = None;
        let mut last_viewport = Value::Null;
        let mut last_context_error = None;
        while Instant::now() < deadline {
            client.drain_events(Duration::from_millis(25))?;
            if !client.errors.is_empty() {
                return Err(anyhow!(
                    "browser reported errors:\n{}",
                    client.errors.join("\n")
                ));
            }
            let status = match read_runtime_status(&mut client) {
                Ok(status) => status,
                Err(error) if is_transient_execution_context_error(&error) => {
                    last_context_error = Some(error.to_string());
                    std::thread::sleep(Duration::from_millis(25));
                    continue;
                }
                Err(error) => return Err(error),
            };
            let measured = match viewport::read(&mut client) {
                Ok(measured) => measured,
                Err(error) if is_transient_execution_context_error(&error) => {
                    last_context_error = Some(error.to_string());
                    std::thread::sleep(Duration::from_millis(25));
                    continue;
                }
                Err(error) => return Err(error),
            };
            let ready = browser_is_ready(&status, options.mode, require_live_control)
                && viewport::acknowledged(&measured, &options, require_live_control, None);
            if ready {
                let report = BrowserSmokeReport {
                    url: options.url.clone(),
                    title: status.title,
                    width: options.viewport_width,
                    height: options.viewport_height,
                    renderer: status.renderer,
                    body_text_len: status.body_text_len,
                    screenshot_path: options.screenshot_path.clone(),
                };
                let bridge_ready_navigation_count =
                    require_live_control.then_some(client.main_frame_navigation_count);
                return Ok(Self {
                    _session: session,
                    client,
                    report,
                    options,
                    require_frame: require_live_control,
                    viewport_after: None,
                    bridge_ready_navigation_count,
                });
            }
            last_viewport = measured;
            last_status = Some(status);
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(anyhow!(
            "browser{} test timed out for {}; last viewport: {last_viewport}; last status: {:?}; last transient context error: {:?}",
            if require_live_control {
                " live-control"
            } else {
                " smoke"
            },
            options.url,
            last_status,
            last_context_error
        ))
    }

    pub(crate) fn report(&self) -> BrowserSmokeReport {
        self.report.clone()
    }

    pub(crate) fn evaluate_json(&mut self, expression: &str) -> Result<Value> {
        let result = self.client.send_with_web_test_trace(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true }),
        )?;
        runtime_exception(&result)?;
        result
            .pointer("/result/value")
            .cloned()
            .context("browser evaluation returned no JSON value")
    }

    pub(crate) fn touch_selector(
        &mut self,
        query: SelectorQuery,
        relative_x: f32,
        relative_y: f32,
    ) -> Result<TestResponse> {
        ensure_response_ok(self.send_runtime_command(TestCommand::ScrollIntoView {
            query: query.clone().include_hidden(),
        })?)?;
        ensure_response_ok(self.send_runtime_command(TestCommand::Pump {})?)?;
        let response = self.send_runtime_command(TestCommand::ResolveSelector { query })?;
        let TestResponse::SelectorResolved { node } = response else {
            return Ok(response);
        };
        let bounds = node.visible_bounds.unwrap_or(node.logical_bounds);
        let (offset_x, offset_y) = self.runtime_viewport_offset()?;
        let x = offset_x + f64::from(bounds.x + bounds.width * relative_x);
        let y = offset_y + f64::from(bounds.y + bounds.height * relative_y);
        self.client.send(
            "Emulation.setTouchEmulationEnabled",
            json!({ "enabled": true, "maxTouchPoints": 1 }),
        )?;
        self.client.send(
            "Input.dispatchTouchEvent",
            json!({
                "type": "touchStart",
                "touchPoints": [{ "x": x, "y": y, "id": 1 }]
            }),
        )?;
        self.client.send(
            "Input.dispatchTouchEvent",
            json!({ "type": "touchEnd", "touchPoints": [] }),
        )?;
        ensure_response_ok(self.send_runtime_command(TestCommand::Pump {})?)?;
        Ok(TestResponse::Ok {})
    }

    pub(crate) fn wheel_selector(
        &mut self,
        query: SelectorQuery,
        delta_x: f64,
        delta_y: f64,
    ) -> Result<TestResponse> {
        let response = self.send_runtime_command(TestCommand::ResolveSelector { query })?;
        let TestResponse::SelectorResolved { node } = response else {
            return Ok(response);
        };
        let bounds = node.visible_bounds.unwrap_or(node.logical_bounds);
        let (offset_x, offset_y) = self.runtime_viewport_offset()?;
        let x = offset_x + f64::from(bounds.x + bounds.width * 0.5);
        let y = offset_y + f64::from(bounds.y + bounds.height * 0.5);
        self.client.send(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseWheel",
                "x": x,
                "y": y,
                "deltaX": delta_x,
                "deltaY": delta_y
            }),
        )?;
        ensure_response_ok(self.send_runtime_command(TestCommand::Pump {})?)?;
        Ok(TestResponse::Ok {})
    }

    pub(crate) fn send_test_command(&mut self, command: TestCommand) -> Result<TestResponse> {
        if self.options.mode == BrowserSmokeMode::Website {
            return self.send_site_test_command(command);
        }
        anyhow::ensure!(
            self.options.mode != BrowserSmokeMode::Dom
                || matches!(&command, TestCommand::SimulateResize { .. } | TestCommand::Wait { .. }
                    | TestCommand::Screenshot { .. } | TestCommand::CaptureScreenshot {}),
            "unsupported_page_mode: DOM sessions support host resize, wait and screenshots; semantic/input commands require a Web build compiled with FISSION_WEB_TEST_CONTROL=1"
        );
        match command {
            TestCommand::SimulateResize { width, height } => {
                self.resize_viewport(width, height)?;
                Ok(TestResponse::Ok {})
            }
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
            TestCommand::Screenshot { path } => match self.capture_renderer_or_page_response()? {
                TestResponse::Screenshot { png_base64, .. } => {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(png_base64)
                        .context("invalid screenshot payload from browser")?;
                    write_screenshot(&PathBuf::from(path), &bytes)?;
                    Ok(TestResponse::Ok {})
                }
                response => Ok(response),
            },
            TestCommand::CaptureScreenshot {} => self.capture_renderer_or_page_response(),
            TestCommand::CaptureAt { ms } => {
                ensure_response_ok(self.send_bridge_command(TestCommand::AdvanceClock { ms })?)?;
                ensure_response_ok(self.send_bridge_command(TestCommand::Pump {})?)?;
                self.capture_renderer_or_page_response()
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
        let description = cdp_key_description(key, modifiers);
        let down_type = if description.text.is_empty() {
            "rawKeyDown"
        } else {
            "keyDown"
        };
        self.client.send(
            "Input.dispatchKeyEvent",
            json!({
                "type": down_type,
                "key": description.key,
                "code": description.code,
                "text": description.text,
                "unmodifiedText": description.text,
                "windowsVirtualKeyCode": description.key_code,
                "location": description.location,
                "modifiers": cdp_modifiers,
                "commands": cdp_editing_commands(key, modifiers),
            }),
        )?;
        self.client.send(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": description.key,
                "code": description.code,
                "windowsVirtualKeyCode": description.key_code,
                "location": description.location,
                "modifiers": cdp_modifiers,
            }),
        )?;
        ensure_response_ok(self.send_runtime_command(TestCommand::Pump {})?)?;
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

    fn runtime_viewport_offset(&mut self) -> Result<(f64, f64)> {
        if self.options.mode == BrowserSmokeMode::FissionCanvas {
            self.canvas_viewport_offset()
        } else {
            Ok((0.0, 0.0))
        }
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
        let diagnostic_query = query.clone().with_candidates();
        let polling_query = query.without_candidates();
        loop {
            let response = self.send_bridge_command(TestCommand::ResolveSelector {
                query: polling_query.clone(),
            })?;
            if selector_wait_matches(&condition, &response) {
                return Ok(TestResponse::Ok {});
            }
            if started.elapsed() >= timeout {
                let response = self.send_bridge_command(TestCommand::ResolveSelector {
                    query: diagnostic_query.clone(),
                })?;
                if selector_wait_matches(&condition, &response) {
                    return Ok(TestResponse::Ok {});
                }
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
                        selector: diagnostic_query,
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
        // Pump, readiness probes, and motion checks share the caller's budget.
        let _deadline = (timeout_ms > 0).then(|| {
            self.client
                .operation_deadline
                .scoped(Instant::now() + Duration::from_millis(timeout_ms))
        });
        wait_for_idle_with(
            timeout_ms,
            ignore_repeating_motion,
            |command| self.send_bridge_command(command),
            std::thread::sleep,
        )
    }

    fn send_bridge_command(&mut self, command: TestCommand) -> Result<TestResponse> {
        anyhow::ensure!(
            self.options.mode == BrowserSmokeMode::FissionCanvas,
            "unsupported_page_mode: this command requires a Web build compiled with FISSION_WEB_TEST_CONTROL=1; DOM sessions support host resize and screenshots"
        );
        let retry_after_navigation = bridge_command_is_safe_to_retry(&command);
        let deadline = Instant::now() + Duration::from_secs(30);
        // Keep startup/readiness checks, the submit call, and every poll under
        // the same bridge budget. Without this outer scope, a Runtime.evaluate
        // in wait_for_live_test_bridge falls back to the shorter 15s default.
        let _operation_deadline = self.client.operation_deadline.scoped(deadline);
        let deadline = self.client.operation_deadline.current();
        loop {
            if self.bridge_ready_navigation_count != Some(self.client.main_frame_navigation_count) {
                self.wait_for_live_test_bridge(deadline)?;
                self.viewport_after = None;
                self.bridge_ready_navigation_count = Some(self.client.main_frame_navigation_count);
            }
            let navigation_count = self.client.main_frame_navigation_count;
            match self.send_bridge_command_once(command.clone(), deadline) {
                Ok(response) => return Ok(response),
                Err(error)
                    if retry_after_navigation && is_transient_bridge_context_error(&error) =>
                {
                    self.wait_for_main_frame_navigation(navigation_count, deadline)?;
                    self.wait_for_live_test_bridge(deadline)?;
                    self.viewport_after = None;
                    self.bridge_ready_navigation_count =
                        Some(self.client.main_frame_navigation_count);
                }
                Err(error) => {
                    self.hold_browser_for_diagnostics(&error);
                    return Err(error);
                }
            }
        }
    }

    /// Optionally keep a failed diagnostic browser alive long enough to collect late events.
    /// This is opt-in and does not change the bridge operation's deadline or its result.
    fn hold_browser_for_diagnostics(&mut self, error: &anyhow::Error) {
        let hold_ms = diagnostic_hold_after_error_ms(
            std::env::var("FISSION_WEB_LIVE_HOLD_OPEN_AFTER_ERROR_MS")
                .ok()
                .as_deref(),
        );
        if hold_ms == 0 {
            return;
        }

        eprintln!(
            "[fission-web-live-diagnostic] holding browser for {hold_ms}ms after error: {error:#}"
        );
        let deadline = Instant::now() + Duration::from_millis(hold_ms);
        while Instant::now() < deadline {
            if let Err(drain_error) = self.client.drain_events(Duration::from_millis(100)) {
                eprintln!(
                    "[fission-web-live-diagnostic] browser event drain stopped: {drain_error:#}"
                );
                break;
            }
        }
        if !self.client.web_test_trace.is_empty() {
            eprintln!(
                "[fission-web-live-diagnostic] stages observed during hold: {}",
                self.client
                    .web_test_trace
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
        }
        if !self.client.webgpu_queue_trace.is_empty() {
            eprintln!(
                "[fission-web-live-diagnostic] WebGPU queue stages observed during hold: {}",
                self.client
                    .webgpu_queue_trace
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
        }
        eprintln!("[fission-web-live-diagnostic] browser observation window ended");
    }

    fn send_bridge_command_once(
        &mut self,
        command: TestCommand,
        deadline: Instant,
    ) -> Result<TestResponse> {
        let _operation_deadline = self.client.operation_deadline.scoped(deadline);
        let command_json = serde_json::to_string(&command)?;
        let submit = format!(
            "globalThis.__FISSION_TEST__.submit({})",
            serde_json::to_string(&command_json)?
        );
        let result = self
            .client
            .send_with_web_test_trace(
                "Runtime.evaluate",
                json!({ "expression": submit, "returnByValue": true }),
            )
            .context("submitting a Fission browser test command over CDP")?;
        runtime_exception(&result)?;
        let request_id = result
            .pointer("/result/value")
            .and_then(Value::as_u64)
            .context("Fission browser test bridge returned no request id")?;

        loop {
            self.client.drain_events(Duration::from_millis(5))?;
            self.fail_on_browser_errors()?;
            let poll = self.client.send_with_web_test_trace(
                "Runtime.evaluate",
                json!({
                    "expression": format!(
                        "(() => {{ const raw=globalThis.__FISSION_TEST__.poll({request_id}); return JSON.parse(raw); }})()"
                    ),
                    "returnByValue": true
                }),
            )
            .with_context(|| format!("polling Fission browser test request {request_id} over CDP"))?;
            runtime_exception(&poll)?;
            if let Some(response) = poll
                .pointer("/result/value")
                .filter(|response| !response.is_null())
            {
                return decode_bridge_response(response.clone());
            }
            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for Fission browser test request {request_id}"
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn wait_for_live_test_bridge(&mut self, deadline: Instant) -> Result<()> {
        let mut last_probe_error = None;
        loop {
            self.client.drain_events(Duration::from_millis(5))?;
            self.fail_on_browser_errors()?;
            let probe_deadline = (Instant::now() + Duration::from_secs(5)).min(deadline);
            let status = {
                let _probe_deadline = self.client.operation_deadline.scoped(probe_deadline);
                read_runtime_status(&mut self.client)
            };
            match status {
                Ok(status) if browser_is_ready(&status, self.options.mode, true) => return Ok(()),
                Ok(_) => {}
                Err(error) if is_transient_bridge_readiness_error(&error) => {
                    last_probe_error = Some(format!("{error:#}"));
                }
                Err(error) => return Err(error),
            }
            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for the Fission Web test bridge to become ready; last transient probe error: {last_probe_error:?}"
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_for_main_frame_navigation(&mut self, after: u64, deadline: Instant) -> Result<()> {
        while self.client.main_frame_navigation_count <= after {
            self.client.drain_events(Duration::from_millis(5))?;
            self.fail_on_browser_errors()?;
            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "Fission browser test bridge was lost without a main-frame navigation"
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    fn send_runtime_command(&mut self, command: TestCommand) -> Result<TestResponse> {
        match self.options.mode {
            BrowserSmokeMode::FissionCanvas => self.send_bridge_command(command),
            BrowserSmokeMode::Website => self.send_site_command(command),
            BrowserSmokeMode::Dom => Err(anyhow!(
                "unsupported_page_mode: semantic commands require website() or fission_canvas()"
            )),
        }
    }

    fn capture_page_response(&mut self) -> Result<TestResponse> {
        let bytes = self.capture_page_screenshot()?;
        let decoded = image::load_from_memory(&bytes)?;
        Ok(TestResponse::Screenshot {
            png_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            width: decoded.width(),
            height: decoded.height(),
        })
    }

    /// Prefer Fission's WebGPU-frame capture for a Fission canvas. Headless Chromium can omit the
    /// WebGPU swapchain from `Page.captureScreenshot`, so that host screenshot may be black even
    /// when the app has rendered. The Web shell reads its actual intermediate frame for this path;
    /// Canvas2D and ordinary web pages use browser viewport capture. A failed GPU readback remains
    /// an error, not a potentially black host-page replacement.
    fn capture_renderer_or_page_response(&mut self) -> Result<TestResponse> {
        if self.options.mode != BrowserSmokeMode::FissionCanvas {
            return self.capture_page_response();
        }

        match self.send_bridge_command(TestCommand::CaptureScreenshot {})? {
            TestResponse::Error { message } if renderer_capture_needs_browser_host(&message) => {
                self.capture_page_response()
            }
            response => {
                if let TestResponse::Screenshot {
                    png_base64,
                    width,
                    height,
                } = &response
                {
                    validate_renderer_screenshot(png_base64, *width, *height, &self.options)?;
                }
                Ok(response)
            }
        }
    }

    fn capture_page_screenshot(&mut self) -> Result<Vec<u8>> {
        let _operation_deadline = self.client.operation_deadline.scoped(
            Instant::now() + Duration::from_millis(self.options.timeout_ms.clamp(1, 60_000)),
        );
        self.capture_verified_viewport()
    }

    fn capture_verified_viewport(&mut self) -> Result<Vec<u8>> {
        self.paint_boundary()?;
        let measured = viewport::read(&mut self.client)?;
        self.reset_viewport_baseline_after_navigation();
        anyhow::ensure!(viewport::acknowledged(&measured, &self.options, self.require_frame, self.viewport_after),
            "capture_viewport: viewport/layout differs from requested dimensions; observed {measured}, expected frame after {:?} on navigation {}; retry LiveTestClient::simulate_resize before capturing", self.viewport_after, self.client.main_frame_navigation_count);
        self.fail_on_browser_errors()?;
        let result = self.client.send(
            "Page.captureScreenshot",
            json!({ "format": "png", "captureBeyondViewport": false, "fromSurface": true }),
        )?;
        let data = result
            .get("data")
            .and_then(Value::as_str)
            .context("Page.captureScreenshot returned no data")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .context("Chrome returned invalid screenshot base64")?;
        let decoded = image::load_from_memory(&bytes)?;
        let (pixel_width, pixel_height) = viewport::pixel_dimensions(&self.options);
        anyhow::ensure!(
            decoded.width() == pixel_width && decoded.height() == pixel_height,
            "capture_viewport: PNG dimensions differ from requested CSS viewport at device scale {} (expected {pixel_width}x{pixel_height})",
            self.options.device_scale_factor
        );
        Ok(bytes)
    }

    fn fail_on_browser_errors(&mut self) -> Result<()> {
        if self.client.errors.is_empty() {
            Ok(())
        } else {
            let errors = std::mem::take(&mut self.client.errors);
            Err(anyhow!("browser reported errors:\n{}", errors.join("\n")))
        }
    }

    pub(super) fn reset_viewport_baseline_after_navigation(&mut self) {
        if self.require_frame
            && self.bridge_ready_navigation_count != Some(self.client.main_frame_navigation_count)
        {
            // The submitted frame counter belongs to one loaded Web runtime and
            // can restart after a page reload. Never compare a new page's frame
            // against a watermark captured from the old one.
            self.viewport_after = None;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_bridge_response(value: Value) -> Result<TestResponse> {
    match value {
        // Large semantic collections cross the Wasm boundary as one JSON string to avoid
        // allocating a large graph of wasm-bindgen externref handles. Parse it in the native
        // driver, where the response already needs to become a Rust TestResponse.
        Value::String(json) => {
            serde_json::from_str(&json).context("failed to decode string Fission browser response")
        }
        value => serde_json::from_value(value).context("failed to decode Fission browser response"),
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
#[derive(Debug, PartialEq, Eq)]
struct CdpKeyDescription {
    key: String,
    code: String,
    text: String,
    key_code: u32,
    location: u8,
}

#[cfg(not(target_arch = "wasm32"))]
fn cdp_key_description(key: &str, modifiers: u8) -> CdpKeyDescription {
    let named = match key {
        "Backspace" => Some(("Backspace", "Backspace", "", 8)),
        "Tab" => Some(("Tab", "Tab", "", 9)),
        "Enter" => Some(("Enter", "Enter", "\r", 13)),
        "Esc" | "Escape" => Some(("Escape", "Escape", "", 27)),
        "Space" => Some((" ", "Space", " ", 32)),
        "PageUp" => Some(("PageUp", "PageUp", "", 33)),
        "PageDown" => Some(("PageDown", "PageDown", "", 34)),
        "End" => Some(("End", "End", "", 35)),
        "Home" => Some(("Home", "Home", "", 36)),
        "Left" => Some(("ArrowLeft", "ArrowLeft", "", 37)),
        "Up" => Some(("ArrowUp", "ArrowUp", "", 38)),
        "Right" => Some(("ArrowRight", "ArrowRight", "", 39)),
        "Down" => Some(("ArrowDown", "ArrowDown", "", 40)),
        "Delete" => Some(("Delete", "Delete", "", 46)),
        _ => None,
    };
    if let Some((dom_key, code, text, key_code)) = named {
        return CdpKeyDescription {
            key: dom_key.to_owned(),
            code: code.to_owned(),
            text: if modifiers & !1 == 0 {
                text.to_owned()
            } else {
                String::new()
            },
            key_code,
            location: 0,
        };
    }
    let mut chars = key.chars();
    if let (Some(ch), None) = (chars.next(), chars.next()) {
        let (code, key_code) = if ch.is_ascii_alphabetic() {
            (
                format!("Key{}", ch.to_ascii_uppercase()),
                ch.to_ascii_uppercase() as u32,
            )
        } else if ch.is_ascii_digit() {
            (format!("Digit{ch}"), ch as u32)
        } else {
            (String::new(), ch as u32)
        };
        CdpKeyDescription {
            key: ch.to_string(),
            code,
            text: if modifiers & !1 == 0 {
                ch.to_string()
            } else {
                String::new()
            },
            key_code,
            location: 0,
        }
    } else {
        CdpKeyDescription {
            key: key.to_owned(),
            code: key.to_owned(),
            text: String::new(),
            key_code: 0,
            location: 0,
        }
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
    body_text_len: usize,
    renderer: Option<String>,
    test_bridge_ready: bool,
    #[serde(default)]
    rendered_frames: u64,
}

#[cfg(not(target_arch = "wasm32"))]
fn browser_is_ready(
    status: &RuntimeStatus,
    mode: BrowserSmokeMode,
    require_live_control: bool,
) -> bool {
    (match mode {
        BrowserSmokeMode::Dom => status.ready_dom,
        BrowserSmokeMode::Website => status.ready_dom,
        BrowserSmokeMode::FissionCanvas => {
            status.ready_canvas && status.renderer.is_some() && status.rendered_frames > 0
        }
    }) && (!require_live_control || status.test_bridge_ready)
}

#[cfg(not(target_arch = "wasm32"))]
fn is_transient_execution_context_error(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}").to_ascii_lowercase();
    [
        "cannot find default execution context",
        "no default execution context found",
        "cannot find context with specified id",
        "execution context was destroyed",
    ]
    .iter()
    .any(|phrase| message.contains(phrase))
}

#[cfg(not(target_arch = "wasm32"))]
fn is_transient_bridge_readiness_error(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}").to_ascii_lowercase();
    is_transient_execution_context_error(error)
        || message.contains("cdp command timed out: runtime.evaluate")
}

fn is_transient_bridge_context_error(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}").to_ascii_lowercase();
    is_transient_execution_context_error(error)
        || message.contains("cannot read properties of undefined (reading 'submit')")
        || message.contains("cannot read properties of undefined (reading 'poll')")
}

fn bridge_command_is_safe_to_retry(command: &TestCommand) -> bool {
    matches!(
        command,
        TestCommand::ResolveSelector { .. } | TestCommand::GetText {} | TestCommand::GetTree {}
    )
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
        body_text_len: body ? body.innerText.trim().length : 0,
        renderer: renderer ? renderer.active : null,
        rendered_frames: globalThis.__FISSION_RENDERED_FRAME_COUNT || 0,
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
fn validate_renderer_screenshot(
    png_base64: &str,
    width: u32,
    height: u32,
    options: &BrowserTestOptions,
) -> Result<()> {
    let expected = viewport::pixel_dimensions(options);
    anyhow::ensure!(
        (width, height) == expected,
        "capture_viewport: renderer declared {width}x{height}, expected {}x{} at device scale {}",
        expected.0,
        expected.1,
        options.device_scale_factor,
    );
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png_base64)
        .context("capture_viewport: invalid renderer screenshot base64")?;
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .context("capture_viewport: invalid renderer PNG")?;
    anyhow::ensure!(
        (image.width(), image.height()) == expected,
        "capture_viewport: decoded renderer PNG is {}x{}, expected {}x{} at device scale {}",
        image.width(),
        image.height(),
        expected.0,
        expected.1,
        options.device_scale_factor,
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn renderer_capture_needs_browser_host(message: &str) -> bool {
    // Only this explicit renderer request authorizes the host capture path.
    // The shared error prefix also covers GPU failures and must not hide them.
    message
        == "renderer_screenshot_unavailable: use browser-host capture with the Canvas2D renderer"
}

#[cfg(not(target_arch = "wasm32"))]
fn wait_for_target(cdp_port: u16, expected_url: &str, timeout: Duration) -> Result<String> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    while Instant::now() < deadline {
        match ureq::get(&format!("http://127.0.0.1:{cdp_port}/json/list")).call() {
            Ok(response) => {
                let targets: Value = response.into_json()?;
                if let Some(url) = targets
                    .as_array()
                    .and_then(|items| target_websocket(items, expected_url))
                {
                    return Ok(url.to_string());
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
fn target_websocket<'a>(targets: &'a [Value], expected_url: &str) -> Option<&'a str> {
    let pages = targets
        .iter()
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("page"))
        .collect::<Vec<_>>();
    pages
        .iter()
        .copied()
        .find(|entry| {
            entry
                .get("url")
                .and_then(Value::as_str)
                .is_some_and(|url| url.starts_with(expected_url))
        })
        // Chrome is launched with a fresh profile and one requested page. If
        // that page redirects before target discovery, its URL no longer
        // matches the requested URL but it remains the only page owned by this
        // browser session. Never guess when Chrome exposes multiple pages.
        .or_else(|| (pages.len() == 1).then_some(pages[0]))
        .and_then(|target| target.get("webSocketDebuggerUrl").and_then(Value::as_str))
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
    child: Option<SupervisedChild>,
    profile_dir: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl ChromeSession {
    fn launch(chrome: &PathBuf, cdp_port: u16, options: &BrowserTestOptions) -> Result<Self> {
        let webgpu_profile =
            match std::env::var_os("FISSION_CHROME_WEBGPU_PROFILE") {
                Some(value) => Some(value.into_string().map_err(|_| {
                    anyhow!("FISSION_CHROME_WEBGPU_PROFILE must contain valid UTF-8")
                })?),
                None => None,
            };
        let webgpu_profile_args = chrome_webgpu_profile_args(webgpu_profile.as_deref())?;
        let profile_dir = std::env::temp_dir().join(format!(
            "fission-cdp-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&profile_dir)?;
        let mut command = Command::new(chrome);
        command
            // Headless browser automation does not use the desktop session bus. A stale
            // inherited address can otherwise block Chromium before its first navigation.
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .arg("--headless=new")
            .arg("--enable-unsafe-webgpu")
            .args(webgpu_profile_args)
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
            .stderr(Stdio::null());
        let child = SupervisedChild::spawn(&mut command)
            .with_context(|| format!("failed to start {}", chrome.display()))?;
        Ok(Self {
            child: Some(child),
            profile_dir,
        })
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.terminate();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn chrome_webgpu_profile_args(profile: Option<&str>) -> Result<&'static [&'static str]> {
    match profile {
        None | Some("default") => Ok(&[]),
        // Chromium's default headless Linux ANGLE path can invalidate the native
        // WebGPU instance when a canvas-backed surface is read back. This
        // software Vulkan profile avoids that canvas/shared-image path.
        Some("swiftshader-vulkan") => Ok(&[
            "--use-angle=swiftshader",
            "--enable-unsafe-swiftshader",
            "--use-vulkan=swiftshader",
            "--enable-features=Vulkan",
        ]),
        Some(other) => Err(anyhow!(
            "unsupported FISSION_CHROME_WEBGPU_PROFILE '{other}'; supported values are 'default' and 'swiftshader-vulkan'"
        )),
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for ChromeSession {
    fn drop(&mut self) {
        self.kill();
        // Chromium descendants may briefly retain and rewrite profile files as
        // they leave the owned process group. Remove only this session's
        // freshly created directory, retrying for that bounded shutdown race.
        for _ in 0..20 {
            match fs::remove_dir_all(&self.profile_dir) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(_) => std::thread::sleep(Duration::from_millis(25)),
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct CdpClient {
    socket: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    next_id: u64,
    backlog: VecDeque<Value>,
    errors: Vec<String>,
    operation_deadline: CdpOperationDeadline,
    review_events: Vec<Value>,
    web_test_trace: VecDeque<String>,
    web_text_input_trace: VecDeque<String>,
    webgpu_queue_trace: VecDeque<String>,
    report_only: bool,
    network_requests: HashSet<String>,
    network_last_activity: Instant,
    main_frame_navigation_count: u64,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Default)]
struct CdpOperationDeadline(Arc<Mutex<Option<Instant>>>);

#[cfg(not(target_arch = "wasm32"))]
impl CdpOperationDeadline {
    fn current(&self) -> Instant {
        let deadline = *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(15))
    }

    fn scoped(&self, deadline: Instant) -> CdpOperationDeadlineGuard {
        let previous = {
            let mut current = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let previous = *current;
            *current = Some(previous.map_or(deadline, |previous| previous.min(deadline)));
            previous
        };
        CdpOperationDeadlineGuard {
            deadline: self.clone(),
            previous,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct CdpOperationDeadlineGuard {
    deadline: CdpOperationDeadline,
    previous: Option<Instant>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for CdpOperationDeadlineGuard {
    fn drop(&mut self) {
        *self
            .deadline
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = self.previous;
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl CdpClient {
    fn connect(ws_url: &str) -> Result<Self> {
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
            operation_deadline: CdpOperationDeadline::default(),
            review_events: Vec::new(),
            web_test_trace: VecDeque::new(),
            web_text_input_trace: VecDeque::new(),
            webgpu_queue_trace: VecDeque::new(),
            report_only: false,
            network_requests: HashSet::new(),
            network_last_activity: Instant::now(),
            main_frame_navigation_count: 0,
        })
    }

    fn send(&mut self, method: &str, params: Value) -> Result<Value> {
        let deadline = self.operation_deadline.current();
        self.set_socket_budget(deadline)?;
        let id = self.next_id;
        self.next_id += 1;
        self.socket.send(Message::Text(serde_json::to_string(
            &json!({ "id": id, "method": method, "params": params }),
        )?))?;
        loop {
            if Instant::now() >= deadline {
                return Err(anyhow!("CDP command timed out: {method}"));
            }
            self.set_socket_budget(deadline)?;
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

    fn send_with_web_test_trace(&mut self, method: &str, params: Value) -> Result<Value> {
        self.send(method, params)
            .map_err(|error| anyhow!("{error}{}", self.web_test_trace_context()))
    }

    fn set_socket_budget(&mut self, deadline: Instant) -> Result<()> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        anyhow::ensure!(!remaining.is_zero(), "CDP operation deadline expired");
        if let tungstenite::stream::MaybeTlsStream::Plain(stream) = self.socket.get_mut() {
            stream.set_read_timeout(Some(remaining.min(Duration::from_millis(100))))?;
            stream.set_write_timeout(Some(remaining.min(Duration::from_secs(15))))?;
        }
        Ok(())
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
        let method = message.get("method").and_then(Value::as_str);
        let request_id = message.pointer("/params/requestId").and_then(Value::as_str);
        if method == Some("Page.frameNavigated")
            && message.pointer("/params/frame/parentId").is_none()
        {
            self.main_frame_navigation_count = self.main_frame_navigation_count.saturating_add(1);
        }
        match (method, request_id) {
            (Some("Network.requestWillBeSent"), Some(request_id)) => {
                self.network_requests.insert(request_id.to_owned());
                self.network_last_activity = Instant::now();
            }
            (Some("Network.loadingFinished" | "Network.loadingFailed"), Some(request_id)) => {
                self.network_requests.remove(request_id);
                self.network_last_activity = Instant::now();
            }
            _ => {}
        }
        if self.review_events.len() < 512
            && matches!(
                message.get("method").and_then(Value::as_str),
                Some(
                    "Network.requestWillBeSent"
                        | "Network.responseReceived"
                        | "Network.loadingFailed"
                        | "Runtime.exceptionThrown"
                        | "Runtime.consoleAPICalled"
                        | "Log.entryAdded"
                )
            )
        {
            self.review_events.push(message.clone());
        }
        if method == Some("Runtime.consoleAPICalled") {
            if let Some(trace) = message
                .pointer("/params/args/0/value")
                .and_then(Value::as_str)
                .filter(|value| value.starts_with("FISSION_WEB_TEST_TRACE "))
            {
                let stage = trace
                    .strip_prefix("FISSION_WEB_TEST_TRACE ")
                    .and_then(|value| value.split_whitespace().next())
                    .and_then(|value| value.parse::<u32>().ok());
                if stage.is_some_and(is_web_text_input_trace_stage) {
                    if self.web_text_input_trace.len() == 256 {
                        self.web_text_input_trace.pop_front();
                    }
                    self.web_text_input_trace.push_back(trace.to_owned());
                }
                if stage.is_some_and(is_webgpu_queue_trace_stage) {
                    if self.webgpu_queue_trace.len() == 2048 {
                        self.webgpu_queue_trace.pop_front();
                    }
                    self.webgpu_queue_trace.push_back(trace.to_owned());
                }
                if self.web_test_trace.len() == 64 {
                    self.web_test_trace.pop_front();
                }
                self.web_test_trace.push_back(trace.to_owned());
            }
        }
        // Review attributes all errors to its typed per-case observations.
        // Optional resource timing must not abort semantic bridge operations.
        if self.report_only {
            return;
        }

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
                let entry = &message["params"]["entry"];
                if entry.get("level").and_then(Value::as_str) == Some("error") {
                    let text = message
                        .pointer("/params/entry/text")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown browser log error");
                    let url = message
                        .pointer("/params/entry/url")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown URL");
                    let source = entry.get("source").and_then(Value::as_str).unwrap_or("");
                    if !ignorable_browser_log(source, url, text) {
                        self.errors
                            .push(format!("browser log error at {url}: {text}"));
                    }
                }
            }
            _ => {}
        }
    }

    fn web_test_trace_context(&self) -> String {
        let mut context = String::new();
        if !self.web_test_trace.is_empty() {
            context.push_str(&format!(
                "; recent Fission Web LiveTest stages: {}",
                self.web_test_trace
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ));
        }
        if !self.web_text_input_trace.is_empty() {
            context.push_str(&format!(
                "; Fission text-input stages: {}",
                self.web_text_input_trace
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ));
        }
        context
    }
}

fn wait_for_idle_with(
    timeout_ms: u64,
    ignore_repeating_motion: bool,
    mut send: impl FnMut(TestCommand) -> Result<TestResponse>,
    mut sleep: impl FnMut(Duration),
) -> Result<TestResponse> {
    let started = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    loop {
        // The Web shell's WaitForIdle command only reads motion counts. Pump
        // first so queued effects and finite motion get a chance to advance.
        match send(TestCommand::Pump {})? {
            TestResponse::Ok {} => {}
            TestResponse::Error { message } => return Ok(TestResponse::Error { message }),
            response => {
                return Ok(TestResponse::Error {
                    message: format!("unexpected response from browser pump: {response:?}"),
                });
            }
        }
        match send(TestCommand::WaitForIdle {
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
            TestResponse::Error { message } => return Ok(TestResponse::Error { message }),
            _ => {}
        }
        if started.elapsed() >= timeout {
            return Ok(TestResponse::Error {
                message: format!(
                    "timed out after {timeout_ms}ms waiting for browser motion to become idle"
                ),
            });
        }
        sleep(Duration::from_millis(8));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn is_web_text_input_trace_stage(stage: u32) -> bool {
    // 410–435 cover bridge submit/query/poll stages; 450–470 and 473–476 cover
    // FillText, redraw scheduling, callback exit, and sender cleanup. Keep the
    // ranges separate because both are observed in the same CDP event stream.
    (410..=435).contains(&stage) || (450..=470).contains(&stage) || (473..=476).contains(&stage)
}

#[cfg(not(target_arch = "wasm32"))]
fn is_webgpu_queue_trace_stage(stage: u32) -> bool {
    (552..=559).contains(&stage) || (560..=572).contains(&stage)
}

#[cfg(not(target_arch = "wasm32"))]
fn diagnostic_hold_after_error_ms(value: Option<&str>) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .min(60_000)
}

#[cfg(not(target_arch = "wasm32"))]
fn ignorable_browser_log(source: &str, url: &str, text: &str) -> bool {
    text.contains("/__fission/renderer")
        || url.contains("/__fission/renderer")
        || (source == "network" && is_root_favicon_url(url))
}

#[cfg(not(target_arch = "wasm32"))]
fn is_root_favicon_url(url: &str) -> bool {
    let Some((scheme, remainder)) = url.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https") {
        return false;
    }
    remainder
        .find('/')
        .is_some_and(|path_start| &remainder[path_start..] == "/favicon.ico")
}

#[cfg(test)]
#[path = "browser_tests.rs"]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "browser_capture_tests.rs"]
mod capture_tests;
