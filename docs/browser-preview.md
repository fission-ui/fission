# Browser preview lifecycle

`fission preview` builds and validates an attached local preview for Static site
and client Web/WASM targets. Existing `run`, `build`, `site serve`, and `serve-web`
commands remain available. Use the preview lifecycle when an agent or CLI wrapper
needs readiness, an exact URL, bounded startup, and owned shutdown.

After `fission init`, read the generated `AGENTS.md` and any referenced
`AGENTS.fission.md` immediately. Retain `fission.toml` and use `fission add-target`
to configure supported targets. This interface works with the existing content
site and Web scaffolds; it does not require a different website starter.

```sh
fission add-target static-site web --project-dir ./app
fission preview --target static-site --project-dir ./app --port 0
fission preview --target web --project-dir ./app --port 0 --mount /repository-name/
fission preview --target web --project-dir ./app --port 0 --live-test
```

The default host is `127.0.0.1`, port is `8123`, mount is `/`, and entry is
`index.html`. Port `0` asks the OS for an available port. An occupied fixed port
is an explicit failure; Fission neither switches ports silently nor terminates
the occupying process. The reported URL uses the actual bound address and port.
Static output comes from the existing `[site].out_dir` configuration; Web output
comes from `platforms/web`. Mounting does not rewrite configuration or assets.
Use relative asset URLs when the same build must work under a repository mount.
Web deep-route fallback HTML receives a mount-root `base` element when it has
no explicit base, so nested refreshes load the same relative bootstrap files.
Set the application's existing `WebNavigationConfig::base_path` to the mount
when it uses pathname routing; the server does not rewrite logical app routes.
`--entry content/getting-started/index.html` selects an existing content route if
the configured site has no root entry. Missing entries fail instead of inventing
a successful root URL.

Startup has one deadline across building, serving, asset verification, and the
optional browser probe. `--startup-timeout-seconds` defaults to 300 and accepts
1–3600. Build/probe output retains only the last 32 KiB from each stream. Missing
tools, compile errors, invalid mounts/entries, bind errors, missing/invalid assets,
unexpected server exits, cancellation, and timeouts are reported explicitly.

Required GET responses must match the expected build files. HTML scripts,
stylesheets, images, meta-refresh destinations, literal JS imports/URL/fetch
references, and CSS resource references are checked; Web
also requires a bootstrap chain reaching a valid WASM file. A bound socket or
HTTP 200 from an unrelated server/SPA fallback is insufficient. Favicons and the
renderer diagnostic POST are best effort and do not gate readiness. External
assets and arbitrary dynamically constructed application fetches are outside
this local asset check; verify those through application/browser tests.

Default readiness is `local_assets`: the build is locally usable, but no browser
render is asserted. `--live-test` is Web-only development mode. It compiles the
existing test bridge, then uses the existing Rust browser driver in a temporary
Chrome session to verify a renderer, presented content frame, bridge, pump,
and tree response. It reports `live_test_ready: true` only after that probe
succeeds. It does not expose an HTTP
control service or attach to the user's browser tab. Rebuild Web without
`--live-test` before production packaging. The static output remains ordinary
files without a control-service dependency.

## Wrapper contract

```sh
fission preview --target web --project-dir ./app --port 0 \
  --mount /repository-name/ --json --stdin-control
```

Stdout is a flushed JSON event stream with schema `fission.preview.v1`. Stages
are `validating`, `building`, `serving`, `verifying`, optionally
`verifying_live_test`, `ready`, and finally `stopped` or `failed`. Each event
contains a unique `session`, the attached CLI's `owner_pid`, target, mount,
actual URL when known, readiness level, and stage-specific `detail`. The server
is owned by that CLI process, not a separate detached daemon. `failed` includes
`failed_stage`, `message` with the bounded diagnostic tail, `retry_argv`, and
recovery guidance. It exits nonzero. Ordinary human usage remains readable;
diagnostics stay on stderr in JSON mode.
Terminal events include `owned_resources_released`. If worker-tree cleanup
fails at the OS level, it is `false` and the error is reported even during
cancellation. Inspect that session's owned process tree before retrying.

Retain the spawned CLI handle and its stdin. With `--stdin-control`, write
`stop\n` or close stdin to cancel at any stage. Without that flag, stdin EOF is
ignored. Ctrl+C/SIGINT and SIGTERM (Unix), or Ctrl+C (Windows), also stop the
attached preview. A stopped/cancelled session releases its owned listener and
build/probe process tree and exits successfully. Wait for its terminal event
and exit before retrying. Do not kill by port or issue a stop command against an
unverified persisted PID. No session database or reusable remote-control token
is needed.

The supervisor uses a fresh Unix process group or Windows Job Object. Internal
build workers keep nested Cargo/wasm-pack commands in that owned tree. This
covers supported cancellation and termination signals; an uncatchable OS kill
or a tool deliberately escaping its process group is outside the attached
shutdown contract. The listener itself closes when the CLI process exits.

## Failure recovery

- Compile/tool failure: correct the compiler error or install the missing target
  tool (`fission doctor web --strict`), then repeat the reported arguments.
- Occupied port: retry with `--port 0` or choose another port. The unrelated
  listener survives.
- Mount/entry/asset failure: check the generated config's output directory,
  `--entry`, and mount-relative references. Required asset failures cannot be
  dismissed as renderer diagnostics or favicons.
- Timeout: inspect the captured tail, resolve a hung tool, or increase the
  startup deadline within its supported bound. The owned worker is terminated.
- Browser probe failure: inspect the driver diagnostic, Chrome availability,
  renderer, and opt-in bridge. A failed probe never emits a ready event.

This command does not provision SSR, backends, hosting, deployment, native apps,
or an MCP service.

## Resize one live browser session

The preview readiness probe closes its temporary browser. To drive an existing
agent session, retain the Rust client you launch against the ready event's URL:

```rust,no_run
use fission::test_driver::{BrowserTestOptions, LiveTestClient};
# fn example(url: &str) -> anyhow::Result<()> {
let mut options = BrowserTestOptions::new(url).fission_canvas();
options.timeout_ms = 10_000;
let client = LiveTestClient::launch_browser(options)?;
// Interact and navigate through this client before resizing.
client.simulate_resize(390, 844)?;
let narrow_png = client.capture_screenshot_png()?;
client.simulate_resize(1280, 900)?;
let wide_png = client.capture_screenshot_png()?;
# Ok(())
# }
```

Web/WASM requires `preview --target web --live-test` and a matching shell.
Static DOM uses `BrowserTestOptions::new(url)` without `.fission_canvas()`;
no test hooks are injected into static HTML. DOM sessions support browser-host
resize and screenshots, not every semantic LiveTest command. There is no CLI
resize flag that attaches to another client's browser.

`SimulateResize { width, height }` keeps its wire shape and `Ok` response. The
Rust browser transport controls Chromium with CDP, retaining page identity,
app state, history and route; it does not reload the page. Dimensions are whole
CSS pixels, 1–8192 per axis and at most 16,777,216 pixels total. Invalid requests
fail before changing host or runtime state. The driver fixes device scale to 1
and desktop emulation (`mobile=false`); screenshot pixels equal CSS pixels.
It does not expose another device scale, touch emulation or mobile user agent.
Browser scopes do not add subtree dimensions to the requested viewport.

Success verifies actual `innerWidth`/`innerHeight` and scale. Canvas mode also
requires canvas CSS size, backing-store size, origin, and the opt-in shell's
matching submitted-layout frame acknowledgment. A changed viewport must advance
the frame counter, preventing an old frame at a previously visited size from
passing. A repeated same-size request may verify the existing matching frame
without forcing a redraw. The driver crosses two animation frames for paint
and rechecks dimensions; this is the shell's submitted frame plus Chromium paint
boundary, not a GPU readback or complete geometry audit. DOM requires metrics
and paint without a Fission frame. Startup, review and capture share the same
dimension verifier. Capture also checks the resulting PNG dimensions; inspect
the image's content before counting a responsive layout as verified.

The resize deadline is `BrowserTestOptions::timeout_ms`, clamped to 1–60,000 ms,
including CDP calls and paint. A raw `__FISSION_TEST__` Web bridge cannot control
Chromium: it returns `TestResponse::Error` with `unsupported_host` and the exact
supported `LiveTestClient::launch_browser(...).simulate_resize(width, height)`
path, and sends no runtime resize event. Production Web retains opt-in isolation;
native resize behavior is unchanged.

On timeout or runtime/host failure, the error includes requested dimensions,
last observed viewport/canvas/frame state, and recovery instructions. Metrics
may already have changed; failure does not promise rollback. `browser_report()`
tracks the last observed size and is a snapshot, not an acknowledgment of a
failed request. Repair runtime or layout errors and retry the supported resize;
capture rejects mismatching or stale frames. For page closure or lost host
control, drop the client and launch a fresh owned session. Dropping closes only
that client's Chromium/profile; it does not stop the preview or other sessions.

### Focused real-browser regression

The driver includes ignored integration tests that use real previews supplied
through `FISSION_RESIZE_WEB_URL` and `FISSION_RESIZE_DOM_URL`. Initialize a
`resize-web` fixture with an **absolute** `--local-path` to the framework,
read generated guidance, add `web`/`static-site` targets, and preserve its
`fission.toml`. The Rust test fixtures in
`crates/tools/fission-test-driver/tests/fixtures/` supply `src/app.rs` and the
Web `src/lib.rs`; use the generated mount selector. For the declared icon,
serve the scaffold's existing app icon at a mount-relative URL (including when
testing `/repository-name/`). This is an asset reference, not a control hook.
Set `RESIZE_BASE_PATH` to the mount **when compiling**. Start the preview with
the target commands above and keep its owner alive, then run:

```sh
FISSION_RESIZE_WEB_URL=http://127.0.0.1:PORT/ FISSION_RESIZE_OUTPUT=resize-captures \
  cargo test -p fission-test-driver --test browser_resize canvas_same_session -- --ignored --nocapture
FISSION_RESIZE_DOM_URL=http://127.0.0.1:PORT/content/start/ FISSION_RESIZE_OUTPUT=resize-dom-captures \
  cargo test -p fission-test-driver --test browser_resize static_dom_host -- --ignored --nocapture
```

Repeat at the repository mount. The Web test increments state and navigates,
then resizes 1280 → 390 → 800 → 1280 in one session. It checks metrics,
canvas/frame dimensions, route/page identity, active responsive semantics,
PNG dimensions/content, raw-bridge rejection, invalid sizes, timeout without
false success, recovery, missing capability and closure. DOM checks metrics,
identity, scale and captures without a bridge. `FISSION_RESIZE_OUTPUT` saves
captures for visual inspection. Stop each owned preview after the test.
