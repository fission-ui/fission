use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

// --- Protocol types (shared between client and server) ---

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextItem {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum TestResponse {
    Ok {},
    Text { items: Vec<TextItem> },
    Tree { nodes: Vec<SemanticNode> },
    Error { message: String },
}

// --- Client ---

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
