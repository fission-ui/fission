# RFC: Web Live Testing

Status: Accepted for implementation

## Summary

Fission will let the existing `LiveTestClient` drive a running Web application
in Chromium. Web tests will use the same `TestCommand`, semantic selectors,
runtime events, deterministic clock, text queries, and semantic-tree responses
as native live tests.

The implementation adds a test-only bridge between the browser host and the
existing Winit `TestEvent` loop. It does not add a second Web-specific widget
query model, a DOM selector layer, or a new browser automation framework.

## Problem

Fission has two testing layers:

1. `fission-test` exercises the shared runtime headlessly and deterministically.
2. `LiveTestClient` drives a real native shell through the test command protocol.

The first layer already covers shared application behavior for Web. The second
does not. `fission test --target web` currently builds and serves the
application, launches Chromium through the existing CDP driver, checks that a
canvas and renderer become ready, and reports browser errors. It cannot query
the Fission semantic tree or drive the running application.

This leaves browser-host regressions dependent on smoke coverage or bespoke
automation even though the Web shell already receives `TestEvent` user events.

## Goals

- Drive a running Fission Web application with `LiveTestClient`.
- Preserve the existing test command and selector semantics.
- Support semantic queries, input, text editing, deterministic time, pumping,
  resize, waits, and screenshots for the initial Chromium implementation.
- Make `fission test --target web` prove that live control is operational, not
  only that a canvas appeared.
- Keep the bridge out of normal production Web builds.
- Preserve all existing native test APIs and behavior.

## Non-goals for the initial release

- Firefox, WebKit, or branded-browser matrices.
- Playwright, Selenium, or WebDriver integration.
- Browser DOM selectors for canvas-rendered widgets.
- Browser accessibility-tree assertions. The Fission semantic tree remains
  available through `GetTree`.
- Real browser clipboard permissions, file objects, drag-and-drop, or IME
  composition automation. Existing deterministic Fission input commands remain
  usable where applicable.
- Renderer readback on WebGPU. Initial Web screenshots capture the browser page,
  including the Fission canvas and browser-composited content.
- Trace recording, video recording, network interception, or service-worker
  control.

These are useful later improvements, but none is required to establish a
coherent first-party Web live-testing path.

## Design

### One protocol

`TestCommand`, `SelectorQuery`, `TestEvent`, and `TestResponse` remain the
authoritative testing vocabulary. Web does not translate widget tests into DOM
queries because most Fission UI is rendered into a canvas.

The public client remains `LiveTestClient`. Native callers continue to use:

```rust
let client = LiveTestClient::connect(port);
```

Web callers use an additive constructor:

```rust
let client = LiveTestClient::launch_browser(
    BrowserTestOptions::new(url).fission_canvas(),
)?;
```

All existing high-level methods, including `tap_selector`, `fill_text_selector`,
`get_tree`, `wait_for_text`, `advance_clock`, and `capture_screenshot_png`, are
then available on the same client type.

### Test-only browser bridge

`fission test --target web` marks its WASM build as a test-control build. The
Web shell installs `globalThis.__FISSION_TEST__` only in that build. Normal
`fission build` and `fission run` Web output do not install the bridge.

The bridge has two small operations:

- submit a serialized `TestCommand` and return a request identifier;
- poll a request identifier for a serialized `TestResponse`.

Submitting a command enqueues the corresponding `TestEvent` through the
existing Winit `EventLoopProxy`. Query events retain their existing per-command
response channel. Polling is necessary because browser WASM runs on the browser
event-loop thread: blocking that thread while waiting for a response would
prevent Winit from handling the event.

The bridge is intentionally private implementation detail. Tests use the Rust
client rather than calling the JavaScript object directly.

### Browser transport

`LiveTestClient` gains a browser transport backed by the existing Chromium CDP
connection. The transport submits commands through the bridge and polls while
allowing the browser event loop to continue.

Operations that are inherently host-side remain in the host driver:

- wait commands poll semantic/text/motion queries with a bounded timeout;
- selector interactions perform the same scroll, pump, and action sequence as
  the native server;
- sleeps occur in the host process;
- screenshots use CDP page capture and are returned as PNG bytes;
- closing the client terminates its isolated browser process.

This preserves behavior without putting blocking timers or filesystem paths in
WASM.

### CLI behavior

For the Web target, `fission test` will:

1. build WASM with the test bridge enabled;
2. serve the generated Web host on an ephemeral loopback port;
3. launch an isolated headless Chromium session;
4. wait for the Fission canvas, renderer, and test bridge;
5. pump the application and query its semantic tree;
6. fail on build, browser, bridge, protocol, or runtime errors.

This remains a smoke/conformance check. Application-specific suites can launch
`LiveTestClient` against their served test application and use the same API as
native live tests.

## Command support

The initial browser transport supports the existing command set with these
bounded differences:

- `CaptureScreenshot` and `CaptureAt` use a browser page screenshot instead of
  renderer texture readback.
- External file commands inject Fission's deterministic external-drag events;
  they do not create browser `File` objects.
- Pointer, keyboard, text, and IME commands enter through `TestEvent`; they do
  not claim to verify browser-generated trusted events.
- `Quit` closes the test application/browser session; it does not expose a
  network control endpoint.

These differences will be documented as limitations rather than hidden behind
unsupported parity claims.

## Safety and production cost

- The bridge is selected at Web test build time and is absent from ordinary
  Web builds.
- It opens no TCP port and makes no cross-origin request.
- The browser uses the existing isolated temporary profile and loopback CDP
  port.
- Dropping the browser client terminates Chromium and removes its profile.
- No new direct dependency is required.

## Validation

The release must prove:

- native `LiveTestClient::connect` behavior remains compatible;
- the Web bridge is absent from a normal Web build configuration;
- command serialization and malformed-command errors are stable;
- asynchronous query completion cannot deadlock the browser event loop;
- semantic tree and visible text can be queried in a real Chromium run;
- selector interaction updates the running application;
- text input, pump, resize, deterministic clock, and bounded waits work;
- browser screenshots return valid PNG bytes;
- `fission test --target web` exercises the bridge through the public client;
- formatting, focused tests, the locked workspace test suite, and release
  publication gates pass for the release candidate.

## Deferred work

The browser controller will remain isolated behind the test client so future
Firefox, WebKit, WebDriver BiDi, or Playwright adapters do not change Fission's
selector and command contract. No such abstraction or dependency is added until
there is a concrete second browser implementation.
