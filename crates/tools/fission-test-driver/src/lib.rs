//! Automated UI testing client and protocol for Fission applications.
//!
//! This crate provides the JSON protocol types (shared between the test client
//! and the desktop shell server) and a [`LiveTestClient`] that drives a running
//! Fission application over HTTP.
//!
//! # Architecture
//!
//! The application must be launched with `FISSION_TEST_CONTROL_PORT=<port>`.
//! The [`LiveTestClient`] connects to `http://127.0.0.1:<port>` and sends
//! [`TestCommand`] JSON payloads to `/cmd`, receiving [`TestResponse`] replies.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

// --- Protocol types (shared between client and server) ---

/// A command sent from the test client to the running application.
///
/// Serialized with `#[serde(tag = "cmd")]`. See the crate-level docs for
/// the full command reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum TestCommand {
    Tap { x: f32, y: f32 },
    TapText { text: String },
    Scroll { x: f32, y: f32, dx: f32, dy: f32 },
    TypeText { text: String },
    PressKey { key: String, modifiers: u8 },
    Screenshot { path: String },
    GetText {},
    GetTree {},
    Wait { ms: u64 },
    Pump {},
    Quit {},
    // NEW: simulate real winit-level events for realistic testing
    SimulateMouseMove { x: f32, y: f32 },
    SimulateRightClick { x: f32, y: f32 },
    SimulateResize { width: u32, height: u32 },
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
    MouseMove { x: f32, y: f32 },
    MouseDown { x: f32, y: f32, button: u8 },   // 0=left, 1=right, 2=middle
    MouseUp { x: f32, y: f32, button: u8 },
    KeyDown { key_code: String, modifiers: u8 },
    KeyUp { key_code: String, modifiers: u8 },
    TextInput { text: String },
    Scroll { x: f32, y: f32, dx: f32, dy: f32 },
    Resize { width: u32, height: u32 },
    // --- Queries / control (need response channel) ---
    Screenshot { path: String },
    GetText,
    GetTree,
    Pump,
    Quit,
    /// Internal: TapText resolves a text label to coordinates; the server
    /// injects this so the main loop can do the lookup with access to the IR.
    TapText { text: String },
    /// Internal: Wait is handled server-side (sleep) then responds.
    Wait { ms: u64 },
}

/// A visible text element with its bounding rectangle, returned by [`TestCommand::GetText`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextItem {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A node in the semantic accessibility tree, returned by [`TestCommand::GetTree`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticNode {
    pub role: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub focusable: bool,
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
    Text { items: Vec<TextItem> },
    Tree { nodes: Vec<SemanticNode> },
    Error { message: String },
}

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
pub struct LiveTestClient {
    base_url: String,
}

impl LiveTestClient {
    pub fn connect(port: u16) -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{}", port),
        }
    }

    pub fn wait_for_ready(&self, timeout_ms: u64) -> Result<()> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        loop {
            match ureq::get(&format!("{}/health", self.base_url)).call() {
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
        let body = serde_json::to_string(&cmd)?;
        let resp = ureq::post(&format!("{}/cmd", self.base_url))
            .set("Content-Type", "application/json")
            .send_string(&body)
            .map_err(|e| anyhow!("request failed: {}", e))?;
        let text = resp.into_string()?;
        let response: TestResponse = serde_json::from_str(&text)?;
        if let TestResponse::Error { message } = &response {
            return Err(anyhow!("server error: {}", message));
        }
        Ok(response)
    }

    pub fn tap(&self, x: f32, y: f32) -> Result<()> {
        self.send(TestCommand::Tap { x, y })?;
        Ok(())
    }

    pub fn tap_text(&self, text: &str) -> Result<()> {
        // Pump first to ensure layout positions are current
        self.pump()?;
        self.send(TestCommand::TapText {
            text: text.to_string(),
        })?;
        // Pump after to render the result of the tap
        self.pump()?;
        Ok(())
    }

    pub fn scroll(&self, x: f32, y: f32, dx: f32, dy: f32) -> Result<()> {
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

    pub fn screenshot(&self, path: &str) -> Result<()> {
        self.send(TestCommand::Screenshot {
            path: path.to_string(),
        })?;
        Ok(())
    }

    pub fn get_text(&self) -> Result<Vec<TextItem>> {
        match self.send(TestCommand::GetText {})? {
            TestResponse::Text { items } => Ok(items),
            other => Err(anyhow!("unexpected response: {:?}", other)),
        }
    }

    pub fn get_tree(&self) -> Result<Vec<SemanticNode>> {
        match self.send(TestCommand::GetTree {})? {
            TestResponse::Tree { nodes } => Ok(nodes),
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
        self.send(TestCommand::SimulateMouseMove { x, y })?;
        Ok(())
    }

    /// Simulate a right-click at (x, y) — move + down + up with right button.
    pub fn right_click(&self, x: f32, y: f32) -> Result<()> {
        self.send(TestCommand::SimulateRightClick { x, y })?;
        Ok(())
    }

    /// Simulate a window resize — goes through the real Resized path.
    pub fn simulate_resize(&self, width: u32, height: u32) -> Result<()> {
        self.send(TestCommand::SimulateResize { width, height })?;
        Ok(())
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
