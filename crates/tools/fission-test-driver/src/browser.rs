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
    let chrome = options
        .chrome_path
        .clone()
        .or_else(detect_chrome)
        .context("Chrome/Chromium was not found; set FISSION_CHROME=/path/to/chrome")?;
    let cdp_port = options.cdp_port.unwrap_or_else(free_port);
    let mut session = ChromeSession::launch(&chrome, cdp_port, &options)?;
    let ws_url = wait_for_target(
        cdp_port,
        &options.url,
        Duration::from_millis(options.timeout_ms),
    )?;
    let mut client = CdpClient::connect(&ws_url)?;
    client.send("Runtime.enable", json!({}))?;
    client.send("Log.enable", json!({}))?;
    client.send("Page.enable", json!({}))?;
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
        };
        if ready {
            if let Some(path) = &options.screenshot_path {
                capture_screenshot(&mut client, path)?;
            }
            let report = BrowserSmokeReport {
                url: options.url.clone(),
                title: status.title,
                width: status.width,
                height: status.height,
                renderer: status.renderer,
                body_text_len: status.body_text_len,
                screenshot_path: options.screenshot_path.clone(),
            };
            session.kill();
            return Ok(report);
        }
        last_status = Some(status);
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(anyhow!(
        "browser smoke test timed out for {}; last status: {:?}",
        options.url,
        last_status
    ))
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
fn capture_screenshot(client: &mut CdpClient, path: &PathBuf) -> Result<()> {
    let result = client.send(
        "Page.captureScreenshot",
        json!({ "format": "png", "captureBeyondViewport": true }),
    )?;
    let data = result
        .get("data")
        .and_then(|value| value.as_str())
        .context("Page.captureScreenshot returned no data")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .context("Chrome returned invalid screenshot base64")?;
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
        })
    }

    fn send(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.socket.send(Message::Text(serde_json::to_string(
            &json!({ "id": id, "method": method, "params": params }),
        )?))?;
        let deadline = Instant::now() + Duration::from_secs(15);
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
                    if !text.contains("/__fission/renderer") {
                        self.errors.push(format!("browser log error: {text}"));
                    }
                }
            }
            _ => {}
        }
    }
}
