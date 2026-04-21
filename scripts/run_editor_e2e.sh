#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCREENSHOT_DIR="${FISSION_SCREENSHOT_DIR:-$ROOT_DIR/test_screenshots/editor_e2e}"
SCREENSHOT_PORT="${FISSION_SCREENSHOT_PORT:-}"
CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-9878}"

choose_free_port() {
  python3 - "$@" <<'PY'
import socket, sys
excluded = {int(a) for a in sys.argv[1:] if a.strip()}
while True:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    if p not in excluded:
        print(p)
        break
PY
}

if [[ -z "$SCREENSHOT_PORT" ]]; then
  SCREENSHOT_PORT="$(choose_free_port "$CONTROL_PORT")"
fi

mkdir -p "$SCREENSHOT_DIR"
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' -delete 2>/dev/null || true

cleanup() {
  echo "Cleaning up..."
  pkill -f "target/debug/fission-editor" >/dev/null 2>&1 || true
  if [[ -n "${screenshot_pid:-}" ]]; then
    kill "$screenshot_pid" >/dev/null 2>&1 || true
  fi
  lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
  lsof -ti tcp:"$SCREENSHOT_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
}
trap cleanup EXIT

# Kill stale processes
pkill -f "target/debug/fission-editor" >/dev/null 2>&1 || true
lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
lsof -ti tcp:"$SCREENSHOT_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
sleep 1

# Build editor
echo "Building fission-editor..."
cargo build -p fission-editor 2>&1 | tail -2

# Start screenshot server
echo "Starting screenshot server on port $SCREENSHOT_PORT"
export WORKA_SCREENSHOT_DIR="$SCREENSHOT_DIR"
export WORKA_SCREENSHOT_PORT="$SCREENSHOT_PORT"
export WORKA_SCREENSHOT_MODE="desktop"
export WORKA_SCREENSHOT_ACTIVATE_APP="fission-editor"
export WORKA_SCREENSHOT_ACTIVATE_DELAY_MS=2000
python3 "$ROOT_DIR/scripts/screenshot_server.py" &
screenshot_pid=$!

# Wait for screenshot server
for i in $(seq 1 10); do
  if nc -z 127.0.0.1 "$SCREENSHOT_PORT" 2>/dev/null; then break; fi
  sleep 0.5
done
echo "Screenshot server ready"

# Start editor
echo "Starting fission-editor on control port $CONTROL_PORT"
FISSION_TEST_CONTROL_PORT="$CONTROL_PORT" "$ROOT_DIR/target/debug/fission-editor" "$ROOT_DIR" &
editor_pid=$!

# Wait for editor
for i in $(seq 1 20); do
  if curl -fs "http://127.0.0.1:$CONTROL_PORT/health" >/dev/null 2>&1; then break; fi
  sleep 1
done
echo "Editor ready"

cmd() { curl -s -X POST "http://127.0.0.1:$CONTROL_PORT/cmd" -H "Content-Type: application/json" -d "$1"; }
shot() {
  local name="$1"
  curl -s "http://127.0.0.1:$SCREENSHOT_PORT?name=$name" >/dev/null
  echo "  Screenshot: $name.png"
}

# --- E2E Test Flow ---
echo ""
echo "=== Running E2E tests ==="

cmd '{"cmd":"Pump"}'
sleep 2

echo "1. Initial state"
shot "01_initial"

echo "2. Expand crates"
cmd '{"cmd":"TapText","text":"crates"}'
cmd '{"cmd":"Pump"}'
shot "02_crates_expanded"

echo "3. Open Cargo.toml"
cmd '{"cmd":"TapText","text":"Cargo.toml"}'
cmd '{"cmd":"Pump"}'
shot "03_file_open"

echo "4. Command palette (Ctrl+Shift+P)"
cmd '{"cmd":"PressKey","key":"P","modifiers":5}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "04_command_palette"
cmd '{"cmd":"Tap","x":10,"y":10}'
cmd '{"cmd":"Pump"}'

echo "5. Save (Ctrl+S)"
cmd '{"cmd":"PressKey","key":"s","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "05_after_save"

echo "6. Switch to Search"
cmd '{"cmd":"Tap","x":25,"y":65}'
cmd '{"cmd":"Pump"}'
shot "06_search_panel"

echo "7. Switch to Git"
cmd '{"cmd":"Tap","x":25,"y":112}'
cmd '{"cmd":"Pump"}'
shot "07_git_panel"

echo "8. Back to Explorer"
cmd '{"cmd":"Tap","x":25,"y":18}'
cmd '{"cmd":"Pump"}'
shot "08_explorer"

echo "9. Toggle terminal (Ctrl+\`)"
cmd '{"cmd":"PressKey","key":"`","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "09_terminal_hidden"
cmd '{"cmd":"PressKey","key":"`","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "10_terminal_shown"

echo "10. PROBLEMS tab"
cmd '{"cmd":"TapText","text":"PROBLEMS"}'
cmd '{"cmd":"Pump"}'
shot "11_problems"

# Check for broken items
echo ""
echo "=== Layout integrity check ==="
cmd '{"cmd":"GetText"}' | python3 -c "
import sys,json
items=json.load(sys.stdin).get('items',[])
broken=[t for t in items if (t['width']<1 or t['height']<3) and t['text'].strip()]
print(f'Total items: {len(items)}')
print(f'Broken items: {len(broken)}')
for b in broken:
    print(f'  {b[\"width\"]:.0f}x{b[\"height\"]:.0f} \"{b[\"text\"]}\"')
"

# Quit
cmd '{"cmd":"Quit"}'
wait "$editor_pid" 2>/dev/null || true

echo ""
echo "=== Captured screenshots ==="
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' | sort | while read -r f; do
  echo "  $(basename "$f") ($(du -k "$f" | cut -f1)KB)"
done
echo ""
echo "Done. Review screenshots in $SCREENSHOT_DIR/"
