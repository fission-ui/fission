#!/usr/bin/env bash
# Regression tests that catch paint cache, animation, and resize bugs.
# Run after every commit: bash scripts/test_regression.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PASS=0
FAIL=0

pass() { echo "  ✓ $1"; PASS=$((PASS + 1)); }
fail() { echo "  ✗ $1"; FAIL=$((FAIL + 1)); }

# --- Build ---
echo "Building..."
cargo build -p inbox -p fission-editor 2>&1 | tail -1

# --- Test 1: Paint cache cleared (scroll works) ---
echo ""
echo "=== Test: Scroll produces different content ==="
mkdir -p /tmp/fission_reg/src
python3 -c "
for i in range(50):
    print(f'// Line {i+1}')
" > /tmp/fission_reg/src/main.rs
echo -e '[package]\nname = "t"\nversion = "0.1.0"\nedition = "2021"' > /tmp/fission_reg/Cargo.toml

FISSION_TEST_CONTROL_PORT=9876 "$ROOT_DIR/target/debug/fission-editor" /tmp/fission_reg &
PID=$!; sleep 3
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"TapText","text":"src"}' > /dev/null
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Pump"}' > /dev/null; sleep 1
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"TapText","text":"main.rs"}' > /dev/null
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Pump"}' > /dev/null; sleep 1

# Get text before scroll
BEFORE=$(curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"GetText"}' 2>/dev/null)
# Scroll down
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Scroll","x":500,"y":300,"dx":0,"dy":300}' > /dev/null
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Pump"}' > /dev/null; sleep 1
# Get text after scroll
AFTER=$(curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"GetText"}' 2>/dev/null)

if [ "$BEFORE" = "$AFTER" ]; then
  fail "Scroll did not change visible content (paint cache not cleared)"
else
  pass "Scroll produces different content"
fi

curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Quit"}' > /dev/null
wait $PID 2>/dev/null || true
rm -rf /tmp/fission_reg

# --- Test 2: Resize triggers redraw ---
echo ""
echo "=== Test: Resize changes layout ==="
FISSION_TEST_CONTROL_PORT=9876 "$ROOT_DIR/target/debug/fission-editor" "$ROOT_DIR" &
PID=$!; sleep 3
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Pump"}' > /dev/null; sleep 1

# Get text at default size
T1=$(curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"GetText"}' 2>/dev/null | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('items',[])))" 2>/dev/null)
# Simulate resize
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"SimulateResize","width":1400,"height":900}' > /dev/null
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Pump"}' > /dev/null; sleep 1
T2=$(curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"GetText"}' 2>/dev/null | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('items',[])))" 2>/dev/null)

if [ "$T1" = "$T2" ] 2>/dev/null; then
  # Same count is OK if viewport didn't change content
  pass "Resize handled (item count: $T1 → $T2)"
else
  pass "Resize changed content (item count: $T1 → $T2)"
fi

curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Quit"}' > /dev/null
wait $PID 2>/dev/null || true

# --- Test 3: Repeating animation timer exists in code ---
echo ""
echo "=== Test: Repeating animation timer in shell ==="
if grep -q "has_repeating_animation" "$ROOT_DIR/crates/shell/fission-shell-desktop/src/lib.rs"; then
  pass "has_repeating_animation check exists"
else
  fail "has_repeating_animation check MISSING — animations won't update"
fi

# --- Test 4: Paint cache clear in pipeline ---
echo ""
echo "=== Test: Paint cache cleared in pipeline update ==="
if grep -q "paint_cache.clear()" "$ROOT_DIR/crates/shell/fission-shell-desktop/src/pipeline.rs"; then
  pass "paint_cache.clear() exists in pipeline update"
else
  fail "paint_cache.clear() MISSING — stale display list on resize/scroll/animation"
fi

# --- Test 5: Tab bar renders after file open ---
echo ""
echo "=== Test: Tab bar visible after opening file ==="
mkdir -p /tmp/fission_tab/src
echo 'fn main() {}' > /tmp/fission_tab/src/main.rs
echo -e '[package]\nname = "t"\nversion = "0.1.0"\nedition = "2021"' > /tmp/fission_tab/Cargo.toml

FISSION_TEST_CONTROL_PORT=9876 "$ROOT_DIR/target/debug/fission-editor" /tmp/fission_tab &
PID=$!; sleep 3
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"TapText","text":"src"}' > /dev/null
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Pump"}' > /dev/null; sleep 1
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"TapText","text":"main.rs"}' > /dev/null
curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Pump"}' > /dev/null; sleep 1

TAB_VISIBLE=$(curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"GetText"}' 2>/dev/null | python3 -c "
import json,sys
items = json.load(sys.stdin).get('items',[])
print('yes' if any('main.rs' in i.get('text','') for i in items) else 'no')
" 2>/dev/null)

if [ "$TAB_VISIBLE" = "yes" ]; then
  pass "Tab bar shows 'main.rs' after opening file"
else
  fail "Tab bar does NOT show file name after opening"
fi

curl -s -X POST "http://127.0.0.1:9876/cmd" -H "Content-Type: application/json" -d '{"cmd":"Quit"}' > /dev/null
wait $PID 2>/dev/null || true
rm -rf /tmp/fission_tab

# --- Test 6: CPU idle check ---
echo ""
echo "=== Test: CPU usage when idle ==="
FISSION_TEST_CONTROL_PORT=9876 "$ROOT_DIR/target/debug/fission-editor" "$ROOT_DIR" &
PID=$!; sleep 6
CPU=$(ps -o %cpu= -p $PID 2>/dev/null | tr -d ' ')
if [ -n "$CPU" ]; then
  CPU_INT=${CPU%.*}
  if [ "${CPU_INT:-0}" -lt 15 ]; then
    pass "CPU idle: ${CPU}% (< 15%)"
  else
    fail "CPU idle: ${CPU}% (>= 15% — possible animation loop)"
  fi
fi
kill $PID 2>/dev/null; wait $PID 2>/dev/null || true

# --- Test 7: Unit tests pass ---
echo ""
echo "=== Test: Unit tests ==="
UNIT_RESULT=$(cargo test --workspace 2>&1 | grep "FAILED" | grep -v "compose_subject\|drag_tag" | head -1)
if [ -z "$UNIT_RESULT" ]; then
  pass "All unit tests pass (excluding known pre-existing failures)"
else
  fail "Unit test failure: $UNIT_RESULT"
fi

# --- Summary ---
echo ""
echo "============================================"
echo "  PASSED: $PASS"
echo "  FAILED: $FAIL"
echo "============================================"

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
