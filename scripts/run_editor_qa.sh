#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCREENSHOT_DIR="${FISSION_SCREENSHOT_DIR:-$ROOT_DIR/test_screenshots/editor_qa}"
CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-9878}"

mkdir -p "$SCREENSHOT_DIR"
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' -delete 2>/dev/null || true

cleanup() {
  echo "Cleaning up..."
  pkill -f "target/debug/fission-editor" >/dev/null 2>&1 || true
  lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
}
trap cleanup EXIT

pkill -f "target/debug/fission-editor" >/dev/null 2>&1 || true
lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
sleep 1

echo "Building fission-editor..."
cargo build -p fission-editor 2>&1 | tail -2

# Create test files
TEST_DIR="$ROOT_DIR/test_screenshots/qa_workspace"
mkdir -p "$TEST_DIR/src"
cat > "$TEST_DIR/src/main.rs" << 'RUST'
fn main() {
    let name = "Fission";
    println!("Hello, {}!", name);

    for i in 0..10 {
        if i % 2 == 0 {
            println!("{} is even", i);
        }
    }
}
RUST

cat > "$TEST_DIR/Cargo.toml" << 'TOML'
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
TOML

# Create a medium-sized file (100 lines)
python3 -c "
for i in range(100):
    print(f'// Line {i+1}: This is a test line with some content to verify scrolling behavior')
" > "$TEST_DIR/src/scroll_test.rs"

echo "Starting fission-editor on control port $CONTROL_PORT"
FISSION_TEST_CONTROL_PORT="$CONTROL_PORT" "$ROOT_DIR/target/debug/fission-editor" "$TEST_DIR" &
editor_pid=$!

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

echo ""
echo "=== QA Test: Full Developer Workflow ==="

# Initialize root path by triggering a key event (Escape is harmless)
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'
sleep 1

# --- 1. Initial state ---
echo "1. Initial state with test workspace"
shot "01_initial"

# --- 2. Open a Rust file ---
echo "2. Open main.rs"
cmd '{"cmd":"TapText","text":"src"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"TapText","text":"main.rs"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "02_file_open"

# --- 3. Focus the editor by tapping in the text area ---
echo "3. Focus editor and type text"
cmd '{"cmd":"Tap","x":450,"y":200}'
cmd '{"cmd":"Pump"}'
sleep 0.3
# Now type — should go to the focused TextInput
cmd '{"cmd":"TypeText","text":"// QA"}'
cmd '{"cmd":"Pump"}'
shot "03_after_typing"

# --- 4. Press Enter and Tab (auto-indent + tab capture) ---
echo "4. Enter + Tab indent + type"
cmd '{"cmd":"PressKey","key":"Enter","modifiers":0}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"PressKey","key":"Tab","modifiers":0}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"TypeText","text":"let x = 42;"}'
cmd '{"cmd":"Pump"}'
shot "04_after_tab_indent"

# --- 5. Test undo (Ctrl+Z) x3 ---
echo "5. Undo x3 (Ctrl+Z)"
cmd '{"cmd":"PressKey","key":"z","modifiers":4}'
cmd '{"cmd":"PressKey","key":"z","modifiers":4}'
cmd '{"cmd":"PressKey","key":"z","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "05_after_undo"

# --- 6. Test redo (Ctrl+Shift+Z) x3 ---
echo "6. Redo x3 (Ctrl+Shift+Z)"
cmd '{"cmd":"PressKey","key":"z","modifiers":5}'
cmd '{"cmd":"PressKey","key":"z","modifiers":5}'
cmd '{"cmd":"PressKey","key":"z","modifiers":5}'
cmd '{"cmd":"Pump"}'
shot "06_after_redo"

# --- 7. Test save (Ctrl+S) ---
echo "7. Save (Ctrl+S) — check dirty indicator clears"
cmd '{"cmd":"PressKey","key":"s","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "07_after_save"

# --- 8. Open second file, verify tab switching ---
echo "8. Open Cargo.toml (second tab)"
cmd '{"cmd":"TapText","text":"Cargo.toml"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "08_second_tab"

# --- 9. Switch back to first tab ---
echo "9. Switch back to main.rs tab"
cmd '{"cmd":"TapText","text":"main.rs"}'
cmd '{"cmd":"Pump"}'
shot "09_tab_switch_back"

# --- 10. Open Find/Replace (Ctrl+F) ---
echo "10. Find/Replace (Ctrl+F)"
# First switch back to main.rs tab
cmd '{"cmd":"TapText","text":"main.rs"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"PressKey","key":"f","modifiers":4}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "10_find_bar_open"

# --- 11. Type search query — tap the Find input first to focus it ---
echo "11. Search for 'name' in main.rs"
cmd '{"cmd":"TapText","text":"Find"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
cmd '{"cmd":"TypeText","text":"name"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "11_find_results"

# --- 12. Close find bar (Escape) ---
echo "12. Close find bar (Escape)"
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'
shot "12_find_closed"

# --- 13. Open command palette ---
echo "13. Command palette (Ctrl+Shift+P)"
cmd '{"cmd":"PressKey","key":"P","modifiers":5}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "13_command_palette"

# --- 14. Close command palette ---
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'

# --- 15. Test menu bar ---
echo "15. Click File menu"
cmd '{"cmd":"TapText","text":"File"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "15_file_menu"

# --- 16. Close menu ---
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'

# --- 17. Toggle sidebar (Ctrl+B) ---
echo "17. Toggle sidebar off (Ctrl+B)"
cmd '{"cmd":"PressKey","key":"b","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "17_sidebar_hidden"

# --- 18. Toggle sidebar back ---
echo "18. Toggle sidebar on (Ctrl+B)"
cmd '{"cmd":"PressKey","key":"b","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "18_sidebar_shown"

# --- 19. Toggle terminal (Ctrl+`) ---
echo "19. Toggle terminal off"
cmd '{"cmd":"PressKey","key":"`","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "19_terminal_hidden"

# --- 20. Toggle terminal back ---
echo "20. Toggle terminal on"
cmd '{"cmd":"PressKey","key":"`","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "20_terminal_shown"

# --- 21. Open scroll test file (100 lines) ---
echo "21. Open large file for scroll test"
cmd '{"cmd":"TapText","text":"scroll_test.rs"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "21_scroll_file"

# --- 22. Close tab (Ctrl+W) ---
echo "22. Close tab (Ctrl+W)"
cmd '{"cmd":"PressKey","key":"w","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "22_tab_closed"

# --- 23. Switch to Search panel ---
echo "23. Search panel"
cmd '{"cmd":"Tap","x":25,"y":85}'
cmd '{"cmd":"Pump"}'
shot "23_search_panel"

# --- 24. Switch to Git panel ---
echo "24. Git panel"
cmd '{"cmd":"Tap","x":25,"y":115}'
cmd '{"cmd":"Pump"}'
shot "24_git_panel"

# --- 25. Switch to Problems tab ---
echo "25. Problems tab"
cmd '{"cmd":"TapText","text":"PROBLEMS"}'
cmd '{"cmd":"Pump"}'
shot "25_problems"

# --- Layout integrity ---
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
echo "Done. Review ALL screenshots in $SCREENSHOT_DIR/"

# Cleanup test workspace
rm -rf "$TEST_DIR"
