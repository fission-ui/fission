#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCREENSHOT_DIR="${FISSION_SCREENSHOT_DIR:-$ROOT_DIR/test_screenshots/gallery_qa}"
CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-9879}"

mkdir -p "$SCREENSHOT_DIR"
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' -delete 2>/dev/null || true

cleanup() {
  echo "Cleaning up..."
  pkill -f "target/debug/chart-gallery" >/dev/null 2>&1 || true
  lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
}
trap cleanup EXIT

pkill -f "target/debug/chart-gallery" >/dev/null 2>&1 || true
lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
sleep 1

echo "Building chart-gallery..."
cargo build -p chart-gallery 2>&1 | tail -2

echo "Starting chart-gallery on control port $CONTROL_PORT"
FISSION_TEST_CONTROL_PORT="$CONTROL_PORT" "$ROOT_DIR/target/debug/chart-gallery" &
editor_pid=$!

for i in $(seq 1 20); do
  if curl -fs "http://127.0.0.1:$CONTROL_PORT/health" >/dev/null 2>&1; then break; fi
  sleep 1
done
echo "Gallery ready"

cmd() { curl -s -X POST "http://127.0.0.1:$CONTROL_PORT/cmd" -H "Content-Type: application/json" -d "$1"; }
shot() {
  local name="$1"
  local path="$SCREENSHOT_DIR/${name}.png"
  cmd "{\"cmd\":\"Screenshot\",\"path\":\"$path\"}"
  echo "  Screenshot: $name.png"
}

echo ""
echo "=== QA Test: Chart Gallery Workflow ==="

# Initialize
cmd '{"cmd":"Pump"}'
sleep 1

# --- 1. Initial state ---
echo "1. Initial state with Line & Bar"
shot "01_line_and_bar"

# --- 2. Switch to Pie ---
echo "2. Open Pie chart"
cmd '{"cmd":"TapText","text":"Pie"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "02_pie"

# --- 3. Switch to Scatter ---
echo "3. Open Scatter chart"
cmd '{"cmd":"TapText","text":"Scatter"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "03_scatter"

# --- 4. Switch to Boxplot ---
echo "4. Open Boxplot"
cmd '{"cmd":"TapText","text":"Boxplot"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "04_boxplot"

# --- 5. Switch to 3D Scene ---
echo "5. Open 3D Scene"
cmd '{"cmd":"TapText","text":"Scene3D"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "05_scene3d"

# Quit
cmd '{"cmd":"Quit"}'
wait "$editor_pid" 2>/dev/null || true

echo ""
echo "=== Captured screenshots ==="
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' | sort | while read -r f; do
  echo "  $(basename "$f") ($(du -k "$f" | cut -f1)KB)"
done
echo ""
echo "Done. Review ALL screenshots in $SCREENSHOT_DIR/"
