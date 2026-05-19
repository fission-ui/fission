#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PROJECT_DIR=$(cd -- "$SCRIPT_DIR/../.." && pwd)
HOST="${FISSION_WEB_HOST:-127.0.0.1}"
PORT="${FISSION_WEB_PORT:-8123}"
CDP_PORT="${FISSION_WEB_CDP_PORT:-9222}"
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
  printf 'Chrome/Chromium was not found. Set FISSION_CHROME=/path/to/chrome or run `cargo fission doctor web --project-dir .`.\n' >&2
  exit 1
}

rm -rf "$PROFILE_DIR"
"$CHROME" \
  --headless=new \
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

async function readCanvas(client) {
  const expression = `(() => {
    const canvas = document.querySelector('canvas');
    if (!canvas) return { ready: false, reason: 'no canvas element' };
    const rect = canvas.getBoundingClientRect();
    return {
      ready: rect.width > 0 && rect.height > 0,
      width: Math.round(rect.width),
      height: Math.round(rect.height),
      gpu: typeof navigator.gpu !== 'undefined',
      title: document.title,
    };
  })()`;
  const result = await client.send('Runtime.evaluate', { expression, returnByValue: true });
  if (result.exceptionDetails) {
    throw new Error(formatException(result.exceptionDetails));
  }
  return result.result?.value ?? { ready: false, reason: 'evaluation returned no value' };
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
    let lastCanvas = null;
    while (Date.now() < deadline) {
      if (client.errors.length > 0) {
        throw new Error(`browser reported runtime/console errors:\n${errorBlock(client.errors)}`);
      }
      lastCanvas = await readCanvas(client);
      if (lastCanvas.ready) {
        readySince ??= Date.now();
        if (Date.now() - readySince >= 1_500) {
          console.log(`Web app rendered canvas ${lastCanvas.width}x${lastCanvas.height}; no runtime console errors observed.`);
          return;
        }
      } else {
        readySince = null;
      }
      await sleep(250);
    }
    throw new Error(`web app did not render a non-empty canvas. Last canvas state: ${JSON.stringify(lastCanvas)}`);
  } finally {
    client.close();
  }
}

main().catch((error) => {
  console.error(error.stack ?? error.message ?? String(error));
  process.exit(1);
});
NODE
