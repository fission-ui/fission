#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCREENSHOT_DIR="${FISSION_SCREENSHOT_DIR:-$ROOT_DIR/test_screenshots/inbox_qa}"
CONTROL_PORT="${FISSION_TEST_CONTROL_PORT:-9879}"

mkdir -p "$SCREENSHOT_DIR"
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' -delete 2>/dev/null || true

cleanup() {
  echo "Cleaning up..."
  pkill -f "target/debug/inbox" >/dev/null 2>&1 || true
  lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
}
trap cleanup EXIT

pkill -f "target/debug/inbox" >/dev/null 2>&1 || true
lsof -ti tcp:"$CONTROL_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
sleep 1

echo "Building inbox..."
cargo build -p inbox 2>&1 | tail -2

echo "Starting inbox on port $CONTROL_PORT"
FISSION_TEST_CONTROL_PORT="$CONTROL_PORT" "$ROOT_DIR/target/debug/inbox" &
inbox_pid=$!

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

echo ""
echo "=== Inbox QA: Click Everything ==="

cmd '{"cmd":"Pump"}'
sleep 1

echo "1. Initial state"
shot "01_initial"

echo "2. Click Dana email"
cmd '{"cmd":"TapText","text":"Dana"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "02_dana_clicked"

echo "3. Click Alex Rivera email"
cmd '{"cmd":"TapText","text":"Alex Rivera"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "03_alex_clicked"

echo "4. Click Starred sidebar"
cmd '{"cmd":"TapText","text":"Starred"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "04_starred"

echo "5. Click Sent sidebar"
cmd '{"cmd":"TapText","text":"Sent"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "05_sent"

echo "6. Click Drafts sidebar"
cmd '{"cmd":"TapText","text":"Drafts"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "06_drafts"

echo "7. Click Trash sidebar"
cmd '{"cmd":"TapText","text":"Trash"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "07_trash"

echo "8. Click back to Inbox"
cmd '{"cmd":"TapText","text":"Inbox"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "08_inbox_back"

echo "9. Click Compose"
cmd '{"cmd":"TapText","text":"Compose"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "09_compose"

echo "10. Close compose (X button)"
cmd '{"cmd":"Tap","x":650,"y":35}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "10_compose_closed"

echo "11. Click Filters"
cmd '{"cmd":"TapText","text":"Filters"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "11_filters"

echo "12. Close filters (tap outside)"
cmd '{"cmd":"Tap","x":10,"y":10}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "12_filters_closed"

echo "13. Click Social tab"
cmd '{"cmd":"TapText","text":"Social"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "13_social"

echo "14. Click Promotions tab"
cmd '{"cmd":"TapText","text":"Promotions"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "14_promotions"

echo "15. Click Primary tab"
cmd '{"cmd":"TapText","text":"Primary"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "15_primary_back"

echo "16. Click All filter"
cmd '{"cmd":"TapText","text":"All"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "16_all"

echo "17. Click Unread filter"
cmd '{"cmd":"TapText","text":"Unread"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "17_unread"

echo "18. Click Newest sort"
cmd '{"cmd":"TapText","text":"Newest"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "18_newest"

echo "19. Click Work label"
cmd '{"cmd":"TapText","text":"Work"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "19_work_label"

echo "20. Click Personal label"
cmd '{"cmd":"TapText","text":"Personal"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "20_personal_label"

echo "21. Click Contacts"
cmd '{"cmd":"TapText","text":"Contacts"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "21_contacts"

echo "22. Click Settings (close contacts first)"
cmd '{"cmd":"TapText","text":"Done"}'
cmd '{"cmd":"Pump"}'
sleep 0.3
cmd '{"cmd":"TapText","text":"Settings"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "22_settings"

echo "22b. Close settings"
cmd '{"cmd":"TapText","text":"Close"}'
cmd '{"cmd":"Pump"}'
sleep 0.3

echo "23. Click New event"
cmd '{"cmd":"TapText","text":"New event"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "23_new_event"

echo "23b. Dismiss toast"
cmd '{"cmd":"Tap","x":370,"y":48}'
cmd '{"cmd":"Pump"}'
sleep 0.3

echo "24. Click New task"
cmd '{"cmd":"TapText","text":"New task"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "24_new_task"

echo "24b. Dismiss toast"
cmd '{"cmd":"Tap","x":370,"y":48}'
cmd '{"cmd":"Pump"}'
sleep 0.3

echo "25. Click star on Dana email"
cmd '{"cmd":"Tap","x":380,"y":175}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "25_star_toggle"

echo "26. Click checkbox on Dana email"
cmd '{"cmd":"Tap","x":115,"y":175}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "26_checkbox"

echo "27. Search"
cmd '{"cmd":"Tap","x":200,"y":52}'
cmd '{"cmd":"Pump"}'
cmd '{"cmd":"TypeText","text":"invoice"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "27_search"

echo "28. Page next"
cmd '{"cmd":"TapText","text":">"}'
cmd '{"cmd":"Pump"}'
sleep 0.5
shot "28_page_next"

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

cmd '{"cmd":"Quit"}'
wait "$inbox_pid" 2>/dev/null || true

echo ""
echo "=== Captured screenshots ==="
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' | sort | while read -r f; do
  echo "  $(basename "$f") ($(du -k "$f" | cut -f1)KB)"
done
echo ""
echo "Done. Review screenshots in $SCREENSHOT_DIR/"
