# fission-test-driver

Automated UI testing client and protocol for Fission applications.

This crate provides the shared JSON protocol and a `LiveTestClient` that drives a running native or Web Fission application.

## Architecture

```
Test process                         Application
+-----------------+                  +------------------------+
| LiveTestClient  | -- native HTTP ->| native test control    |
|                 | -- Chromium CDP ->| Web test-only bridge   |
+-----------------+                  +-----------+------------+
                                                |
                                                v
                                          TestEvent / Runtime
```

Native applications use `FISSION_TEST_CONTROL_PORT=<port>`. Web applications use a test-only bridge included by `fission test --target web` or a WASM build compiled with `FISSION_WEB_TEST_CONTROL=1`. The Web variable is read at compile time: it must be present on the WASM build command, and setting it only while serving an existing build or running the browser test has no effect.

See the [complete environment-variable reference](https://fission.rs/reference/config/environment-variables/) for test, shell, renderer, diagnostics, storage, build, packaging, signing, and publishing variables.

## Protocol types

### `TestCommand`

All commands are serialized with `#[serde(tag = "cmd")]`:

| Command | Fields | Description |
|---------|--------|-------------|
| `Tap` | `x: f32, y: f32` | Simulate a pointer down + up at the given logical coordinates. |
| `TapText` | `text: String` | Find visible text matching the string and tap its center. |
| `Scroll` | `x, y, dx, dy: f32` | Simulate a scroll event at logical position `(x, y)` with logical delta `(dx, dy)`. |
| `TypeText` | `text: String` | Type each character as a keyboard event into the focused input. |
| `PressKey` | `key: String, modifiers: u8` | Press a named key (e.g., `"Enter"`, `"Escape"`, `"Tab"`, `"a"`) with modifier flags. |
| `Screenshot` | `path: String` | Capture the current frame to a PNG file at the given path. The PNG dimensions are in logical test-space pixels so they align with `GetText` / `GetTree` coordinates. |
| `GetText` | (none) | Return all visible text items with bounding rects in logical test-space pixels. |
| `GetTree` | (none) | Return the semantic accessibility tree with bounds in logical test-space pixels. |
| `Wait` | `ms: u64` | Sleep for the given duration (server-side). |
| `Pump` | (none) | Force a frame render and wait for it to complete. |
| `Quit` | (none) | Exit the application. |
| `SimulateResize` | `width: u32, height: u32` | Resize the test viewport to the given logical size. |

### `TestResponse`

| Variant | Fields | Description |
|---------|--------|-------------|
| `Ok` | (none) | Command succeeded. |
| `Text` | `items: Vec<TextItem>` | Response to `GetText`. |
| `Tree` | `nodes: Vec<SemanticNode>` | Response to `GetTree`. |
| `Error` | `message: String` | Command failed with a reason. |

### `TextItem`

```rust
pub struct TextItem {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
```

All geometry in `TextItem` is reported in logical test-space pixels.

### `SemanticNode`

```rust
pub struct SemanticNode {
    pub role: String,       // e.g., "Button", "TextInput", "Generic"
    pub label: Option<String>,
    pub value: Option<String>,
    pub focusable: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
```

All geometry in `SemanticNode` is reported in logical test-space pixels.

## `LiveTestClient`

The client provides both low-level command methods and high-level convenience helpers.

### Connection

```rust
use fission_test_driver::LiveTestClient;

let client = LiveTestClient::connect(9876);
client.wait_for_ready(5000)?; // Wait up to 5s for the app to start
```

When the application is already running inside Fission Developer, discover it
by project and keep using the same client API:

```rust
use fission_test_driver::LiveTestClient;

let client = LiveTestClient::connect_developer(env!("CARGO_MANIFEST_DIR"))?;
client.wait_for_ready(5_000)?;
client.tap_semantic_identifier("compose.open")?;
client.wait_for_text("New Message", 5_000)?;
```

The discovered client is authenticated and scoped to the imported application.
Semantic trees, text geometry, coordinates, viewport resizing, and screenshots
exclude Fission Developer's own dashboard.

Tests that need to coordinate source edits with hot reload can observe the
session separately without replacing `LiveTestClient`:

```rust
use std::time::Duration;
use fission_test_driver::{DeveloperSessionClient, ReloadOutcome};

let mut developer =
    DeveloperSessionClient::discover(env!("CARGO_MANIFEST_DIR"))?;
let client = developer.live_test_client();
let previous = developer.active_generation();

// Save an application source change here.
match developer.wait_for_reload_after(previous, Duration::from_secs(30))? {
    ReloadOutcome::Activated { .. } => client.wait_for_idle(5_000, true)?,
    ReloadOutcome::Rejected { diagnostic, .. } => panic!("{diagnostic}"),
}
```

For a served Web test build:

```rust
use fission_test_driver::{BrowserTestOptions, LiveTestClient};

let client = LiveTestClient::launch_browser(
    BrowserTestOptions::new("http://127.0.0.1:8123/platforms/web/")
        .fission_canvas(),
)?;
```

The initial Web transport uses Chromium. Its screenshots capture the composited page, and its input commands exercise Fission's deterministic event path rather than claiming trusted browser-event coverage.

### Low-level methods

| Method | Description |
|--------|-------------|
| `tap(x, y)` | Tap at coordinates. |
| `tap_text(text)` | Find and tap text (pumps before and after). |
| `scroll(x, y, dx, dy)` | Scroll at coordinates. |
| `type_text(text)` | Type characters into the focused input. |
| `press_key(key, modifiers)` | Press a key with modifiers (pumps after). |
| `screenshot(path)` | Save a screenshot PNG. |
| `get_text()` | Get all visible text items. |
| `get_tree()` | Get the semantic tree. |
| `wait(ms)` | Server-side sleep. |
| `pump()` | Force a frame and wait for completion. |
| `quit()` | Exit the application. |

### High-level helpers

| Method | Description |
|--------|-------------|
| `tap_text_and_wait(text, ms)` | Tap text then wait. |
| `assert_text_visible(needle)` | Assert that text containing `needle` is on screen. |
| `assert_text_not_visible(needle)` | Assert that text containing `needle` is not on screen. |

## Usage example

```rust
use fission_test_driver::LiveTestClient;

#[test]
fn test_login_flow() {
    let client = LiveTestClient::connect(9876);
    client.wait_for_ready(10_000).unwrap();

    // Type into the email field
    client.tap_text("Email").unwrap();
    client.type_text("user@example.com").unwrap();
    client.pump().unwrap();

    // Click the login button
    client.tap_text("Log In").unwrap();

    // Verify navigation
    client.assert_text_visible("Dashboard").unwrap();
    client.assert_text_not_visible("Log In").unwrap();

    // Take a screenshot for visual regression
    client.screenshot("/tmp/dashboard.png").unwrap();

    client.quit().unwrap();
}
```

## Modifier flags

The `modifiers` parameter is a bitmask:

| Bit | Modifier |
|-----|----------|
| `0x01` | Shift |
| `0x02` | Alt/Option |
| `0x04` | Control |
| `0x08` | Super/Command |
