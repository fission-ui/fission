#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCREENSHOT_DIR="${WORKA_SCREENSHOT_DIR:-$ROOT_DIR/.tmp/worka/v2-live-screenshots}"
TEST_FILE="${WORKA_E2E_TEST_FILE:-integration_test/live_onboarding_e2e_test.dart}"
DEVICE="${WORKA_E2E_DEVICE:-macos}"

PORT="${WORKA_CLOUD_PORT:-8091}"
GRPC_PORT="$PORT"
SCREENSHOT_PORT="${WORKA_SCREENSHOT_PORT:-}"
# Always use the test database for E2E — ignore any WORKA_CLOUD_DATABASE_URL from the shell
DB_URL="postgres://worka:worka@127.0.0.1:5432/worka_test"
STARTUP_TIMEOUT_SECONDS="${WORKA_STARTUP_TIMEOUT_SECONDS:-60}"

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
  SCREENSHOT_PORT="$(choose_free_port "$PORT" "$GRPC_PORT")"
fi

mkdir -p "$SCREENSHOT_DIR" "$ROOT_DIR/.tmp/worka"
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' -delete 2>/dev/null || true

clear_stale_listeners() {
  local port="$1" label="$2"
  local pids
  pids="$(lsof -ti tcp:"$port" 2>/dev/null || true)"
  if [[ -n "$pids" ]]; then
    echo "Stopping stale $label on port $port: $pids"
    kill $pids >/dev/null 2>&1 || true
    sleep 1
  fi
}

wait_for_port() {
  local host="$1" port="$2" label="$3"
  for _ in $(seq 1 "$STARTUP_TIMEOUT_SECONDS"); do
    if nc -z "$host" "$port" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "Timed out waiting for $label on $host:$port" >&2
  return 1
}

cleanup() {
  if [[ -n "${headless_pid:-}" ]]; then
    kill "$headless_pid" >/dev/null 2>&1 || true
    wait "$headless_pid" >/dev/null 2>&1 || true
  fi
  if [[ -n "${cloud_pid:-}" ]]; then
    kill "$cloud_pid" >/dev/null 2>&1 || true
    wait "$cloud_pid" >/dev/null 2>&1 || true
  fi
  if [[ -n "${screenshot_pid:-}" ]]; then
    kill "$screenshot_pid" >/dev/null 2>&1 || true
    wait "$screenshot_pid" >/dev/null 2>&1 || true
  fi
  clear_stale_listeners "$PORT" "cloud-api"
  clear_stale_listeners "$GRPC_PORT" "cloud-api gRPC"
  clear_stale_listeners "$SCREENSHOT_PORT" "screenshot server"
}
trap cleanup EXIT

export WORKA_CLOUD_PORT="$PORT"
export WORKA_CLOUD_GRPC_PORT="$GRPC_PORT"
export WORKA_CLOUD_DATABASE_URL="$DB_URL"
export WORKA_SCREENSHOT_PORT="$SCREENSHOT_PORT"
export WORKA_SCREENSHOT_DIR="$SCREENSHOT_DIR"
export WORKA_SCREENSHOT_SERVER_URL="http://127.0.0.1:$SCREENSHOT_PORT"
export WORKA_SCREENSHOT_MODE="${WORKA_SCREENSHOT_MODE:-desktop}"

clear_stale_listeners "$PORT" "cloud-api"
clear_stale_listeners "$GRPC_PORT" "cloud-api gRPC"
clear_stale_listeners "$SCREENSHOT_PORT" "screenshot server"

# Start screenshot server
echo "Starting screenshot server on port $SCREENSHOT_PORT"
python3 "$ROOT_DIR/apps/worka/screenshot_server.py" >"$ROOT_DIR/.tmp/worka/v2-live-screenshot-server.log" 2>&1 &
screenshot_pid=$!
wait_for_port 127.0.0.1 "$SCREENSHOT_PORT" "screenshot server"

# Reset database
echo "Resetting E2E database"
"$ROOT_DIR/scripts/reset_worka_e2e_db.sh"

# Create cloud-api config with dev_auth enabled
TEMP_CONFIG="$(mktemp "$ROOT_DIR/.tmp/worka/application.live-e2e.XXXXXX")"
python3 - "$ROOT_DIR/config/application.yaml" "$TEMP_CONFIG" "$PORT" <<'PY'
import pathlib, sys
source = pathlib.Path(sys.argv[1])
target = pathlib.Path(sys.argv[2])
port = sys.argv[3]
content = source.read_text()
content = content.replace("  port: 8081", f"  port: {port}")
content = content.replace("  cloud_base_url: http://127.0.0.1:8081", f"  cloud_base_url: http://127.0.0.1:{port}")
content = content.replace("  artifact_base: http://127.0.0.1:8081", f"  artifact_base: http://127.0.0.1:{port}")
target.write_text(content)
PY
export WORKA_CONFIG_PATH="$TEMP_CONFIG"

# Skip Wasm pack building — not needed for E2E
export WORKA_SKIP_PUBLISH_REGISTRY=1

# Headless client registration code (must match --code arg)
export HEADLESS_REGISTRATION_CODE="worka"

# Start cloud-api
echo "Starting cloud-api on port $PORT (gRPC on $GRPC_PORT)"
"$ROOT_DIR/scripts/run_cloud_api_local.sh" >"$ROOT_DIR/.tmp/worka/v2-live-cloud-api.log" 2>&1 &
cloud_pid=$!
wait_for_port 127.0.0.1 "$PORT" "cloud-api"
wait_for_port 127.0.0.1 "$GRPC_PORT" "cloud-api gRPC"

# Wait for schema migration
echo "Waiting for schema migration..."
for _ in $(seq 1 "$STARTUP_TIMEOUT_SECONDS"); do
  ready="$(psql "$DB_URL" -Atqc "SELECT CASE WHEN to_regclass('public.user_account') IS NOT NULL THEN 'ready' ELSE 'waiting' END" 2>/dev/null || true)"
  if [[ "$ready" == "ready" ]]; then break; fi
  sleep 1
done

# Wait for health endpoint
echo "Waiting for cloud-api ready..."
for _ in $(seq 1 "$STARTUP_TIMEOUT_SECONDS"); do
  if curl -fsS "http://127.0.0.1:$PORT/health/ready" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

# Wait for bootstrap to complete (system agents exist)
echo "Waiting for bootstrap..."
for _ in $(seq 1 "$STARTUP_TIMEOUT_SECONDS"); do
  bootstrapped="$(psql "$DB_URL" -Atqc "SELECT CASE WHEN (SELECT count(*) FROM agent WHERE name = 'onboarding') > 0 THEN 'ready' ELSE 'waiting' END" 2>/dev/null || true)"
  if [[ "$bootstrapped" == "ready" ]]; then break; fi
  sleep 1
done

echo ""
echo "=== Cloud-API is ready ==="
echo "  HTTP: http://127.0.0.1:$PORT"
echo "  gRPC: 127.0.0.1:$GRPC_PORT"
echo ""

# Start headless client (provides CLI inference targets: codex, claude, gemini)
HEADLESS_BIN="$ROOT_DIR/target/debug/worka-headless-client"
if [[ -x "$HEADLESS_BIN" ]]; then
  echo "Starting headless client for CLI inference..."
  # Remove stale credentials so it re-registers on the test DB
  rm -f ~/.worka/headless-credentials.json
  "$HEADLESS_BIN" --server "http://127.0.0.1:$PORT" --code worka --name e2e-headless \
    >"$ROOT_DIR/.tmp/worka/v2-live-headless-client.log" 2>&1 &
  headless_pid=$!
  # Wait for it to register and connect (give it 15s)
  echo "Waiting for headless client to connect..."
  for _ in $(seq 1 15); do
    if grep -q "Connected and acknowledged" "$ROOT_DIR/.tmp/worka/v2-live-headless-client.log" 2>/dev/null; then
      echo "Headless client connected."
      break
    fi
    sleep 1
  done
  if ! grep -q "Connected and acknowledged" "$ROOT_DIR/.tmp/worka/v2-live-headless-client.log" 2>/dev/null; then
    echo "WARNING: Headless client may not have connected yet (check log)"
  fi
else
  echo "WARNING: Headless client binary not found at $HEADLESS_BIN — CLI inference targets unavailable"
  echo "  Run: cargo build -p worka-headless-client"
fi

# Reduce codex priority below other CLIs (user request)
echo "Setting codex_cli priority below other CLIs..."
psql "$DB_URL" -Atqc "UPDATE inference_target SET static_priority = 50 WHERE provider = 'codex_cli'" 2>/dev/null || true

cd "$ROOT_DIR/apps/v2"

E2E_EMAIL="${WORKA_E2E_EMAIL:-e2e-test@worka.ai}"
E2E_PASSWORD="${WORKA_E2E_PASSWORD:-WorkaE2E!2024}"

echo "Running live E2E test: $TEST_FILE"
echo "  E2E email: $E2E_EMAIL"
set +e
flutter test "$TEST_FILE" \
  -d "$DEVICE" \
  --dart-define=WORKA_CLOUD_BASE_URL="http://127.0.0.1:$PORT" \
  --dart-define=WORKA_CLOUD_GRPC_PORT="$GRPC_PORT" \
  --dart-define=WORKA_SCREENSHOT_SERVER_URL="$WORKA_SCREENSHOT_SERVER_URL" \
  --dart-define=WORKA_SCREENSHOT_DIR="$SCREENSHOT_DIR" \
  --dart-define=WORKA_E2E_EMAIL="$E2E_EMAIL" \
  --dart-define=WORKA_E2E_PASSWORD="$E2E_PASSWORD"
flutter_status=$?
set -e

echo ""
echo "=== Captured screenshots ==="
find "$SCREENSHOT_DIR" -maxdepth 1 -type f -name '*.png' | sort | while read -r f; do
  echo "  $(basename "$f")"
done

if [[ "$flutter_status" -ne 0 ]]; then
  echo "" >&2
  echo "--- cloud-api.log (tail) ---" >&2
  tail -n 40 "$ROOT_DIR/.tmp/worka/v2-live-cloud-api.log" >&2 || true
fi

exit "$flutter_status"
