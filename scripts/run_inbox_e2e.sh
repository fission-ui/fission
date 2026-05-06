#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCREENSHOT_DIR="${FISSION_SCREENSHOT_DIR:-$ROOT_DIR/.artifacts/screenshots/scripts/inbox_e2e}"
CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-9879}"

mkdir -p "$SCREENSHOT_DIR"
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' -delete 2>/dev/null || true

cleanup() {
  echo "Cleaning up..."
  pkill -f "target/debug/inbox" >/dev/null 2>&1 || true
  lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
}
trap cleanup EXIT

# Kill stale processes
pkill -f "target/debug/inbox" >/dev/null 2>&1 || true
lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
sleep 1

# Build inbox
echo "Building inbox..."
cargo build -p inbox 2>&1 | tail -2

# Start inbox
echo "Starting inbox on control port $CONTROL_PORT"
FISSION_TEST_CONTROL_PORT="$CONTROL_PORT" "$ROOT_DIR/target/debug/inbox" &
inbox_pid=$!

# Wait for inbox
for i in $(seq 1 20); do
  if curl -fs "http://127.0.0.1:$CONTROL_PORT/health" >/dev/null 2>&1; then break; fi
  sleep 1
done
echo "Inbox ready"

cmd() { curl -s -X POST "http://127.0.0.1:$CONTROL_PORT/cmd" -H "Content-Type: application/json" -d "$1"; }
shot() {
  local name="$1"
  local path="$SCREENSHOT_DIR/${name}.png"
  cmd "{\"cmd\":\"Screenshot\",\"path\":\"$path\"}"
  echo "  Screenshot: $name.png"
}

# --- E2E Test Flow ---
echo ""
echo "=== Running Inbox E2E tests ==="

cmd '{"cmd":"Pump"}'
sleep 2

echo "1. Initial state"
shot "01_initial"

echo "2. Click first email"
cmd '{"cmd":"TapText","text":"Quarterly planning sync"}'
cmd '{"cmd":"Pump"}'
shot "02_email_selected"

echo "3. Click Compose"
cmd '{"cmd":"TapText","text":"Compose"}'
cmd '{"cmd":"Pump"}'
shot "03_compose"

echo "4. Close compose (tap elsewhere)"
cmd '{"cmd":"Tap","x":10,"y":10}'
cmd '{"cmd":"Pump"}'
shot "04_compose_closed"

echo "5. Click Filters"
cmd '{"cmd":"TapText","text":"Filters"}'
cmd '{"cmd":"Pump"}'
shot "05_filters"

# Layout check
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
wait "$inbox_pid" 2>/dev/null || true

echo ""
echo "=== Captured screenshots ==="
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' | sort | while read -r f; do
  echo "  $(basename "$f") ($(du -k "$f" | cut -f1)KB)"
done
echo ""
echo "Done. Review screenshots in $SCREENSHOT_DIR/"
