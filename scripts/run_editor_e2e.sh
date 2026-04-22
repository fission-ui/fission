#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCREENSHOT_DIR="${FISSION_SCREENSHOT_DIR:-$ROOT_DIR/test_screenshots/editor_e2e}"
CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-9878}"

mkdir -p "$SCREENSHOT_DIR"
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' -delete 2>/dev/null || true

cleanup() {
  echo "Cleaning up..."
  pkill -f "target/debug/fission-editor" >/dev/null 2>&1 || true
  lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
}
trap cleanup EXIT

# Kill stale processes
pkill -f "target/debug/fission-editor" >/dev/null 2>&1 || true
lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
sleep 1

# Build editor
echo "Building fission-editor..."
cargo build -p fission-editor 2>&1 | tail -2

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
  local path="$SCREENSHOT_DIR/${name}.png"
  cmd "{\"cmd\":\"Screenshot\",\"path\":\"$path\"}"
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

echo "11. Open a Rust file for syntax highlight test"
# Make sure we're on Explorer
cmd '{"cmd":"PressKey","key":"P","modifiers":5}'
cmd '{"cmd":"Pump"}'
sleep 0.3
# Type "Show Explorer" and tap it
cmd '{"cmd":"TapText","text":"Show Explorer"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
# Expand examples > editor > src
cmd '{"cmd":"TapText","text":"examples"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
# Look for "editor" directory (might need to scroll)
cmd '{"cmd":"TapText","text":"editor"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
cmd '{"cmd":"TapText","text":"src"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
cmd '{"cmd":"TapText","text":"main.rs"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "12_rust_syntax"

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
