use super::*;

pub(super) fn render_web_index(project: &FissionProject) -> String {
    let title = ios_bundle_name(project);
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <link rel="icon" type="image/png" href="../../assets/app-icon.png" />
    <style>
      :root {{
        color-scheme: dark;
        background: #14171f;
      }}
      html, body {{
        margin: 0;
        width: 100%;
        height: 100%;
        overflow: hidden;
        overscroll-behavior: none;
        background: #14171f;
      }}
      body, #fission-web-mount {{
        width: 100vw;
        height: 100vh;
      }}
      canvas {{
        display: block;
        width: 100vw;
        height: 100vh;
        border: 0;
        outline: none;
        user-select: none;
        -webkit-user-drag: none;
        touch-action: none;
        -webkit-tap-highlight-color: transparent;
      }}
      canvas:focus, canvas:focus-visible {{
        outline: none;
      }}
    </style>
  </head>
  <body>
    <main id="fission-web-mount" aria-label="{title}"></main>
    <script type="module" src="./bootstrap.mjs"></script>
  </body>
</html>
"#,
        title = title,
    )
}

pub(super) fn render_web_bootstrap(project: &FissionProject) -> String {
    let module_name = project.app.name.replace('-', "_");
    format!(
        "import init from \"./pkg/{}.js\";\n\nawait init();\n",
        module_name
    )
}

pub(super) fn render_web_build_script() -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
PROFILE="${FISSION_WEB_PROFILE:-dev}"
BUILD_ARGS=(build "$PROJECT_DIR" --target web --out-dir "$SCRIPT_DIR/pkg")

if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
else
  BUILD_ARGS+=(--dev)
fi

wasm-pack "${BUILD_ARGS[@]}"
"#
    .to_string()
}

pub(super) fn render_web_run_script(_project: &FissionProject) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
HOST="${{FISSION_WEB_HOST:-127.0.0.1}}"
REQUESTED_PORT="${{FISSION_WEB_PORT:-8123}}"
PORT="$REQUESTED_PORT"
if [[ -z "${{FISSION_WEB_PORT:-}}" ]]; then
  PORT=$(python3 - "$HOST" "$REQUESTED_PORT" <<'PY'
import socket
import sys

host = sys.argv[1]
start = int(sys.argv[2])
for port in range(start, start + 51):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            probe.bind((host, port))
        except OSError:
            continue
        print(port)
        raise SystemExit(0)
raise SystemExit(f"no free web port found from {{host}}:{{start}}")
PY
)
  if [[ "$PORT" != "$REQUESTED_PORT" ]]; then
    printf 'Port %s:%s is already in use; using %s:%s.\n' "$HOST" "$REQUESTED_PORT" "$HOST" "$PORT"
  fi
fi
URL="http://${{HOST}}:${{PORT}}/platforms/web/"

"$SCRIPT_DIR/build-wasm.sh"

printf 'Serving %s\n' "$URL"
printf 'Press Ctrl+C to stop the local server.\n'
if [[ "${{FISSION_WEB_OPEN:-0}}" == "1" ]]; then
  if command -v open >/dev/null 2>&1; then
    open "$URL"
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$URL"
  elif command -v cmd.exe >/dev/null 2>&1; then
    cmd.exe /C start "$URL"
  else
    printf 'No browser opener found. Open %s manually.\n' "$URL"
  fi
fi

cd "$PROJECT_DIR"
python3 -m http.server "$PORT" --bind "$HOST"
"#
    )
}

pub(super) fn render_web_test_script(_project: &FissionProject) -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
HOST="${FISSION_WEB_HOST:-127.0.0.1}"
REQUESTED_PORT="${FISSION_WEB_PORT:-8123}"
PORT="$REQUESTED_PORT"
if [[ -z "${FISSION_WEB_PORT:-}" ]]; then
  PORT=$(python3 - "$HOST" "$REQUESTED_PORT" <<'PY'
import socket
import sys

host = sys.argv[1]
start = int(sys.argv[2])
for port in range(start, start + 51):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            probe.bind((host, port))
        except OSError:
            continue
        print(port)
        raise SystemExit(0)
raise SystemExit(f"no free web port found from {host}:{start}")
PY
)
  if [[ "$PORT" != "$REQUESTED_PORT" ]]; then
    printf 'Port %s:%s is already in use; using %s:%s.\n' "$HOST" "$REQUESTED_PORT" "$HOST" "$PORT"
  fi
fi
REQUESTED_CDP_PORT="${FISSION_WEB_CDP_PORT:-9222}"
CDP_PORT="$REQUESTED_CDP_PORT"
if [[ -z "${FISSION_WEB_CDP_PORT:-}" ]]; then
  CDP_PORT=$(python3 - "127.0.0.1" "$REQUESTED_CDP_PORT" <<'PY'
import socket
import sys

host = sys.argv[1]
start = int(sys.argv[2])
for port in range(start, start + 51):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            probe.bind((host, port))
        except OSError:
            continue
        print(port)
        raise SystemExit(0)
raise SystemExit(f"no free CDP port found from {host}:{start}")
PY
)
  if [[ "$CDP_PORT" != "$REQUESTED_CDP_PORT" ]]; then
    printf 'CDP port 127.0.0.1:%s is already in use; using 127.0.0.1:%s.\n' "$REQUESTED_CDP_PORT" "$CDP_PORT"
  fi
fi
URL="http://${HOST}:${PORT}/platforms/web/"
PROFILE_DIR="$SCRIPT_DIR/build/chrome-profile"

require_node_websocket() {
  if ! command -v node >/dev/null 2>&1; then
    printf 'Node.js was not found. Install Node 22+ so the generated browser smoke test can inspect Chrome CDP console/runtime errors.\n' >&2
    exit 1
  fi
  if ! node -e 'process.exit(typeof WebSocket === "function" ? 0 : 1)' >/dev/null 2>&1; then
    printf 'Node.js is available but does not expose the built-in WebSocket client. Install Node 22+ for Chrome CDP smoke tests.\n' >&2
    exit 1
  fi
}

detect_chrome() {
  if [[ -n "${FISSION_CHROME:-}" && -x "$FISSION_CHROME" ]]; then
    printf '%s\n' "$FISSION_CHROME"
    return
  fi
  local candidate
  for candidate in \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "/Applications/Chromium.app/Contents/MacOS/Chromium" \
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"; do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return
    fi
  done
  for candidate in google-chrome chromium chromium-browser chrome; do
    if command -v "$candidate" >/dev/null 2>&1; then
      command -v "$candidate"
      return
    fi
  done
  return 1
}

require_node_websocket
"$SCRIPT_DIR/build-wasm.sh"

mkdir -p "$SCRIPT_DIR/build"
cd "$PROJECT_DIR"
python3 -m http.server "$PORT" --bind "$HOST" >"$SCRIPT_DIR/build/web-server.log" 2>&1 &
SERVER_PID=$!

cleanup() {
  if [[ -n "${CHROME_PID:-}" ]]; then
    kill "$CHROME_PID" >/dev/null 2>&1 || true
  fi
  kill "$SERVER_PID" >/dev/null 2>&1 || true
}
trap cleanup EXIT

printf 'Running transient web smoke test at %s\n' "$URL"
printf 'The local server is stopped automatically when this script exits.\n'

python3 - <<'PY' "$URL"
import sys
import time
import urllib.request

url = sys.argv[1]
deadline = time.time() + 30
last_error = None
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            if response.status == 200:
                raise SystemExit(0)
    except Exception as error:
        last_error = error
    time.sleep(0.5)
raise SystemExit(f"web server did not serve {url}: {last_error}")
PY

CHROME=$(detect_chrome) || {
  printf 'Chrome/Chromium was not found. Set FISSION_CHROME=/path/to/chrome or run `fission doctor web --project-dir .`.\n' >&2
  exit 1
}

rm -rf "$PROFILE_DIR"
"$CHROME" \
  --headless=new \
  --enable-unsafe-webgpu \
  --no-first-run \
  --no-default-browser-check \
  --remote-debugging-port="$CDP_PORT" \
  --user-data-dir="$PROFILE_DIR" \
  "$URL" >"$SCRIPT_DIR/build/chrome.log" 2>&1 &
CHROME_PID=$!

CDP_PORT="$CDP_PORT" FISSION_WEB_URL="$URL" node <<'NODE'
const cdpPort = process.env.CDP_PORT;
const expectedUrl = process.env.FISSION_WEB_URL;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function waitForTarget() {
  const deadline = Date.now() + 60_000;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
      const targets = await response.json();
      const target = targets.find((entry) => entry.type === 'page' && entry.url.startsWith(expectedUrl));
      if (target?.webSocketDebuggerUrl) {
        return target.webSocketDebuggerUrl;
      }
    } catch (error) {
      lastError = error;
    }
    await sleep(250);
  }
  throw new Error(`Chrome CDP target did not become ready for ${expectedUrl}: ${lastError?.message ?? lastError}`);
}

class CdpClient {
  constructor(url) {
    this.url = url;
    this.ws = null;
    this.nextId = 1;
    this.pending = new Map();
    this.errors = [];
  }

  async open() {
    await new Promise((resolve, reject) => {
      const ws = new WebSocket(this.url);
      this.ws = ws;
      ws.addEventListener('open', resolve, { once: true });
      ws.addEventListener('error', (event) => reject(new Error(`CDP websocket error: ${event.message ?? 'unknown error'}`)), { once: true });
      ws.addEventListener('message', (event) => this.onMessage(event.data));
      ws.addEventListener('close', () => {
        for (const { reject: rejectPending } of this.pending.values()) {
          rejectPending(new Error('CDP websocket closed'));
        }
        this.pending.clear();
      });
    });
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const message = { id, method, params };
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP command timed out: ${method}`));
      }, 10_000);
      this.pending.set(id, { resolve, reject, timeout, method });
      this.ws.send(JSON.stringify(message));
    });
  }

  onMessage(raw) {
    const message = JSON.parse(raw);
    if (message.id) {
      const pending = this.pending.get(message.id);
      if (!pending) return;
      clearTimeout(pending.timeout);
      this.pending.delete(message.id);
      if (message.error) {
        pending.reject(new Error(`${pending.method}: ${message.error.message}`));
      } else {
        pending.resolve(message.result ?? {});
      }
      return;
    }

    if (message.method === 'Runtime.exceptionThrown') {
      this.errors.push(formatException(message.params?.exceptionDetails));
    } else if (message.method === 'Runtime.consoleAPICalled') {
      const type = message.params?.type;
      if (type === 'error' || type === 'assert') {
        this.errors.push(`console.${type}: ${(message.params?.args ?? []).map(formatRemoteObject).join(' ')}`);
      }
    } else if (message.method === 'Log.entryAdded') {
      const entry = message.params?.entry;
      if (entry?.level === 'error') {
        if ((entry.url ?? '').endsWith('/__fission/renderer')) {
          return;
        }
        this.errors.push(`browser log error: ${entry.text}${entry.url ? ` (${entry.url}:${entry.lineNumber ?? 0})` : ''}`);
      }
    }
  }

  close() {
    this.ws?.close();
  }
}

function formatRemoteObject(value) {
  if (!value) return '<missing>';
  if (Object.prototype.hasOwnProperty.call(value, 'value')) return JSON.stringify(value.value);
  return value.description ?? value.unserializableValue ?? value.type ?? '<unknown>';
}

function formatException(details) {
  if (!details) return 'runtime exception: <missing details>';
  const exception = details.exception?.description ?? details.exception?.value ?? details.text ?? 'unknown exception';
  const location = details.url ? ` at ${details.url}:${details.lineNumber ?? 0}:${details.columnNumber ?? 0}` : '';
  return `runtime exception: ${exception}${location}`;
}

function errorBlock(errors) {
  return errors.slice(0, 10).map((error, index) => `${index + 1}. ${error}`).join('\n');
}

async function readRuntimeStatus(client) {
  const expression = `(() => {
    const canvas = document.querySelector('canvas');
    if (!canvas) return { ready: false, reason: 'no canvas element' };
    const rect = canvas.getBoundingClientRect();
    const perf = globalThis.__FISSION_PERF ?? { frames: [], inputLatencies: [] };
    return {
      ready: rect.width > 0 && rect.height > 0,
      width: Math.round(rect.width),
      height: Math.round(rect.height),
      gpu: typeof navigator.gpu !== 'undefined',
      renderer: globalThis.__FISSION_RENDERER_INFO ?? null,
      frames: Array.isArray(perf.frames) ? perf.frames.slice(-120) : [],
      inputLatencies: Array.isArray(perf.inputLatencies) ? perf.inputLatencies.slice(-30) : [],
      title: document.title,
    };
  })()`;
  const result = await client.send('Runtime.evaluate', { expression, returnByValue: true });
  if (result.exceptionDetails) {
    throw new Error(formatException(result.exceptionDetails));
  }
  return result.result?.value ?? { ready: false, reason: 'evaluation returned no value' };
}

function average(values) {
  if (!values.length) return 0;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

async function clickCanvasCenter(client, status) {
  const x = Math.max(1, Math.floor(status.width / 2));
  const y = Math.max(1, Math.floor(status.height / 2));
  await client.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'none' });
  await client.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
  await client.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
}

async function main() {
  const wsUrl = await waitForTarget();
  const client = new CdpClient(wsUrl);
  await client.open();
  try {
    await Promise.all([
      client.send('Runtime.enable'),
      client.send('Log.enable'),
      client.send('Page.enable'),
    ]);

    const deadline = Date.now() + 60_000;
    let readySince = null;
    let lastStatus = null;
    while (Date.now() < deadline) {
      if (client.errors.length > 0) {
        throw new Error(`browser reported runtime/console errors:\n${errorBlock(client.errors)}`);
      }
      lastStatus = await readRuntimeStatus(client);
      if (lastStatus.ready && lastStatus.renderer) {
        readySince ??= Date.now();
        if (Date.now() - readySince >= 1_500) {
          const renderer = lastStatus.renderer.active;
          if (lastStatus.gpu && renderer === 'canvas2d-software' && !lastStatus.renderer.fallback_reason && process.env.FISSION_ALLOW_WEBGPU_FALLBACK !== '1') {
            throw new Error(`WebGPU is exposed but Fission used canvas2d-software without a fallback reason: ${JSON.stringify(lastStatus.renderer)}`);
          }
          await clickCanvasCenter(client, lastStatus);
          const inputDeadline = Date.now() + 10_000;
          while (Date.now() < inputDeadline) {
            lastStatus = await readRuntimeStatus(client);
            if ((lastStatus.inputLatencies ?? []).length > 0) break;
            await sleep(100);
          }
          const frames = lastStatus.frames ?? [];
          const latencies = lastStatus.inputLatencies ?? [];
          if (frames.length < 2) {
            throw new Error(`web perf smoke did not capture enough frame samples: ${JSON.stringify(lastStatus)}`);
          }
          if (latencies.length < 1) {
            throw new Error(`web perf smoke did not capture input latency samples: ${JSON.stringify(lastStatus)}`);
          }
          const avgFrame = average(frames.slice(-30));
          const avgLatency = average(latencies.slice(-10));
          if (avgFrame > Number(process.env.FISSION_WEB_MAX_AVG_FRAME_MS ?? 80)) {
            throw new Error(`web average frame time ${avgFrame.toFixed(2)}ms exceeded smoke threshold`);
          }
          if (avgLatency > Number(process.env.FISSION_WEB_MAX_INPUT_LATENCY_MS ?? 180)) {
            throw new Error(`web input latency ${avgLatency.toFixed(2)}ms exceeded smoke threshold`);
          }
          console.log(`Web app renderer ${renderer}; canvas ${lastStatus.width}x${lastStatus.height}; avg frame ${avgFrame.toFixed(2)}ms; avg input latency ${avgLatency.toFixed(2)}ms.`);
          return;
        }
      } else {
        readySince = null;
      }
      await sleep(250);
    }
    throw new Error(`web app did not render a non-empty canvas with renderer diagnostics. Last state: ${JSON.stringify(lastStatus)}`);
  } finally {
    client.close();
  }
}

main().catch((error) => {
  console.error(error.stack ?? error.message ?? String(error));
  process.exit(1);
});
NODE
"#
    .to_string()
}
pub(super) fn render_app_main(package_name: &str) -> String {
    let lib_name = package_name.replace('-', "_");
    format!(
        r#"#[cfg(target_os = "android")]
fn main() {{}}

#[cfg(target_arch = "wasm32")]
fn main() {{}}

#[cfg(target_os = "ios")]
fn main() -> anyhow::Result<()> {{
    {lib_name}::run_mobile()
}}

#[cfg(not(any(target_arch = "wasm32", target_os = "ios", target_os = "android")))]
fn main() -> anyhow::Result<()> {{
    {lib_name}::run_desktop()
}}
"#
    )
}

pub(super) const APP_LIB: &str = r#"pub mod app;

use crate::app::CounterApp;
use fission::prelude::*;

#[cfg(target_os = "android")]
const ANDROID_TEST_CONTROL_PORT: u16 = 48761;

#[cfg(any(target_os = "android", target_os = "ios"))]
fn mobile_app() -> MobileApp<crate::app::CounterState, CounterApp> {
    let app = MobileApp::<crate::app::CounterState, _>::new(CounterApp).with_title("Fission App");
    #[cfg(target_os = "android")]
    let app = app.with_test_control_port(ANDROID_TEST_CONTROL_PORT);
    app
}

#[cfg(target_arch = "wasm32")]
fn web_app() -> WebApp<crate::app::CounterState, CounterApp> {
    WebApp::<crate::app::CounterState, _>::new(CounterApp).with_title("Fission App")
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> anyhow::Result<()> {
    DesktopApp::<crate::app::CounterState, _>::new(CounterApp).run()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn run_mobile() -> anyhow::Result<()> {
    mobile_app().run()
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app_handle: AndroidApp) {
    let _ = mobile_app().run_with_android_app(app_handle);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}
"#;

pub(super) const APP_RS: &str = r#"use fission::prelude::*;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CounterState {
    pub count: i32,
}

impl GlobalState for CounterState {}

#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}

#[derive(Clone)]
pub struct CounterApp;

impl From<CounterApp> for Widget {
    fn from(component: CounterApp) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let increment = with_reducer!(ctx, Increment, on_increment);

        Column {
            gap: Some(16.0),
            children: vec![
                Text::new(format!("Count: {}", view.state().count)).size(28.0).into(),
                Button {
                    on_press: Some(increment),
                    child: Some(Text::new("Increment").into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()

    }
}
"#;
