#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCREENSHOT_DIR="${FISSION_SCREENSHOT_DIR:-$ROOT_DIR/.artifacts/screenshots/scripts/editor_qa}"
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

# Create test workspace with files
TEST_DIR="$ROOT_DIR/.artifacts/qa_workspace"
rm -rf "$TEST_DIR"
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

# Create a 100-line file for scroll testing
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

cmd() { curl -s -X POST "http://127.0.0.1:$CONTROL_PORT/cmd" -H "Content-Type: application/json" -d "$1"; echo ""; }
shot() {
  local name="$1"
  local path="$SCREENSHOT_DIR/${name}.png"
  cmd "{\"cmd\":\"Screenshot\",\"path\":\"$path\"}"
  echo "  Screenshot: $name.png"
}

# Initialize: press Escape to trigger initial state setup
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'
sleep 1

echo ""
echo "============================================"
echo "  FISSION EDITOR QA - 11 BUG REPRODUCTION"
echo "============================================"

# ==========================================================================
# BUG 1: Window resize doesn't redraw
# ==========================================================================
echo ""
echo "=== BUG 1: Window resize doesn't redraw ==="
echo "TEST: Capture initial size, resize larger, verify content fills new size"
shot "bug1_01_before_resize"
cmd '{"cmd":"SimulateResize","width":1200,"height":800}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "bug1_02_after_resize"
# Verify: screenshot should show content at 1200x800 (not stuck at old size)

# ==========================================================================
# BUG 2: Scrolling broken
# ==========================================================================
echo ""
echo "=== BUG 2: Scrolling broken ==="
echo "TEST: Open a 100-line file and scroll down"
cmd '{"cmd":"TapText","text":"src"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"TapText","text":"scroll_test.rs"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "bug2_01_file_opened"
# Move cursor to editor area first and pump to update layout
cmd '{"cmd":"SimulateMouseMove","x":500,"y":250}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
# Scroll down multiple times with large deltas
# In the winit real handler: LineDelta maps to delta*50, so 300 = 6 line scrolls
cmd '{"cmd":"Scroll","x":500,"y":250,"dx":0,"dy":300}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Scroll","x":500,"y":250,"dx":0,"dy":300}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Scroll","x":500,"y":250,"dx":0,"dy":300}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Scroll","x":500,"y":250,"dx":0,"dy":300}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug2_02_after_scroll"
# Verify: line numbers should start > 1 after scrolling

# ==========================================================================
# BUG 3: Menu dropdown offset
# ==========================================================================
echo ""
echo "=== BUG 3: Menu dropdown offset ==="
echo "TEST: Open File menu, verify dropdown appears near the button"
cmd '{"cmd":"TapText","text":"File"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug3_01_file_menu_open"
# Verify: dropdown should appear below the "File" text, not far away
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'

echo "TEST: Open Edit menu"
cmd '{"cmd":"TapText","text":"Edit"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug3_02_edit_menu_open"
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'

# ==========================================================================
# BUG 4: Left click shows context menu
# ==========================================================================
echo ""
echo "=== BUG 4: Left click shows context menu ==="
echo "TEST: Left-click in editor should NOT show context menu"
# First make sure no context menu is showing
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'
# Left-click in the editor area
cmd '{"cmd":"Tap","x":600,"y":300}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug4_01_after_left_click"
# Verify: NO context menu should be visible

echo "TEST: Right-click in editor SHOULD show context menu"
cmd '{"cmd":"SimulateRightClick","x":600,"y":300}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug4_02_after_right_click"
# Verify: context menu (Undo, Redo, Copy, etc.) should be visible

echo "TEST: Left-click again should dismiss context menu"
cmd '{"cmd":"Tap","x":400,"y":200}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug4_03_dismiss_context_menu"
# Verify: context menu should be gone

# ==========================================================================
# BUG 5: Cursor can't move down to empty space
# ==========================================================================
echo ""
echo "=== BUG 5: Cursor can't move down to empty space ==="
echo "TEST: Click on a line, press Down arrow multiple times"
# Open main.rs (short file with varying line lengths)
cmd '{"cmd":"TapText","text":"main.rs"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
# Click at start of first line
cmd '{"cmd":"Tap","x":350,"y":120}'
cmd '{"cmd":"Pump"}'
# Press Down arrow 5 times to move through lines
cmd '{"cmd":"PressKey","key":"Down","modifiers":0}'
cmd '{"cmd":"PressKey","key":"Down","modifiers":0}'
cmd '{"cmd":"PressKey","key":"Down","modifiers":0}'
cmd '{"cmd":"PressKey","key":"Down","modifiers":0}'
cmd '{"cmd":"PressKey","key":"Down","modifiers":0}'
cmd '{"cmd":"Pump"}'
shot "bug5_01_cursor_moved_down"
# Verify: cursor should have moved down past short/empty lines

# ==========================================================================
# BUG 6: Cursor blink only on mouse move
# ==========================================================================
echo ""
echo "=== BUG 6: Cursor blink only on mouse move ==="
echo "TEST: Focus editor, wait for blink cycle, capture"
cmd '{"cmd":"Tap","x":450,"y":200}'
cmd '{"cmd":"Pump"}'
shot "bug6_01_cursor_visible"
# Wait for one blink period (~530ms) without moving mouse
cmd '{"cmd":"Wait","ms":600}'
cmd '{"cmd":"Pump"}'
shot "bug6_02_after_blink_wait"
# Wait another cycle
cmd '{"cmd":"Wait","ms":600}'
cmd '{"cmd":"Pump"}'
shot "bug6_03_second_blink"
# Verify: cursor should have toggled visibility between shots

# ==========================================================================
# BUG 7: Git icon broken on sidebar
# ==========================================================================
echo ""
echo "=== BUG 7: Git icon broken on sidebar ==="
echo "TEST: Verify activity bar icons are visible"
shot "bug7_01_activity_bar"
# Click on the Source Control icon (3rd icon, approximately y=155)
cmd '{"cmd":"Tap","x":25,"y":155}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug7_02_git_panel"
# Verify: git/source control icon should render correctly, panel should show
# Switch back to explorer
cmd '{"cmd":"Tap","x":25,"y":60}'
cmd '{"cmd":"Pump"}'

# ==========================================================================
# BUG 8: Status bar blue looks out of place
# ==========================================================================
echo ""
echo "=== BUG 8: Status bar blue looks out of place ==="
echo "TEST: Status bar should use dark gray, not VS Code blue"
shot "bug8_01_status_bar"
# Verify: bottom status bar should be dark gray (37,37,38), NOT blue (0,122,204)

# ==========================================================================
# BUG 9: Tab bar broken
# ==========================================================================
echo ""
echo "=== BUG 9: Tab bar broken ==="
echo "TEST: Open a file and verify tab bar is visible"
# Switch to Explorer panel and wait for tree to load
cmd '{"cmd":"Tap","x":25,"y":60}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
sleep 1
# Expand src folder and open main.rs
cmd '{"cmd":"TapText","text":"src"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
cmd '{"cmd":"TapText","text":"main.rs"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "bug9_01_tab_visible"
# Verify: "main.rs" tab should be visible at the top of the editor area

echo "TEST: Open second file, verify both tabs visible"
cmd '{"cmd":"TapText","text":"Cargo.toml"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug9_02_two_tabs"
# Verify: both "main.rs" and "Cargo.toml" tabs should be visible

echo "TEST: Switch between tabs"
cmd '{"cmd":"TapText","text":"main.rs"}'
cmd '{"cmd":"Pump"}'
shot "bug9_03_switch_back"
# Verify: main.rs tab should be active (brighter)

# ==========================================================================
# BUG 10: Can't rename new folder
# ==========================================================================
echo ""
echo "=== BUG 10: Can't rename new folder ==="
echo "TEST: Create a new folder via file tree toolbar"
# Click the new folder button in the file tree toolbar (folder icon)
# The toolbar buttons are: [spacer] [new file +] [new folder] [refresh]
# They are near the top of the sidebar
cmd '{"cmd":"Tap","x":25,"y":60}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug10_01_before_create"

# Right-click on a file to get context menu with New Folder option
cmd '{"cmd":"SimulateRightClick","x":150,"y":150}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug10_02_context_menu"

# Click "New Folder" if visible
cmd '{"cmd":"TapText","text":"New Folder"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug10_03_folder_created"
# Verify: new folder should appear and rename mode should be active
# (TextInput should be visible with the default name)

# Type a new name
cmd '{"cmd":"TypeText","text":"my_module"}'
cmd '{"cmd":"Pump"}'
shot "bug10_04_rename_typed"

# Press Enter to confirm rename
cmd '{"cmd":"PressKey","key":"Enter","modifiers":0}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug10_05_rename_confirmed"
# Verify: folder should be renamed to "my_module"

# ==========================================================================
# BUG 11: Can't edit new/untitled files
# ==========================================================================
echo ""
echo "=== BUG 11: Can't edit new/untitled files ==="
echo "TEST: Create a new file and type in it"
# Click the + (new file) button in the file tree toolbar
# Look for the + icon near the top of the sidebar
cmd '{"cmd":"Tap","x":25,"y":60}'
cmd '{"cmd":"Pump"}'

# Right-click to get context menu
cmd '{"cmd":"SimulateRightClick","x":150,"y":120}'
cmd '{"cmd":"Pump"}'
sleep 0.3
cmd '{"cmd":"TapText","text":"New File"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "bug11_01_new_file_opened"
# Verify: new untitled file should be open with an empty editor

# Click in the editor area to focus the TextInput
cmd '{"cmd":"Tap","x":500,"y":300}'
cmd '{"cmd":"Pump"}'
sleep 0.3

# Type some text
cmd '{"cmd":"TypeText","text":"Hello from new file!"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "bug11_02_typed_in_new_file"
# Verify: "Hello from new file!" should be visible in the editor

# ==========================================================================
# ADDITIONAL WORKFLOW TESTS
# ==========================================================================
echo ""
echo "=== Additional workflow tests ==="

# Test: Find/Replace
echo "TEST: Find/Replace (Ctrl+F)"
cmd '{"cmd":"TapText","text":"main.rs"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"PressKey","key":"f","modifiers":4}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "extra_01_find_bar"

# Type search query
cmd '{"cmd":"TapText","text":"Find"}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"TypeText","text":"name"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "extra_02_find_results"

# Close find bar
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'

# Test: Command palette
echo "TEST: Command palette (Ctrl+Shift+P)"
cmd '{"cmd":"PressKey","key":"P","modifiers":5}'
cmd '{"cmd":"Pump"}'
sleep 0.3
shot "extra_03_command_palette"
cmd '{"cmd":"PressKey","key":"Escape","modifiers":0}'
cmd '{"cmd":"Pump"}'

# Test: Toggle sidebar
echo "TEST: Toggle sidebar (Ctrl+B)"
cmd '{"cmd":"PressKey","key":"b","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "extra_04_sidebar_hidden"
cmd '{"cmd":"PressKey","key":"b","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "extra_05_sidebar_shown"

# Test: Save file
echo "TEST: Save file (Ctrl+S)"
cmd '{"cmd":"PressKey","key":"s","modifiers":4}'
cmd '{"cmd":"Pump"}'
shot "extra_06_after_save"

# ==========================================================================
# LAYOUT INTEGRITY CHECK
# ==========================================================================
echo ""
echo "=== Layout integrity check ==="
cmd '{"cmd":"GetText"}' | python3 -c "
import sys,json
items=json.load(sys.stdin).get('items',[])
broken=[t for t in items if (t['width']<1 or t['height']<3) and t['text'].strip()]
print(f'Total text items: {len(items)}')
print(f'Broken items (zero-size): {len(broken)}')
for b in broken[:10]:
    print(f'  {b[\"width\"]:.0f}x{b[\"height\"]:.0f} \"{b[\"text\"][:40]}\"')
"

# ==========================================================================
# CLEANUP
# ==========================================================================
cmd '{"cmd":"Quit"}'
wait "$editor_pid" 2>/dev/null || true

echo ""
echo "============================================"
echo "  SCREENSHOTS CAPTURED"
echo "============================================"
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' | sort | while read -r f; do
  echo "  $(basename "$f") ($(du -k "$f" | cut -f1)KB)"
done
echo ""
echo "Review ALL screenshots in $SCREENSHOT_DIR/"

# Cleanup test workspace
rm -rf "$TEST_DIR"
