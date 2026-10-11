# Web test control and browser viewport tests

Compile the Web app with `FISSION_WEB_TEST_CONTROL=1` on the compiling command.
For a generated project, use:

```sh
FISSION_WEB_TEST_CONTROL=1 ./platforms/web/build-wasm.sh
fission serve-web --project-dir . --host 127.0.0.1 --port 8123
```

The existing `fission test --target web` path also enables test control when it
builds its Web fixture. A custom test host can serve the generated Web bundle.
There is no additional preview or review command requirement.

Use the existing Rust browser session authority, and retain the client while
changing viewport sizes:

```rust
use fission_test_driver::{BrowserTestOptions, LiveTestClient};
let client = LiveTestClient::launch_browser(
    BrowserTestOptions::new("http://127.0.0.1:8123/").fission_canvas(),
)?;
client.simulate_resize(390, 900)?;
let png = client.capture_screenshot_png()?;
```

The owned Chromium transport changes the host viewport in CSS pixels without
reloading the page, so runtime state, route and page identity survive. Device
scale and mobile mode retain their configured values (scale 1 and desktop mode
by default). Each CSS dimension must be 1–8192, with at most 16,777,216 CSS pixels
total. PNG dimensions include the configured scale. Invalid dimensions fail
before dispatch. Static DOM sessions use
`BrowserTestOptions::new(url)` and need no test bridge; their semantic/input
commands remain unsupported. Native `simulate_resize` retains its logical
viewport behavior. Raw Web `SimulateResize` returns an `Error` containing
`unsupported_host` and the supported Rust driver call, without mutating runtime
dimensions: page JavaScript has no authority over Chromium's host viewport.

## Generic frame acknowledgment

An opt-in Web build exposes a read-only snapshot getter alongside the existing
`submit` and `poll` methods: `globalThis.__FISSION_TEST__.frame`. It returns
`null` before successful renderer submission, then a value shaped as:

```json
{"width":390.0,"height":900.0,"frame":42,"phase":"submitted"}
```

The shared Rust protocol is `WebTestFrame` / `WebTestFramePhase`. Width and
height are the submitted frame's layout viewport in CSS pixels. The sequence
advances after successful submission within this page. `submitted` identifies
the shell's acknowledgment; it does not assert GPU readback or compositor
presentation. Mutating a returned snapshot does not modify the shell's state.
Production builds expose neither the bridge nor this getter. No review-specific
global is involved.

The driver verifies actual browser metrics, canvas CSS/backing dimensions and
the matching submitted-frame acknowledgment, crosses two browser animation
frames for paint, and checks again before capture. A changed viewport requires
a frame newer than the previous acknowledgment; a same-size request can verify
the existing matching frame. After a timeout, a same-size retry or screenshot
must still satisfy the outstanding newer-frame requirement. Capture validates
PNG dimensions, but content still needs visual review.

Operations use the browser options timeout capped at 60 seconds. Failure reports
last observed state; Chromium may already have changed size. Repair the cause
and retry through the same client. Drop and relaunch for a closed page or lost
host. Dropping a client releases only its owned Chromium process and CDP port.

## Focused regression fixtures

`crates/tools/fission-test-driver/tests/fixtures/resize_app.rs` and
`resize_web.rs` provide a Rust/Fission counter, route and responsive layout.
Use them in a generated Web project with the existing build script; compile
`RESIZE_BASE_PATH=/repository-name/` for a mounted fixture and serve the bundle
at that path. No application UI is injected by the tests. Build a static DOM
site with the existing `fission site build` path and serve its output normally.

Run the ignored real-browser tests with `FISSION_RESIZE_WEB_URL` and
`FISSION_RESIZE_DOM_URL` set to those running fixtures:

```sh
cargo test -p fission-test-driver --test browser_resize -- --ignored --nocapture
```

They preserve counter/route/page identity across desktop → narrow → tablet →
desktop sizes, verify semantic bounds and captures, reject stale acknowledgments
and raw bridge resize, recover in the same session, and check owned resource
cleanup. Set `FISSION_RESIZE_OUTPUT` to retain PNG evidence. Run the internal
`production_canvas_has_no_test_control` test against a separately compiled
non-opt-in fixture using `FISSION_RESIZE_PRODUCTION_URL`.
